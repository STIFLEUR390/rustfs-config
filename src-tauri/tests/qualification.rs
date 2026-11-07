use app_lib::admin::AdminClient;
use app_lib::config::Connection;
use app_lib::qualify::{qualify, KeyLevel};

fn connection(access_key: &str, secret_key: &str) -> Connection {
    Connection {
        endpoint: std::env::var("RUSTFS_TEST_ENDPOINT")
            .unwrap_or_else(|_| "http://127.0.0.1:9000".to_string()),
        region: "us-east-1".to_string(),
        path_style: true,
        tls_verify: true,
        access_key: access_key.to_string(),
        secret_key: secret_key.to_string(),
    }
}

fn env_credentials() -> Option<(String, String)> {
    let access = std::env::var("RUSTFS_TEST_ACCESS_KEY").ok()?;
    let secret = std::env::var("RUSTFS_TEST_SECRET_KEY").ok()?;
    Some((access, secret))
}

fn unique(prefix: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    format!("{prefix}-{nanos}")
}

#[tokio::test]
async fn root_key_is_detected() {
    let Some((access, secret)) = env_credentials() else {
        eprintln!("skipping: RUSTFS_TEST_ACCESS_KEY/SECRET_KEY not set");
        return;
    };
    let admin = AdminClient::new(&connection(&access, &secret)).expect("client");
    let result = qualify(&admin, &access).await.expect("qualify root");
    assert_eq!(result.level, KeyLevel::Root, "{result:?}");
    assert!(result.policy_name.is_none());
}

#[tokio::test]
async fn admin_and_scoped_keys_are_distinguished() {
    let Some((root_access, root_secret)) = env_credentials() else {
        eprintln!("skipping: RUSTFS_TEST_ACCESS_KEY/SECRET_KEY not set");
        return;
    };
    let root = AdminClient::new(&connection(&root_access, &root_secret)).expect("client");

    let admin_user = unique("rc-admin-test");
    let admin_secret = "rc-admin-test-secret";
    let scoped_user = unique("rc-scoped-test");
    let scoped_secret = "rc-scoped-test-secret";

    root.add_user(&admin_user, admin_secret).await.expect("add admin user");
    root.attach_policy("consoleAdmin", &admin_user).await.expect("attach consoleAdmin");
    root.add_user(&scoped_user, scoped_secret).await.expect("add scoped user");
    root.attach_policy("readonly", &scoped_user).await.expect("attach readonly");

    let admin = AdminClient::new(&connection(&admin_user, admin_secret)).expect("admin client");
    let admin_result = qualify(&admin, &admin_user).await.expect("qualify admin");
    assert_eq!(admin_result.level, KeyLevel::Admin, "{admin_result:?}");
    assert_eq!(admin_result.policy_name.as_deref(), Some("consoleAdmin"));

    let scoped = AdminClient::new(&connection(&scoped_user, scoped_secret)).expect("scoped client");
    let scoped_result = qualify(&scoped, &scoped_user).await.expect("qualify scoped");
    assert_eq!(scoped_result.level, KeyLevel::Limited, "{scoped_result:?}");

    // Cleanup runs even if an assertion above fails hard only on the next run; keep it simple.
    let _ = root.detach_policy("consoleAdmin", &admin_user).await;
    let _ = root.detach_policy("readonly", &scoped_user).await;
    let _ = root.remove_user(&admin_user).await;
    let _ = root.remove_user(&scoped_user).await;
}
