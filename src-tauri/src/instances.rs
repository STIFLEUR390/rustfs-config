use serde::Serialize;

use crate::backend::{NativeBackend, RustfsBackend};
use crate::config::Connection;
use crate::error::{AppError, AppResult};
use crate::qualify::{self, KeyLevel, KeyQualification};
use crate::secrets;
use crate::store::{Instance, Store};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceTest {
    pub buckets: Vec<String>,
    pub location: Option<String>,
    pub qualification: KeyQualification,
}

pub fn list() -> AppResult<Vec<Instance>> {
    Ok(Store::load()?.instances)
}

/// Connection probe of SPEC §6.1: ListBuckets + GetBucketLocation + key qualification.
pub async fn test(connection: &Connection) -> AppResult<InstanceTest> {
    let backend = NativeBackend::new(connection)?;
    let qualification = qualify::qualify(&backend.admin, &connection.access_key).await?;
    if qualification.level == KeyLevel::Invalid {
        return Err(AppError::new(
            "invalid_key",
            qualification
                .detail
                .clone()
                .unwrap_or_else(|| "RustFS rejected these credentials".to_string()),
        )
        .with_hint("Vérifiez l'access key, le secret et l'endpoint."));
    }

    // A bucket-scoped key legitimately cannot list every bucket.
    let buckets = match backend.list_buckets().await {
        Ok(buckets) => buckets,
        Err(_) if qualification.level == KeyLevel::Limited => Vec::new(),
        Err(err) => return Err(err),
    };
    let location = match buckets.first() {
        Some(bucket) => backend.get_bucket_location(bucket).await.ok(),
        None => None,
    };

    Ok(InstanceTest {
        buckets,
        location,
        qualification,
    })
}

/// Saves an instance and moves its credentials into the OS keyring. Existing
/// credentials are kept when `secret_key` is absent or empty (edit without re-typing).
pub fn save(
    mut instance: Instance,
    access_key: Option<String>,
    secret_key: Option<String>,
) -> AppResult<Instance> {
    instance.label = instance.label.trim().to_string();
    instance.endpoint = instance.endpoint.trim().trim_end_matches('/').to_string();
    if instance.label.is_empty() || instance.endpoint.is_empty() {
        return Err(AppError::new(
            "invalid_instance",
            "Un libellé et un endpoint sont requis",
        ));
    }

    let mut store = Store::load()?;
    if !instance.id.is_empty() && !store.instances.iter().any(|item| item.id == instance.id) {
        return Err(AppError::new("instance_not_found", "Instance inconnue"));
    }

    let stored = if instance.id.is_empty() {
        None
    } else {
        secrets::get_instance_credentials(&instance.id)?
    };
    let access_key = access_key
        .filter(|value| !value.trim().is_empty())
        .or_else(|| stored.as_ref().map(|c| c.access_key.clone()));
    let secret_key = secret_key
        .filter(|value| !value.is_empty())
        .or_else(|| stored.map(|c| c.secret_key));
    let (Some(access_key), Some(secret_key)) = (access_key, secret_key) else {
        return Err(AppError::new(
            "missing_credentials",
            "L'access key et le secret sont requis",
        ));
    };

    if instance.id.is_empty() {
        instance.id = format!(
            "ins_{:x}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        );
    }
    instance.admin_access_key_suffix = key_suffix(&access_key);

    secrets::set_instance_credentials(&instance.id, &access_key, &secret_key)?;
    match store
        .instances
        .iter_mut()
        .find(|item| item.id == instance.id)
    {
        Some(slot) => *slot = instance.clone(),
        None => store.instances.push(instance.clone()),
    }
    store.save()?;
    Ok(instance)
}

pub fn delete(id: &str) -> AppResult<()> {
    let mut store = Store::load()?;
    let credential_ids: Vec<String> = store
        .credentials
        .iter()
        .filter(|credential| credential.instance_id == id)
        .map(|credential| credential.id.clone())
        .collect();
    store.instances.retain(|instance| instance.id != id);
    store.buckets.retain(|bucket| bucket.instance_id != id);
    store.credentials.retain(|credential| credential.instance_id != id);
    store.save()?;

    secrets::delete(&secrets::instance_key(id))?;
    for credential_id in credential_ids {
        secrets::delete(&secrets::credential_key(&credential_id))?;
    }
    Ok(())
}

fn key_suffix(access_key: &str) -> String {
    let tail: String = access_key
        .chars()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("…{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suffix_shows_the_last_four_characters_only() {
        assert_eq!(key_suffix("kwiktalk-app"), "…-app");
        assert_eq!(key_suffix("abc"), "…abc");
    }
}
