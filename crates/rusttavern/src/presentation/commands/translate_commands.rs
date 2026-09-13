use std::sync::Arc;

use serde_json::Value;
use crate::app::AppState;
use crate::presentation::commands::helpers::{log_command, map_command_error};
use crate::presentation::errors::CommandError;

pub async fn translate_text(
    provider: String,
    body: Value,
    state: Arc<AppState>,
) -> Result<String, CommandError> {
    log_command(format!("translate_text {}", provider));

    state
        .services
        .translate_service
        .translate(&provider, body)
        .await
        .map_err(map_command_error("Translation failed"))
}
