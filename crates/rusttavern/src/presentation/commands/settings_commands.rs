use std::sync::Arc;

use crate::app::AppState;
use crate::presentation::commands::helpers::{
    ensure_ios_policy_allows, log_command, map_command_error,
};
use crate::presentation::errors::CommandError;
use tt_application::dto::settings_dto::{
    SettingsSnapshotDto, SillyTavernSettingsResponseDto, RustTavernSettingsDto,
    UpdateRustTavernSettingsDto, UserSettingsDto, UserSettingsPatchDto, UserSettingsSaveResultDto,
};
use tt_contracts::chat::ChatBackupStorageStats;

pub async fn get_rusttavern_settings(
    state: Arc<AppState>,
) -> Result<RustTavernSettingsDto, CommandError> {
    log_command("get_rusttavern_settings");

    state
        .services
        .settings_service
        .get_rusttavern_settings()
        .await
        .map_err(map_command_error("Failed to get RustTavern settings"))
}

pub async fn get_chat_backup_storage_stats(
    state: Arc<AppState>,
) -> Result<Option<ChatBackupStorageStats>, CommandError> {
    log_command("get_chat_backup_storage_stats");

    state
        .services
        .settings_service
        .get_chat_backup_storage_stats()
        .await
        .map_err(map_command_error("Failed to get chat backup storage stats"))
}

pub async fn update_rusttavern_settings(
    dto: UpdateRustTavernSettingsDto,
    state: Arc<AppState>,
) -> Result<RustTavernSettingsDto, CommandError> {
    log_command("update_rusttavern_settings");

    let agent_retention_settings_updated = has_agent_retention_settings_update(&dto);
    if dto
        .request_proxy
        .as_ref()
        .is_some_and(|settings| settings.enabled)
    {
        ensure_ios_policy_allows(
            &state.ios_policy,
            state.ios_policy.capabilities.network.request_proxy,
            "network.request_proxy",
        )?;
    }

    let settings = state
        .services
        .settings_service
        .update_rusttavern_settings(dto)
        .await
        .map_err(map_command_error("Failed to update RustTavern settings"))?;

    state
        .host
        .host_resources
        .set_avatar_persona_original_images_enabled(
            settings.avatar_persona_original_images_enabled,
        );

    state
        .host
        .observability
        .apply_llm_api_log_retention(settings.dev.llm_api_keep);

    if agent_retention_settings_updated {
        state
            .services
            .agent_run_retention_automation_service
            .notify_settings_changed();
    }

    Ok(settings)
}

pub async fn save_user_settings(
    settings: UserSettingsDto,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command("save_user_settings");

    state
        .services
        .settings_service
        .save_user_settings(settings)
        .await
        .map_err(map_command_error("Failed to save user settings"))
}

pub async fn save_user_settings_patch(
    patch: UserSettingsPatchDto,
    state: Arc<AppState>,
) -> Result<UserSettingsSaveResultDto, CommandError> {
    log_command("save_user_settings_patch");

    state
        .services
        .settings_service
        .save_user_settings_patch(patch)
        .await
        .map_err(map_command_error("Failed to save user settings patch"))
}

pub async fn get_sillytavern_settings(
    state: Arc<AppState>,
) -> Result<SillyTavernSettingsResponseDto, CommandError> {
    log_command("get_sillytavern_settings");

    state
        .services
        .settings_service
        .get_sillytavern_settings()
        .await
        .map_err(map_command_error("Failed to get SillyTavern settings"))
}

pub async fn create_settings_snapshot(
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command("create_settings_snapshot");

    state
        .services
        .settings_service
        .create_snapshot()
        .await
        .map_err(map_command_error("Failed to create settings snapshot"))
}

pub async fn get_settings_snapshots(
    state: Arc<AppState>,
) -> Result<Vec<SettingsSnapshotDto>, CommandError> {
    log_command("get_settings_snapshots");

    state
        .services
        .settings_service
        .get_snapshots()
        .await
        .map_err(map_command_error("Failed to get settings snapshots"))
}

pub async fn load_settings_snapshot(
    name: String,
    state: Arc<AppState>,
) -> Result<UserSettingsDto, CommandError> {
    log_command(format!("load_settings_snapshot - {}", name));

    state
        .services
        .settings_service
        .load_snapshot(&name)
        .await
        .map_err(map_command_error("Failed to load settings snapshot"))
}

pub async fn restore_settings_snapshot(
    name: String,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command(format!("restore_settings_snapshot - {}", name));

    state
        .services
        .settings_service
        .restore_snapshot(&name)
        .await
        .map_err(map_command_error("Failed to restore settings snapshot"))
}

fn has_agent_retention_settings_update(dto: &UpdateRustTavernSettingsDto) -> bool {
    dto.agent
        .as_ref()
        .and_then(|agent| agent.retention.as_ref())
        .is_some()
}
