use std::sync::Arc;

use crate::app::AppState;
use crate::presentation::commands::helpers::{
    ensure_ios_policy_allows, log_command, map_command_error,
};
use crate::presentation::errors::CommandError;
use tt_application::services::asset_service::AssetDownloadResult;
use tt_domain::models::asset::AssetCatalog;

pub async fn get_assets_library(
    state: Arc<AppState>,
) -> Result<AssetCatalog, CommandError> {
    log_command("get_assets_library");

    state
        .services
        .asset_service
        .list_assets()
        .await
        .map_err(map_command_error("Failed to list assets library"))
}

pub async fn download_asset(
    url: String,
    category: String,
    filename: String,
    state: Arc<AppState>,
) -> Result<AssetDownloadResult, CommandError> {
    log_command(format!("download_asset {}", category));

    ensure_ios_policy_allows(
        &state.ios_policy,
        state.ios_policy.capabilities.content.external_import,
        "content.external_import",
    )?;

    state
        .services
        .asset_service
        .download_asset(&url, &category, &filename)
        .await
        .map_err(map_command_error("Failed to download asset"))
}

pub async fn delete_asset(
    category: String,
    filename: String,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command(format!("delete_asset {}", category));

    state
        .services
        .asset_service
        .delete_asset_file(&category, &filename)
        .await
        .map_err(map_command_error("Failed to delete asset"))
}

pub async fn get_character_assets(
    name: String,
    category: String,
    state: Arc<AppState>,
) -> Result<Vec<String>, CommandError> {
    log_command(format!("get_character_assets {}", category));

    state
        .services
        .asset_service
        .list_character_assets(&name, &category)
        .await
        .map_err(map_command_error("Failed to list character assets"))
}
