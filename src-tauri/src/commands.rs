use serde::Serialize;

use crate::admin::{PolicyEntry, PolicyMapping, UserRecord};
use crate::backend::{NativeBackend, RustfsBackend};
use crate::config::Connection;
use crate::error::AppResult;
use crate::s3::ObjectEntry;

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
pub async fn put_object(
    connection: Connection,
    bucket: String,
    key: String,
    body: Vec<u8>,
) -> AppResult<()> {
    backend(&connection)?
        .put_object(&bucket, &key, body)
        .await
}

#[tauri::command]
pub async fn get_object(
    connection: Connection,
    bucket: String,
    key: String,
) -> AppResult<Vec<u8>> {
    backend(&connection)?.get_object(&bucket, &key).await
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
pub async fn get_policy(
    connection: Connection,
    name: String,
) -> AppResult<serde_json::Value> {
    backend(&connection)?.get_policy(&name).await
}

#[tauri::command]
pub async fn add_policy(
    connection: Connection,
    name: String,
    document: String,
) -> AppResult<()> {
    backend(&connection)?.add_policy(&name, &document).await
}

#[tauri::command]
pub async fn remove_policy(connection: Connection, name: String) -> AppResult<()> {
    backend(&connection)?.remove_policy(&name).await
}

#[tauri::command]
pub async fn attach_policy(
    connection: Connection,
    policy: String,
    user: String,
) -> AppResult<()> {
    backend(&connection)?.attach_policy(&policy, &user).await
}

#[tauri::command]
pub async fn detach_policy(
    connection: Connection,
    policy: String,
    user: String,
) -> AppResult<()> {
    backend(&connection)?.detach_policy(&policy, &user).await
}

#[tauri::command]
pub async fn policy_entities(connection: Connection) -> AppResult<Vec<PolicyMapping>> {
    backend(&connection)?.policy_entities().await
}
