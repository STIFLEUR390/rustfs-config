use crate::config::Connection;
use crate::error::{AppError, AppResult};
use crate::secrets;
use crate::store::Store;

/// Build a connection for a stored instance.
///
/// Credentials live in the OS keyring, so the webview passes only the instance id and
/// never sees the secret.
pub fn connection(instance_id: &str) -> AppResult<Connection> {
    let store = Store::load()?;
    let instance = store
        .instances
        .iter()
        .find(|item| item.id == instance_id)
        .ok_or_else(|| {
            AppError::new(
                "instance_not_found",
                format!("instance {instance_id} introuvable"),
            )
        })?;
    let credentials = secrets::get_instance_credentials(instance_id)?.ok_or_else(|| {
        AppError::new(
            "missing_credentials",
            "Aucun secret enregistré pour cette instance.",
        )
        .with_hint("Modifiez l'instance et saisissez à nouveau le secret.")
    })?;
    Ok(Connection {
        endpoint: instance.endpoint.clone(),
        region: instance.region.clone(),
        path_style: instance.path_style,
        tls_verify: instance.tls_verify,
        access_key: credentials.access_key,
        secret_key: credentials.secret_key,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_instance_is_reported() {
        let path = std::env::temp_dir().join("rc-browser-unknown-instance.json");
        std::env::set_var("RC_STORE_PATH", &path);
        let error = connection("ins_missing").unwrap_err();
        assert_eq!(error.code, "instance_not_found");
        let _ = std::fs::remove_file(path);
    }
}
