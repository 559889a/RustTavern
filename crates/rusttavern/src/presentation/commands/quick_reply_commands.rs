use serde_json::Value;
use std::sync::Arc;
use crate::app::AppState;
use crate::presentation::commands::helpers::{log_command, map_command_error};
use crate::presentation::errors::CommandError;

pub async fn save_quick_reply_set(
    payload: Value,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command("save_quick_reply_set");

    state
        .services
        .quick_reply_service
        .save_quick_reply_set(payload)
        .await
        .map_err(map_command_error("Failed to save quick reply set"))
}

pub async fn delete_quick_reply_set(
    payload: Value,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command("delete_quick_reply_set");

    state
        .services
        .quick_reply_service
        .delete_quick_reply_set(payload)
        .await
        .map_err(map_command_error("Failed to delete quick reply set"))
}
