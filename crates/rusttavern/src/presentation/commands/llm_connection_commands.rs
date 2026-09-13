use std::sync::Arc;

use crate::app::AppState;
use crate::presentation::commands::helpers::{log_command, map_command_error};
use crate::presentation::errors::CommandError;
use tt_application::dto::llm_connection_dto::{
    ListLlmConnectionsResultDto, LlmConnectionIdDto, LoadLlmConnectionResultDto,
    SaveLlmConnectionDto,
};

pub async fn list_llm_connections(
    state: Arc<AppState>,
) -> Result<ListLlmConnectionsResultDto, CommandError> {
    log_command("list_llm_connections");

    state
        .services
        .llm_connection_service
        .list_connections()
        .await
        .map(|connections| ListLlmConnectionsResultDto { connections })
        .map_err(map_command_error("Failed to list LLM connections"))
}

pub async fn load_llm_connection(
    dto: LlmConnectionIdDto,
    state: Arc<AppState>,
) -> Result<LoadLlmConnectionResultDto, CommandError> {
    log_command("load_llm_connection");

    state
        .services
        .llm_connection_service
        .load_connection(&dto.connection_id)
        .await
        .map(|connection| LoadLlmConnectionResultDto { connection })
        .map_err(map_command_error("Failed to load LLM connection"))
}

pub async fn save_llm_connection(
    dto: SaveLlmConnectionDto,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command("save_llm_connection");

    state
        .services
        .llm_connection_service
        .save_connection(dto.connection)
        .await
        .map_err(map_command_error("Failed to save LLM connection"))
}

pub async fn delete_llm_connection(
    dto: LlmConnectionIdDto,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command("delete_llm_connection");

    state
        .services
        .llm_connection_service
        .delete_connection(&dto.connection_id)
        .await
        .map_err(map_command_error("Failed to delete LLM connection"))
}
