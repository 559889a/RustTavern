use std::sync::Arc;

use crate::app::AppState;
use crate::presentation::commands::helpers::{log_command, log_command_lazy, map_command_error};
use crate::presentation::errors::CommandError;
use crate::server::fs_resources::{upload_staging_root, validate_server_path_in_roots};
use tt_application::dto::data_archive_dto::{DataArchiveJobStatus, UserBackupArchiveResult};

pub async fn start_import_data_archive(
    archive_path: String,
    archive_is_temporary: bool,
    state: Arc<AppState>) -> Result<String, CommandError> {
    log_command(format!(
        "start_import_data_archive {} temporary={}",
        archive_path, archive_is_temporary
    ));

    // Client-supplied path: archives legitimately arrive from the upload
    // staging root, the data root, or the archive-imports staging root that
    // `prepare_data_archive_import_target_path` hands out.
    let archive_path = validate_server_path_in_roots(
        &archive_path,
        &[
            state.host.data_root.clone(),
            upload_staging_root(),
            state.host.archive_imports_root.clone(),
        ],
    )
    .await?
    .to_string_lossy()
    .to_string();

    state
        .services
        .data_archive_service
        .start_import(std::path::Path::new(&archive_path), archive_is_temporary)
        .map_err(map_command_error("Failed to start data archive import"))
}

pub fn start_export_data_archive(
    state: Arc<AppState>,
) -> Result<String, CommandError> {
    log_command("start_export_data_archive");

    state
        .services
        .data_archive_service
        .start_export()
        .map_err(map_command_error("Failed to start data archive export"))
}

pub fn prepare_data_archive_import_target_path(
    state: Arc<AppState>,
) -> Result<String, CommandError> {
    log_command("prepare_data_archive_import_target_path");

    let path = state
        .services
        .data_archive_service
        .prepare_incoming_import_archive_path()
        .map_err(map_command_error(
            "Failed to prepare data archive import target path",
        ))?;

    Ok(path.to_string_lossy().to_string())
}

pub fn get_data_archive_job_status(
    job_id: String,
    state: Arc<AppState>) -> Result<DataArchiveJobStatus, CommandError> {
    // Lazy detail: import/export jobs are polled every second while active.
    log_command_lazy("get_data_archive_job_status", || job_id.clone());

    state
        .services
        .data_archive_service
        .get_status(&job_id)
        .map_err(map_command_error("Failed to get data archive job status"))
}

pub fn cancel_data_archive_job(
    job_id: String,
    state: Arc<AppState>) -> Result<(), CommandError> {
    log_command(format!("cancel_data_archive_job {}", job_id));

    state
        .services
        .data_archive_service
        .cancel(&job_id)
        .map_err(map_command_error("Failed to cancel data archive job"))
}

pub async fn save_export_data_archive(
    job_id: String,
    state: Arc<AppState>) -> Result<String, CommandError> {
    log_command(format!("save_export_data_archive {}", job_id));

    let saved_path = state
        .services
        .data_archive_service
        .save_export(job_id)
        .await
        .map_err(map_command_error("Failed to save export data archive"))?;

    Ok(saved_path.to_string_lossy().to_string())
}

pub fn cleanup_export_data_archive(
    job_id: String,
    state: Arc<AppState>) -> Result<(), CommandError> {
    log_command(format!("cleanup_export_data_archive {}", job_id));

    state
        .services
        .data_archive_service
        .cleanup_export(&job_id)
        .map_err(map_command_error("Failed to cleanup export data archive"))
}

pub fn finalize_export_data_archive_delivery(
    job_id: String,
    saved_path: Option<String>,
    state: Arc<AppState>) -> Result<Option<String>, CommandError> {
    log_command(format!("finalize_export_data_archive_delivery {}", job_id));

    let saved_target = saved_path
        .map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty());

    state
        .services
        .data_archive_service
        .finalize_export_delivery(&job_id, saved_target)
        .map_err(map_command_error(
            "Failed to finalize export data archive delivery",
        ))
}

pub async fn export_user_backup_archive(
    handle: String,
    include_secrets: bool,
    state: Arc<AppState>) -> Result<UserBackupArchiveResult, CommandError> {
    log_command(format!(
        "export_user_backup_archive {} include_secrets={}",
        handle, include_secrets
    ));

    state
        .services
        .data_archive_service
        .export_user_backup(handle, include_secrets)
        .await
        .map_err(map_command_error("Failed to export user backup archive"))
}

pub async fn save_user_backup_archive(
    archive_path: String,
    file_name: String,
    state: Arc<AppState>) -> Result<String, CommandError> {
    log_command("save_user_backup_archive");

    let saved_path = state
        .services
        .data_archive_service
        .save_user_backup(archive_path, file_name)
        .await
        .map_err(map_command_error("Failed to save user backup archive"))?;

    Ok(saved_path.to_string_lossy().to_string())
}

pub fn cleanup_user_backup_archive(
    archive_path: String,
    state: Arc<AppState>) -> Result<(), CommandError> {
    log_command("cleanup_user_backup_archive");

    state
        .services
        .data_archive_service
        .cleanup_user_backup(&archive_path)
        .map_err(map_command_error("Failed to cleanup user backup archive"))
}
