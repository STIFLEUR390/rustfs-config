use app_lib::config::Connection;
use app_lib::instances;
use app_lib::lifecycle::{self, CorsSpec};
use app_lib::s3::S3Client;
use app_lib::store::{Instance, LifecycleRule, Store};
use aws_sdk_s3::types::{
    ExpirationStatus, LifecycleExpiration, LifecycleRule as SdkRule, LifecycleRuleFilter,
};

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

async fn purge_versions(connection: &Connection, bucket: &str) {
    use aws_sdk_s3::types::{Delete, ObjectIdentifier};
    let client = aws_sdk_s3::Client::from_conf(
        aws_sdk_s3::Config::builder()
            .behavior_version(aws_sdk_s3::config::BehaviorVersion::latest())
            .region(aws_sdk_s3::config::Region::new(connection.region.clone()))
            .credentials_provider(aws_sdk_s3::config::Credentials::new(
                &connection.access_key,
                &connection.secret_key,
                None,
                None,
                "rc-test",
            ))
            .endpoint_url(connection.endpoint.trim_end_matches('/'))
            .force_path_style(connection.path_style)
            .build(),
    );
    // RustFS removes only one version per key per call, so loop until drained.
    for _ in 0..100 {
        let listed = client
            .list_object_versions()
            .bucket(bucket)
            .send()
            .await
            .expect("list versions");
        let mut objects = Vec::new();
        for version in listed.versions() {
            if let (Some(key), Some(id)) = (version.key(), version.version_id()) {
                objects.push(
                    ObjectIdentifier::builder()
                        .key(key)
                        .version_id(id)
                        .build()
                        .expect("identifier"),
                );
            }
        }
        for marker in listed.delete_markers() {
            if let (Some(key), Some(id)) = (marker.key(), marker.version_id()) {
                objects.push(
                    ObjectIdentifier::builder()
                        .key(key)
                        .version_id(id)
                        .build()
                        .expect("identifier"),
                );
            }
        }
        if objects.is_empty() {
            return;
        }
        client
            .delete_objects()
            .bucket(bucket)
            .delete(Delete::builder().set_objects(Some(objects)).build().expect("delete"))
            .send()
            .await
            .expect("delete versions");
    }
}

fn cors(origin: &str) -> CorsSpec {
    CorsSpec {
        origins: vec![origin.to_string()],
        methods: vec!["GET".to_string()],
        headers: vec!["*".to_string()],
        max_age_seconds: Some(600),
    }
}

#[tokio::test]
async fn lifecycle_cors_and_versioning_are_idempotent() {
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

    let store_path = std::env::temp_dir().join(format!("rc-lifecycle-{}.json", unique("store")));
    std::env::set_var("RC_STORE_PATH", &store_path);
    let _ = std::fs::remove_file(&store_path);

    let instance = instances::save(
        Instance {
            label: "lifecycle-test".to_string(),
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
    let bucket = unique("rc-lifecycle-test");
    s3.create_bucket(&bucket).await.expect("create bucket");

    let legacy = SdkRule::builder()
        .id("legacy-expire")
        .status(ExpirationStatus::Enabled)
        .filter(LifecycleRuleFilter::builder().prefix("old/").build())
        .expiration(LifecycleExpiration::builder().days(30).build())
        .build()
        .expect("legacy rule");
    s3.put_bucket_lifecycle(&bucket, vec![legacy])
        .await
        .expect("seed legacy lifecycle");

    let rules = vec![LifecycleRule {
        prefix: "uploads/media/".to_string(),
        expire_days: 180,
    }];

    let plan = lifecycle::plan(&instance.id, &bucket, &rules, Some(true), None)
        .await
        .expect("plan");
    assert!(plan.rules_changed);
    assert_eq!(plan.versioning_target.as_deref(), Some("Enabled"));
    assert_eq!(
        plan.rule_diff
            .iter()
            .filter(|change| change.action == "add")
            .count(),
        2
    );

    let applied = lifecycle::apply(&instance.id, &bucket, &rules, Some(true), None, false)
        .await
        .expect("apply");
    assert!(applied.rules_changed);
    assert!(applied.probe.is_some());

    let remote = s3
        .get_bucket_lifecycle(&bucket)
        .await
        .expect("read lifecycle");
    assert_eq!(remote.len(), 3);
    assert!(remote.iter().any(|rule| rule.id() == Some("legacy-expire")));
    assert!(remote
        .iter()
        .any(|rule| rule.id() == Some("rc-expire-uploads-media")));
    assert!(remote
        .iter()
        .any(|rule| rule.id() == Some("rc-abort-incomplete-multipart")));
    assert_eq!(
        s3.get_bucket_versioning(&bucket)
            .await
            .expect("read versioning")
            .as_deref(),
        Some("Enabled")
    );
    let leftovers = s3
        .list_objects(&bucket, None)
        .await
        .expect("list probe leftovers");
    assert!(leftovers
        .iter()
        .all(|object| !object.key.contains("_rc-probe")));

    let again = lifecycle::apply(&instance.id, &bucket, &rules, Some(true), None, false)
        .await
        .expect("apply twice");
    assert!(!again.rules_changed);
    assert!(again.rule_diff.is_empty());
    assert!(again.versioning_target.is_none());
    assert!(!again.cors_changed);

    let applied_cors = lifecycle::apply(
        &instance.id,
        &bucket,
        &rules,
        None,
        Some(cors("https://example.test")),
        false,
    )
    .await
    .expect("apply cors");
    assert!(applied_cors.cors_changed);
    assert!(!applied_cors.cors_conflict);

    let changed = cors("https://other.test");
    let conflict = lifecycle::plan(&instance.id, &bucket, &rules, None, Some(changed.clone()))
        .await
        .expect("plan changed cors");
    assert!(conflict.cors_conflict);
    let error = lifecycle::apply(
        &instance.id,
        &bucket,
        &rules,
        None,
        Some(changed.clone()),
        false,
    )
    .await
    .expect_err("overwrite needs confirmation");
    assert_eq!(error.code, "cors_overwrite_confirmation");
    let applied_changed =
        lifecycle::apply(&instance.id, &bucket, &rules, None, Some(changed), true)
            .await
            .expect("confirmed cors overwrite");
    assert!(applied_changed.cors_changed);

    let store = Store::load().expect("store");
    let entry = store
        .buckets
        .iter()
        .find(|item| item.instance_id == instance.id && item.name == bucket)
        .expect("bucket entry");
    assert_eq!(entry.lifecycle, rules);
    assert!(entry.managed.lifecycle);

    purge_versions(&root, &bucket).await;
    s3.delete_bucket(&bucket).await.expect("delete bucket");
    let _ = std::fs::remove_file(&store_path);
}
