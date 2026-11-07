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
        let mut req = self.client.list_objects_v2().bucket(bucket);
        if let Some(p) = prefix {
            req = req.prefix(p);
        }
        let out = req.send().await.map_err(map_sdk)?;
        Ok(out
            .contents()
            .iter()
            .map(|o| ObjectEntry {
                key: o.key().unwrap_or_default().to_string(),
                size: o.size().unwrap_or_default(),
                last_modified: o.last_modified().map(|d| d.to_string()),
            })
            .collect())
    }

    pub async fn put_object(&self, bucket: &str, key: &str, body: Vec<u8>) -> AppResult<()> {
        self.client
            .put_object()
            .bucket(bucket)
            .key(key)
            .body(body.into())
            .send()
            .await
            .map_err(map_sdk)?;
        Ok(())
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

fn map_sdk<E, R>(err: aws_sdk_s3::error::SdkError<E, R>) -> AppError
where
    E: std::error::Error + Send + Sync + 'static,
    R: std::fmt::Debug,
{
    match &err {
        aws_sdk_s3::error::SdkError::ServiceError(e) => {
            AppError::new("s3_error", format!("{}", e.err()))
        }
        _ => AppError::new("s3_error", format!("{err}")),
    }
}
