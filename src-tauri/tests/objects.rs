use std::time::{SystemTime, UNIX_EPOCH};

use app_lib::admin::AdminClient;
use app_lib::config::Connection;
use app_lib::s3::S3Client;

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
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

fn unique(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .subsec_nanos();
    format!("{prefix}-{nanos}")
}

fn scoped_policy(bucket: &str) -> String {
    serde_json::json!({
        "Version": "2012-10-17",
        "Statement": [
            {
                "Sid": "ListBucket",
                "Effect": "Allow",
                "Action": ["s3:ListBucket", "s3:GetBucketLocation"],
                "Resource": [format!("arn:aws:s3:::{bucket}")]
            },
            {
                "Sid": "ReadWriteObjects",
                "Effect": "Allow",
                "Action": [
                    "s3:GetObject",
                    "s3:PutObject",
                    "s3:DeleteObject",
                    "s3:AbortMultipartUpload",
                    "s3:ListMultipartUploadParts"
                ],
                "Resource": [format!("arn:aws:s3:::{bucket}/*")]
            }
        ]
    })
    .to_string()
}

#[tokio::test]
async fn bucket_and_object_round_trip_with_scoped_key() {
    let Some(root) = admin_connection() else {
        eprintln!("RUSTFS_TEST_* not set, skipping");
        return;
    };

    let bucket = unique("rc-obj-readwrite");
    let control = unique("rc-obj-control");
    let access_key = unique("rc-obj-user");
    let secret_key = "rc-obj-secret-0123456789";
    let policy_name = unique("rc-obj-policy");
    let admin = AdminClient::new(&root).unwrap();
    let root_s3 = S3Client::new(&root).unwrap();

    root_s3.create_bucket(&bucket).await.unwrap();
    root_s3.create_bucket(&control).await.unwrap();
    root_s3
        .put_object(&bucket, "a/one.txt", b"hello".to_vec())
        .await
        .unwrap();
    root_s3
        .put_object(&bucket, "root.txt", b"root".to_vec())
        .await
        .unwrap();

    let page = root_s3
        .list_objects_page(&bucket, Some(""), Some("/"), None, Some(10))
        .await
        .unwrap();
    assert!(page.prefixes.contains(&"a/".to_string()));
    assert!(page.entries.iter().any(|e| e.key == "root.txt"));

    admin.add_user(&access_key, secret_key).await.unwrap();
    admin
        .add_policy(&policy_name, &scoped_policy(&bucket))
        .await
        .unwrap();
    admin
        .attach_policy(&policy_name, &access_key)
        .await
        .unwrap();

    let scoped = Connection {
        access_key: access_key.clone(),
        secret_key: secret_key.to_string(),
        ..root.clone()
    };
    let s3 = S3Client::new(&scoped).unwrap();

    // The scoped key works inside its bucket.
    s3.put_object(&bucket, "b/two.txt", b"two".to_vec())
        .await
        .unwrap();
    let body = s3.get_object(&bucket, "b/two.txt").await.unwrap();
    assert_eq!(body, b"two");
    let listing = s3
        .list_objects_page(&bucket, Some("b/"), Some("/"), None, None)
        .await
        .unwrap();
    assert!(listing.entries.iter().any(|e| e.key == "b/two.txt"));

    // And is refused on another bucket.
    let denied = s3
        .list_objects_page(&control, Some(""), Some("/"), None, None)
        .await;
    assert!(denied.is_err(), "scoped key must not list another bucket");

    // Cleanup: objects, buckets, policy, user.
    for key in ["a/one.txt", "b/two.txt", "root.txt"] {
        let _ = root_s3.delete_object(&bucket, key).await;
    }
    let _ = root_s3.delete_bucket(&bucket).await;
    let _ = root_s3.delete_bucket(&control).await;
    let _ = admin.detach_policy(&policy_name, &access_key).await;
    let _ = admin.remove_policy(&policy_name).await;
    let _ = admin.remove_user(&access_key).await;
}
