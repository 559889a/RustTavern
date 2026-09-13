use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use serde::Serialize;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use url::Url;

use crate::app::AppState;
use crate::presentation::commands::helpers::{log_command, log_command_lazy};
use crate::presentation::errors::CommandError;
use tt_domain::models::filename::sanitize_filename as sanitize_filename_contract;
use uuid::Uuid;
use tt_domain::models::filename::UNSAFE_EXTENSIONS;

#[derive(Debug, Serialize)]
pub struct UserFileUploadResult {
    pub path: String,
}

fn normalize_relative_path(raw: &str) -> Result<PathBuf, CommandError> {
    let normalized = String::from(raw)
        .replace('\\', "/")
        .trim()
        .trim_start_matches('/')
        .to_string();

    if normalized.is_empty() {
        return Err(CommandError::BadRequest(
            "File path cannot be empty".to_string(),
        ));
    }

    let mut path = PathBuf::new();
    for component in Path::new(&normalized).components() {
        match component {
            Component::Normal(segment) => path.push(segment),
            _ => {
                return Err(CommandError::BadRequest("Invalid file path".to_string()));
            }
        }
    }

    if path.as_os_str().is_empty() {
        return Err(CommandError::BadRequest(
            "File path cannot be empty".to_string(),
        ));
    }

    Ok(path)
}

fn validate_upload_name(raw: &str) -> Result<String, CommandError> {
    let name = String::from(raw).trim().to_string();
    if name.is_empty() {
        return Err(CommandError::BadRequest(
            "No upload name specified".to_string(),
        ));
    }

    if name.starts_with('.') {
        return Err(CommandError::BadRequest(
            "Filename cannot start with '.'".to_string(),
        ));
    }

    if name.contains('/') || name.contains('\\') {
        return Err(CommandError::BadRequest(
            "Illegal character in filename".to_string(),
        ));
    }

    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.')
    {
        return Err(CommandError::BadRequest(
            "Illegal character in filename; only alphanumeric, '_', '-' are accepted.".to_string(),
        ));
    }

    let extension = Path::new(&name)
        .extension()
        .map(|ext| format!(".{}", ext.to_string_lossy().to_lowercase()))
        .unwrap_or_default();

    if UNSAFE_EXTENSIONS.contains(&extension.as_str()) {
        return Err(CommandError::BadRequest(
            "Forbidden file extension.".to_string(),
        ));
    }

    Ok(name)
}

fn normalize_user_file_reference(raw: &str) -> Result<PathBuf, CommandError> {
    let mut value = String::from(raw).trim().to_string();
    if value.is_empty() {
        return Err(CommandError::BadRequest("No path specified".to_string()));
    }

    if let Ok(parsed_url) = Url::parse(&value) {
        value = parsed_url.path().to_string();
    }

    let normalized = value.replace('\\', "/");
    let without_leading = normalized.trim_start_matches('/');
    let relative = without_leading
        .strip_prefix("user/files/")
        .ok_or_else(|| CommandError::BadRequest("Invalid path".to_string()))?;

    normalize_relative_path(relative)
}

fn resolve_target_path(root: &Path, relative: &Path) -> Result<PathBuf, CommandError> {
    let target = root.join(relative);
    if !target.starts_with(root) {
        return Err(CommandError::BadRequest("Invalid path".to_string()));
    }
    Ok(target)
}

async fn get_default_user_files_directory(
    state: &Arc<AppState>,
) -> Result<PathBuf, CommandError> {
    let directory = state
        .services
        .user_directory_service
        .get_default_user_directory()
        .await?;
    let files_dir = PathBuf::from(directory.files);

    fs::create_dir_all(&files_dir).await.map_err(|error| {
        CommandError::InternalServerError(format!("Failed to ensure files directory: {}", error))
    })?;

    Ok(files_dir)
}

pub async fn sanitize_filename(file_name: String) -> Result<String, CommandError> {
    log_command(format!("sanitize_filename {}", file_name));

    if file_name.is_empty() {
        return Err(CommandError::BadRequest(
            "No fileName specified".to_string(),
        ));
    }

    Ok(sanitize_filename_contract(&file_name))
}

pub async fn upload_user_file(
    name: String,
    data_base64: String,
    state: Arc<AppState>,
) -> Result<UserFileUploadResult, CommandError> {
    log_command(format!("upload_user_file {}", name));

    let validated_name = validate_upload_name(&name)?;

    let files_dir = get_default_user_files_directory(&state).await?;
    let target_path = resolve_target_path(&files_dir, Path::new(&validated_name))?;

    // Write to a temp sibling and swap it in only when the whole body decoded
    // and landed: `File::create` on the target would truncate the existing
    // file up front, so a mid-upload failure would destroy the old file and
    // leave a half-written one in its place.
    let temp_path = target_path.with_file_name(format!(
        "{}.{}.uploading",
        validated_name,
        Uuid::new_v4().simple()
    ));

    // Decode in base64-aligned slices and stream to disk, so memory stays
    // bounded by one chunk instead of the whole file (the invoke body limit
    // caps requests at 64 MiB; buffering the decoded file on top of that
    // would roughly double the peak).
    let write_result: Result<(), CommandError> = async {
        let mut file = fs::File::create(&temp_path).await.map_err(|error| {
            CommandError::InternalServerError(format!("Failed to save file: {}", error))
        })?;
        const DECODE_ALIGNED_CHUNK: usize = (256 * 1024 / 3) * 4;
        for chunk in data_base64.as_bytes().chunks(DECODE_ALIGNED_CHUNK) {
            let decoded = BASE64_STANDARD.decode(chunk).map_err(|error| {
                CommandError::BadRequest(format!("No upload data specified: {}", error))
            })?;
            file.write_all(&decoded).await.map_err(|error| {
                CommandError::InternalServerError(format!("Failed to save file: {}", error))
            })?;
        }
        file.flush().await.map_err(|error| {
            CommandError::InternalServerError(format!("Failed to save file: {}", error))
        })?;
        Ok(())
    }
    .await;

    if let Err(error) = write_result {
        let _ = fs::remove_file(&temp_path).await;
        return Err(error);
    }

    fs::rename(&temp_path, &target_path)
        .await
        .map_err(|error| {
            CommandError::InternalServerError(format!("Failed to save file: {}", error))
        })?;

    Ok(UserFileUploadResult {
        path: format!("/user/files/{}", validated_name),
    })
}

pub async fn delete_user_file(
    path: String,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command(format!("delete_user_file {}", path));

    let relative = normalize_user_file_reference(&path)?;
    let files_dir = get_default_user_files_directory(&state).await?;
    let target_path = resolve_target_path(&files_dir, &relative)?;

    if !is_regular_file(&target_path).await {
        return Err(CommandError::NotFound("File not found".to_string()));
    }

    fs::remove_file(&target_path).await.map_err(|error| {
        CommandError::InternalServerError(format!("Failed to delete file: {}", error))
    })?;

    Ok(())
}

/// Non-blocking `is_file`. `Path::is_file` is a synchronous stat, and
/// `verify_user_files` runs one per attachment inside an async command, so on a
/// two-worker runtime a large attachment set stalls half the server.
async fn is_regular_file(path: &std::path::Path) -> bool {
    fs::metadata(path)
        .await
        .map(|metadata| metadata.is_file())
        .unwrap_or(false)
}

pub async fn verify_user_files(
    urls: Vec<String>,
    state: Arc<AppState>,
) -> Result<HashMap<String, bool>, CommandError> {
    // Lazy detail: this command can run on a timer; skip formatting when the
    // command log level is off.
    log_command_lazy("verify_user_files", || format!("{}", urls.len()));

    let files_dir = get_default_user_files_directory(&state).await?;
    let mut result = HashMap::with_capacity(urls.len());

    for original_url in urls {
        let Ok(relative) = normalize_user_file_reference(&original_url) else {
            continue;
        };
        let Ok(path) = resolve_target_path(&files_dir, &relative) else {
            continue;
        };
        let exists = is_regular_file(&path).await;
        result.insert(original_url, exists);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{normalize_relative_path, normalize_user_file_reference, validate_upload_name};

    #[test]
    fn validate_upload_name_accepts_safe_filename() {
        assert_eq!(
            validate_upload_name("LittleWhiteBox_CommonSettings.json").unwrap(),
            "LittleWhiteBox_CommonSettings.json"
        );
    }

    #[test]
    fn validate_upload_name_rejects_unsafe_extension() {
        assert!(validate_upload_name("payload.js").is_err());
    }

    #[test]
    fn normalize_relative_path_rejects_parent_segments() {
        assert!(normalize_relative_path("../secret.txt").is_err());
    }

    #[test]
    fn normalize_user_file_reference_extracts_relative_part() {
        let path = normalize_user_file_reference("user/files/test.json").unwrap();
        assert_eq!(path.to_string_lossy(), "test.json");
    }
}
