use std::sync::Arc;

use crate::app::AppState;
use crate::presentation::commands::helpers::{log_command, map_command_error};
use crate::presentation::errors::CommandError;
use tt_application::dto::native_regex_dto::{
    NativeRegexBatchRequestDto, NativeRegexBatchResponseDto,
};

pub async fn apply_native_regex_batch(
    dto: NativeRegexBatchRequestDto,
    state: Arc<AppState>,
) -> Result<NativeRegexBatchResponseDto, CommandError> {
    log_command("apply_native_regex_batch");

    state
        .services
        .native_regex_service
        .apply_batch(dto)
        .await
        .map_err(map_command_error("Failed to apply native regex batch"))
}
