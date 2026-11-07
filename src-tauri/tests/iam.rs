use app_lib::config::Connection;
use app_lib::iam;
use app_lib::instances;
use app_lib::s3::S3Client;
use app_lib::store::Instance;

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

/// Full create → verify → rotate → revoke round-trip against a local RustFS.
#[tokio::test]
async fn credential_lifecycle_round_trip() {
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

    let store_path = std::env::temp_dir().join(format!("rc-iam-{}.json", unique("store")));
    std::env::set_var("RC_STORE_PATH", &store_path);
    let _ = std::fs::remove_file(&store_path);

    let instance = instances::save(
        Instance {
            label: "iam-test".to_string(),
            endpoint: endpoint.clone(),
            ..Default::default()
        },
        Some(root_key.clone()),
        Some(root_secret.clone()),
    )
    .expect("save instance");

    let root = connection(&endpoint, &root_key, &root_secret);
    let root_s3 = S3Client::new(&root).expect("root client");
    let bucket = unique("rc-iam-test");
    let control = unique("rc-iam-control");
    root_s3.create_bucket(&bucket).await.expect("create bucket");
    root_s3
        .create_bucket(&control)
        .await
        .expect("create control bucket");

    let access_key = unique("rc-iam-user");
    let issued = iam::create(&instance.id, &bucket, "rw", Some(access_key.clone()))
        .await
        .expect("create credential");
    let secret = issued.secret.clone().expect("secret shown once");
    assert_eq!(issued.credential.policy_name, format!("rc-{bucket}-rw"));

    let scoped = connection(&endpoint, &access_key, &secret);
    let scoped_s3 = S3Client::new(&scoped).expect("scoped client");
    scoped_s3
        .put_object(&bucket, "probe/one.txt", b"hello".to_vec())
        .await
        .expect("scoped put");
    let body = scoped_s3
        .get_object(&bucket, "probe/one.txt")
        .await
        .expect("scoped get");
    assert_eq!(body, b"hello");
    // SPEC §10 tolerates RustFS returning bucket names to any authenticated
    // user; the real isolation check is data access on another bucket.
    assert!(
        scoped_s3.list_objects(&control, None).await.is_err(),
        "a scoped key must not read another bucket"
    );

    // A second create must not duplicate the credential.
    assert!(
        iam::create(&instance.id, &bucket, "rw", Some(access_key.clone()))
            .await
            .is_err()
    );

    let rotated = iam::rotate(&instance.id, &issued.credential.id)
        .await
        .expect("rotate");
    assert_ne!(rotated, secret);
    let stale = connection(&endpoint, &access_key, &secret);
    assert!(
        S3Client::new(&stale)
            .expect("stale client")
            .get_object(&bucket, "probe/one.txt")
            .await
            .is_err(),
        "the old secret must stop working"
    );
    let fresh = connection(&endpoint, &access_key, &rotated);
    S3Client::new(&fresh)
        .expect("fresh client")
        .get_object(&bucket, "probe/one.txt")
        .await
        .expect("new secret works");

    iam::revoke(&instance.id, &issued.credential.id)
        .await
        .expect("revoke");
    assert!(
        S3Client::new(&fresh)
            .expect("client")
            .get_object(&bucket, "probe/one.txt")
            .await
            .is_err(),
        "a revoked user must lose access"
    );
    assert!(iam::list(&instance.id, &bucket)
        .expect("list credentials")
        .is_empty());

    let store_text = std::fs::read_to_string(&store_path).unwrap_or_default();
    assert!(!store_text.contains(&rotated));
    assert!(!store_text.contains(&secret));

    root_s3
        .delete_object(&bucket, "probe/one.txt")
        .await
        .expect("delete probe");
    root_s3.delete_bucket(&bucket).await.expect("delete bucket");
    root_s3
        .delete_bucket(&control)
        .await
        .expect("delete control bucket");
    let _ = std::fs::remove_file(&store_path);
}
