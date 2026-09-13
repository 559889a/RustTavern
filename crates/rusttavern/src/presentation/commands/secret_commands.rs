use std::sync::Arc;

use crate::app::AppState;
use crate::presentation::commands::helpers::{log_command, map_command_error};
use crate::presentation::errors::CommandError;
use tt_application::dto::secret_dto::{
    AllSecretsDto, DeleteSecretDto, FindSecretDto, FindSecretResponseDto, RenameSecretDto,
    RotateSecretDto, SecretSettingsDto, SecretStateDto, WriteSecretDto,
};

pub async fn write_secret(
    dto: WriteSecretDto,
    state: Arc<AppState>,
) -> Result<String, CommandError> {
    log_command(format!("write_secret {}", dto.key));

    let id = state
        .services
        .secret_service
        .write_secret(&dto.key, &dto.value, dto.label.as_deref())
        .await
        .map_err(map_command_error(format!(
            "Failed to write secret {}",
            dto.key
        )))?;

    Ok(id)
}

pub async fn read_secret_state(
    state: Arc<AppState>,
) -> Result<SecretStateDto, CommandError> {
    log_command("read_secret_state");

    state
        .services
        .secret_service
        .read_secret_state()
        .await
        .map_err(map_command_error("Failed to read secret state"))
}

pub async fn read_secret_settings(
    state: Arc<AppState>,
) -> Result<SecretSettingsDto, CommandError> {
    log_command("read_secret_settings");

    Ok(state.services.secret_service.read_settings())
}

pub async fn view_secrets(
    state: Arc<AppState>,
) -> Result<AllSecretsDto, CommandError> {
    log_command("view_secrets");

    state
        .services
        .secret_service
        .view_secrets()
        .await
        .map_err(map_command_error("Failed to view secrets"))
}

pub async fn find_secret(
    dto: FindSecretDto,
    state: Arc<AppState>,
) -> Result<FindSecretResponseDto, CommandError> {
    log_command(format!("find_secret {}", dto.key));

    state
        .services
        .secret_service
        .find_secret(&dto.key, dto.id.as_deref())
        .await
        .map_err(map_command_error(format!(
            "Failed to find secret {}",
            dto.key
        )))
}

pub async fn delete_secret(
    dto: DeleteSecretDto,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command(format!("delete_secret {}", dto.key));

    state
        .services
        .secret_service
        .delete_secret(&dto.key, dto.id.as_deref())
        .await
        .map_err(map_command_error(format!(
            "Failed to delete secret {}",
            dto.key
        )))
}

pub async fn rotate_secret(
    dto: RotateSecretDto,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command(format!("rotate_secret {}", dto.key));

    state
        .services
        .secret_service
        .rotate_secret(&dto.key, &dto.id)
        .await
        .map_err(map_command_error(format!(
            "Failed to rotate secret {}",
            dto.key
        )))
}

pub async fn rename_secret(
    dto: RenameSecretDto,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command(format!("rename_secret {}", dto.key));

    state
        .services
        .secret_service
        .rename_secret(&dto.key, &dto.id, &dto.label)
        .await
        .map_err(map_command_error(format!(
            "Failed to rename secret {}",
            dto.key
        )))
}
