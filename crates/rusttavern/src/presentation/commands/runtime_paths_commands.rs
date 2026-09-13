use std::sync::Arc;

use serde::Serialize;

use crate::app::AppState;
use crate::presentation::commands::helpers::{log_command, map_command_error};
use crate::presentation::errors::CommandError;
use tt_application::services::runtime_paths_service::{
    RuntimeModeInfo, RuntimePathsInfo,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct RuntimePathsDto {
    pub mode: String,
    pub data_root: String,
    pub configured_data_root: Option<String>,
    pub migration_pending: bool,
    pub migration_error: Option<String>,
}

fn runtime_mode_to_string(mode: RuntimeModeInfo) -> String {
    match mode {
        RuntimeModeInfo::Standard => "standard".to_string(),
        RuntimeModeInfo::Portable => "portable".to_string(),
    }
}

fn runtime_paths_dto(info: RuntimePathsInfo) -> RuntimePathsDto {
    RuntimePathsDto {
        mode: runtime_mode_to_string(info.mode),
        data_root: info.data_root.to_string_lossy().to_string(),
        configured_data_root: info
            .configured_data_root
            .map(|path| path.to_string_lossy().to_string()),
        migration_pending: info.migration_pending,
        migration_error: info.migration_error,
    }
}

pub fn get_runtime_paths(state: Arc<AppState>) -> Result<RuntimePathsDto, CommandError> {
    log_command("get_runtime_paths");

    state
        .host
        .runtime_paths
        .get_runtime_paths()
        .map(runtime_paths_dto)
        .map_err(CommandError::from)
}

pub async fn set_data_root(
    data_root: String,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    let raw = data_root.trim();
    log_command(format!("set_data_root {}", raw));

    state
        .host
        .runtime_paths
        .request_data_root_change(raw)
        .await
        .map_err(map_set_data_root_error)
}

fn map_set_data_root_error(error: tt_domain::errors::DomainError) -> CommandError {
    let command_error = CommandError::from(error);
    if matches!(&command_error, CommandError::InternalServerError(_)) {
        map_command_error("Failed to set data root")(command_error)
    } else {
        command_error
    }
}
