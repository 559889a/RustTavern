use std::sync::Arc;

use crate::app::AppState;
use crate::presentation::commands::helpers::{
    ensure_ios_policy_allows, log_command, map_command_error,
};
use crate::presentation::errors::CommandError;
use tt_application::services::content_service::ExternalImportDownloadResult;

pub async fn initialize_default_content(
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command("initialize_default_content");

    state
        .services
        .content_service
        .initialize_default_content("default-user")
        .await
        .map_err(map_command_error("Failed to initialize default content"))
}

pub async fn is_default_content_initialized(
    state: Arc<AppState>,
) -> Result<bool, CommandError> {
    log_command("is_default_content_initialized");

    state
        .services
        .content_service
        .is_default_content_initialized("default-user")
        .await
        .map_err(map_command_error(
            "Failed to check default content initialization state",
        ))
}

pub async fn download_external_import_url(
    url: String,
    state: Arc<AppState>,
) -> Result<ExternalImportDownloadResult, CommandError> {
    log_command("download_external_import_url");

    ensure_ios_policy_allows(
        &state.ios_policy,
        state.ios_policy.capabilities.content.external_import,
        "content.external_import",
    )?;

    state
        .services
        .content_service
        .download_external_import_url(&url)
        .await
        .map_err(map_command_error("Failed to download external import URL"))
}
