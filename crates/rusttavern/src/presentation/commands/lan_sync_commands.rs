use std::sync::Arc;

use qrcode::QrCode;
use serde::Serialize;
use ttsync_contract::sync::{OverwritePolicy, SyncMode};

use crate::app::AppState;
use crate::presentation::commands::helpers::{
    ensure_ios_policy_allows, log_command, map_command_error,
};
use crate::presentation::errors::CommandError;
use tt_contracts::sync::{SyncJobReport, SyncOperationOptions};
use tt_domain::models::lan_sync::{LanSyncPairedDeviceSummary, LanSyncStatus};

fn ensure_lan_sync_allowed(state: &AppState) -> Result<(), CommandError> {
    ensure_ios_policy_allows(
        &state.ios_policy,
        state.ios_policy.capabilities.sync.lan,
        "sync.lan",
    )
}

pub async fn lan_sync_get_status(
    state: Arc<AppState>,
) -> Result<LanSyncStatus, CommandError> {
    log_command("lan_sync_get_status");
    ensure_lan_sync_allowed(&state)?;

    state
        .services
        .lan_sync_service
        .get_status()
        .await
        .map_err(map_command_error("Failed to get LAN sync status"))
}

pub async fn lan_sync_start_server(
    state: Arc<AppState>,
) -> Result<LanSyncStatus, CommandError> {
    log_command("lan_sync_start_server");
    ensure_lan_sync_allowed(&state)?;

    state
        .services
        .lan_sync_service
        .start_server()
        .await
        .map_err(map_command_error("Failed to start LAN sync server"))
}

pub async fn lan_sync_stop_server(state: Arc<AppState>) -> Result<(), CommandError> {
    log_command("lan_sync_stop_server");
    ensure_lan_sync_allowed(&state)?;

    state
        .services
        .lan_sync_service
        .stop_server()
        .await
        .map_err(map_command_error("Failed to stop LAN sync server"))
}

#[derive(Debug, Clone, Serialize)]
pub struct LanSyncPairingInfoDto {
    pub address: String,
    pub pair_uri: String,
    pub qr_svg: String,
    pub expires_at_ms: u64,
}

pub async fn lan_sync_enable_pairing(
    address: Option<String>,
    state: Arc<AppState>) -> Result<LanSyncPairingInfoDto, CommandError> {
    log_command("lan_sync_enable_pairing");
    ensure_lan_sync_allowed(&state)?;

    state
        .services
        .lan_sync_service
        .enable_pairing(address)
        .await
        .and_then(|info| {
            let qr_svg = generate_qr_svg(&info.pair_uri)?;
            Ok(LanSyncPairingInfoDto {
                address: info.address,
                pair_uri: info.pair_uri,
                qr_svg,
                expires_at_ms: info.expires_at_ms,
            })
        })
        .map_err(map_command_error("Failed to enable LAN sync pairing"))
}

pub async fn lan_sync_get_pairing_info(
    address: String,
    state: Arc<AppState>) -> Result<LanSyncPairingInfoDto, CommandError> {
    log_command("lan_sync_get_pairing_info");
    ensure_lan_sync_allowed(&state)?;

    state
        .services
        .lan_sync_service
        .get_pairing_info(&address)
        .await
        .and_then(|info| {
            let qr_svg = generate_qr_svg(&info.pair_uri)?;
            Ok(LanSyncPairingInfoDto {
                address: info.address,
                pair_uri: info.pair_uri,
                qr_svg,
                expires_at_ms: info.expires_at_ms,
            })
        })
        .map_err(map_command_error("Failed to get LAN sync pairing info"))
}

#[derive(Debug, Clone, Serialize)]
pub struct LanSyncPairedDeviceDto {
    pub device_id: String,
    pub device_name: String,
    pub last_known_address: Option<String>,
    pub paired_at_ms: u64,
    pub last_sync_ms: Option<u64>,
}

impl From<LanSyncPairedDeviceSummary> for LanSyncPairedDeviceDto {
    fn from(device: LanSyncPairedDeviceSummary) -> Self {
        Self {
            device_id: device.device_id,
            device_name: device.device_name,
            last_known_address: device.last_known_address,
            paired_at_ms: device.paired_at_ms,
            last_sync_ms: device.last_sync_ms,
        }
    }
}

pub async fn lan_sync_request_pairing(
    pair_uri: String,
    state: Arc<AppState>) -> Result<LanSyncPairedDeviceDto, CommandError> {
    log_command("lan_sync_request_pairing");
    ensure_lan_sync_allowed(&state)?;

    state
        .services
        .lan_sync_service
        .request_pairing(&pair_uri)
        .await
        .map(LanSyncPairedDeviceDto::from)
        .map_err(map_command_error("Failed to request LAN sync pairing"))
}

pub async fn lan_sync_confirm_pairing(
    request_id: String,
    accept: bool,
    state: Arc<AppState>) -> Result<(), CommandError> {
    log_command("lan_sync_confirm_pairing");
    ensure_lan_sync_allowed(&state)?;

    state
        .services
        .lan_sync_service
        .confirm_pairing(&request_id, accept)
        .await
        .map_err(map_command_error("Failed to confirm LAN sync pairing"))
}

pub async fn lan_sync_list_devices(
    state: Arc<AppState>,
) -> Result<Vec<LanSyncPairedDeviceDto>, CommandError> {
    log_command("lan_sync_list_devices");
    ensure_lan_sync_allowed(&state)?;

    state
        .services
        .lan_sync_service
        .list_paired_devices()
        .await
        .map(|devices| {
            devices
                .into_iter()
                .map(LanSyncPairedDeviceDto::from)
                .collect()
        })
        .map_err(map_command_error("Failed to list LAN sync devices"))
}

pub async fn lan_sync_remove_device(
    device_id: String,
    state: Arc<AppState>) -> Result<(), CommandError> {
    log_command("lan_sync_remove_device");
    ensure_lan_sync_allowed(&state)?;

    state
        .services
        .lan_sync_service
        .remove_paired_device(&device_id)
        .await
        .map_err(map_command_error("Failed to remove LAN sync device"))
}

pub async fn lan_sync_sync_from_device(
    device_id: String,
    options: SyncOperationOptions,
    state: Arc<AppState>) -> Result<SyncJobReport, CommandError> {
    log_command("lan_sync_sync_from_device");
    ensure_lan_sync_allowed(&state)?;

    state
        .services
        .lan_sync_service
        .sync_from_device(&device_id, options)
        .await
        .map_err(map_command_error("Failed to run LAN sync pull"))
}

pub async fn lan_sync_push_to_device(
    device_id: String,
    options: SyncOperationOptions,
    state: Arc<AppState>) -> Result<SyncJobReport, CommandError> {
    log_command("lan_sync_push_to_device");
    ensure_lan_sync_allowed(&state)?;

    state
        .services
        .lan_sync_service
        .push_to_device(&device_id, options)
        .await
        .map_err(map_command_error("Failed to request LAN sync push"))
}

pub async fn lan_sync_set_sync_mode(
    mode: SyncMode,
    persist: bool,
    state: Arc<AppState>) -> Result<(), CommandError> {
    log_command("lan_sync_set_sync_mode");
    ensure_lan_sync_allowed(&state)?;

    state
        .services
        .lan_sync_service
        .set_sync_mode(mode, persist)
        .await
        .map_err(map_command_error("Failed to set LAN sync mode"))
}

pub async fn lan_sync_clear_sync_mode_override(
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command("lan_sync_clear_sync_mode_override");
    ensure_lan_sync_allowed(&state)?;

    state
        .services
        .lan_sync_service
        .clear_sync_mode_override()
        .await;
    Ok(())
}

pub async fn lan_sync_set_overwrite_policy(
    overwrite_policy: OverwritePolicy,
    state: Arc<AppState>) -> Result<(), CommandError> {
    log_command("lan_sync_set_overwrite_policy");
    ensure_lan_sync_allowed(&state)?;

    state
        .services
        .lan_sync_service
        .set_overwrite_policy(overwrite_policy)
        .await
        .map_err(map_command_error("Failed to set sync overwrite policy"))
}

fn generate_qr_svg(text: &str) -> Result<String, tt_domain::errors::DomainError> {
    let code = QrCode::new(text.as_bytes())
        .map_err(|error| tt_domain::errors::DomainError::InternalError(error.to_string()))?;
    Ok(code
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(200, 200)
        .build())
}
