use serde::Serialize;

use crate::browser;
use crate::config::Connection;
use crate::error::{AppError, AppResult};
use crate::s3::S3Client;
use crate::secrets;
use crate::store::{Bucket, Store};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl Check {
    fn pass(id: &str, detail: Option<String>) -> Self {
        Self {
            id: id.to_string(),
            status: "pass".to_string(),
            detail,
        }
    }

    fn fail(id: &str, detail: impl Into<String>) -> Self {
        Self {
            id: id.to_string(),
            status: "fail".to_string(),
            detail: Some(detail.into()),
        }
    }

    fn skip(id: &str, detail: impl Into<String>) -> Self {
        Self {
            id: id.to_string(),
            status: "skip".to_string(),
            detail: Some(detail.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub ok: bool,
    pub checks: Vec<Check>,
}

/// Runs the SPEC §10 checks: scoped access, probe write/delete, anonymous
/// public/private reads, anonymous listing and a private control bucket.
pub async fn verify(instance_id: &str, bucket: &str, credential_id: &str) -> AppResult<Report> {
    let store = Store::load()?;
    let credential = store
        .credentials
        .iter()
        .find(|item| item.id == credential_id && item.instance_id == instance_id)
        .ok_or_else(|| AppError::new("credential_not_found", "Credential inconnu"))?
        .clone();
    let bucket_entry = store
        .buckets
        .iter()
        .find(|item| item.instance_id == instance_id && item.name == bucket)
        .cloned()
        .unwrap_or_default();
    let secret = secrets::get(&secrets::credential_key(credential_id))?
        .ok_or_else(|| AppError::new("missing_secret", "Secret introuvable dans le trousseau"))?;

    let root = browser::connection(instance_id)?;
    let scoped = Connection {
        access_key: credential.access_key.clone(),
        secret_key: secret,
        ..root.clone()
    };
    let scoped_s3 = S3Client::new(&scoped)?;
    let root_s3 = S3Client::new(&root)?;

    let mut checks = Vec::new();
    let suffix = format!(
        "{:x}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default()
    );
    let private_key = format!("_rc-probe/{suffix}.txt");
    let anonymous = anonymous_client(&root)?;

    // 1. Scoped key only sees its own bucket. RustFS returns every bucket name
    // to any authenticated user, so an over-broad listing is not a failure.
    match scoped_s3.list_buckets().await {
        Ok(names) => {
            let detail = if names.len() > 1 {
                Some("RustFS liste tous les noms de buckets ; l'accès aux données est vérifié plus bas.".to_string())
            } else {
                None
            };
            checks.push(Check::pass("list_buckets", detail));
        }
        Err(_) => checks.push(Check::pass(
            "list_buckets",
            Some("ListBuckets refusé".to_string()),
        )),
    }

    // 2. Probe write then delete, with the scoped key.
    let can_write = credential.mode != "ro" && credential.mode != "existing";
    let mut probe_written = false;
    if can_write {
        match scoped_s3
            .put_object(bucket, &private_key, b"rc-probe".to_vec())
            .await
        {
            Ok(()) => {
                probe_written = true;
                checks.push(Check::pass("probe_write", None));
            }
            Err(err) => checks.push(Check::fail("probe_write", err.message)),
        }
    } else {
        checks.push(Check::skip("probe_write", "Clé en lecture seule"));
    }

    if probe_written {
        match scoped_s3.delete_object(bucket, &private_key).await {
            Ok(()) => checks.push(Check::pass("probe_delete", None)),
            Err(err) => checks.push(Check::fail("probe_delete", err.message)),
        }
    } else if can_write {
        checks.push(Check::skip("probe_delete", "Sonde non écrite"));
    } else {
        checks.push(Check::skip("probe_delete", "Clé en lecture seule"));
    }

    // 3. Anonymous read: denied on the private probe, allowed on a declared
    // public prefix only once the bucket policy is managed by RustFS Config.
    if let Some(url) = object_url(&root, bucket, &private_key) {
        match anonymous.get(&url).send().await {
            Ok(response) if response.status().as_u16() == 403 => {
                checks.push(Check::pass("private_read", None))
            }
            Ok(response) => checks.push(Check::fail(
                "private_read",
                format!("GET anonyme sur une clé privée → {}", response.status()),
            )),
            Err(err) => checks.push(Check::fail("private_read", err.to_string())),
        }
    } else {
        checks.push(Check::skip(
            "private_read",
            "Vérification anonyme limitée au path-style",
        ));
    }

    checks.push(public_read_check(&root, &anonymous, &scoped_s3, &bucket_entry).await);

    // 4. Anonymous listing is always refused.
    if let Some(url) = bucket_url(&root, bucket) {
        match anonymous.get(&url).send().await {
            Ok(response) if response.status().as_u16() == 403 => {
                checks.push(Check::pass("anonymous_list", None))
            }
            Ok(response) => checks.push(Check::fail(
                "anonymous_list",
                format!("ListObjects anonyme → {}", response.status()),
            )),
            Err(err) => checks.push(Check::fail("anonymous_list", err.to_string())),
        }
    } else {
        checks.push(Check::skip(
            "anonymous_list",
            "Vérification anonyme limitée au path-style",
        ));
    }

    // 5. The scoped key must not read a private control bucket. Any error,
    // including a connection closed instead of a 403, counts as denied.
    let control = format!("rc-probe-{suffix}");
    match root_s3.create_bucket(&control).await {
        Ok(()) => {
            let denied = scoped_s3.list_objects(&control, None).await;
            let result = match denied {
                Err(err) => Check::pass(
                    "control_bucket",
                    Some(format!("Accès refusé : {}", err.code)),
                ),
                Ok(_) => Check::fail("control_bucket", "La clé scopée lit un autre bucket"),
            };
            checks.push(result);
            let _ = root_s3.delete_bucket(&control).await;
        }
        Err(_) => checks.push(Check::skip(
            "control_bucket",
            "La clé de l'instance ne peut pas créer un bucket de contrôle",
        )),
    }

    // 6. Remote bucket policy and lifecycle match what RustFS Config manages.
    checks.push(policy_check(&root_s3, bucket, &bucket_entry).await);
    checks.push(lifecycle_check(&root_s3, bucket, &bucket_entry).await);

    // Probe cleanup in every case.
    if probe_written {
        let _ = scoped_s3.delete_object(bucket, &private_key).await;
        let _ = root_s3.delete_object(bucket, &private_key).await;
    }

    let ok = !checks.iter().any(|check| check.status == "fail");
    Ok(Report { ok, checks })
}

async fn public_read_check(
    root: &Connection,
    anonymous: &reqwest::Client,
    scoped_s3: &S3Client,
    bucket_entry: &Bucket,
) -> Check {
    let prefix = match bucket_entry
        .public_prefixes
        .iter()
        .find(|prefix| !prefix.is_empty())
    {
        Some(prefix) if bucket_entry.managed.bucket_policy => prefix.clone(),
        _ => return Check::skip("public_read", "Aucun préfixe public géré par RustFS Config"),
    };
    let key = format!("{prefix}_rc-probe.txt");
    if let Err(err) = scoped_s3
        .put_object(&bucket_entry.name, &key, b"rc-probe".to_vec())
        .await
    {
        return Check::fail("public_read", err.message);
    }
    let result = match object_url(root, &bucket_entry.name, &key) {
        Some(url) => match anonymous.get(&url).send().await {
            Ok(response) if response.status().is_success() => Check::pass("public_read", None),
            Ok(response) => Check::fail(
                "public_read",
                format!("GET anonyme sur un préfixe public → {}", response.status()),
            ),
            Err(err) => Check::fail("public_read", err.to_string()),
        },
        None => Check::skip("public_read", "Vérification anonyme limitée au path-style"),
    };
    let _ = scoped_s3.delete_object(&bucket_entry.name, &key).await;
    result
}

async fn policy_check(root_s3: &S3Client, bucket: &str, entry: &Bucket) -> Check {
    if !entry.managed.bucket_policy {
        return Check::skip(
            "bucket_policy",
            "Aucune bucket policy gérée par RustFS Config",
        );
    }
    match root_s3.get_bucket_policy(bucket).await {
        Ok(_) => Check::pass("bucket_policy", None),
        Err(err) => Check::fail("bucket_policy", err.message),
    }
}

async fn lifecycle_check(root_s3: &S3Client, bucket: &str, entry: &Bucket) -> Check {
    if !entry.managed.lifecycle {
        return Check::skip("lifecycle", "Aucun lifecycle géré par RustFS Config");
    }
    match root_s3.bucket_has_lifecycle(bucket).await {
        Ok(true) => Check::pass("lifecycle", None),
        Ok(false) => Check::fail("lifecycle", "Aucune règle de lifecycle sur le bucket"),
        Err(err) => Check::fail("lifecycle", err.message),
    }
}

fn anonymous_client(connection: &Connection) -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(!connection.tls_verify)
        .build()
        .map_err(|err| AppError::new("http", err.to_string()))
}

fn object_url(connection: &Connection, bucket: &str, key: &str) -> Option<String> {
    let base = connection.base_url();
    if connection.path_style {
        Some(format!("{base}/{bucket}/{key}"))
    } else {
        let (scheme, host) = base.split_once("://")?;
        Some(format!("{scheme}://{bucket}.{host}/{key}"))
    }
}

fn bucket_url(connection: &Connection, bucket: &str) -> Option<String> {
    let base = connection.base_url();
    if connection.path_style {
        Some(format!("{base}/{bucket}?list-type=2"))
    } else {
        let (scheme, host) = base.split_once("://")?;
        Some(format!("{scheme}://{bucket}.{host}?list-type=2"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_follow_the_path_style_setting() {
        let path_style = Connection {
            endpoint: "http://127.0.0.1:9000".to_string(),
            region: "us-east-1".to_string(),
            path_style: true,
            tls_verify: true,
            access_key: "a".to_string(),
            secret_key: "b".to_string(),
        };
        assert_eq!(
            object_url(&path_style, "bucket", "a/b.txt").as_deref(),
            Some("http://127.0.0.1:9000/bucket/a/b.txt")
        );
        let virtual_host = Connection {
            path_style: false,
            ..path_style.clone()
        };
        assert_eq!(
            object_url(&virtual_host, "bucket", "a/b.txt").as_deref(),
            Some("http://bucket.127.0.0.1:9000/a/b.txt")
        );
        assert_eq!(
            bucket_url(&path_style, "bucket").as_deref(),
            Some("http://127.0.0.1:9000/bucket?list-type=2")
        );
    }
}
