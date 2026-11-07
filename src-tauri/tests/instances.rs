use std::path::PathBuf;

use app_lib::config::Connection;
use app_lib::instances;
use app_lib::qualify::KeyLevel;
use app_lib::store::Instance;

fn connection() -> Connection {
    Connection {
        endpoint: std::env::var("RUSTFS_TEST_ENDPOINT")
            .unwrap_or_else(|_| "http://127.0.0.1:9000".to_string()),
        region: "us-east-1".to_string(),
        path_style: true,
        tls_verify: true,
        access_key: std::env::var("RUSTFS_TEST_ACCESS_KEY").unwrap_or_else(|_| "lerd".to_string()),
        secret_key: std::env::var("RUSTFS_TEST_SECRET_KEY")
            .unwrap_or_else(|_| "lerdpassword".to_string()),
    }
}

fn temp_store_path() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir()
        .join(format!("rc-instances-test-{nanos}"))
        .join("store.json")
}

/// Full instance lifecycle against the local RustFS. Needs a running Secret Service,
/// hence the `RC_TEST_KEYRING=1` gate (same as `secrets::keyring_round_trip`).
#[tokio::test]
async fn local_instance_round_trip() {
    if std::env::var("RC_TEST_KEYRING").as_deref() != Ok("1") {
        eprintln!("skipping: set RC_TEST_KEYRING=1 to run the keyring-backed test");
        return;
    }
    let path = temp_store_path();
    std::env::set_var("RC_STORE_PATH", &path);

    let connection = connection();
    let probe = instances::test(&connection).await.expect("test connection");
    assert_eq!(probe.qualification.level, KeyLevel::Root, "{probe:?}");
    assert!(!probe.buckets.is_empty(), "expected buckets, got {probe:?}");
    assert_eq!(probe.location.as_deref(), Some("us-east-1"));

    let saved = instances::save(
        Instance {
            label: "Local (lerd)".to_string(),
            endpoint: connection.endpoint.clone(),
            ..Instance::default()
        },
        Some(connection.access_key.clone()),
        Some(connection.secret_key.clone()),
    )
    .expect("save instance");
    assert_eq!(saved.admin_access_key_suffix, "…lerd");

    let raw = std::fs::read_to_string(&path).expect("read store");
    assert!(
        !raw.contains(&connection.secret_key),
        "store leaked the secret: {raw}"
    );

    assert_eq!(instances::list().expect("list").len(), 1);
    instances::delete(&saved.id).expect("delete");
    assert!(instances::list().expect("list").is_empty());

    std::fs::remove_dir_all(path.parent().expect("parent")).ok();
}
