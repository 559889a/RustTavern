//! Tauri plugin compat commands (server mode).
//!
//! The frontend still routes a handful of `plugin:*` commands through the
//! generic invoke dispatch. Server mode implements the ones that stay
//! meaningful and returns explicit errors for the ones that cannot work:
//!
//! - `plugin:fs|open/read` + `plugin:resources|close`: rid-based file reads
//!   (`readable-file-stream-service`, `asset-io` Android path). Backed by
//!   `server::fs_resources::FsResourceRegistry`; paths are validated against
//!   the data root / upload staging root.
//! - `plugin:fs|remove`: staging cleanup (`upload-service.removeTempUploadFile`).
//! - `plugin:fs|write_file` / `plugin:fs|mkdir`: explicit errors — the caller
//!   (`file-export`) falls back to browser downloads in server mode.
//! - `plugin:dialog|open`: `null` (callers fall back to browser file inputs).
//! - `plugin:opener|*`: explicit errors (extensions surface their own toasts).

use std::sync::Arc;

use base64::Engine as _;
use serde_json::Value;

use crate::app::AppState;
use crate::presentation::errors::CommandError;
use crate::server::fs_resources::{parse_open_options, validate_server_path};

const DEFAULT_FS_READ_BYTES: usize = 512 * 1024;

pub async fn plugin_fs_open(
    path: String,
    options: Option<Value>,
    state: Arc<AppState>,
) -> Result<u64, CommandError> {
    let canonical = validate_server_path(&path, &state.host.data_root).await?;
    let open_options = parse_open_options(options.as_ref());
    state
        .host
        .fs_resources
        .open(&canonical, &open_options)
        .await
}

pub async fn plugin_fs_read(
    rid: u64,
    len: Option<usize>,
    state: Arc<AppState>,
) -> Result<String, CommandError> {
    let bytes = state
        .host
        .fs_resources
        .read(rid, len.unwrap_or(DEFAULT_FS_READ_BYTES))
        .await?;
    // Base64 instead of a JSON number array: the raw bytes would expand
    // ~4.5x as an array of integers, base64 only ~1.33x (M5).
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

pub async fn plugin_resources_close(
    rid: u64,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    state.host.fs_resources.close(rid).await
}

pub async fn plugin_fs_remove(
    path: String,
    options: Option<Value>,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    let canonical = validate_server_path(&path, &state.host.data_root).await?;
    let recursive = options
        .as_ref()
        .and_then(|value| value.get("recursive"))
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let metadata = tokio::fs::symlink_metadata(&canonical).await.map_err(|error| {
        CommandError::InternalServerError(format!(
            "Failed to stat {}: {}",
            canonical.display(),
            error
        ))
    })?;

    if metadata.is_dir() {
        if !recursive {
            return Err(CommandError::BadRequest(format!(
                "Refusing to remove directory without recursive option: {}",
                canonical.display()
            )));
        }
        tokio::fs::remove_dir_all(&canonical).await.map_err(|error| {
            CommandError::InternalServerError(format!(
                "Failed to remove directory {}: {}",
                canonical.display(),
                error
            ))
        })
    } else {
        tokio::fs::remove_file(&canonical).await.map_err(|error| {
            CommandError::InternalServerError(format!(
                "Failed to remove file {}: {}",
                canonical.display(),
                error
            ))
        })
    }
}

/// `plugin:fs|write_file` cannot be served through the generic JSON dispatch
/// (the frontend sends raw bytes in the Tauri IPC body). Server mode callers
/// (`file-export`) already fall back to browser downloads.
pub async fn plugin_fs_write_file(
    _path: String,
    _data: Value,
    _state: Arc<AppState>,
) -> Result<(), CommandError> {
    Err(CommandError::BadRequest(
        "plugin:fs|write_file is not available in server mode; use the browser download bridge"
            .to_string(),
    ))
}

/// `plugin:fs|mkdir` — explicit error: server mode has no native staging
/// directories (callers fall back to browser downloads).
pub async fn plugin_fs_mkdir(
    _path: String,
    _options: Option<Value>,
    _state: Arc<AppState>,
) -> Result<(), CommandError> {
    Err(CommandError::BadRequest(
        "plugin:fs|mkdir is not available in server mode".to_string(),
    ))
}

/// `plugin:dialog|open` — return `null` so callers (character cards, skills)
/// fall back to their browser file-input paths.
pub async fn plugin_dialog_open() -> Result<Option<Value>, CommandError> {
    Ok(None)
}

/// `plugin:opener|open_url` — explicit error; `host-bridge.openExternalUrl`
/// uses `window.open` in server mode.
pub async fn plugin_opener_open_url(
    _url: String,
    _with: Option<Value>,
) -> Result<(), CommandError> {
    Err(CommandError::BadRequest(
        "plugin:opener|open_url is not available in server mode".to_string(),
    ))
}

/// `plugin:opener|reveal_item_in_dir` — explicit error; the data-migration
/// extension already surfaces the failure toast.
pub async fn plugin_opener_reveal_item_in_dir(
    _paths: Vec<String>,
) -> Result<(), CommandError> {
    Err(CommandError::BadRequest(
        "plugin:opener|reveal_item_in_dir is not available in server mode".to_string(),
    ))
}
