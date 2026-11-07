use crate::error::{AppError, AppResult};
use keyring::Entry;

/// Keyring service name; the individual keys carry the `rc://` prefix.
const SERVICE: &str = "rustfs-config";
const INSTANCE_PREFIX: &str = "rc://instance/";
const CREDENTIAL_PREFIX: &str = "rc://credential/";

/// Logical keyring key for an instance secret: `rc://instance/<id>`.
pub fn instance_key(id: &str) -> String {
    format!("{INSTANCE_PREFIX}{id}")
}

/// Logical keyring key for a generated credential: `rc://credential/<id>`.
pub fn credential_key(id: &str) -> String {
    format!("{CREDENTIAL_PREFIX}{id}")
}

fn entry(key: &str) -> AppResult<Entry> {
    Entry::new(SERVICE, key).map_err(map_error)
}

/// Stores or replaces the secret for `key`.
pub fn set(key: &str, secret: &str) -> AppResult<()> {
    entry(key)?.set_password(secret).map_err(map_error)
}

/// Reads a secret. `None` means no secret is stored for this key.
pub fn get(key: &str) -> AppResult<Option<String>> {
    match entry(key)?.get_password() {
        Ok(secret) => Ok(Some(secret)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(map_error(e)),
    }
}

/// Removes a secret; a missing entry is not an error.
pub fn delete(key: &str) -> AppResult<()> {
    match entry(key)?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(map_error(e)),
    }
}

fn map_error(e: keyring::Error) -> AppError {
    let error = AppError::new("keyring", format!("Secret storage error: {e}"));
    match e {
        // No Secret Service (GNOME Keyring, KWallet) running on this machine.
        keyring::Error::NoStorageAccess(_) | keyring::Error::PlatformFailure(_) => error.with_hint(
            "No OS secret store available. Start gnome-keyring or KWallet; \
                 RC never falls back to plain-text storage.",
        ),
        _ => error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_use_the_documented_prefixes() {
        assert_eq!(instance_key("ins_01"), "rc://instance/ins_01");
        assert_eq!(credential_key("cred_01"), "rc://credential/cred_01");
    }

    /// Real Secret Service round-trip; run with `RC_TEST_KEYRING=1 cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn keyring_round_trip() {
        if std::env::var("RC_TEST_KEYRING").as_deref() != Ok("1") {
            return;
        }
        let key = instance_key(&format!(
            "test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        set(&key, "s3cr3t").expect("store secret");
        assert_eq!(get(&key).expect("read secret").as_deref(), Some("s3cr3t"));
        delete(&key).expect("delete secret");
        assert_eq!(get(&key).expect("read deleted secret"), None);
    }
}
