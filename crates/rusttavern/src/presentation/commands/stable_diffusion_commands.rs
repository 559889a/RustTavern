use std::sync::Arc;

use serde_json::Value;
use crate::app::AppState;
use crate::presentation::commands::helpers::{
    ensure_ios_policy_allows, log_command, map_command_error,
};
use crate::presentation::errors::CommandError;
use tt_application::dto::stable_diffusion_dto::SdRouteResponseDto;

pub async fn sd_handle(
    request_id: String,
    path: String,
    body: Value,
    state: Arc<AppState>,
) -> Result<SdRouteResponseDto, CommandError> {
    let request_id = request_id.trim().to_string();
    validate_request_id(&request_id)?;
    log_command(format!("sd_handle {} {}", request_id, path));

    ensure_ios_policy_allows(
        &state.ios_policy,
        state.ios_policy.capabilities.ai.image_generation,
        "ai.image_generation",
    )?;

    state
        .services
        .stable_diffusion_service
        .handle_request(&request_id, path, body)
        .await
        .map_err(map_command_error("SD request failed"))
}

pub async fn cancel_sd_request(
    request_id: String,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    let request_id = request_id.trim().to_string();
    validate_request_id(&request_id)?;
    log_command(format!("cancel_sd_request {}", request_id));

    state
        .services
        .stable_diffusion_service
        .cancel_request(&request_id)
        .await;

    Ok(())
}

fn validate_request_id(request_id: &str) -> Result<(), CommandError> {
    let request_id = request_id.trim();
    if request_id.is_empty() || request_id.len() > 128 {
        return Err(CommandError::BadRequest(
            "Invalid request id length".to_string(),
        ));
    }

    if !request_id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err(CommandError::BadRequest(
            "Invalid request id characters".to_string(),
        ));
    }

    Ok(())
}
