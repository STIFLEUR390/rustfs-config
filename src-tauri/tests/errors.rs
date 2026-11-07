use app_lib::config::Connection;
use app_lib::s3::S3Client;

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

fn root() -> Option<Connection> {
    Some(Connection {
        endpoint: env_var("RUSTFS_TEST_ENDPOINT")
            .unwrap_or_else(|| "http://127.0.0.1:9000".to_string()),
        region: "us-east-1".to_string(),
        path_style: true,
        tls_verify: true,
        access_key: env_var("RUSTFS_TEST_ACCESS_KEY")?,
        secret_key: env_var("RUSTFS_TEST_SECRET_KEY")?,
    })
}

/// Error cases from SPEC §12 must surface as readable `{ code, message }`, never a panic.
#[tokio::test]
async fn spec_error_cases_are_mapped() {
    let Some(connection) = root() else {
        eprintln!("RUSTFS_TEST_* unset, skipping");
        return;
    };
    let s3 = S3Client::new(&connection).expect("client");

    // Bucket non vide : suppression refusée avec un message lisible.
    let bucket = unique("rc-errors-test");
    s3.create_bucket(&bucket).await.expect("create bucket");
    s3.put_object(&bucket, "keep.txt", b"x".to_vec())
        .await
        .expect("put object");
    let not_empty = s3.delete_bucket(&bucket).await.expect_err("bucket is not empty");
    assert_eq!(not_empty.code, "s3_error");
    assert!(
        not_empty.message.to_lowercase().contains("not empty")
            || not_empty.message.to_lowercase().contains("bucketnotempty"),
        "unexpected message: {}",
        not_empty.message
    );
    s3.delete_object(&bucket, "keep.txt")
        .await
        .expect("cleanup object");
    s3.delete_bucket(&bucket).await.expect("cleanup bucket");

    // Bucket absent : 404 mappé sur `s3_not_found`.
    let missing = s3
        .head_bucket(&unique("rc-errors-missing"))
        .await
        .expect_err("bucket does not exist");
    assert_eq!(missing.code, "s3_not_found");

    // Clé invalide : refus d'authentification mappé, pas un panic.
    let mut wrong = connection.clone();
    wrong.secret_key = "definitely-wrong".to_string();
    let rejected = S3Client::new(&wrong)
        .expect("client")
        .list_buckets()
        .await
        .expect_err("bad credentials");
    assert!(
        matches!(
            rejected.code.as_str(),
            "s3_unauthorized" | "s3_forbidden" | "s3_error"
        ),
        "unexpected code: {}",
        rejected.code
    );

    // Endpoint injoignable : erreur de transport remontée proprement.
    let mut unreachable = connection.clone();
    unreachable.endpoint = "http://127.0.0.1:1".to_string();
    let failed = S3Client::new(&unreachable)
        .expect("client")
        .list_buckets()
        .await
        .expect_err("endpoint unreachable");
    assert_eq!(failed.code, "s3_error");
}
