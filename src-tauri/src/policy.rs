use crate::backend::NativeBackend;
use crate::error::{AppError, AppResult};
use crate::s3::S3Client;
use crate::store::{Bucket, Managed, Store};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashSet;

const SID_PREFIX: &str = "rc-public-";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyDiff {
    pub sid: String,
    pub action: String,
    pub before: Option<Value>,
    pub after: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyPlan {
    pub bucket: String,
    pub public_prefixes: Vec<String>,
    pub document: Value,
    pub diff: Vec<PolicyDiff>,
    pub changed: bool,
    pub requires_confirmation: bool,
}

/// `uploads/media` and `uploads/media/` are the same prefix.
pub fn normalize_prefix(prefix: &str) -> String {
    let prefix = prefix.trim().trim_start_matches('/');
    if prefix.is_empty() || prefix.ends_with('/') {
        prefix.to_string()
    } else {
        format!("{prefix}/")
    }
}

pub fn managed_sid(prefix: &str) -> String {
    let slug: String = prefix
        .trim_matches('/')
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        format!("{SID_PREFIX}root")
    } else {
        format!("{SID_PREFIX}{slug}")
    }
}

pub fn statement(bucket: &str, prefix: &str) -> Value {
    json!({
        "Sid": managed_sid(prefix),
        "Effect": "Allow",
        "Principal": {"AWS": ["*"]},
        "Action": ["s3:GetObject"],
        "Resource": [format!("arn:aws:s3:::{bucket}/{prefix}*")],
    })
}

fn statement_list(document: &Value) -> Vec<Value> {
    match document.get("Statement") {
        Some(Value::Array(items)) => items.clone(),
        Some(value @ Value::Object(_)) => vec![value.clone()],
        _ => Vec::new(),
    }
}

fn sid_of(statement: &Value) -> Option<&str> {
    statement.get("Sid").and_then(Value::as_str)
}

fn is_managed(statement: &Value) -> bool {
    sid_of(statement).is_some_and(|sid| sid.starts_with(SID_PREFIX))
}

/// Merges the desired public-read statements into an existing policy.
/// Statements without an `rc-public-` Sid are preserved untouched.
pub fn merge(
    existing: Option<&Value>,
    bucket: &str,
    prefixes: &[String],
) -> AppResult<(Value, Vec<PolicyDiff>)> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for prefix in prefixes {
        let prefix = normalize_prefix(prefix);
        if seen.insert(prefix.clone()) {
            normalized.push(prefix);
        }
    }
    let mut sids = HashSet::new();
    for prefix in &normalized {
        if !sids.insert(managed_sid(prefix)) {
            return Err(AppError::new(
                "policy_sid_conflict",
                "Two prefixes map to the same policy Sid, rename one of them",
            ));
        }
    }

    let old: Vec<Value> = existing.map(statement_list).unwrap_or_default();
    let mut merged: Vec<Value> = old
        .iter()
        .filter(|item| !is_managed(item))
        .cloned()
        .collect();
    let mut diff = Vec::new();

    for prefix in &normalized {
        let sid = managed_sid(prefix);
        let after = statement(bucket, prefix);
        let before = old
            .iter()
            .find(|item| sid_of(item) == Some(sid.as_str()))
            .cloned();
        match before {
            None => diff.push(PolicyDiff {
                sid: sid.clone(),
                action: "add".into(),
                before: None,
                after: Some(after.clone()),
            }),
            Some(ref current) if current != &after => diff.push(PolicyDiff {
                sid: sid.clone(),
                action: "update".into(),
                before: Some(current.clone()),
                after: Some(after.clone()),
            }),
            Some(_) => {}
        }
        merged.push(after);
    }

    for item in &old {
        if is_managed(item) {
            let sid = sid_of(item).unwrap_or_default();
            if !sids.contains(sid) {
                diff.push(PolicyDiff {
                    sid: sid.to_string(),
                    action: "remove".into(),
                    before: Some(item.clone()),
                    after: None,
                });
            }
        }
    }

    let document = json!({
        "Version": "2012-10-17",
        "Statement": merged,
    });
    Ok((document, diff))
}

async fn existing_policy(s3: &S3Client, bucket: &str) -> AppResult<Option<Value>> {
    match s3.get_bucket_policy(bucket).await {
        Ok(document) => Ok(Some(document)),
        Err(error) if error.code == "s3_not_found" => Ok(None),
        Err(error) => Err(error),
    }
}

fn confirmation_needed(prefixes: &[String]) -> bool {
    prefixes
        .iter()
        .any(|prefix| normalize_prefix(prefix).is_empty())
}

pub async fn plan(instance_id: &str, bucket: &str, prefixes: &[String]) -> AppResult<PolicyPlan> {
    let connection = crate::browser::connection(instance_id)?;
    let backend = NativeBackend::new(&connection)?;
    let existing = existing_policy(&backend.s3, bucket).await?;
    build_plan(bucket, prefixes, existing.as_ref())
}

fn build_plan(
    bucket: &str,
    prefixes: &[String],
    existing: Option<&Value>,
) -> AppResult<PolicyPlan> {
    let requires_confirmation = confirmation_needed(prefixes);
    let (document, diff) = merge(existing, bucket, prefixes)?;
    Ok(PolicyPlan {
        bucket: bucket.to_string(),
        public_prefixes: prefixes
            .iter()
            .map(|prefix| normalize_prefix(prefix))
            .collect(),
        changed: !diff.is_empty(),
        document,
        diff,
        requires_confirmation,
    })
}

pub async fn apply(
    instance_id: &str,
    bucket: &str,
    prefixes: &[String],
    allow_root: bool,
) -> AppResult<PolicyPlan> {
    let connection = crate::browser::connection(instance_id)?;
    let backend = NativeBackend::new(&connection)?;
    let existing = existing_policy(&backend.s3, bucket).await?;
    if confirmation_needed(prefixes) && !allow_root {
        return Err(AppError::new(
            "empty_prefix_requires_confirmation",
            "A public prefix covering the whole bucket needs an explicit confirmation",
        )
        .with_hint(
            "Ticking the whole-bucket option is a deliberate choice: every object becomes public.",
        ));
    }
    let policy_plan = build_plan(bucket, prefixes, existing.as_ref())?;

    if policy_plan.changed {
        backend
            .s3
            .put_bucket_policy(bucket, &policy_plan.document)
            .await?;
    }
    remember(instance_id, bucket, &policy_plan.public_prefixes)?;
    Ok(policy_plan)
}

fn remember(instance_id: &str, bucket: &str, prefixes: &[String]) -> AppResult<()> {
    let mut store = Store::load()?;
    match store
        .buckets
        .iter_mut()
        .find(|entry| entry.instance_id == instance_id && entry.name == bucket)
    {
        Some(entry) => {
            entry.public_prefixes = prefixes.to_vec();
            entry.managed.bucket_policy = true;
        }
        None => store.buckets.push(Bucket {
            instance_id: instance_id.to_string(),
            name: bucket.to_string(),
            public_prefixes: prefixes.to_vec(),
            managed: Managed {
                bucket_policy: true,
                ..Default::default()
            },
            ..Default::default()
        }),
    }
    store.save()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_public_read_is_generated() {
        let document = statement("photos", "uploads/media/");
        assert_eq!(document["Action"], json!(["s3:GetObject"]));
        assert_eq!(
            document["Resource"],
            json!(["arn:aws:s3:::photos/uploads/media/*"])
        );
        assert_eq!(document["Sid"], json!("rc-public-uploads-media"));
    }

    #[test]
    fn prefixes_are_normalized() {
        assert_eq!(normalize_prefix("/uploads/media"), "uploads/media/");
        assert_eq!(normalize_prefix("uploads/media/"), "uploads/media/");
        assert_eq!(normalize_prefix(""), "");
    }

    #[test]
    fn merge_preserves_unknown_statements_and_is_idempotent() {
        let existing = json!({
            "Version": "2012-10-17",
            "Statement": [
                {"Sid": "Legacy", "Effect": "Allow", "Principal": {"AWS": ["*"]},
                 "Action": ["s3:GetObject"], "Resource": ["arn:aws:s3:::photos/old/*"]}
            ]
        });
        let prefixes = vec!["uploads/media/".to_string()];
        let (first, diff) = merge(Some(&existing), "photos", &prefixes).expect("merge");
        assert_eq!(diff.len(), 1);
        assert_eq!(diff[0].action, "add");
        let statements = statement_list(&first);
        assert_eq!(statements.len(), 2);
        assert!(statements.iter().any(|item| sid_of(item) == Some("Legacy")));

        let (second, diff) = merge(Some(&first), "photos", &prefixes).expect("merge");
        assert!(diff.is_empty());
        assert_eq!(first, second);
    }

    #[test]
    fn dropped_prefixes_remove_only_managed_statements() {
        let existing = json!({"Statement": [
            {"Sid": "Legacy", "Effect": "Allow", "Action": ["s3:ListBucket"]},
            {"Sid": "rc-public-old", "Effect": "Allow", "Action": ["s3:GetObject"]}
        ]});
        let (document, diff) = merge(Some(&existing), "photos", &[]).expect("merge");
        assert_eq!(diff.len(), 1);
        assert_eq!(diff[0].action, "remove");
        let statements = statement_list(&document);
        assert_eq!(statements.len(), 1);
        assert_eq!(sid_of(&statements[0]), Some("Legacy"));
    }
}
