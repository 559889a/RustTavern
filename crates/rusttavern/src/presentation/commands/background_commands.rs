use std::sync::Arc;

use crate::app::AppState;
use crate::presentation::commands::helpers::{log_command, map_command_error};
use crate::presentation::errors::CommandError;
use crate::server::fs_resources::validate_server_path;
use tt_application::dto::background_dto::{DeleteBackgroundDto, RenameBackgroundDto};
use tt_domain::models::background::BackgroundListEntry;
use tt_domain::models::image_metadata::ImageMetadataIndex;

pub async fn get_all_backgrounds(
    state: Arc<AppState>,
) -> Result<Vec<BackgroundListEntry>, CommandError> {
    log_command("get_all_backgrounds");

    state
        .services
        .image_metadata_service
        .get_background_list_entries()
        .await
        .map_err(map_command_error("Failed to get all backgrounds"))
}

pub async fn get_all_background_metadata(
    prefix: Option<String>,
    state: Arc<AppState>) -> Result<ImageMetadataIndex, CommandError> {
    log_command(format!(
        "get_all_background_metadata, prefix: {}",
        prefix.clone().unwrap_or_default()
    ));

    state
        .services
        .image_metadata_service
        .get_all_background_metadata(prefix.as_deref())
        .await
        .map_err(map_command_error("Failed to get background metadata"))
}

pub async fn delete_background(
    dto: DeleteBackgroundDto,
    state: Arc<AppState>) -> Result<(), CommandError> {
    log_command(format!("delete_background, filename: {}", dto.bg));

    state
        .services
        .background_service
        .delete_background(&dto.bg)
        .await
        .map_err(map_command_error("Failed to delete background"))
}

pub async fn rename_background(
    dto: RenameBackgroundDto,
    state: Arc<AppState>) -> Result<(), CommandError> {
    log_command(format!(
        "rename_background, from: {} to: {}",
        dto.old_bg, dto.new_bg
    ));

    state
        .services
        .background_service
        .rename_background(&dto.old_bg, &dto.new_bg)
        .await
        .map_err(map_command_error("Failed to rename background"))
}

pub async fn upload_background(
    filename: String,
    data: Vec<u8>,
    state: Arc<AppState>) -> Result<String, CommandError> {
    log_command(format!("upload_background, filename: {}", filename));

    state
        .services
        .background_service
        .upload_background(&filename, &data)
        .await
        .map_err(map_command_error("Failed to upload background"))
}

pub async fn upload_background_from_path(
    filename: String,
    file_path: String,
    state: Arc<AppState>) -> Result<String, CommandError> {
    log_command(format!(
        "upload_background_from_path, filename: {}",
        filename
    ));

    // Client-supplied path: must be a staged upload (see import_chat in
    // chat_commands.rs).
    let path = validate_server_path(&file_path, &state.host.data_root).await?;
    state
        .services
        .background_service
        .upload_background_from_path(&filename, &path)
        .await
        .map_err(map_command_error("Failed to upload background from path"))
}
