use std::sync::Arc;

use crate::app::AppState;
use crate::presentation::commands::helpers::{log_command, map_command_error};
use crate::presentation::errors::CommandError;
use tt_application::dto::group_dto::{CreateGroupDto, DeleteGroupDto, GroupDto, UpdateGroupDto};

pub async fn get_all_groups(
    state: Arc<AppState>,
) -> Result<Vec<GroupDto>, CommandError> {
    log_command("get_all_groups");

    state
        .services
        .group_service
        .get_all_groups()
        .await
        .map(|groups| groups.into_iter().map(GroupDto::from).collect())
        .map_err(map_command_error("Failed to get all groups"))
}

pub async fn get_group(
    id: String,
    state: Arc<AppState>,
) -> Result<Option<GroupDto>, CommandError> {
    log_command(format!("get_group {}", id));

    state
        .services
        .group_service
        .get_group(&id)
        .await
        .map(|group| group.map(GroupDto::from))
        .map_err(map_command_error(format!("Failed to get group {}", id)))
}

pub async fn create_group(
    dto: CreateGroupDto,
    state: Arc<AppState>,
) -> Result<GroupDto, CommandError> {
    log_command(format!("create_group {}", dto.name));

    state
        .services
        .group_service
        .create_group(dto)
        .await
        .map(GroupDto::from)
        .map_err(map_command_error("Failed to create group"))
}

pub async fn update_group(
    dto: UpdateGroupDto,
    state: Arc<AppState>,
) -> Result<GroupDto, CommandError> {
    log_command(format!("update_group {}", dto.id));

    state
        .services
        .group_service
        .update_group(dto)
        .await
        .map(GroupDto::from)
        .map_err(map_command_error("Failed to update group"))
}

pub async fn delete_group(
    dto: DeleteGroupDto,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command(format!("delete_group {}", dto.id));

    state
        .services
        .group_service
        .delete_group(dto)
        .await
        .map_err(map_command_error("Failed to delete group"))
}

pub async fn get_group_chat_paths(
    state: Arc<AppState>,
) -> Result<Vec<String>, CommandError> {
    log_command("get_group_chat_paths");

    state
        .services
        .group_service
        .get_group_chat_paths()
        .await
        .map_err(map_command_error("Failed to get group chat paths"))
}

pub async fn clear_group_cache(state: Arc<AppState>) -> Result<(), CommandError> {
    log_command("clear_group_cache");

    state
        .services
        .group_service
        .clear_cache()
        .await
        .map_err(map_command_error("Failed to clear group cache"))
}
