use app_lib::config::Connection;
use app_lib::env;
use app_lib::iam;
use app_lib::instances;
use app_lib::s3::S3Client;
use app_lib::store::Instance;

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

fn parse(text: &str) -> std::collections::HashMap<String, String> {
    text.lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

#[tokio::test]
async fn laravel_env_works_end_to_end() {
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

    let store_path = std::env::temp_dir().join(format!("rc-env-{}.json", unique("store")));
    std::env::set_var("RC_STORE_PATH", &store_path);
    let _ = std::fs::remove_file(&store_path);

    let instance = instances::save(
        Instance {
            label: "env-test".to_string(),
            endpoint: endpoint.clone(),
            ..Default::default()
        },
        Some(root_key.clone()),
        Some(root_secret.clone()),
    )
    .expect("save instance");

    let root = Connection {
        endpoint: endpoint.clone(),
        region: "us-east-1".to_string(),
        path_style: true,
        tls_verify: true,
        access_key: root_key,
        secret_key: root_secret,
    };
    let root_s3 = S3Client::new(&root).expect("root client");
    let bucket = unique("rc-env-test");
    root_s3.create_bucket(&bucket).await.expect("create bucket");

    let issued = iam::create(&instance.id, &bucket, "rw", None)
        .await
        .expect("create credential");
    let secret = issued.secret.clone().expect("secret once");

    let text = env::generate(&instance.id, &issued.credential.id, "laravel").expect("env");
    let values = parse(&text);
    assert_eq!(values["FILESYSTEM_DISK"], "s3");
    assert_eq!(values["AWS_ACCESS_KEY_ID"], issued.credential.access_key);
    assert_eq!(values["AWS_SECRET_ACCESS_KEY"], secret);
    assert_eq!(values["AWS_BUCKET"], bucket);
    assert_eq!(values["AWS_ENDPOINT"], endpoint);
    assert_eq!(values["AWS_URL"], format!("{endpoint}/{bucket}"));
    assert_eq!(values["AWS_USE_PATH_STYLE_ENDPOINT"], "true");
    // Plain HTTP endpoints must not carry AWS_SSL_VERIFY (SPEC §8).
    assert!(!values.contains_key("AWS_SSL_VERIFY"));

    let scoped = Connection {
        endpoint: values["AWS_ENDPOINT"].clone(),
        region: values["AWS_DEFAULT_REGION"].clone(),
        path_style: values["AWS_USE_PATH_STYLE_ENDPOINT"] == "true",
        tls_verify: true,
        access_key: values["AWS_ACCESS_KEY_ID"].clone(),
        secret_key: values["AWS_SECRET_ACCESS_KEY"].clone(),
    };
    let scoped_s3 = S3Client::new(&scoped).expect("scoped client");
    scoped_s3
        .put_object(&bucket, "probe/env.txt", b"from-env".to_vec())
        .await
        .expect("upload with generated credentials");
    let body = scoped_s3
        .get_object(&bucket, "probe/env.txt")
        .await
        .expect("read with generated credentials");
    assert_eq!(body, b"from-env");

    let stored = std::fs::read_to_string(&store_path).expect("store file");
    assert!(!stored.contains(&secret));

    scoped_s3
        .delete_object(&bucket, "probe/env.txt")
        .await
        .expect("delete object");
    root_s3.delete_bucket(&bucket).await.expect("delete bucket");
    let _ = std::fs::remove_file(&store_path);
}
