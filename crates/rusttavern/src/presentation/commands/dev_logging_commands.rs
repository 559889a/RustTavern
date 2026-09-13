use std::sync::Arc;

use crate::app::AppState;
use crate::presentation::commands::bridge::{VersionInfo, get_client_version};
use crate::presentation::commands::helpers::log_command;
use crate::presentation::errors::CommandError;
use tt_application::dto::dev_observability_dto::{
    BackendLogEntryDto, DevBundleVersionDto, FrontendLogEntryDto, FrontendLogEntrySnapshotDto,
    LlmApiLogIndexEntryDto, LlmApiLogPreviewDto, LlmApiLogRawDto,
};

pub async fn devlog_append_frontend_logs(
    entries: Vec<FrontendLogEntryDto>,
) -> Result<(), CommandError> {
    log_command("devlog_append_frontend_logs");

    for entry in entries {
        let normalized_level = entry.level.trim().to_ascii_lowercase();
        let message = match entry.target.as_deref() {
            Some(target) => format!("[{target}] {}", entry.message),
            None => entry.message,
        };
        match normalized_level.as_str() {
            "debug" => tracing::debug!(target: "frontend", "{message}"),
            "warn" | "warning" => tracing::warn!(target: "frontend", "{message}"),
            "error" => tracing::error!(target: "frontend", "{message}"),
            _ => tracing::info!(target: "frontend", "{message}"),
        }
    }

    Ok(())
}

pub async fn devlog_set_backend_log_stream_enabled(
    enabled: bool,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command("devlog_set_backend_log_stream_enabled");
    state.host.observability.set_backend_stream_enabled(enabled);
    Ok(())
}

pub async fn devlog_get_backend_log_tail(
    limit: Option<u32>,
    state: Arc<AppState>,
) -> Result<Vec<BackendLogEntryDto>, CommandError> {
    log_command("devlog_get_backend_log_tail");

    let limit = limit.unwrap_or(800) as usize;
    Ok(state.host.observability.tail_backend_logs(limit))
}

pub async fn devlog_set_llm_api_log_stream_enabled(
    enabled: bool,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command("devlog_set_llm_api_log_stream_enabled");
    state.host.observability.set_llm_api_stream_enabled(enabled);

    Ok(())
}

pub async fn devlog_get_llm_api_log_index(
    limit: Option<u32>,
    state: Arc<AppState>,
) -> Result<Vec<LlmApiLogIndexEntryDto>, CommandError> {
    log_command("devlog_get_llm_api_log_index");
    let limit = limit.unwrap_or(50).max(1) as usize;
    Ok(state.host.observability.tail_llm_api_index(limit))
}

pub async fn devlog_get_llm_api_log_preview(
    id: u64,
    state: Arc<AppState>,
) -> Result<LlmApiLogPreviewDto, CommandError> {
    log_command(format!("devlog_get_llm_api_log_preview {}", id));

    state
        .host
        .observability
        .get_llm_api_preview(id)
        .await
        .map_err(CommandError::from)
}

pub async fn devlog_get_llm_api_log_raw(
    id: u64,
    state: Arc<AppState>,
) -> Result<LlmApiLogRawDto, CommandError> {
    log_command(format!("devlog_get_llm_api_log_raw {}", id));

    state
        .host
        .observability
        .get_llm_api_raw(id)
        .await
        .map_err(CommandError::from)
}

pub async fn devlog_export_bundle(
    frontend_entries: Vec<FrontendLogEntrySnapshotDto>,
    state: Arc<AppState>,
) -> Result<String, CommandError> {
    log_command("devlog_export_bundle");

    let output_path = state
        .host
        .observability
        .export_bundle(
            frontend_entries,
            dev_bundle_version_dto(get_client_version()?),
        )
        .await?;

    Ok(output_path.to_string_lossy().to_string())
}

fn dev_bundle_version_dto(version: VersionInfo) -> DevBundleVersionDto {
    DevBundleVersionDto {
        agent: version.agent,
        pkg_version: version.pkg_version,
        product_version: version.product_version,
        git_revision: version.git_revision,
        git_branch: version.git_branch,
    }
}
