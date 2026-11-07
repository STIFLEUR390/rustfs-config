use std::time::SystemTime;

use aws_credential_types::Credentials;
use aws_sigv4::http_request::{
    sign, PayloadChecksumKind, SignableBody, SignableRequest, SigningSettings,
};
use aws_sigv4::sign::v4;
use serde::{Deserialize, Serialize};

use crate::config::Connection;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserRecord {
    pub access_key: String,
    pub status: String,
    pub policy_name: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyEntry {
    pub name: String,
    pub document: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyMapping {
    pub policy: String,
    pub users: Vec<String>,
    pub groups: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawUser {
    policy_name: Option<String>,
    status: String,
    updated_at: Option<String>,
}

#[derive(Deserialize)]
struct RawPolicyInfo {
    policy: serde_json::Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMappings {
    policy_mappings: Vec<PolicyMapping>,
}

pub struct AdminClient {
    http: reqwest::Client,
    base: String,
    region: String,
    access_key: String,
    secret_key: String,
}

impl AdminClient {
    pub fn new(conn: &Connection) -> AppResult<Self> {
        let http = reqwest::Client::builder()
            .danger_accept_invalid_certs(!conn.tls_verify)
            .build()
            .map_err(|e| AppError::new("http_client", e.to_string()))?;
        Ok(Self {
            http,
            base: conn.base_url(),
            region: conn.region.clone(),
            access_key: conn.access_key.clone(),
            secret_key: conn.secret_key.clone(),
        })
    }

    async fn call(
        &self,
        method: http::Method,
        path_and_query: &str,
        body: Option<&[u8]>,
    ) -> AppResult<Vec<u8>> {
        let url = format!("{}/rustfs/admin/v3/{}", self.base, path_and_query);
        let payload = body.unwrap_or_default();
        let builder = self.signed_request(method, &url, payload)?;
        let response = builder.send().await.map_err(|e| {
            AppError::new("connection", format!("{e}")).with_hint(
                "Vérifiez l'endpoint, le mode TLS et que l'instance RustFS est joignable.",
            )
        })?;
        let status = response.status().as_u16();
        let bytes = response
            .bytes()
            .await
            .map_err(|e| AppError::new("http_body", e.to_string()))?;
        if (200..300).contains(&status) {
            Ok(bytes.to_vec())
        } else {
            Err(parse_error(status, &bytes))
        }
    }

    fn signed_request(
        &self,
        method: http::Method,
        url: &str,
        body: &[u8],
    ) -> AppResult<reqwest::RequestBuilder> {
        let uri: http::Uri = url
            .parse()
            .map_err(|e| AppError::new("invalid_url", format!("{e}")))?;
        let host = uri
            .authority()
            .map(|a| a.as_str().to_string())
            .unwrap_or_default();

        let mut request = http::Request::builder()
            .method(method.clone())
            .uri(&uri)
            .header("host", &host)
            .body(body.to_vec())
            .map_err(|e| AppError::new("http_request", e.to_string()))?;

        let identity = Credentials::new(
            &self.access_key,
            &self.secret_key,
            None,
            None,
            "rustfs-config",
        )
        .into();
        // S3-signed requests (and RustFS admin, which mirrors S3) must carry
        // x-amz-content-sha256, otherwise the server rejects with "missing header".
        let mut settings = SigningSettings::default();
        settings.payload_checksum_kind = PayloadChecksumKind::XAmzSha256;
        let params = v4::SigningParams::builder()
            .identity(&identity)
            .region(&self.region)
            .name("s3")
            .time(SystemTime::now())
            .settings(settings)
            .build()
            .map_err(|e| AppError::new("signing", e.to_string()))?
            .into();

        let signable = SignableRequest::new(
            method.as_str(),
            url,
            [("host", host.as_str())].into_iter(),
            SignableBody::Bytes(body),
        )
        .map_err(|e| AppError::new("signing", e.to_string()))?;
        let (instructions, _signature) = sign(signable, &params)
            .map_err(|e| AppError::new("signing", e.to_string()))?
            .into_parts();
        instructions.apply_to_request_http1x(&mut request);

        let (parts, body) = request.into_parts();
        let mut builder = self.http.request(parts.method, parts.uri.to_string());
        for (name, value) in parts.headers.iter() {
            builder = builder.header(name, value);
        }
        Ok(builder.body(body))
    }

    pub async fn list_users(&self) -> AppResult<Vec<UserRecord>> {
        let body = self.call(http::Method::GET, "list-users", None).await?;
        let raw: std::collections::BTreeMap<String, RawUser> =
            serde_json::from_slice(&body).map_err(|e| AppError::new("json", e.to_string()))?;
        Ok(raw
            .into_iter()
            .map(|(access_key, user)| UserRecord {
                access_key,
                status: user.status,
                policy_name: user.policy_name,
                updated_at: user.updated_at,
            })
            .collect())
    }

    pub async fn get_user(&self, access_key: &str) -> AppResult<UserRecord> {
        let body = self
            .call(
                http::Method::GET,
                &format!("user-info?accessKey={}", urlencode(access_key)),
                None,
            )
            .await?;
        let raw: RawUser =
            serde_json::from_slice(&body).map_err(|e| AppError::new("json", e.to_string()))?;
        Ok(UserRecord {
            access_key: access_key.to_string(),
            status: raw.status,
            policy_name: raw.policy_name,
            updated_at: raw.updated_at,
        })
    }

    pub async fn add_user(&self, access_key: &str, secret_key: &str) -> AppResult<()> {
        let payload = serde_json::json!({ "secretKey": secret_key, "status": "enabled" });
        let body =
            serde_json::to_vec(&payload).map_err(|e| AppError::new("json", e.to_string()))?;
        self.call(
            http::Method::PUT,
            &format!("add-user?accessKey={}", urlencode(access_key)),
            Some(&body),
        )
        .await?;
        Ok(())
    }

    pub async fn set_user_status(&self, access_key: &str, status: &str) -> AppResult<()> {
        self.call(
            http::Method::PUT,
            &format!(
                "set-user-status?accessKey={}&status={}",
                urlencode(access_key),
                urlencode(status)
            ),
            None,
        )
        .await?;
        Ok(())
    }

    pub async fn remove_user(&self, access_key: &str) -> AppResult<()> {
        self.call(
            http::Method::DELETE,
            &format!("remove-user?accessKey={}", urlencode(access_key)),
            None,
        )
        .await?;
        Ok(())
    }

    pub async fn list_policies(&self) -> AppResult<Vec<PolicyEntry>> {
        let body = self
            .call(http::Method::GET, "list-canned-policies", None)
            .await?;
        let raw: std::collections::BTreeMap<String, serde_json::Value> =
            serde_json::from_slice(&body).map_err(|e| AppError::new("json", e.to_string()))?;
        Ok(raw
            .into_iter()
            .map(|(name, document)| PolicyEntry { name, document })
            .collect())
    }

    pub async fn get_policy(&self, name: &str) -> AppResult<serde_json::Value> {
        let body = self
            .call(
                http::Method::GET,
                &format!("info-canned-policy?name={}", urlencode(name)),
                None,
            )
            .await?;
        let raw: RawPolicyInfo =
            serde_json::from_slice(&body).map_err(|e| AppError::new("json", e.to_string()))?;
        Ok(raw.policy)
    }

    pub async fn add_policy(&self, name: &str, document: &str) -> AppResult<()> {
        self.call(
            http::Method::PUT,
            &format!("add-canned-policy?name={}", urlencode(name)),
            Some(document.as_bytes()),
        )
        .await?;
        Ok(())
    }

    pub async fn remove_policy(&self, name: &str) -> AppResult<()> {
        self.call(
            http::Method::DELETE,
            &format!("remove-canned-policy?name={}", urlencode(name)),
            None,
        )
        .await?;
        Ok(())
    }

    pub async fn attach_policy(&self, policy: &str, user: &str) -> AppResult<()> {
        self.call(
            http::Method::PUT,
            &format!(
                "set-user-or-group-policy?policyName={}&userOrGroup={}&isGroup=false",
                urlencode(policy),
                urlencode(user)
            ),
            None,
        )
        .await?;
        Ok(())
    }

    pub async fn detach_policy(&self, policy: &str, user: &str) -> AppResult<()> {
        let payload = serde_json::json!({ "policies": [policy], "user": user });
        let body =
            serde_json::to_vec(&payload).map_err(|e| AppError::new("json", e.to_string()))?;
        self.call(http::Method::POST, "idp/builtin/policy/detach", Some(&body))
            .await?;
        Ok(())
    }

    pub async fn policy_entities(&self) -> AppResult<Vec<PolicyMapping>> {
        let body = self
            .call(http::Method::GET, "idp/builtin/policy-entities", None)
            .await?;
        let raw: RawMappings =
            serde_json::from_slice(&body).map_err(|e| AppError::new("json", e.to_string()))?;
        Ok(raw.policy_mappings)
    }
}

fn urlencode(value: &str) -> String {
    // Admin keys/policy names are safe token characters; percent-encode the rest.
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

fn parse_error(status: u16, body: &[u8]) -> AppError {
    let text = String::from_utf8_lossy(body);
    if let (Some(code), Some(message)) = (extract_tag(&text, "Code"), extract_tag(&text, "Message"))
    {
        return with_hint(AppError::new(code, message));
    }
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
        if let (Some(code), Some(message)) = (
            json.get("code").and_then(|v| v.as_str()),
            json.get("message").and_then(|v| v.as_str()),
        ) {
            return with_hint(AppError::new(code, message));
        }
    }
    let message = text.trim();
    with_hint(AppError::new(
        format!("http_{status}"),
        if message.is_empty() {
            format!("HTTP {status}")
        } else {
            message.to_string()
        },
    ))
}

fn with_hint(err: AppError) -> AppError {
    match err.code.as_str() {
        "AccessDenied" | "AccessDeniedException" | "InvalidAccessKeyId" => {
            err.with_hint("La clé utilisée n'a pas les droits admin:* requis pour gérer l'IAM.")
        }
        "SignatureDoesNotMatch" => err
            .with_hint("Le secret ne correspond pas à l'access key, ou l'horloge système dérive."),
        _ => err,
    }
}

fn extract_tag(body: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = body.find(&open)? + open.len();
    let end = start + body[start..].find(&close)?;
    Some(body[start..end].to_string())
}
