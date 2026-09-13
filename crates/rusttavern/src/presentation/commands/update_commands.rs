use std::sync::Arc;

use crate::app::AppState;
use crate::presentation::commands::helpers::{
    ensure_ios_policy_allows, log_command, map_command_error,
};
use crate::presentation::errors::CommandError;
use tt_domain::models::update::{UpdateChannel, UpdateCheckResult};

pub async fn check_for_update(
    channel: UpdateChannel,
    state: Arc<AppState>,
) -> Result<UpdateCheckResult, CommandError> {
    log_command("check_for_update");

    ensure_ios_policy_allows(
        &state.ios_policy,
        state.ios_policy.capabilities.updates.manual_check,
        "updates.manual_check",
    )?;

    state
        .services
        .update_service
        .check_for_update(channel)
        .await
        .map_err(map_command_error("Failed to check for update"))
}
