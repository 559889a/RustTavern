use std::sync::Arc;

use crate::app::AppState;
use crate::presentation::commands::helpers::{
    log_command, log_user_visible_error, map_command_error,
};
use crate::presentation::errors::CommandError;
use crate::server::fs_resources::validate_server_path;
use tt_domain::models::avatar::{AvatarUploadResult, CropInfo};

pub async fn get_avatars(state: Arc<AppState>) -> Result<Vec<String>, CommandError> {
    log_command("get_avatars");

    state
        .services
        .avatar_service
        .get_avatars()
        .await
        .map_err(map_command_error("Failed to get avatars"))
}

pub async fn delete_avatar(
    avatar: String,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command(format!("delete_avatar {}", avatar));

    state
        .services
        .avatar_service
        .delete_avatar(&avatar)
        .await
        .map_err(map_command_error("Failed to delete avatar"))
}

pub async fn upload_avatar(
    file_path: String,
    overwrite_name: Option<String>,
    crop: Option<String>,
    state: Arc<AppState>,
) -> Result<AvatarUploadResult, CommandError> {
    log_command(format!("upload_avatar {}", file_path));

    let crop_info = match crop {
        Some(crop_str) => match serde_json::from_str::<CropInfo>(&crop_str) {
            Ok(info) => Some(info),
            Err(error) => {
                let message = format!("Invalid avatar crop information: {}", error);
                log_user_visible_error(&message);
                None
            }
        },
        None => None,
    };

    // Client-supplied path: must be a staged upload (see import_chat in
    // chat_commands.rs).
    let path = validate_server_path(&file_path, &state.host.data_root).await?;
    state
        .services
        .avatar_service
        .upload_avatar(&path, overwrite_name, crop_info)
        .await
        .map_err(map_command_error("Failed to upload avatar"))
}
