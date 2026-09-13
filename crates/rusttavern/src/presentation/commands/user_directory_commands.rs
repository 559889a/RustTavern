use std::sync::Arc;

use crate::app::AppState;
use crate::presentation::commands::helpers::{log_command, map_command_error};
use crate::presentation::errors::CommandError;
use tt_application::dto::user_directory_dto::UserDirectoryDto;

pub async fn get_user_directory(
    handle: String,
    state: Arc<AppState>,
) -> Result<UserDirectoryDto, CommandError> {
    log_command(format!("get_user_directory {}", handle));

    state
        .services
        .user_directory_service
        .get_user_directory(&handle)
        .await
        .map_err(map_command_error(format!(
            "Failed to get user directory for {}",
            handle
        )))
}

pub async fn ensure_user_directories_exist(
    handle: String,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command(format!("ensure_user_directories_exist {}", handle));

    state
        .services
        .user_directory_service
        .ensure_user_directories_exist(&handle)
        .await
        .map_err(map_command_error(format!(
            "Failed to ensure directories exist for user {}",
            handle
        )))
}

pub async fn ensure_default_user_directories_exist(
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command("ensure_default_user_directories_exist");

    state
        .services
        .user_directory_service
        .ensure_default_user_directories_exist()
        .await
        .map_err(map_command_error(
            "Failed to ensure directories exist for default user",
        ))
}
