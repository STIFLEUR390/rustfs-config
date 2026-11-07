use crate::browser;
use crate::config::Connection;
use crate::error::{AppError, AppResult};
use crate::secrets;
use crate::store::Store;

pub const FORMATS: [&str; 3] = ["laravel", "node", "cli"];

/// Render the `.env` block (SPEC §8) for one credential.
///
/// This is the only place a stored secret is put into text the webview can see, and only
/// because the user explicitly asked for the file. Never log the result.
pub fn render(
    connection: &Connection,
    bucket: &str,
    access_key: &str,
    secret_key: &str,
    format: &str,
) -> AppResult<String> {
    if !FORMATS.contains(&format) {
        return Err(
            AppError::new("invalid_format", format!("Format inconnu : {format}"))
                .with_hint("Formats disponibles : laravel, node, cli."),
        );
    }

    let endpoint = connection.base_url();
    let region = &connection.region;
    let path_style = connection.path_style;
    let tls = endpoint.starts_with("https://");

    Ok(match format {
        "node" => {
            let mut lines = vec![
                format!("S3_ENDPOINT={endpoint}"),
                format!("S3_REGION={region}"),
                format!("S3_BUCKET={bucket}"),
                format!("S3_ACCESS_KEY_ID={access_key}"),
                format!("S3_SECRET_ACCESS_KEY={secret_key}"),
                format!("S3_FORCE_PATH_STYLE={path_style}"),
            ];
            if tls {
                lines.push(format!("S3_SSL_VERIFY={}", connection.tls_verify));
            }
            lines.join("\n")
        }
        "cli" => {
            let profile = bucket;
            let lines = [
                format!("aws configure set aws_access_key_id {access_key} --profile {profile}"),
                format!("aws configure set aws_secret_access_key {secret_key} --profile {profile}"),
                format!("aws configure set region {region} --profile {profile}"),
                format!("aws --profile {profile} --endpoint-url {endpoint} s3 ls s3://{bucket}"),
            ];
            lines.join("\n")
        }
        _ => {
            let url = if path_style {
                format!("{endpoint}/{bucket}")
            } else {
                virtual_host_url(&endpoint, bucket)?
            };
            let mut lines = vec![
                "FILESYSTEM_DISK=s3".to_string(),
                format!("AWS_ACCESS_KEY_ID={access_key}"),
                format!("AWS_SECRET_ACCESS_KEY={secret_key}"),
                format!("AWS_DEFAULT_REGION={region}"),
                format!("AWS_BUCKET={bucket}"),
                format!("AWS_ENDPOINT={endpoint}"),
                format!("AWS_URL={url}"),
                format!("AWS_USE_PATH_STYLE_ENDPOINT={path_style}"),
            ];
            if tls {
                lines.push(format!("AWS_SSL_VERIFY={}", connection.tls_verify));
            }
            lines.join("\n")
        }
    })
}

fn virtual_host_url(endpoint: &str, bucket: &str) -> AppResult<String> {
    let (scheme, rest) = endpoint.split_once("://").ok_or_else(|| {
        AppError::new(
            "invalid_endpoint",
            format!("Endpoint sans schéma : {endpoint}"),
        )
    })?;
    Ok(format!("{scheme}://{bucket}.{rest}"))
}

/// Generate the file for a credential stored in the vault.
pub fn generate(instance_id: &str, credential_id: &str, format: &str) -> AppResult<String> {
    let connection = browser::connection(instance_id)?;
    let store = Store::load()?;
    let credential = store
        .credentials
        .iter()
        .find(|item| item.id == credential_id && item.instance_id == instance_id)
        .ok_or_else(|| {
            AppError::new(
                "credential_not_found",
                format!("Credential {credential_id} introuvable"),
            )
        })?;
    let key = secrets::credential_key(credential_id);
    let secret = secrets::get(&key)?.ok_or_else(|| {
        AppError::new(
            "missing_secret",
            "Le secret de ce credential n'est pas dans le trousseau.",
        )
        .with_hint("Un credential adopté n'a pas de secret connu. Faites une rotation.")
    })?;
    render(
        &connection,
        &credential.bucket,
        &credential.access_key,
        &secret,
        format,
    )
}

/// Write the `.env` file with `0600` permissions. The backend writes it, so the secret
/// never has to pass through a frontend file API.
pub async fn write_to_path(
    instance_id: &str,
    credential_id: &str,
    format: &str,
    path: &str,
    allow_overwrite: bool,
) -> AppResult<String> {
    let contents = generate(instance_id, credential_id, format)?;
    if std::path::Path::new(path).exists() && !allow_overwrite {
        return Err(
            AppError::new("file_exists", format!("Le fichier {path} existe déjà."))
                .with_hint("Confirmez le remplacement pour écraser ce fichier."),
        );
    }
    tokio::fs::write(path, contents)
        .await
        .map_err(|error| AppError::new("file_write", format!("Écriture impossible : {error}")))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .await
            .map_err(|error| {
                AppError::new("file_write", format!("Permissions impossibles : {error}"))
            })?;
    }
    Ok(path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connection(path_style: bool) -> Connection {
        Connection {
            endpoint: "https://storage.example.test/".to_string(),
            region: "eu-west-3".to_string(),
            path_style,
            tls_verify: true,
            access_key: "app".to_string(),
            secret_key: "s3cr3t".to_string(),
        }
    }

    #[test]
    fn laravel_path_style_uses_bucket_in_the_path() {
        let text = render(&connection(true), "media", "app", "s3cr3t", "laravel").unwrap();
        assert!(text.contains("AWS_URL=https://storage.example.test/media"));
        assert!(text.contains("AWS_ENDPOINT=https://storage.example.test"));
        assert!(text.contains("AWS_USE_PATH_STYLE_ENDPOINT=true"));
        assert!(text.contains("AWS_SSL_VERIFY=true"));
        assert!(text.contains("AWS_SECRET_ACCESS_KEY=s3cr3t"));
    }

    #[test]
    fn laravel_virtual_host_moves_bucket_into_the_host() {
        let text = render(&connection(false), "media", "app", "s3cr3t", "laravel").unwrap();
        assert!(text.contains("AWS_URL=https://media.storage.example.test"));
        assert!(text.contains("AWS_USE_PATH_STYLE_ENDPOINT=false"));
    }

    #[test]
    fn tls_verify_is_omitted_for_plain_http() {
        let mut plain = connection(true);
        plain.endpoint = "http://127.0.0.1:9000".to_string();
        let text = render(&plain, "media", "app", "s3cr3t", "laravel").unwrap();
        assert!(!text.contains("AWS_SSL_VERIFY"));
    }

    #[test]
    fn node_and_cli_formats_carry_the_same_credentials() {
        let node = render(&connection(true), "media", "app", "s3cr3t", "node").unwrap();
        assert!(node.contains("S3_BUCKET=media"));
        assert!(node.contains("S3_FORCE_PATH_STYLE=true"));
        let cli = render(&connection(true), "media", "app", "s3cr3t", "cli").unwrap();
        assert!(cli.contains("--profile media"));
        assert!(cli.contains("s3://media"));
    }

    #[test]
    fn unknown_format_is_rejected() {
        let error = render(&connection(true), "media", "app", "s3cr3t", "dotnet").unwrap_err();
        assert_eq!(error.code, "invalid_format");
    }
}
