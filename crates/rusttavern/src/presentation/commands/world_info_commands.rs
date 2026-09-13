use serde_json::Value;
use std::sync::Arc;
use crate::app::AppState;
use crate::presentation::commands::helpers::{log_command, map_command_error};
use crate::presentation::errors::CommandError;
use crate::server::fs_resources::validate_server_path;
use tt_application::dto::world_info_dto::{
    DeleteWorldInfoDto, GetWorldInfoDto, GetWorldInfosBatchDto, GetWorldInfosBatchResponseDto,
    ImportWorldInfoDto, ImportWorldInfoResponseDto, NormalizeWorldInfoNameDto,
    NormalizeWorldInfoNameResponseDto, SaveWorldInfoDto,
};

pub async fn get_world_info(
    dto: GetWorldInfoDto,
    state: Arc<AppState>,
) -> Result<Value, CommandError> {
    log_command(format!("get_world_info, name: {}", dto.name));

    state
        .services
        .world_info_service
        .get_world_info(&dto.name)
        .await
        .map_err(map_command_error("Failed to get world info"))
}

pub async fn get_world_infos_batch(
    dto: GetWorldInfosBatchDto,
    state: Arc<AppState>,
) -> Result<GetWorldInfosBatchResponseDto, CommandError> {
    log_command(format!("get_world_infos_batch, count: {}", dto.names.len()));

    let items = state
        .services
        .world_info_service
        .get_world_infos_batch(dto.names)
        .await
        .map_err(map_command_error("Failed to get world infos batch"))?;

    Ok(GetWorldInfosBatchResponseDto { items })
}

pub async fn normalize_world_info_name(
    dto: NormalizeWorldInfoNameDto,
    state: Arc<AppState>,
) -> Result<NormalizeWorldInfoNameResponseDto, CommandError> {
    log_command(format!(
        "normalize_world_info_name, import_filename: {}",
        dto.import_filename
    ));

    let name = state
        .services
        .world_info_service
        .normalize_world_info_name(&dto.name, dto.import_filename)
        .map_err(map_command_error("Failed to normalize world info name"))?;

    Ok(NormalizeWorldInfoNameResponseDto { name })
}

pub async fn save_world_info(
    dto: SaveWorldInfoDto,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command(format!("save_world_info, name: {}", dto.name));

    state
        .services
        .world_info_service
        .save_world_info(&dto.name, dto.data)
        .await
        .map_err(map_command_error("Failed to save world info"))
}

pub async fn delete_world_info(
    dto: DeleteWorldInfoDto,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command(format!("delete_world_info, name: {}", dto.name));

    state
        .services
        .world_info_service
        .delete_world_info(&dto.name)
        .await
        .map_err(map_command_error("Failed to delete world info"))
}

pub async fn import_world_info(
    dto: ImportWorldInfoDto,
    state: Arc<AppState>,
) -> Result<ImportWorldInfoResponseDto, CommandError> {
    log_command(format!(
        "import_world_info, original_filename: {}",
        dto.original_filename
    ));

    // An empty file_path means the in-memory converted_data import; anything
    // else is a client-supplied path and must be a staged upload.
    if !dto.file_path.trim().is_empty() {
        validate_server_path(&dto.file_path, &state.host.data_root).await?;
    }

    let name = state
        .services
        .world_info_service
        .import_world_info(&dto.file_path, &dto.original_filename, dto.converted_data)
        .await
        .map_err(map_command_error("Failed to import world info"))?;

    Ok(ImportWorldInfoResponseDto { name })
}
