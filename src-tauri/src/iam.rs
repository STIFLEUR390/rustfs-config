use serde::Serialize;
use serde_json::{json, Value};

use crate::backend::NativeBackend;
use crate::browser;
use crate::error::{AppError, AppResult};
use crate::secrets;
use crate::store::{Bucket, Credential, Store};

pub const MODES: [&str; 3] = ["rw", "ro", "rw-sans-suppression"];

/// The secret is returned once, at creation or rotation. `None` means the
/// credential was adopted: RustFS already knows a secret we never saw.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssuedCredential {
    pub credential: Credential,
    pub secret: Option<String>,
}

/// Builds the SPEC §7.1 policy for one bucket.
pub fn policy_document(bucket: &str, mode: &str) -> AppResult<Value> {
    let read = ["s3:GetObject", "s3:GetObjectVersion"];
    let write = [
        "s3:PutObject",
        "s3:DeleteObject",
        "s3:AbortMultipartUpload",
        "s3:ListMultipartUploadParts",
    ];
    let object_actions: Vec<&str> = match mode {
        "ro" => read.to_vec(),
        "rw" | "rw-sans-suppression" => read.iter().chain(write.iter()).copied().collect(),
        other => {
            return Err(AppError::new(
                "invalid_mode",
                format!("Mode de credential inconnu : {other}"),
            )
            .with_hint("Modes acceptés : rw, ro, rw-sans-suppression."))
        }
    };

    let mut statements = vec![json!({
        "Sid": "ListBucket",
        "Effect": "Allow",
        "Action": ["s3:ListBucket", "s3:GetBucketLocation"],
        "Resource": [format!("arn:aws:s3:::{bucket}")],
    })];
    statements.push(json!({
        "Sid": "ObjectAccess",
        "Effect": "Allow",
        "Action": object_actions,
        "Resource": [format!("arn:aws:s3:::{bucket}/*")],
    }));
    if mode == "rw-sans-suppression" {
        // An explicit Deny always wins, whatever another statement allows.
        statements.push(json!({
            "Sid": "DenyDelete",
            "Effect": "Deny",
            "Action": ["s3:DeleteObject", "s3:DeleteObjectVersion"],
            "Resource": [format!("arn:aws:s3:::{bucket}/*")],
        }));
    }
    Ok(json!({ "Version": "2012-10-17", "Statement": statements }))
}

pub fn policy_name(bucket: &str, mode: &str) -> String {
    format!("rc-{bucket}-{mode}")
}

pub fn list(instance_id: &str, bucket: &str) -> AppResult<Vec<Credential>> {
    Ok(Store::load()?
        .credentials
        .into_iter()
        .filter(|item| item.instance_id == instance_id && item.bucket == bucket)
        .collect())
}

/// Creates a user scoped to one bucket, attaches the generated policy and
/// stores the secret in the keyring. The secret is returned exactly once.
pub async fn create(
    instance_id: &str,
    bucket: &str,
    mode: &str,
    access_key: Option<String>,
) -> AppResult<IssuedCredential> {
    if !MODES.contains(&mode) {
        return Err(AppError::new(
            "invalid_mode",
            format!("Mode de credential inconnu : {mode}"),
        ));
    }
    let access_key = access_key
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("{bucket}-app"));
    if Store::load()?
        .credentials
        .iter()
        .any(|item| item.instance_id == instance_id && item.access_key == access_key)
    {
        return Err(AppError::new(
            "credential_exists",
            format!("La clé {access_key} est déjà gérée par RustFS Config"),
        )
        .with_hint("Faites tourner son secret au lieu de la recréer."));
    }

    let connection = browser::connection(instance_id)?;
    let backend = NativeBackend::new(&connection)?;
    if backend.admin.get_user(&access_key).await.is_ok() {
        return Err(AppError::new(
            "user_exists",
            format!("L'utilisateur {access_key} existe déjà sur cette instance"),
        )
        .with_hint(
            "Adoptez-le pour le gérer sans le modifier, ou faites tourner son secret avec confirmation.",
        ));
    }

    let policy_name = policy_name(bucket, mode);
    let document = policy_document(bucket, mode)?;
    let document =
        serde_json::to_string(&document).map_err(|err| AppError::new("json", err.to_string()))?;

    backend.admin.add_policy(&policy_name, &document).await?;
    let secret = generate_secret();
    if let Err(err) = backend.admin.add_user(&access_key, &secret).await {
        let _ = backend.admin.remove_policy(&policy_name).await;
        return Err(err);
    }
    if let Err(err) = backend.admin.attach_policy(&policy_name, &access_key).await {
        let _ = backend.admin.remove_user(&access_key).await;
        let _ = backend.admin.remove_policy(&policy_name).await;
        return Err(err);
    }

    let credential = Credential {
        id: format!("cred_{:x}", nanos()),
        instance_id: instance_id.to_string(),
        bucket: bucket.to_string(),
        access_key,
        policy_name,
        mode: mode.to_string(),
        created_at: now_iso(),
        rotated_at: None,
    };
    secrets::set(&secrets::credential_key(&credential.id), &secret)?;
    remember(&credential)?;
    Ok(IssuedCredential {
        credential,
        secret: Some(secret),
    })
}

/// Records an existing RustFS user without changing it on the server. Its
/// secret stays unknown until it is rotated.
pub async fn adopt(
    instance_id: &str,
    bucket: &str,
    access_key: &str,
) -> AppResult<IssuedCredential> {
    let access_key = access_key.trim();
    if access_key.is_empty() {
        return Err(AppError::new(
            "invalid_access_key",
            "L'access key est requise",
        ));
    }
    if Store::load()?
        .credentials
        .iter()
        .any(|item| item.instance_id == instance_id && item.access_key == access_key)
    {
        return Err(AppError::new(
            "credential_exists",
            format!("La clé {access_key} est déjà gérée par RustFS Config"),
        ));
    }
    let connection = browser::connection(instance_id)?;
    let backend = NativeBackend::new(&connection)?;
    let user = backend.admin.get_user(access_key).await?;
    let credential = Credential {
        id: format!("cred_{:x}", nanos()),
        instance_id: instance_id.to_string(),
        bucket: bucket.to_string(),
        access_key: access_key.to_string(),
        policy_name: user.policy_name.unwrap_or_default(),
        mode: "existing".to_string(),
        created_at: now_iso(),
        rotated_at: None,
    };
    remember(&credential)?;
    Ok(IssuedCredential {
        credential,
        secret: None,
    })
}

/// Replaces the secret on RustFS and in the keyring. The old secret stops
/// working immediately.
pub async fn rotate(instance_id: &str, credential_id: &str) -> AppResult<String> {
    let mut store = Store::load()?;
    let credential = find(&store, instance_id, credential_id)?.clone();
    let connection = browser::connection(instance_id)?;
    let backend = NativeBackend::new(&connection)?;
    let secret = generate_secret();
    backend
        .admin
        .add_user(&credential.access_key, &secret)
        .await?;
    secrets::set(&secrets::credential_key(&credential.id), &secret)?;
    if let Some(slot) = store
        .credentials
        .iter_mut()
        .find(|item| item.id == credential_id)
    {
        slot.rotated_at = Some(now_iso());
    }
    store.save()?;
    Ok(secret)
}

/// Disables, detaches then deletes the user. A policy is removed only when RC
/// created it (`rc-` prefix) and no user is left on it.
pub async fn revoke(instance_id: &str, credential_id: &str) -> AppResult<()> {
    let mut store = Store::load()?;
    let credential = find(&store, instance_id, credential_id)?.clone();
    let connection = browser::connection(instance_id)?;
    let backend = NativeBackend::new(&connection)?;

    backend
        .admin
        .detach_policy(&credential.policy_name, &credential.access_key)
        .await
        .ok();
    backend
        .admin
        .set_user_status(&credential.access_key, "disabled")
        .await
        .ok();
    backend.admin.remove_user(&credential.access_key).await?;

    if credential.policy_name.starts_with("rc-") {
        let still_used = backend
            .admin
            .policy_entities()
            .await
            .map(|mappings| {
                mappings.iter().any(|mapping| {
                    mapping.policy == credential.policy_name && !mapping.users.is_empty()
                })
            })
            .unwrap_or(true);
        if !still_used {
            backend
                .admin
                .remove_policy(&credential.policy_name)
                .await
                .ok();
        }
    }

    store.credentials.retain(|item| item.id != credential_id);
    store.save()?;
    secrets::delete(&secrets::credential_key(credential_id))?;
    Ok(())
}

fn find<'a>(store: &'a Store, instance_id: &str, credential_id: &str) -> AppResult<&'a Credential> {
    store
        .credentials
        .iter()
        .find(|item| item.id == credential_id && item.instance_id == instance_id)
        .ok_or_else(|| AppError::new("credential_not_found", "Credential inconnu"))
}

fn remember(credential: &Credential) -> AppResult<()> {
    let mut store = Store::load()?;
    store.credentials.push(credential.clone());
    if let Some(bucket) = store
        .buckets
        .iter_mut()
        .find(|item| item.instance_id == credential.instance_id && item.name == credential.bucket)
    {
        bucket.managed.iam = true;
    } else {
        store.buckets.push(Bucket {
            instance_id: credential.instance_id.clone(),
            name: credential.bucket.clone(),
            managed: crate::store::Managed {
                iam: true,
                ..Default::default()
            },
            ..Default::default()
        });
    }
    store.save()
}

fn generate_secret() -> String {
    use rand::distr::{Alphanumeric, SampleString};
    Alphanumeric.sample_string(&mut rand::rng(), 40)
}

fn nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

fn now_iso() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_policy_has_no_write_actions() {
        let document = policy_document("kwiktalk", "ro").unwrap();
        let text = document.to_string();
        assert!(text.contains("s3:GetObject"));
        assert!(!text.contains("s3:PutObject"));
        assert!(!text.contains("s3:DeleteObject"));
        assert!(!text.contains("Deny"));
    }

    #[test]
    fn rw_sans_suppression_denies_delete() {
        let document = policy_document("kwiktalk", "rw-sans-suppression").unwrap();
        let text = document.to_string();
        assert!(text.contains("\"Effect\":\"Deny\""));
        assert!(text.contains("s3:DeleteObject"));
        assert_eq!(document["Statement"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn scoped_policy_only_points_at_the_bucket() {
        let document = policy_document("kwiktalk", "rw").unwrap();
        let resources = document["Statement"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|statement| statement["Resource"].as_array().unwrap().clone())
            .map(|value| value.as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            resources,
            vec![
                "arn:aws:s3:::kwiktalk".to_string(),
                "arn:aws:s3:::kwiktalk/*".to_string()
            ]
        );
    }

    #[test]
    fn unknown_mode_is_rejected() {
        assert_eq!(
            policy_document("kwiktalk", "root").unwrap_err().code,
            "invalid_mode"
        );
    }

    #[test]
    fn secrets_are_long_and_unique() {
        let first = generate_secret();
        let second = generate_secret();
        assert_eq!(first.len(), 40);
        assert!(first.chars().all(|c| c.is_ascii_alphanumeric()));
        assert_ne!(first, second);
    }

    #[test]
    fn policy_name_is_prefixed_with_rc() {
        assert_eq!(policy_name("kwiktalk", "rw"), "rc-kwiktalk-rw");
    }
}
