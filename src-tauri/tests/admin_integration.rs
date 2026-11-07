use std::time::{SystemTime, UNIX_EPOCH};

use app_lib::backend::{NativeBackend, RustfsBackend};
use app_lib::config::Connection;

fn test_connection() -> Option<Connection> {
    let endpoint = std::env::var("RUSTFS_TEST_ENDPOINT").ok()?;
    let access_key = std::env::var("RUSTFS_TEST_ACCESS_KEY").ok()?;
    let secret_key = std::env::var("RUSTFS_TEST_SECRET_KEY").ok()?;
    Some(Connection {
        endpoint,
        region: "us-east-1".to_string(),
        path_style: true,
        tls_verify: true,
        access_key,
        secret_key,
    })
}

#[tokio::test]
async fn creates_user_and_policy_then_reads_them_back() {
    let Some(connection) = test_connection() else {
        eprintln!(
            "skipping: set RUSTFS_TEST_ENDPOINT / RUSTFS_TEST_ACCESS_KEY / RUSTFS_TEST_SECRET_KEY"
        );
        return;
    };

    let backend = NativeBackend::new(&connection).expect("build backend");
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let user = format!("rc-test-user-{suffix}");
    let policy = format!("rc-test-policy-{suffix}");
    let document = r#"{"Version":"2012-10-17","Statement":[{"Sid":"Test","Effect":"Allow","Action":["s3:GetObject"],"Resource":["arn:aws:s3:::rc-test-bucket/*"]}]}"#.to_string();

    // Create, attach and read back.
    backend
        .add_user(&user, "rc-test-secret-123")
        .await
        .expect("add user");
    backend
        .add_policy(&policy, &document)
        .await
        .expect("add policy");
    backend
        .attach_policy(&policy, &user)
        .await
        .expect("attach policy");

    let info = backend.get_user(&user).await.expect("get user");
    assert_eq!(info.status, "enabled");
    assert_eq!(info.policy_name.as_deref(), Some(policy.as_str()));

    let read_policy = backend.get_policy(&policy).await.expect("get policy");
    assert_eq!(read_policy["Version"], "2012-10-17");

    let users = backend.list_users().await.expect("list users");
    assert!(users.iter().any(|u| u.access_key == user));

    let entities = backend.policy_entities().await.expect("policy entities");
    let mapping = entities
        .iter()
        .find(|m| m.policy == policy)
        .expect("policy mapping");
    assert!(mapping.users.contains(&user));

    // Cleanup, always.
    let _ = backend.detach_policy(&policy, &user).await;
    let _ = backend.remove_policy(&policy).await;
    let _ = backend.remove_user(&user).await;
}

#[test]
fn base_url_strips_trailing_slash() {
    let connection = Connection {
        endpoint: "http://127.0.0.1:9000/".to_string(),
        region: "us-east-1".to_string(),
        path_style: true,
        tls_verify: true,
        access_key: "a".to_string(),
        secret_key: "b".to_string(),
    };
    assert_eq!(connection.base_url(), "http://127.0.0.1:9000");
}
