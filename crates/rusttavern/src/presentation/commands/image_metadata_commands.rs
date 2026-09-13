use std::sync::Arc;

use crate::app::AppState;
use crate::presentation::commands::helpers::{log_command, map_command_error};
use crate::presentation::errors::CommandError;
use tt_application::dto::image_metadata_dto::{
    CreateImageMetadataFolderDto, DeleteImageMetadataFolderDto, ImageMetadataFolderAssignmentDto,
    SetImageMetadataFolderThumbnailsDto, UpdateImageMetadataFolderDto,
};
use tt_domain::models::image_metadata::{BackgroundFoldersPayload, ImageMetadataFolder};

pub async fn get_background_folders(
    state: Arc<AppState>,
) -> Result<BackgroundFoldersPayload, CommandError> {
    log_command("get_background_folders");

    state
        .services
        .image_metadata_service
        .get_background_folders()
        .await
        .map_err(map_command_error("Failed to get background folders"))
}

pub async fn create_image_metadata_folder(
    dto: CreateImageMetadataFolderDto,
    state: Arc<AppState>) -> Result<ImageMetadataFolder, CommandError> {
    log_command("create_image_metadata_folder");

    state
        .services
        .image_metadata_service
        .create_folder(dto)
        .await
        .map_err(map_command_error("Failed to create image metadata folder"))
}

pub async fn update_image_metadata_folder(
    dto: UpdateImageMetadataFolderDto,
    state: Arc<AppState>) -> Result<ImageMetadataFolder, CommandError> {
    log_command(format!("update_image_metadata_folder, id: {}", dto.id));

    state
        .services
        .image_metadata_service
        .update_folder(dto)
        .await
        .map_err(map_command_error("Failed to update image metadata folder"))
}

pub async fn delete_image_metadata_folder(
    dto: DeleteImageMetadataFolderDto,
    state: Arc<AppState>) -> Result<(), CommandError> {
    log_command(format!("delete_image_metadata_folder, id: {}", dto.id));

    state
        .services
        .image_metadata_service
        .delete_folder(dto)
        .await
        .map_err(map_command_error("Failed to delete image metadata folder"))
}

pub async fn set_image_metadata_folder_thumbnails(
    dto: SetImageMetadataFolderThumbnailsDto,
    state: Arc<AppState>) -> Result<(), CommandError> {
    log_command("set_image_metadata_folder_thumbnails");

    state
        .services
        .image_metadata_service
        .set_folder_thumbnails(dto)
        .await
        .map_err(map_command_error(
            "Failed to set image metadata folder thumbnails",
        ))
}

pub async fn assign_images_to_metadata_folder(
    dto: ImageMetadataFolderAssignmentDto,
    state: Arc<AppState>) -> Result<(), CommandError> {
    log_command(format!("assign_images_to_metadata_folder, id: {}", dto.id));

    state
        .services
        .image_metadata_service
        .assign_images_to_folder(dto)
        .await
        .map_err(map_command_error(
            "Failed to assign images to metadata folder",
        ))
}

pub async fn unassign_images_from_metadata_folder(
    dto: ImageMetadataFolderAssignmentDto,
    state: Arc<AppState>) -> Result<(), CommandError> {
    log_command(format!(
        "unassign_images_from_metadata_folder, id: {}",
        dto.id
    ));

    state
        .services
        .image_metadata_service
        .unassign_images_from_folder(dto)
        .await
        .map_err(map_command_error(
            "Failed to unassign images from metadata folder",
        ))
}
