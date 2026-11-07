use std::collections::HashSet;

use aws_sdk_s3::types::{
    AbortIncompleteMultipartUpload, CorsRule, ExpirationStatus, LifecycleExpiration,
    LifecycleRule as SdkRule, LifecycleRuleFilter,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::backend::NativeBackend;
use crate::error::{AppError, AppResult};
use crate::policy::normalize_prefix;
use crate::s3::S3Client;
use crate::store::{LifecycleRule, Store};

const MANAGED_ID_PREFIX: &str = "rc-";
const MULTIPART_ID: &str = "rc-abort-incomplete-multipart";
const MULTIPART_DAYS: i32 = 7;
const PROBE_SUFFIX: &str = "_rc-probe-lifecycle.txt";

/// Simple CORS configuration managed by RustFS Config (one rule).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CorsSpec {
    pub origins: Vec<String>,
    pub methods: Vec<String>,
    pub headers: Vec<String>,
    pub max_age_seconds: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigDiff {
    pub id: String,
    pub action: String,
    pub before: Option<Value>,
    pub after: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeReport {
    pub key: String,
    pub expiration: Option<String>,
    pub ok: bool,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BucketConfigPlan {
    pub bucket: String,
    pub rules: Vec<LifecycleRule>,
    pub rule_diff: Vec<ConfigDiff>,
    pub rules_changed: bool,
    pub versioning: Option<String>,
    pub versioning_target: Option<String>,
    pub cors: Option<CorsSpec>,
    pub cors_before: Vec<Value>,
    pub cors_changed: bool,
    pub cors_conflict: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probe: Option<ProbeReport>,
}

struct Prepared {
    plan: BucketConfigPlan,
    rules: Vec<SdkRule>,
    cors: Option<CorsRule>,
}

fn build_error(error: impl std::fmt::Display) -> AppError {
    AppError::new("lifecycle_build", error.to_string())
}

/// `uploads` → `rc-expire-uploads`; the empty prefix (whole bucket) → `rc-expire-bucket`.
pub fn expire_id(prefix: &str) -> String {
    let slug: String = prefix
        .trim_matches('/')
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        format!("{MANAGED_ID_PREFIX}expire-bucket")
    } else {
        format!("{MANAGED_ID_PREFIX}expire-{slug}")
    }
}

fn is_managed(rule: &SdkRule) -> bool {
    rule.id()
        .is_some_and(|id| id.starts_with(MANAGED_ID_PREFIX))
}

#[allow(deprecated)]
fn rule_prefix(rule: &SdkRule) -> String {
    rule.filter()
        .and_then(|filter| filter.prefix())
        .or_else(|| rule.prefix())
        .unwrap_or_default()
        .to_string()
}

fn rule_json(rule: &SdkRule) -> Value {
    json!({
        "id": rule.id(),
        "prefix": rule_prefix(rule),
        "expireDays": rule.expiration().and_then(|expiration| expiration.days()),
        "abortDays": rule
            .abort_incomplete_multipart_upload()
            .and_then(|abort| abort.days_after_initiation()),
        "status": rule.status().as_str(),
    })
}

fn cors_json(rule: &CorsRule) -> Value {
    json!({
        "origins": rule.allowed_origins(),
        "methods": rule.allowed_methods(),
        "headers": rule.allowed_headers(),
        "maxAgeSeconds": rule.max_age_seconds(),
    })
}

/// Rules RustFS Config owns: one expiration rule per prefix plus the multipart abort rule.
pub fn desired_rules(rules: &[LifecycleRule]) -> AppResult<Vec<SdkRule>> {
    let mut out = vec![SdkRule::builder()
        .id(MULTIPART_ID)
        .status(ExpirationStatus::Enabled)
        .filter(LifecycleRuleFilter::builder().prefix("").build())
        .abort_incomplete_multipart_upload(
            AbortIncompleteMultipartUpload::builder()
                .days_after_initiation(MULTIPART_DAYS)
                .build(),
        )
        .build()
        .map_err(build_error)?];
    let mut ids = HashSet::new();
    for rule in rules {
        if rule.expire_days == 0 {
            return Err(AppError::new(
                "lifecycle_days",
                "The expiration must be at least one day",
            ));
        }
        let prefix = normalize_prefix(&rule.prefix);
        let id = expire_id(&prefix);
        if !ids.insert(id.clone()) {
            return Err(AppError::new(
                "lifecycle_id_conflict",
                format!("Two prefixes map to the same lifecycle rule id ({id})"),
            ));
        }
        out.push(
            SdkRule::builder()
                .id(id)
                .status(ExpirationStatus::Enabled)
                .filter(LifecycleRuleFilter::builder().prefix(prefix).build())
                .expiration(
                    LifecycleExpiration::builder()
                        .days(rule.expire_days as i32)
                        .build(),
                )
                .build()
                .map_err(build_error)?,
        );
    }
    Ok(out)
}

/// Merges managed rules into the existing set. Rules without an `rc-` id are preserved.
pub fn merge(existing: &[SdkRule], desired: &[SdkRule]) -> (Vec<SdkRule>, Vec<ConfigDiff>) {
    let mut merged: Vec<SdkRule> = existing
        .iter()
        .filter(|rule| !is_managed(rule))
        .cloned()
        .collect();
    let desired_ids: HashSet<&str> = desired.iter().filter_map(|rule| rule.id()).collect();
    let mut diff = Vec::new();

    for rule in desired {
        let id = rule.id().unwrap_or_default().to_string();
        let after = rule_json(rule);
        let before = existing
            .iter()
            .find(|current| current.id() == Some(id.as_str()));
        match before {
            None => diff.push(ConfigDiff {
                id: id.clone(),
                action: "add".to_string(),
                before: None,
                after: Some(after),
            }),
            Some(current) if rule_json(current) != after => diff.push(ConfigDiff {
                id: id.clone(),
                action: "update".to_string(),
                before: Some(rule_json(current)),
                after: Some(after),
            }),
            Some(_) => {}
        }
        merged.push(rule.clone());
    }

    for rule in existing.iter().filter(|rule| is_managed(rule)) {
        let Some(id) = rule.id() else { continue };
        if !desired_ids.contains(id) {
            diff.push(ConfigDiff {
                id: id.to_string(),
                action: "remove".to_string(),
                before: Some(rule_json(rule)),
                after: None,
            });
        }
    }

    (merged, diff)
}

pub fn cors_rule(spec: &CorsSpec) -> AppResult<CorsRule> {
    if spec.origins.is_empty() || spec.methods.is_empty() {
        return Err(AppError::new(
            "cors_incomplete",
            "CORS needs at least one origin and one method",
        ));
    }
    let mut methods = Vec::new();
    for method in &spec.methods {
        let method = method.trim().to_ascii_uppercase();
        if !["GET", "PUT", "POST", "DELETE", "HEAD"].contains(&method.as_str()) {
            return Err(AppError::new(
                "cors_method",
                format!("Unsupported CORS method: {method}"),
            ));
        }
        methods.push(method);
    }
    let headers: Vec<String> = spec
        .headers
        .iter()
        .map(|header| header.trim().to_string())
        .filter(|header| !header.is_empty())
        .collect();
    let mut builder = CorsRule::builder()
        .set_allowed_origins(Some(spec.origins.clone()))
        .set_allowed_methods(Some(methods));
    if !headers.is_empty() {
        builder = builder.set_allowed_headers(Some(headers));
    }
    if let Some(age) = spec.max_age_seconds {
        builder = builder.max_age_seconds(age);
    }
    builder.build().map_err(build_error)
}

async fn prepare(
    s3: &S3Client,
    bucket: &str,
    rules: &[LifecycleRule],
    versioning: Option<bool>,
    cors: Option<&CorsSpec>,
) -> AppResult<Prepared> {
    let desired = desired_rules(rules)?;
    let existing = s3.get_bucket_lifecycle(bucket).await?;
    let (merged, rule_diff) = merge(&existing, &desired);

    let remote_versioning = s3.get_bucket_versioning(bucket).await?;
    let versioning_target = versioning.and_then(|enabled| {
        let target = if enabled { "Enabled" } else { "Suspended" };
        (remote_versioning.as_deref() != Some(target)).then(|| target.to_string())
    });

    let remote_cors = s3.get_bucket_cors(bucket).await?;
    let cors_rule = cors.map(cors_rule).transpose()?;
    let cors_changed = cors_rule.as_ref().is_some_and(|wanted| {
        remote_cors.len() != 1 || cors_json(&remote_cors[0]) != cors_json(wanted)
    });
    let cors_conflict = cors_changed && !remote_cors.is_empty();

    let plan = BucketConfigPlan {
        bucket: bucket.to_string(),
        rules: rules
            .iter()
            .map(|rule| LifecycleRule {
                prefix: normalize_prefix(&rule.prefix),
                expire_days: rule.expire_days,
            })
            .collect(),
        rules_changed: !rule_diff.is_empty(),
        rule_diff,
        versioning: remote_versioning,
        versioning_target,
        cors: cors.cloned(),
        cors_before: remote_cors.iter().map(cors_json).collect(),
        cors_changed,
        cors_conflict,
        probe: None,
    };
    Ok(Prepared {
        plan,
        rules: merged,
        cors: cors_rule,
    })
}

async fn plan_for(
    instance_id: &str,
    bucket: &str,
    rules: &[LifecycleRule],
    versioning: Option<bool>,
    cors: Option<&CorsSpec>,
) -> AppResult<Prepared> {
    let connection = crate::browser::connection(instance_id)?;
    let backend = NativeBackend::new(&connection)?;
    prepare(&backend.s3, bucket, rules, versioning, cors).await
}

pub async fn plan(
    instance_id: &str,
    bucket: &str,
    rules: &[LifecycleRule],
    versioning: Option<bool>,
    cors: Option<CorsSpec>,
) -> AppResult<BucketConfigPlan> {
    Ok(plan_for(instance_id, bucket, rules, versioning, cors.as_ref())
        .await?
        .plan)
}

pub async fn apply(
    instance_id: &str,
    bucket: &str,
    rules: &[LifecycleRule],
    versioning: Option<bool>,
    cors: Option<CorsSpec>,
    confirm_cors: bool,
) -> AppResult<BucketConfigPlan> {
    let connection = crate::browser::connection(instance_id)?;
    let backend = NativeBackend::new(&connection)?;
    let mut prepared = prepare(&backend.s3, bucket, rules, versioning, cors.as_ref()).await?;
    if prepared.plan.cors_conflict && !confirm_cors {
        return Err(AppError::new(
            "cors_overwrite_confirmation",
            "The bucket already has a CORS configuration RustFS Config did not create",
        )
        .with_hint("Confirm the replacement explicitly: the existing CORS rules will be overwritten."));
    }

    if prepared.plan.rules_changed {
        backend
            .s3
            .put_bucket_lifecycle(bucket, prepared.rules.clone())
            .await?;
    }
    if prepared.plan.versioning_target.is_some() {
        let enabled = prepared.plan.versioning_target.as_deref() == Some("Enabled");
        backend.s3.put_bucket_versioning(bucket, enabled).await?;
    }
    if prepared.plan.cors_changed {
        if let Some(rule) = prepared.cors.take() {
            backend.s3.put_bucket_cors(bucket, rule).await?;
        }
    }
    remember(instance_id, bucket, &prepared.plan.rules)?;

    prepared.plan.probe = probe(&backend.s3, bucket, &prepared.plan.rules).await;
    Ok(prepared.plan)
}

fn remember(instance_id: &str, bucket: &str, rules: &[LifecycleRule]) -> AppResult<()> {
    let mut store = Store::load()?;
    if let Some(entry) = store
        .buckets
        .iter_mut()
        .find(|entry| entry.instance_id == instance_id && entry.name == bucket)
    {
        entry.lifecycle = rules.to_vec();
        entry.managed.lifecycle = true;
        store.save()?;
    }
    Ok(())
}

/// A `PutObject` under an expiring prefix returns an `Expiration` header when the rule is
/// active (RUSTFS-NOTES §6.10). The probe object is removed in every case.
async fn probe(s3: &S3Client, bucket: &str, rules: &[LifecycleRule]) -> Option<ProbeReport> {
    let prefix = rules
        .iter()
        .find(|rule| rule.expire_days > 0)
        .map(|rule| rule.prefix.clone())?;
    let key = format!("{prefix}{PROBE_SUFFIX}");
    let result = s3
        .put_object_with_expiration(bucket, &key, b"rc-probe".to_vec())
        .await;
    let _ = s3.delete_object(bucket, &key).await;
    Some(match result {
        Ok(expiration) => ProbeReport {
            key,
            ok: expiration.is_some(),
            expiration,
            detail: None,
        },
        Err(error) => ProbeReport {
            key,
            expiration: None,
            ok: false,
            detail: Some(error.message),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(prefix: &str, days: u32) -> LifecycleRule {
        LifecycleRule {
            prefix: prefix.to_string(),
            expire_days: days,
        }
    }

    #[test]
    fn ids_are_stable_and_distinct() {
        assert_eq!(expire_id("uploads/media/"), "rc-expire-uploads-media");
        assert_eq!(expire_id(""), "rc-expire-bucket");
        assert_eq!(expire_id("/logs"), "rc-expire-logs");
    }

    #[test]
    fn desired_rules_always_include_multipart_abort() {
        let rules = desired_rules(&[rule("uploads/media", 180)]).expect("desired");
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].id(), Some(MULTIPART_ID));
        assert_eq!(
            rules[1].filter().and_then(|filter| filter.prefix()),
            Some("uploads/media/")
        );
        assert!(desired_rules(&[rule("a", 0)]).is_err());
    }

    #[test]
    fn merge_preserves_foreign_rules_and_is_idempotent() {
        let existing = vec![
            SdkRule::builder()
                .id("legacy-expire")
                .status(ExpirationStatus::Enabled)
                .filter(LifecycleRuleFilter::builder().prefix("old/").build())
                .expiration(LifecycleExpiration::builder().days(30).build())
                .build()
                .expect("rule"),
            SdkRule::builder()
                .id("rc-stale")
                .status(ExpirationStatus::Enabled)
                .filter(LifecycleRuleFilter::builder().prefix("gone/").build())
                .expiration(LifecycleExpiration::builder().days(10).build())
                .build()
                .expect("rule"),
        ];
        let desired = desired_rules(&[rule("uploads/media/", 180)]).expect("desired");
        let (merged, diff) = merge(&existing, &desired);
        assert_eq!(diff.len(), 2); // add both managed rules, remove rc-stale
        assert!(merged.iter().any(|rule| rule.id() == Some("legacy-expire")));
        assert!(!merged.iter().any(|rule| rule.id() == Some("rc-stale")));

        let (merged_again, diff_again) = merge(&merged, &desired);
        assert!(diff_again.is_empty());
        assert_eq!(merged_again.len(), merged.len());
    }

    #[test]
    fn cors_rule_is_validated() {
        let mut spec = CorsSpec {
            origins: vec!["https://example.test".into()],
            methods: vec!["get".into()],
            headers: vec!["*".into()],
            max_age_seconds: Some(600),
        };
        let built = cors_rule(&spec).expect("cors");
        assert_eq!(built.allowed_methods(), ["GET"]);
        spec.methods = vec!["TRACE".into()];
        assert_eq!(cors_rule(&spec).unwrap_err().code, "cors_method");
        spec.methods = vec![];
        assert_eq!(cors_rule(&spec).unwrap_err().code, "cors_incomplete");
    }
}
