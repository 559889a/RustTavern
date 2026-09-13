use std::sync::Arc;

use serde_json::Value;
use crate::app::AppState;
use crate::presentation::commands::helpers::{log_command, map_command_error};
use crate::presentation::errors::CommandError;
use tt_application::dto::tts_dto::TtsRouteResponseDto;

pub async fn tts_handle(
    path: String,
    body: Value,
    state: Arc<AppState>,
) -> Result<TtsRouteResponseDto, CommandError> {
    log_command(format!("tts_handle {}", path));

    state
        .services
        .tts_service
        .handle_request(path, body)
        .await
        .map_err(map_command_error("TTS request failed"))
}
