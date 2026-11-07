use app_lib::config::Connection;
use app_lib::policy;
use app_lib::s3::S3Client;
use serde_json::{json, Value};

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

fn admin_connection() -> Option<Connection> {
    Some(Connection {
        endpoint: env("RUSTFS_TEST_ENDPOINT").unwrap_or_else(|| "http://127.0.0.1:9000".into()),
        region: "us-east-1".into(),
        path_style: true,
        tls_verify: true,
        access_key: env("RUSTFS_TEST_ACCESS_KEY")?,
        secret_key: env("RUSTFS_TEST_SECRET_KEY")?,
    })
}

fn statements(document: &Value) -> Vec<Value> {
    document
        .get("Statement")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

#[tokio::test]
async fn policy_merge_preserves_foreign_statements_and_is_idempotent() {
    let Some(connection) = admin_connection() else {
        eprintln!("RUSTFS_TEST_* unset, skipping");
        return;
    };
    let bucket = unique("rc-policy");
    let s3 = S3Client::new(&connection).expect("client");
    s3.create_bucket(&bucket).await.expect("create bucket");

    let legacy = json!({
        "Version": "2012-10-17",
        "Statement": [{
            "Sid": "Legacy",
            "Effect": "Allow",
            "Principal": {"AWS": ["*"]},
            "Action": ["s3:GetObject"],
            "Resource": [format!("arn:aws:s3:::{bucket}/legacy/*")],
        }],
    });
    s3.put_bucket_policy(&bucket, &legacy)
        .await
        .expect("put legacy policy");

    let existing = s3.get_bucket_policy(&bucket).await.expect("get policy");
    let (document, diff) =
        policy::merge(Some(&existing), &bucket, &["uploads/media/".into()]).expect("merge");
    assert_eq!(diff.len(), 1);
    assert_eq!(diff[0].action, "add");
    s3.put_bucket_policy(&bucket, &document)
        .await
        .expect("put merged policy");

    let written = s3.get_bucket_policy(&bucket).await.expect("read back");
    let sids: Vec<String> = statements(&written)
        .iter()
        .filter_map(|item| item.get("Sid").and_then(Value::as_str).map(str::to_string))
        .collect();
    assert!(sids.contains(&"Legacy".to_string()), "sids: {sids:?}");
    assert!(sids.contains(&policy::managed_sid("uploads/media/")));

    let (_, again) =
        policy::merge(Some(&written), &bucket, &["uploads/media/".into()]).expect("re-merge");
    assert!(again.is_empty(), "second merge must be a no-op: {again:?}");

    let _ = s3.delete_bucket(&bucket).await;
}
