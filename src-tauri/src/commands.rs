use serde::Serialize;

use crate::admin::{PolicyEntry, PolicyMapping, UserRecord};
use crate::backend::{NativeBackend, RustfsBackend};
use crate::browser;
use crate::config::Connection;
use crate::error::AppResult;
use crate::iam::{self, IssuedCredential};
use crate::instances::{self, InstanceTest};
use crate::s3::{ObjectEntry, ObjectPage, S3Client};
use crate::store::{Credential, Instance};
use crate::verify::{self, Report};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionInfo {
    pub buckets: usize,
}

fn backend(connection: &Connection) -> AppResult<NativeBackend> {
    NativeBackend::new(connection)
}

#[tauri::command]
pub async fn test_connection(connection: Connection) -> AppResult<ConnectionInfo> {
    let buckets = backend(&connection)?.list_buckets().await?.len();
    Ok(ConnectionInfo { buckets })
}

#[tauri::command]
pub async fn list_buckets(connection: Connection) -> AppResult<Vec<String>> {
    backend(&connection)?.list_buckets().await
}

#[tauri::command]
pub async fn head_bucket(connection: Connection, bucket: String) -> AppResult<()> {
    backend(&connection)?.head_bucket(&bucket).await
}

#[tauri::command]
pub async fn list_objects(
    connection: Connection,
    bucket: String,
    prefix: Option<String>,
) -> AppResult<Vec<ObjectEntry>> {
    backend(&connection)?
        .list_objects(&bucket, prefix.as_deref())
        .await
}

#[tauri::command]
pub async fn list_objects_page(
    connection: Connection,
    bucket: String,
    prefix: Option<String>,
    token: Option<String>,
    max_keys: Option<i32>,
) -> AppResult<ObjectPage> {
    backend(&connection)?
        .list_objects_page(&bucket, prefix.as_deref(), token.as_deref(), max_keys)
        .await
}

#[tauri::command]
pub async fn create_bucket(connection: Connection, bucket: String) -> AppResult<()> {
    backend(&connection)?.create_bucket(&bucket).await
}

#[tauri::command]
pub async fn delete_bucket(connection: Connection, bucket: String) -> AppResult<()> {
    backend(&connection)?.delete_bucket(&bucket).await
}

#[tauri::command]
pub async fn put_object(
    connection: Connection,
    bucket: String,
    key: String,
    body: Vec<u8>,
) -> AppResult<()> {
    backend(&connection)?.put_object(&bucket, &key, body).await
}

#[tauri::command]
pub async fn put_object_from_path(
    connection: Connection,
    bucket: String,
    key: String,
    path: String,
) -> AppResult<()> {
    backend(&connection)?
        .put_object_from_path(&bucket, &key, &path)
        .await
}

#[tauri::command]
pub async fn get_object(connection: Connection, bucket: String, key: String) -> AppResult<Vec<u8>> {
    backend(&connection)?.get_object(&bucket, &key).await
}

#[tauri::command]
pub async fn get_object_to_path(
    connection: Connection,
    bucket: String,
    key: String,
    path: String,
) -> AppResult<()> {
    backend(&connection)?
        .get_object_to_path(&bucket, &key, &path)
        .await
}

#[tauri::command]
pub async fn delete_object(connection: Connection, bucket: String, key: String) -> AppResult<()> {
    backend(&connection)?.delete_object(&bucket, &key).await
}

#[tauri::command]
pub async fn list_users(connection: Connection) -> AppResult<Vec<UserRecord>> {
    backend(&connection)?.list_users().await
}

#[tauri::command]
pub async fn get_user(connection: Connection, access_key: String) -> AppResult<UserRecord> {
    backend(&connection)?.get_user(&access_key).await
}

#[tauri::command]
pub async fn add_user(
    connection: Connection,
    access_key: String,
    secret_key: String,
) -> AppResult<()> {
    backend(&connection)?
        .add_user(&access_key, &secret_key)
        .await
}

#[tauri::command]
pub async fn set_user_status(
    connection: Connection,
    access_key: String,
    status: String,
) -> AppResult<()> {
    backend(&connection)?
        .set_user_status(&access_key, &status)
        .await
}

#[tauri::command]
pub async fn remove_user(connection: Connection, access_key: String) -> AppResult<()> {
    backend(&connection)?.remove_user(&access_key).await
}

#[tauri::command]
pub async fn list_policies(connection: Connection) -> AppResult<Vec<PolicyEntry>> {
    backend(&connection)?.list_policies().await
}

#[tauri::command]
pub async fn get_policy(connection: Connection, name: String) -> AppResult<serde_json::Value> {
    backend(&connection)?.get_policy(&name).await
}

#[tauri::command]
pub async fn add_policy(connection: Connection, name: String, document: String) -> AppResult<()> {
    backend(&connection)?.add_policy(&name, &document).await
}

#[tauri::command]
pub async fn remove_policy(connection: Connection, name: String) -> AppResult<()> {
    backend(&connection)?.remove_policy(&name).await
}

#[tauri::command]
pub async fn attach_policy(connection: Connection, policy: String, user: String) -> AppResult<()> {
    backend(&connection)?.attach_policy(&policy, &user).await
}

#[tauri::command]
pub async fn detach_policy(connection: Connection, policy: String, user: String) -> AppResult<()> {
    backend(&connection)?.detach_policy(&policy, &user).await
}

#[tauri::command]
pub async fn policy_entities(connection: Connection) -> AppResult<Vec<PolicyMapping>> {
    backend(&connection)?.policy_entities().await
}

#[tauri::command]
pub async fn list_instances() -> AppResult<Vec<Instance>> {
    instances::list()
}

#[tauri::command]
pub async fn test_instance(connection: Connection) -> AppResult<InstanceTest> {
    instances::test(&connection).await
}

#[tauri::command]
pub async fn save_instance(
    instance: Instance,
    access_key: Option<String>,
    secret_key: Option<String>,
) -> AppResult<Instance> {
    instances::save(instance, access_key, secret_key)
}

#[tauri::command]
pub async fn delete_instance(id: String) -> AppResult<()> {
    instances::delete(&id)
}

fn instance_s3(instance_id: &str) -> AppResult<S3Client> {
    S3Client::new(&browser::connection(instance_id)?)
}

#[tauri::command]
pub async fn list_instance_buckets(instance_id: String) -> AppResult<Vec<String>> {
    instance_s3(&instance_id)?.list_buckets().await
}

#[tauri::command]
pub async fn list_instance_objects(
    instance_id: String,
    bucket: String,
    prefix: Option<String>,
    token: Option<String>,
    max_keys: Option<i32>,
) -> AppResult<ObjectPage> {
    instance_s3(&instance_id)?
        .list_objects_page(
            &bucket,
            prefix.as_deref(),
            Some("/"),
            token.as_deref(),
            max_keys,
        )
        .await
}

#[tauri::command]
pub async fn create_instance_bucket(instance_id: String, bucket: String) -> AppResult<()> {
    instance_s3(&instance_id)?.create_bucket(&bucket).await
}

#[tauri::command]
pub async fn delete_instance_bucket(instance_id: String, bucket: String) -> AppResult<()> {
    instance_s3(&instance_id)?.delete_bucket(&bucket).await
}

#[tauri::command]
pub async fn upload_instance_object(
    instance_id: String,
    bucket: String,
    key: String,
    path: String,
) -> AppResult<()> {
    instance_s3(&instance_id)?
        .put_object_from_path(&bucket, &key, &path)
        .await
}

#[tauri::command]
pub async fn download_instance_object(
    instance_id: String,
    bucket: String,
    key: String,
    path: String,
) -> AppResult<()> {
    instance_s3(&instance_id)?
        .get_object_to_path(&bucket, &key, &path)
        .await
}

#[tauri::command]
pub async fn delete_instance_object(
    instance_id: String,
    bucket: String,
    key: String,
) -> AppResult<()> {
    instance_s3(&instance_id)?
        .delete_object(&bucket, &key)
        .await
}

#[tauri::command]
pub fn list_credentials(instance_id: String, bucket: String) -> AppResult<Vec<Credential>> {
    iam::list(&instance_id, &bucket)
}

#[tauri::command]
pub async fn create_credential(
    instance_id: String,
    bucket: String,
    mode: String,
    access_key: Option<String>,
) -> AppResult<IssuedCredential> {
    iam::create(&instance_id, &bucket, &mode, access_key).await
}

#[tauri::command]
pub async fn adopt_credential(
    instance_id: String,
    bucket: String,
    access_key: String,
) -> AppResult<IssuedCredential> {
    iam::adopt(&instance_id, &bucket, &access_key).await
}

#[tauri::command]
pub async fn rotate_credential(instance_id: String, credential_id: String) -> AppResult<String> {
    iam::rotate(&instance_id, &credential_id).await
}

#[tauri::command]
pub async fn revoke_credential(instance_id: String, credential_id: String) -> AppResult<()> {
    iam::revoke(&instance_id, &credential_id).await
}

#[tauri::command]
pub async fn verify_isolation(
    instance_id: String,
    bucket: String,
    credential_id: String,
) -> AppResult<Report> {
    verify::verify(&instance_id, &bucket, &credential_id).await
}
