use serde::Serialize;

use crate::admin::AdminClient;
use crate::error::{AppError, AppResult};

/// How much power the connection key has, per SPEC §6.1.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyLevel {
    /// Instance root key: not an IAM user, full access.
    Root,
    /// IAM user allowed to manage users/policies (consoleAdmin or admin:*).
    Admin,
    /// Application key scoped to a bucket; IAM screens are unavailable.
    Limited,
    /// RustFS rejected the credentials.
    Invalid,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyQualification {
    pub level: KeyLevel,
    pub policy_name: Option<String>,
    pub detail: Option<String>,
}

/// Probes the admin API with the connection key itself: `list-users` tells whether the
/// key can manage IAM, then `user-info` tells a root key (no such IAM user) from an
/// admin user. Transport errors are returned as-is so the UI can show an actionable
/// connection message instead of blaming the credentials.
pub async fn qualify(admin: &AdminClient, access_key: &str) -> AppResult<KeyQualification> {
    match admin.list_users().await {
        Ok(_) => match admin.get_user(access_key).await {
            Ok(user) => Ok(KeyQualification {
                level: KeyLevel::Admin,
                policy_name: user.policy_name,
                detail: None,
            }),
            Err(err) if err.code == "NoSuchResource" || err.message.contains("does not exist") => {
                Ok(KeyQualification {
                    level: KeyLevel::Root,
                    policy_name: None,
                    detail: None,
                })
            }
            Err(err) => Err(err),
        },
        Err(err) if is_denied(&err) => Ok(KeyQualification {
            level: KeyLevel::Limited,
            policy_name: None,
            detail: None,
        }),
        Err(err) if is_auth_rejection(&err) => Ok(KeyQualification {
            level: KeyLevel::Invalid,
            policy_name: None,
            detail: Some(err.message),
        }),
        Err(err) => Err(err),
    }
}

fn is_denied(err: &AppError) -> bool {
    matches!(
        err.code.as_str(),
        "AccessDenied" | "AccessDeniedException" | "http_403"
    )
}

fn is_auth_rejection(err: &AppError) -> bool {
    matches!(
        err.code.as_str(),
        "InvalidAccessKeyId" | "SignatureDoesNotMatch" | "http_401"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denied_is_the_limited_case() {
        let err = AppError::new("AccessDenied", "denied");
        assert!(is_denied(&err));
        assert!(!is_auth_rejection(&err));
    }

    #[test]
    fn auth_rejections_are_not_limited() {
        for code in ["InvalidAccessKeyId", "SignatureDoesNotMatch", "http_401"] {
            let err = AppError::new(code, "nope");
            assert!(is_auth_rejection(&err), "{code}");
            assert!(!is_denied(&err), "{code}");
        }
    }
}
