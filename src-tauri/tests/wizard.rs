use app_lib::config::Connection;
use app_lib::env;
use app_lib::iam;
use app_lib::instances;
use app_lib::policy;
use app_lib::s3::S3Client;
use app_lib::{lifecycle, store::Instance, store::LifecycleRule};

fn env_var(name: &str) -> Option<String> {
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

#[tokio::test]
async fn wizard_produces_bucket_user_and_env_then_replays_without_change() {
    if env_var("RC_TEST_KEYRING").is_none() {
        eprintln!("RC_TEST_KEYRING unset, skipping");
        return;
    }
    let Some(root_key) = env_var("RUSTFS_TEST_ACCESS_KEY") else {
        eprintln!("RUSTFS_TEST_ACCESS_KEY unset, skipping");
        return;
    };
    let Some(root_secret) = env_var("RUSTFS_TEST_SECRET_KEY") else {
        eprintln!("RUSTFS_TEST_SECRET_KEY unset, skipping");
        return;
    };
    let endpoint =
        env_var("RUSTFS_TEST_ENDPOINT").unwrap_or_else(|| "http://127.0.0.1:9000".to_string());

    let store_path = std::env::temp_dir().join(format!("rc-wizard-{}.json", unique("store")));
    std::env::set_var("RC_STORE_PATH", &store_path);
    let _ = std::fs::remove_file(&store_path);

    let instance = instances::save(
        Instance {
            label: "wizard-test".to_string(),
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
    let root_s3 = S3Client::new(&root).expect("root client");
    let bucket = unique("rc-wizard-test");
    let prefixes = vec!["uploads/media/".to_string()];
    let rules = vec![LifecycleRule {
        prefix: "uploads/media/".to_string(),
        expire_days: 180,
    }];

    // Step 1: bucket.
    root_s3.create_bucket(&bucket).await.expect("create bucket");

    // Step 2: public prefix policy.
    let applied = policy::apply(&instance.id, &bucket, &prefixes, false)
        .await
        .expect("apply policy");
    assert!(applied.changed);

    // Step 3: scoped user.
    let issued = iam::create(&instance.id, &bucket, "rw", None)
        .await
        .expect("create credential");
    assert_eq!(issued.credential.policy_name, format!("rc-{bucket}-rw"));
    let secret = issued.secret.clone().expect("secret once");

    // Step 4: lifecycle.
    let applied = lifecycle::apply(&instance.id, &bucket, &rules, None, None, false)
        .await
        .expect("apply lifecycle");
    assert!(applied.rules_changed);

    // Deliverable 3: the .env uses the generated credentials.
    let text = env::generate(&instance.id, &issued.credential.id, "laravel").expect("env");
    assert!(text.contains(&format!(
        "AWS_ACCESS_KEY_ID={}",
        issued.credential.access_key
    )));
    assert!(text.contains(&format!("AWS_SECRET_ACCESS_KEY={secret}")));
    assert!(text.contains(&format!("AWS_URL={}", root.endpoint)));

    // Replaying the wizard must not change the remote state.
    let again = policy::apply(&instance.id, &bucket, &prefixes, false)
        .await
        .expect("replay policy");
    assert!(!again.changed);
    let lifecycle_again = lifecycle::apply(&instance.id, &bucket, &rules, None, None, false)
        .await
        .expect("replay lifecycle");
    assert!(!lifecycle_again.rules_changed);
    let duplicate = iam::create(&instance.id, &bucket, "rw", None).await;
    assert_eq!(
        duplicate.expect_err("second create must be refused").code,
        "credential_exists"
    );

    // Cleanup: revoke the user, drop the policy, delete the bucket.
    iam::revoke(&instance.id, &issued.credential.id)
        .await
        .expect("revoke");
    let _ = policy::apply(&instance.id, &bucket, &[], false).await;
    let _ = root_s3.delete_bucket(&bucket).await;
    let _ = std::fs::remove_file(&store_path);
}
