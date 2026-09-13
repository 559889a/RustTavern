use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::app::AppState;
use crate::presentation::commands::chunk_body::decode_base64_chunk;
use crate::presentation::commands::helpers::log_command;
use crate::presentation::errors::CommandError;

const STAGING_ROOT_NAME: &str = "rusttavern-upload-staging";
const DEFAULT_KIND: &str = "generic";
const MOBILE_SMALL_ASSET_CHUNK_BYTES: u64 = 512 * 1024;
const DESKTOP_DEFAULT_CHUNK_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Deserialize)]
pub struct StageUploadBeginDto {
    pub kind: Option<String>,
    pub preferred_extension: Option<String>,
    pub size: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct StageUploadBeginResult {
    pub file_path: String,
    pub chunk_size: u64,
}

#[derive(Debug, Serialize)]
pub struct StageUploadFinishResult {
    pub file_path: String,
    pub size: u64,
}

fn normalize_kind(value: Option<&str>) -> Result<String, CommandError> {
    let kind = value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_KIND);

    if kind.len() > 48
        || !kind
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
    {
        return Err(CommandError::BadRequest("Invalid upload kind".to_string()));
    }

    Ok(kind.to_string())
}

fn normalize_extension(value: Option<&str>) -> Result<String, CommandError> {
    let extension = value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("bin")
        .trim_start_matches('.')
        .to_ascii_lowercase();

    if extension.is_empty()
        || extension.len() > 12
        || !extension.chars().all(|ch| ch.is_ascii_alphanumeric())
    {
        return Err(CommandError::BadRequest(
            "Invalid upload extension".to_string(),
        ));
    }

    Ok(extension)
}

fn chunk_size_for_kind(kind: &str) -> u64 {
    match kind {
        "avatar" | "user-avatar" | "worldinfo-import" => MOBILE_SMALL_ASSET_CHUNK_BYTES,
        _ => DESKTOP_DEFAULT_CHUNK_BYTES,
    }
}

/// Upload staging lives in the OS temp directory (transient by design).
fn staging_root(kind: &str) -> PathBuf {
    std::env::temp_dir().join(STAGING_ROOT_NAME).join(kind)
}

async fn validate_staged_path(file_path: &str) -> Result<PathBuf, CommandError> {
    let requested = PathBuf::from(file_path.trim());
    if !requested.is_absolute() {
        return Err(CommandError::BadRequest(
            "Upload staging path must be absolute".to_string(),
        ));
    }

    let root = staging_root(DEFAULT_KIND)
        .parent()
        .ok_or_else(|| CommandError::InternalServerError("Invalid upload staging root".to_string()))?
        .to_path_buf();
    let parent = requested.parent().ok_or_else(|| {
        CommandError::BadRequest("Upload staging path is missing a parent directory".to_string())
    })?;

    let canonical_root = canonicalize_existing(&root, "upload staging root").await?;
    let canonical_parent = canonicalize_existing(parent, "upload staging parent").await?;
    if !canonical_parent.starts_with(&canonical_root) {
        return Err(CommandError::BadRequest(
            "Upload staging path is outside the staging directory".to_string(),
        ));
    }

    // Check the leaf as well: a symlink planted in the staging directory keeps its
    // parent looking legitimate while the append/remove lands somewhere else. Only
    // an existing leaf can be checked, and a missing one is left to the caller —
    // `stage_upload_discard` treats "already gone" as success.
    if let Ok(canonical_file) = tokio::fs::canonicalize(&requested).await
        && !canonical_file.starts_with(&canonical_root)
    {
        return Err(CommandError::BadRequest(
            "Upload staging path is outside the staging directory".to_string(),
        ));
    }

    Ok(requested)
}

async fn canonicalize_existing(path: &Path, label: &str) -> Result<PathBuf, CommandError> {
    tokio::fs::canonicalize(path).await.map_err(|error| {
        CommandError::InternalServerError(format!("Failed to canonicalize {}: {}", label, error))
    })
}

pub async fn stage_upload_begin(
    dto: StageUploadBeginDto,
    _state: Arc<AppState>,
) -> Result<StageUploadBeginResult, CommandError> {
    let kind = normalize_kind(dto.kind.as_deref())?;
    let extension = normalize_extension(dto.preferred_extension.as_deref())?;
    log_command(format!(
        "stage_upload_begin kind={} size={}",
        kind,
        dto.size.unwrap_or(0)
    ));

    let directory = staging_root(&kind);
    fs::create_dir_all(&directory).await.map_err(|error| {
        CommandError::InternalServerError(format!(
            "Failed to create upload staging directory: {}",
            error
        ))
    })?;

    let file_name = format!("{}.{}", uuid::Uuid::new_v4().simple(), extension);
    let file_path = directory.join(file_name);
    fs::File::create(&file_path).await.map_err(|error| {
        CommandError::InternalServerError(format!(
            "Failed to create upload staging file: {}",
            error
        ))
    })?;

    Ok(StageUploadBeginResult {
        file_path: file_path.to_string_lossy().to_string(),
        chunk_size: chunk_size_for_kind(&kind),
    })
}

/// Append a base64-encoded chunk. `data` is the base64 payload; `file_path`
/// and `offset` identify the staged file and expected position.
pub async fn stage_upload_chunk(
    file_path: String,
    offset: u64,
    data: String,
    _state: Arc<AppState>,
) -> Result<u64, CommandError> {
    let data = decode_base64_chunk(&data)?;
    let path = validate_staged_path(&file_path).await?;
    let metadata = fs::metadata(&path).await.map_err(|error| {
        CommandError::InternalServerError(format!("Failed to stat upload staging file: {}", error))
    })?;
    let current_len = metadata.len();
    if current_len != offset {
        return Err(CommandError::BadRequest(format!(
            "Upload staging offset mismatch: expected {}, got {}",
            current_len, offset
        )));
    }

    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .await
        .map_err(|error| {
            CommandError::InternalServerError(format!(
                "Failed to open upload staging file: {}",
                error
            ))
        })?;

    file.write_all(&data).await.map_err(|error| {
        CommandError::InternalServerError(format!(
            "Failed to write upload staging chunk: {}",
            error
        ))
    })?;

    Ok(offset + data.len() as u64)
}

pub async fn stage_upload_finish(
    file_path: String,
    expected_size: u64,
    _state: Arc<AppState>,
) -> Result<StageUploadFinishResult, CommandError> {
    let path = validate_staged_path(&file_path).await?;
    let metadata = fs::metadata(&path).await.map_err(|error| {
        CommandError::InternalServerError(format!("Failed to stat upload staging file: {}", error))
    })?;
    let size = metadata.len();
    if size != expected_size {
        return Err(CommandError::BadRequest(format!(
            "Upload staging size mismatch: expected {}, got {}",
            expected_size, size
        )));
    }

    Ok(StageUploadFinishResult {
        file_path: path.to_string_lossy().to_string(),
        size,
    })
}

pub async fn stage_upload_discard(
    file_path: String,
    _state: Arc<AppState>,
) -> Result<(), CommandError> {
    let path = validate_staged_path(&file_path).await?;
    match fs::remove_file(&path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(CommandError::InternalServerError(format!(
            "Failed to remove upload staging file: {}",
            error
        ))),
    }
}
