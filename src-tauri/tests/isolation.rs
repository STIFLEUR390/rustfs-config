use app_lib::config::Connection;
use app_lib::iam;
use app_lib::instances;
use app_lib::s3::S3Client;
use app_lib::store::{Instance, Store};
use app_lib::verify;

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

fn unique(prefix: &str) -> String {
    format!(
        "{prefix}-{:x}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default()
    )
}

fn connection(endpoint: &str, access_key: &str, secret_key: &str) -> Connection {
    Connection {
        endpoint: endpoint.to_string(),
        region: "us-east-1".to_string(),
        path_style: true,
        tls_verify: true,
        access_key: access_key.to_string(),
        secret_key: secret_key.to_string(),
    }
}

#[tokio::test]
async fn isolation_report_passes_then_flags_a_missing_policy() {
    if env("RC_TEST_KEYRING").is_none() {
        eprintln!("RC_TEST_KEYRING unset, skipping");
        return;
    }
    let Some(root_key) = env("RUSTFS_TEST_ACCESS_KEY") else {
        eprintln!("RUSTFS_TEST_ACCESS_KEY unset, skipping");
        return;
    };
    let Some(root_secret) = env("RUSTFS_TEST_SECRET_KEY") else {
        eprintln!("RUSTFS_TEST_SECRET_KEY unset, skipping");
        return;
    };
    let endpoint =
        env("RUSTFS_TEST_ENDPOINT").unwrap_or_else(|| "http://127.0.0.1:9000".to_string());

    let store_path = std::env::temp_dir().join(format!("rc-verify-{}.json", unique("store")));
    std::env::set_var("RC_STORE_PATH", &store_path);
    let _ = std::fs::remove_file(&store_path);

    let instance = instances::save(
        Instance {
            label: "verify-test".to_string(),
            endpoint: endpoint.clone(),
            ..Default::default()
        },
        Some(root_key.clone()),
        Some(root_secret.clone()),
    )
    .expect("save instance");

    let root = connection(&endpoint, &root_key, &root_secret);
    let root_s3 = S3Client::new(&root).expect("root client");
    let bucket = unique("rc-verify-test");
    root_s3.create_bucket(&bucket).await.expect("create bucket");

    let issued = iam::create(&instance.id, &bucket, "rw", Some(unique("rc-verify-user")))
        .await
        .expect("create credential");

    let report = verify::verify(&instance.id, &bucket, &issued.credential.id)
        .await
        .expect("verify");
    let status = |id: &str| {
        report
            .checks
            .iter()
            .find(|check| check.id == id)
            .map(|check| check.status.as_str())
            .unwrap_or("missing")
    };
    assert!(report.ok, "report should pass: {:?}", report.checks);
    assert_eq!(status("list_buckets"), "pass");
    assert_eq!(status("probe_write"), "pass");
    assert_eq!(status("probe_delete"), "pass");
    assert_eq!(status("private_read"), "pass");
    assert_eq!(status("anonymous_list"), "pass");
    assert_eq!(status("control_bucket"), "pass");
    assert_eq!(status("public_read"), "skip");
    assert_eq!(status("bucket_policy"), "skip");
    assert_eq!(status("lifecycle"), "skip");

    // Claiming a managed bucket policy that does not exist must fail the report.
    let mut store = Store::load().expect("load store");
    let entry = store
        .buckets
        .iter_mut()
        .find(|item| item.instance_id == instance.id && item.name == bucket)
        .expect("bucket entry");
    entry.managed.bucket_policy = true;
    store.save().expect("save store");

    let failing = verify::verify(&instance.id, &bucket, &issued.credential.id)
        .await
        .expect("verify");
    assert!(!failing.ok);
    assert_eq!(
        failing
            .checks
            .iter()
            .find(|check| check.id == "bucket_policy")
            .map(|check| check.status.as_str()),
        Some("fail")
    );

    iam::revoke(&instance.id, &issued.credential.id)
        .await
        .expect("revoke");
    root_s3.delete_bucket(&bucket).await.expect("delete bucket");
    let _ = std::fs::remove_file(&store_path);
}
