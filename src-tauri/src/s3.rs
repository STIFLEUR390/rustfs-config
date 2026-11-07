use aws_credential_types::Credentials;
use aws_sdk_s3::config::Region;
use aws_sdk_s3::Client;

use crate::config::Connection;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectEntry {
    pub key: String,
    pub size: i64,
    pub last_modified: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectPage {
    pub entries: Vec<ObjectEntry>,
    pub prefixes: Vec<String>,
    pub next_token: Option<String>,
}

pub struct S3Client {
    client: Client,
}

impl S3Client {
    pub fn new(conn: &Connection) -> AppResult<Self> {
        let creds = Credentials::new(
            &conn.access_key,
            &conn.secret_key,
            None,
            None,
            "rustfs-config",
        );
        // shortcut: tls_verify is wired on the admin HTTP client only; the shared S3
        // connector still verifies TLS. Local endpoints are plain http, so this only
        // affects self-signed https endpoints. Replace with a custom smithy connector
        // when that case shows up.
        let config = aws_sdk_s3::Config::builder()
            .behavior_version(aws_config::BehaviorVersion::latest())
            .region(Region::new(conn.region.clone()))
            .credentials_provider(creds)
            .endpoint_url(conn.base_url())
            .force_path_style(conn.path_style)
            .build();
        Ok(Self {
            client: Client::from_conf(config),
        })
    }

    pub async fn list_buckets(&self) -> AppResult<Vec<String>> {
        let out = self.client.list_buckets().send().await.map_err(map_sdk)?;
        Ok(out
            .buckets()
            .iter()
            .filter_map(|b| b.name().map(str::to_string))
            .collect())
    }

    pub async fn head_bucket(&self, bucket: &str) -> AppResult<()> {
        self.client
            .head_bucket()
            .bucket(bucket)
            .send()
            .await
            .map_err(map_sdk)?;
        Ok(())
    }

    pub async fn get_bucket_policy(&self, bucket: &str) -> AppResult<serde_json::Value> {
        let output = self
            .client
            .get_bucket_policy()
            .bucket(bucket)
            .send()
            .await
            .map_err(map_sdk)?;
        let text = output.policy().unwrap_or_default();
        serde_json::from_str(text).map_err(|err| AppError::new("json", err.to_string()))
    }

    pub async fn put_bucket_policy(
        &self,
        bucket: &str,
        document: &serde_json::Value,
    ) -> AppResult<()> {
        self.client
            .put_bucket_policy()
            .bucket(bucket)
            .policy(document.to_string())
            .send()
            .await
            .map_err(map_sdk)?;
        Ok(())
    }

    pub async fn bucket_has_lifecycle(&self, bucket: &str) -> AppResult<bool> {
        Ok(!self.get_bucket_lifecycle(bucket).await?.is_empty())
    }

    pub async fn get_bucket_lifecycle(
        &self,
        bucket: &str,
    ) -> AppResult<Vec<aws_sdk_s3::types::LifecycleRule>> {
        match self
            .client
            .get_bucket_lifecycle_configuration()
            .bucket(bucket)
            .send()
            .await
        {
            Ok(output) => Ok(output.rules().to_vec()),
            // NoSuchLifecycleConfiguration is a legitimate "not configured".
            Err(err) => match map_sdk(err) {
                mapped if mapped.code == "s3_not_found" => Ok(Vec::new()),
                mapped => Err(mapped),
            },
        }
    }

    pub async fn put_bucket_lifecycle(
        &self,
        bucket: &str,
        rules: Vec<aws_sdk_s3::types::LifecycleRule>,
    ) -> AppResult<()> {
        let config = aws_sdk_s3::types::BucketLifecycleConfiguration::builder()
            .set_rules(Some(rules))
            .build()
            .map_err(|e| AppError::new("lifecycle_build", e.to_string()))?;
        self.client
            .put_bucket_lifecycle_configuration()
            .bucket(bucket)
            .lifecycle_configuration(config)
            .send()
            .await
            .map_err(map_sdk)?;
        Ok(())
    }

    pub async fn get_bucket_versioning(&self, bucket: &str) -> AppResult<Option<String>> {
        let out = self
            .client
            .get_bucket_versioning()
            .bucket(bucket)
            .send()
            .await
            .map_err(map_sdk)?;
        Ok(out.status().map(|status| status.as_str().to_string()))
    }

    pub async fn put_bucket_versioning(&self, bucket: &str, enabled: bool) -> AppResult<()> {
        let status = if enabled {
            aws_sdk_s3::types::BucketVersioningStatus::Enabled
        } else {
            aws_sdk_s3::types::BucketVersioningStatus::Suspended
        };
        let config = aws_sdk_s3::types::VersioningConfiguration::builder()
            .status(status)
            .build();
        self.client
            .put_bucket_versioning()
            .bucket(bucket)
            .versioning_configuration(config)
            .send()
            .await
            .map_err(map_sdk)?;
        Ok(())
    }

    pub async fn get_bucket_cors(&self, bucket: &str) -> AppResult<Vec<aws_sdk_s3::types::CorsRule>> {
        match self.client.get_bucket_cors().bucket(bucket).send().await {
            Ok(out) => Ok(out.cors_rules().to_vec()),
            // NoSuchCORSConfiguration is a legitimate "not configured".
            Err(err) => match map_sdk(err) {
                mapped if mapped.code == "s3_not_found" => Ok(Vec::new()),
                mapped => Err(mapped),
            },
        }
    }

    pub async fn put_bucket_cors(
        &self,
        bucket: &str,
        rule: aws_sdk_s3::types::CorsRule,
    ) -> AppResult<()> {
        let config = aws_sdk_s3::types::CorsConfiguration::builder()
            .cors_rules(rule)
            .build()
            .map_err(|e| AppError::new("cors_build", e.to_string()))?;
        self.client
            .put_bucket_cors()
            .bucket(bucket)
            .cors_configuration(config)
            .send()
            .await
            .map_err(map_sdk)?;
        Ok(())
    }

    pub async fn get_bucket_location(&self, bucket: &str) -> AppResult<String> {
        let out = self
            .client
            .get_bucket_location()
            .bucket(bucket)
            .send()
            .await
            .map_err(map_sdk)?;
        Ok(out
            .location_constraint()
            .map(|l| l.as_str().to_string())
            .unwrap_or_else(|| "us-east-1".to_string()))
    }

    pub async fn list_objects(
        &self,
        bucket: &str,
        prefix: Option<&str>,
    ) -> AppResult<Vec<ObjectEntry>> {
        Ok(self
            .list_objects_page(bucket, prefix, None, None, None)
            .await?
            .entries)
    }

    pub async fn list_objects_page(
        &self,
        bucket: &str,
        prefix: Option<&str>,
        delimiter: Option<&str>,
        token: Option<&str>,
        max_keys: Option<i32>,
    ) -> AppResult<ObjectPage> {
        let mut req = self.client.list_objects_v2().bucket(bucket);
        if let Some(p) = prefix {
            req = req.prefix(p);
        }
        if let Some(d) = delimiter {
            req = req.delimiter(d);
        }
        if let Some(t) = token {
            req = req.continuation_token(t);
        }
        if let Some(m) = max_keys {
            req = req.max_keys(m);
        }
        let out = req.send().await.map_err(map_sdk)?;
        Ok(ObjectPage {
            entries: out
                .contents()
                .iter()
                .map(|o| ObjectEntry {
                    key: o.key().unwrap_or_default().to_string(),
                    size: o.size().unwrap_or_default(),
                    last_modified: o.last_modified().map(|d| d.to_string()),
                })
                .collect(),
            prefixes: out
                .common_prefixes()
                .iter()
                .filter_map(|p| p.prefix().map(str::to_string))
                .collect(),
            next_token: out.next_continuation_token().map(str::to_string),
        })
    }

    pub async fn create_bucket(&self, bucket: &str) -> AppResult<()> {
        self.client
            .create_bucket()
            .bucket(bucket)
            .send()
            .await
            .map_err(map_sdk)?;
        Ok(())
    }

    pub async fn delete_bucket(&self, bucket: &str) -> AppResult<()> {
        self.client
            .delete_bucket()
            .bucket(bucket)
            .send()
            .await
            .map_err(map_sdk)?;
        Ok(())
    }

    pub async fn put_object(&self, bucket: &str, key: &str, body: Vec<u8>) -> AppResult<()> {
        self.put_object_with_expiration(bucket, key, body).await?;
        Ok(())
    }

    /// Returns the `Expiration` header when a lifecycle rule applies to the key.
    pub async fn put_object_with_expiration(
        &self,
        bucket: &str,
        key: &str,
        body: Vec<u8>,
    ) -> AppResult<Option<String>> {
        let out = self
            .client
            .put_object()
            .bucket(bucket)
            .key(key)
            .body(body.into())
            .send()
            .await
            .map_err(map_sdk)?;
        Ok(out.expiration().map(str::to_string))
    }

    pub async fn put_object_from_path(&self, bucket: &str, key: &str, path: &str) -> AppResult<()> {
        let body = tokio::fs::read(path)
            .await
            .map_err(|e| AppError::new("file_read", format!("{path}: {e}")))?;
        self.put_object(bucket, key, body).await
    }

    pub async fn get_object(&self, bucket: &str, key: &str) -> AppResult<Vec<u8>> {
        let out = self
            .client
            .get_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
            .map_err(map_sdk)?;
        let data = out
            .body
            .collect()
            .await
            .map_err(|e| AppError::new("s3_body", e.to_string()))?;
        Ok(data.into_bytes().to_vec())
    }

    pub async fn get_object_to_path(&self, bucket: &str, key: &str, path: &str) -> AppResult<()> {
        let body = self.get_object(bucket, key).await?;
        tokio::fs::write(path, body)
            .await
            .map_err(|e| AppError::new("file_write", format!("{path}: {e}")))
    }

    pub async fn delete_object(&self, bucket: &str, key: &str) -> AppResult<()> {
        self.client
            .delete_object()
            .bucket(bucket)
            .key(key)
            .send()
            .await
            .map_err(map_sdk)?;
        Ok(())
    }
}

fn map_sdk<E>(
    err: aws_sdk_s3::error::SdkError<E, aws_sdk_s3::config::http::HttpResponse>,
) -> AppError
where
    E: std::error::Error + aws_sdk_s3::error::ProvideErrorMetadata + Send + Sync + 'static,
{
    match &err {
        aws_sdk_s3::error::SdkError::ServiceError(e) => {
            let status = e.raw().status().as_u16();
            let code = e.err().code().unwrap_or_default();
            let message = e.err().message().unwrap_or("S3 request failed").to_string();
            let text = if code.is_empty() {
                message
            } else {
                format!("{code}: {message}")
            };
            match status {
                403 => AppError::new("s3_forbidden", text).with_hint(
                    "La clé n'a pas le droit d'effectuer cette opération. Un bucket inaccessible n'est pas un bucket vide.",
                ),
                404 => AppError::new("s3_not_found", text)
                    .with_hint("Bucket ou objet introuvable."),
                401 => AppError::new("s3_unauthorized", text)
                    .with_hint("Vérifiez l'access key et le secret."),
                _ => AppError::new("s3_error", text),
            }
        }
        _ => AppError::new("s3_error", format!("{err}")),
    }
}
