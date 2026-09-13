use std::sync::Arc;

use crate::app::AppState;
use crate::presentation::commands::helpers::{log_command, map_command_error};
use crate::presentation::errors::CommandError;
use tt_application::services::user_media_service::{
    ListUserImagesInput, UploadUserImageInput, UserImageUploadResult,
};

pub async fn upload_user_image(
    image_base64: String,
    format: String,
    filename: Option<String>,
    ch_name: Option<String>,
    state: Arc<AppState>,
) -> Result<UserImageUploadResult, CommandError> {
    log_command("upload_user_image");

    state
        .host
        .user_media
        .upload_user_image(UploadUserImageInput {
            image_base64,
            format,
            filename,
            ch_name,
        })
        .await
        .map_err(map_command_error("Failed to upload user image"))
}

pub async fn list_user_images(
    folder: String,
    sort_field: Option<String>,
    sort_order: Option<String>,
    media_type: Option<u32>,
    state: Arc<AppState>,
) -> Result<Vec<String>, CommandError> {
    log_command("list_user_images");

    state
        .host
        .user_media
        .list_user_images(ListUserImagesInput {
            folder,
            sort_field,
            sort_order,
            media_type,
        })
        .await
        .map_err(map_command_error("Failed to list user images"))
}

pub async fn list_user_image_folders(
    state: Arc<AppState>,
) -> Result<Vec<String>, CommandError> {
    log_command("list_user_image_folders");

    state
        .host
        .user_media
        .list_user_image_folders()
        .await
        .map_err(map_command_error("Failed to list user image folders"))
}

pub async fn delete_user_image(
    path: String,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command("delete_user_image");

    state
        .host
        .user_media
        .delete_user_image(&path)
        .await
        .map_err(map_command_error("Failed to delete user image"))
}
