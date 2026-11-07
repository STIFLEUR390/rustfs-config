use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const STORE_VERSION: u32 = 1;

/// Local, non-secret state. Secrets live in the OS keyring (see `secrets`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Store {
    pub version: u32,
    pub instances: Vec<Instance>,
    pub buckets: Vec<Bucket>,
    pub credentials: Vec<Credential>,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            version: STORE_VERSION,
            instances: Vec::new(),
            buckets: Vec::new(),
            credentials: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Instance {
    pub id: String,
    pub label: String,
    pub endpoint: String,
    pub region: String,
    pub path_style: bool,
    pub tls_verify: bool,
    pub console_url: Option<String>,
    pub admin_access_key_suffix: String,
    pub last_check: Option<LastCheck>,
}

impl Default for Instance {
    fn default() -> Self {
        Self {
            id: String::new(),
            label: String::new(),
            endpoint: String::new(),
            region: "us-east-1".into(),
            path_style: true,
            tls_verify: true,
            console_url: None,
            admin_access_key_suffix: String::new(),
            last_check: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LastCheck {
    pub at: String,
    pub ok: bool,
    pub buckets: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Bucket {
    pub instance_id: String,
    pub name: String,
    pub template: Option<String>,
    pub public_prefixes: Vec<String>,
    pub private_prefixes: Vec<String>,
    pub lifecycle: Vec<LifecycleRule>,
    pub managed: Managed,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LifecycleRule {
    pub prefix: String,
    pub expire_days: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Managed {
    pub bucket_policy: bool,
    pub lifecycle: bool,
    pub iam: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Credential {
    pub id: String,
    pub instance_id: String,
    pub bucket: String,
    pub access_key: String,
    pub policy_name: String,
    pub mode: String,
    pub created_at: String,
    pub rotated_at: Option<String>,
}

impl Store {
    /// `~/.config/rc/store.json` (or `$XDG_CONFIG_HOME/rc/store.json`).
    /// `RC_STORE_PATH` overrides it, for tests and portable setups.
    pub fn path() -> AppResult<PathBuf> {
        if let Ok(path) = std::env::var("RC_STORE_PATH") {
            return Ok(PathBuf::from(path));
        }
        dirs::config_dir()
            .map(|dir| dir.join("rc").join("store.json"))
            .ok_or_else(|| {
                AppError::new(
                    "no_config_dir",
                    "Cannot resolve the user configuration directory",
                )
            })
    }

    pub fn load() -> AppResult<Self> {
        Self::load_from(&Self::path()?)
    }

    pub fn load_from(path: &Path) -> AppResult<Self> {
        match std::fs::read_to_string(path) {
            Ok(raw) => serde_json::from_str(&raw).map_err(|e| {
                AppError::new(
                    "store_corrupt",
                    format!("Invalid store at {}: {e}", path.display()),
                )
                .with_hint("Fix or remove the file; it was left untouched.")
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(AppError::new(
                "store_read",
                format!("Cannot read {}: {e}", path.display()),
            )),
        }
    }

    pub fn save(&self) -> AppResult<()> {
        self.save_to(&Self::path()?)
    }

    /// Writes to a sibling temporary file then renames, so a crash never
    /// leaves a half-written store.
    pub fn save_to(&self, path: &Path) -> AppResult<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| {
                AppError::new(
                    "store_write",
                    format!("Cannot create {}: {e}", dir.display()),
                )
            })?;
        }
        let body = serde_json::to_vec_pretty(self)
            .map_err(|e| AppError::new("store_serialize", e.to_string()))?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, &body).map_err(|e| {
            AppError::new(
                "store_write",
                format!("Cannot write {}: {e}", tmp.display()),
            )
        })?;
        std::fs::rename(&tmp, path).map_err(|e| {
            AppError::new(
                "store_write",
                format!("Cannot replace {}: {e}", path.display()),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_store_path() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir()
            .join(format!("rc-store-test-{nanos}"))
            .join("store.json")
    }

    fn instance() -> Instance {
        Instance {
            id: "ins_01".into(),
            label: "Local".into(),
            endpoint: "http://127.0.0.1:9000".into(),
            admin_access_key_suffix: "…9XQ2".into(),
            ..Instance::default()
        }
    }

    #[test]
    fn instance_survives_reload() {
        let path = temp_store_path();
        let store = Store {
            instances: vec![instance()],
            ..Store::default()
        };
        store.save_to(&path).expect("save");
        let reloaded = Store::load_from(&path).expect("load");
        assert_eq!(reloaded, store);
        std::fs::remove_dir_all(path.parent().expect("parent")).ok();
    }

    #[test]
    fn store_file_never_contains_secrets() {
        let path = temp_store_path();
        let store = Store {
            instances: vec![instance()],
            credentials: vec![Credential {
                id: "cred_01".into(),
                instance_id: "ins_01".into(),
                bucket: "kwiktalk".into(),
                access_key: "kwiktalk-app".into(),
                policy_name: "rc-kwiktalk-rw".into(),
                mode: "rw".into(),
                created_at: "2026-11-07T09:10:00Z".into(),
                rotated_at: None,
            }],
            ..Store::default()
        };
        store.save_to(&path).expect("save");
        let raw = std::fs::read_to_string(&path).expect("read");
        let lower = raw.to_lowercase();
        for forbidden in ["secret", "password", "privatekey", "private_key"] {
            assert!(
                !lower.contains(forbidden),
                "store leaked {forbidden}: {raw}"
            );
        }
        std::fs::remove_dir_all(path.parent().expect("parent")).ok();
    }

    #[test]
    fn corrupt_store_errors_and_is_not_overwritten() {
        let path = temp_store_path();
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        std::fs::write(&path, "{ not json").expect("write");
        let err = Store::load_from(&path).expect_err("must reject corruption");
        assert_eq!(err.code, "store_corrupt");
        assert_eq!(std::fs::read_to_string(&path).expect("read"), "{ not json");
        std::fs::remove_dir_all(path.parent().expect("parent")).ok();
    }
}
