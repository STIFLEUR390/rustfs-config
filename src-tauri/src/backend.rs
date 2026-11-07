use async_trait::async_trait;

use crate::admin::{AdminClient, PolicyEntry, PolicyMapping, UserRecord};
use crate::config::Connection;
use crate::error::AppResult;
use crate::s3::{ObjectEntry, ObjectPage, S3Client};

/// Abstraction over the two RustFS surfaces (S3 + admin v3).
///
/// A CLI adapter (`rc`) can implement the same trait later without touching the UI.
#[async_trait]
pub trait RustfsBackend: Send + Sync {
    async fn list_buckets(&self) -> AppResult<Vec<String>>;
    async fn head_bucket(&self, bucket: &str) -> AppResult<()>;
    async fn get_bucket_location(&self, bucket: &str) -> AppResult<String>;
    async fn list_objects(&self, bucket: &str, prefix: Option<&str>)
        -> AppResult<Vec<ObjectEntry>>;
    async fn list_objects_page(
        &self,
        bucket: &str,
        prefix: Option<&str>,
        token: Option<&str>,
        max_keys: Option<i32>,
    ) -> AppResult<ObjectPage>;
    async fn create_bucket(&self, bucket: &str) -> AppResult<()>;
    async fn delete_bucket(&self, bucket: &str) -> AppResult<()>;
    async fn put_object(&self, bucket: &str, key: &str, body: Vec<u8>) -> AppResult<()>;
    async fn put_object_from_path(&self, bucket: &str, key: &str, path: &str) -> AppResult<()>;
    async fn get_object(&self, bucket: &str, key: &str) -> AppResult<Vec<u8>>;
    async fn get_object_to_path(&self, bucket: &str, key: &str, path: &str) -> AppResult<()>;
    async fn delete_object(&self, bucket: &str, key: &str) -> AppResult<()>;

    async fn list_users(&self) -> AppResult<Vec<UserRecord>>;
    async fn get_user(&self, access_key: &str) -> AppResult<UserRecord>;
    async fn add_user(&self, access_key: &str, secret_key: &str) -> AppResult<()>;
    async fn set_user_status(&self, access_key: &str, status: &str) -> AppResult<()>;
    async fn remove_user(&self, access_key: &str) -> AppResult<()>;
    async fn list_policies(&self) -> AppResult<Vec<PolicyEntry>>;
    async fn get_policy(&self, name: &str) -> AppResult<serde_json::Value>;
    async fn add_policy(&self, name: &str, document: &str) -> AppResult<()>;
    async fn remove_policy(&self, name: &str) -> AppResult<()>;
    async fn attach_policy(&self, policy: &str, user: &str) -> AppResult<()>;
    async fn detach_policy(&self, policy: &str, user: &str) -> AppResult<()>;
    async fn policy_entities(&self) -> AppResult<Vec<PolicyMapping>>;
}

pub struct NativeBackend {
    pub s3: S3Client,
    pub admin: AdminClient,
}

impl NativeBackend {
    pub fn new(conn: &Connection) -> AppResult<Self> {
        Ok(Self {
            s3: S3Client::new(conn)?,
            admin: AdminClient::new(conn)?,
        })
    }
}

#[async_trait]
impl RustfsBackend for NativeBackend {
    async fn list_buckets(&self) -> AppResult<Vec<String>> {
        self.s3.list_buckets().await
    }

    async fn head_bucket(&self, bucket: &str) -> AppResult<()> {
        self.s3.head_bucket(bucket).await
    }

    async fn get_bucket_location(&self, bucket: &str) -> AppResult<String> {
        self.s3.get_bucket_location(bucket).await
    }

    async fn list_objects(
        &self,
        bucket: &str,
        prefix: Option<&str>,
    ) -> AppResult<Vec<ObjectEntry>> {
        self.s3.list_objects(bucket, prefix).await
    }

    async fn list_objects_page(
        &self,
        bucket: &str,
        prefix: Option<&str>,
        token: Option<&str>,
        max_keys: Option<i32>,
    ) -> AppResult<ObjectPage> {
        self.s3
            .list_objects_page(bucket, prefix, Some("/"), token, max_keys)
            .await
    }

    async fn create_bucket(&self, bucket: &str) -> AppResult<()> {
        self.s3.create_bucket(bucket).await
    }

    async fn delete_bucket(&self, bucket: &str) -> AppResult<()> {
        self.s3.delete_bucket(bucket).await
    }

    async fn put_object(&self, bucket: &str, key: &str, body: Vec<u8>) -> AppResult<()> {
        self.s3.put_object(bucket, key, body).await
    }

    async fn put_object_from_path(&self, bucket: &str, key: &str, path: &str) -> AppResult<()> {
        self.s3.put_object_from_path(bucket, key, path).await
    }

    async fn get_object(&self, bucket: &str, key: &str) -> AppResult<Vec<u8>> {
        self.s3.get_object(bucket, key).await
    }

    async fn get_object_to_path(&self, bucket: &str, key: &str, path: &str) -> AppResult<()> {
        self.s3.get_object_to_path(bucket, key, path).await
    }

    async fn delete_object(&self, bucket: &str, key: &str) -> AppResult<()> {
        self.s3.delete_object(bucket, key).await
    }

    async fn list_users(&self) -> AppResult<Vec<UserRecord>> {
        self.admin.list_users().await
    }

    async fn get_user(&self, access_key: &str) -> AppResult<UserRecord> {
        self.admin.get_user(access_key).await
    }

    async fn add_user(&self, access_key: &str, secret_key: &str) -> AppResult<()> {
        self.admin.add_user(access_key, secret_key).await
    }

    async fn set_user_status(&self, access_key: &str, status: &str) -> AppResult<()> {
        self.admin.set_user_status(access_key, status).await
    }

    async fn remove_user(&self, access_key: &str) -> AppResult<()> {
        self.admin.remove_user(access_key).await
    }

    async fn list_policies(&self) -> AppResult<Vec<PolicyEntry>> {
        self.admin.list_policies().await
    }

    async fn get_policy(&self, name: &str) -> AppResult<serde_json::Value> {
        self.admin.get_policy(name).await
    }

    async fn add_policy(&self, name: &str, document: &str) -> AppResult<()> {
        self.admin.add_policy(name, document).await
    }

    async fn remove_policy(&self, name: &str) -> AppResult<()> {
        self.admin.remove_policy(name).await
    }

    async fn attach_policy(&self, policy: &str, user: &str) -> AppResult<()> {
        self.admin.attach_policy(policy, user).await
    }

    async fn detach_policy(&self, policy: &str, user: &str) -> AppResult<()> {
        self.admin.detach_policy(policy, user).await
    }

    async fn policy_entities(&self) -> AppResult<Vec<PolicyMapping>> {
        self.admin.policy_entities().await
    }
}
