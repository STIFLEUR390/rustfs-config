use app_lib::config::Connection;
use app_lib::instances;
use app_lib::policy;
use app_lib::s3::S3Client;
use app_lib::store::{Instance, Store};
use serde_json::json;

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

fn statements(document: &serde_json::Value) -> Vec<serde_json::Value> {
    document
        .get("Statement")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default()
}

#[tokio::test]
async fn policy_merge_is_idempotent_and_preserves_foreign_statements() {
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

    let store_path = std::env::temp_dir().join(format!("rc-policy-{}.json", unique("store")));
    std::env::set_var("RC_STORE_PATH", &store_path);
    let _ = std::fs::remove_file(&store_path);

    let instance = instances::save(
        Instance {
            label: "policy-test".to_string(),
            endpoint: endpoint.clone(),
            ..Default::default()
        },
        Some(root_key.clone()),
        Some(root_secret.clone()),
    )
    .expect("save instance");

    let root = Connection {
        endpoint,
        region: "us-east-1".to_string(),
        path_style: true,
        tls_verify: true,
        access_key: root_key,
        secret_key: root_secret,
    };
    let s3 = S3Client::new(&root).expect("client");
    let bucket = unique("rc-policy-test");
    s3.create_bucket(&bucket).await.expect("create bucket");

    let legacy = json!({
        "Version": "2012-10-17",
        "Statement": [{
            "Sid": "Legacy",
            "Effect": "Allow",
            "Principal": {"AWS": ["*"]},
            "Action": ["s3:GetObject"],
            "Resource": [format!("arn:aws:s3:::{bucket}/legacy/*")]
        }]
    });
    s3.put_bucket_policy(&bucket, &legacy)
        .await
        .expect("seed legacy policy");

    let prefixes = vec!["uploads/media".to_string()];
    let plan = policy::plan(&instance.id, &bucket, &prefixes)
        .await
        .expect("plan");
    assert!(plan.changed);
    assert_eq!(plan.diff.len(), 1);
    assert_eq!(plan.diff[0].action, "add");
    assert_eq!(plan.public_prefixes, vec!["uploads/media/".to_string()]);

    let applied = policy::apply(&instance.id, &bucket, &prefixes, false)
        .await
        .expect("apply");
    assert!(applied.changed);
    let first = s3.get_bucket_policy(&bucket).await.expect("read policy");
    let items = statements(&first);
    assert_eq!(items.len(), 2);
    assert!(items.iter().any(|item| item["Sid"] == "Legacy"));
    assert!(items
        .iter()
        .any(|item| item["Sid"] == "rc-public-uploads-media"));

    let again = policy::apply(&instance.id, &bucket, &prefixes, false)
        .await
        .expect("apply twice");
    assert!(!again.changed);
    assert!(again.diff.is_empty());
    assert_eq!(s3.get_bucket_policy(&bucket).await.expect("read"), first);

    let two = vec!["uploads/media/".to_string(), "media".to_string()];
    let added = policy::apply(&instance.id, &bucket, &two, false)
        .await
        .expect("add second prefix");
    assert_eq!(added.diff.len(), 1);
    assert_eq!(added.diff[0].action, "add");

    let error = policy::apply(&instance.id, &bucket, &[String::new()], false)
        .await
        .expect_err("whole bucket needs a confirmation");
    assert_eq!(error.code, "empty_prefix_requires_confirmation");
    let whole = policy::plan(&instance.id, &bucket, &[String::new()])
        .await
        .expect("plan whole bucket");
    assert!(whole.requires_confirmation);
    let applied_whole = policy::apply(&instance.id, &bucket, &[String::new()], true)
        .await
        .expect("apply whole bucket");
    assert!(applied_whole.changed);
    assert_eq!(applied_whole.diff.len(), 3);

    let cleared = policy::apply(&instance.id, &bucket, &[], false)
        .await
        .expect("clear prefixes");
    assert_eq!(cleared.diff.len(), 1);
    assert!(cleared.diff.iter().all(|change| change.action == "remove"));
    let final_document = s3.get_bucket_policy(&bucket).await.expect("read policy");
    let remaining = statements(&final_document);
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0]["Sid"], "Legacy");

    let store = Store::load().expect("store");
    let entry = store
        .buckets
        .iter()
        .find(|item| item.instance_id == instance.id && item.name == bucket)
        .expect("bucket entry");
    assert!(entry.public_prefixes.is_empty());
    assert!(entry.managed.bucket_policy);

    s3.delete_bucket(&bucket).await.expect("delete bucket");
    let _ = std::fs::remove_file(&store_path);
}
