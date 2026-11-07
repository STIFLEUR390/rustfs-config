use serde::Deserialize;

/// Credentials + endpoint used to talk to one RustFS instance.
///
/// Secrets live in memory only (the OS keyring store is wired in a later milestone).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub endpoint: String,
    #[serde(default = "default_region")]
    pub region: String,
    #[serde(default = "default_true")]
    pub path_style: bool,
    #[serde(default = "default_true")]
    pub tls_verify: bool,
    pub access_key: String,
    pub secret_key: String,
}

impl Connection {
    /// Base URL without a trailing slash.
    pub fn base_url(&self) -> String {
        self.endpoint.trim_end_matches('/').to_string()
    }
}

fn default_true() -> bool {
    true
}

fn default_region() -> String {
    "us-east-1".to_string()
}
