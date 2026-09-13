use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use serde::Serialize;
use crate::app::AppState;
use crate::presentation::commands::helpers::{
    ensure_ios_policy_allows, log_command, map_command_error,
};
use crate::presentation::errors::CommandError;
use tt_domain::models::skill::{
    DEFAULT_SKILL_READ_FALLBACK_MAX_CHARS, SkillFileRef, SkillImportInput, SkillImportPreview,
    SkillIndexEntry, SkillInstallRequest, SkillInstallResult, SkillMoveRequest, SkillReadRequest,
    SkillReadResult, SkillScope, SkillScopeFilter, SkillScopeRetargetRequest,
    SkillScopeRetargetResult, SkillWriteRequest,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillExportPayload {
    pub file_name: String,
    pub content_base64: String,
    pub sha256: String,
}

pub async fn download_skill_import_url(
    url: String,
    state: Arc<AppState>,
) -> Result<SkillImportInput, CommandError> {
    log_command("download_skill_import_url");

    ensure_ios_policy_allows(
        &state.ios_policy,
        state.ios_policy.capabilities.content.external_import,
        "content.external_import",
    )?;

    state
        .services
        .skill_service
        .download_import_url(&url)
        .await
        .map_err(map_command_error(
            "Failed to download Agent Skill import URL",
        ))
}

pub async fn list_skills(
    scope: Option<SkillScopeFilter>,
    state: Arc<AppState>,
) -> Result<Vec<SkillIndexEntry>, CommandError> {
    log_command("list_skills");

    state
        .services
        .skill_service
        .list_skills(scope.unwrap_or_default())
        .await
        .map_err(map_command_error("Failed to list Agent Skills"))
}

pub async fn list_skill_files(
    name: String,
    scope: Option<SkillScope>,
    state: Arc<AppState>,
) -> Result<Vec<SkillFileRef>, CommandError> {
    log_command(format!("list_skill_files {}", name));

    state
        .services
        .skill_service
        .list_skill_files(scope.unwrap_or_default(), &name)
        .await
        .map_err(map_command_error("Failed to list Agent Skill files"))
}

pub async fn preview_skill_import(
    input: SkillImportInput,
    target_scope: Option<SkillScope>,
    state: Arc<AppState>,
) -> Result<SkillImportPreview, CommandError> {
    log_command("preview_skill_import");

    state
        .services
        .skill_service
        .preview_import(input, target_scope.unwrap_or_default())
        .await
        .map_err(map_command_error("Failed to preview Agent Skill import"))
}

pub async fn install_skill_import(
    request: SkillInstallRequest,
    state: Arc<AppState>,
) -> Result<SkillInstallResult, CommandError> {
    log_command("install_skill_import");

    state
        .services
        .skill_service
        .install_import(request)
        .await
        .map_err(map_command_error("Failed to install Agent Skill"))
}

#[expect(
    clippy::too_many_arguments,
    reason = "Tauri command parameters intentionally preserve the flat invoke ABI"
)]
pub async fn read_skill_file(
    name: String,
    path: String,
    scope: Option<SkillScope>,
    max_chars: Option<usize>,
    start_line: Option<usize>,
    line_count: Option<usize>,
    start_char: Option<usize>,
    state: Arc<AppState>,
) -> Result<SkillReadResult, CommandError> {
    log_command(format!("read_skill_file {}/{}", name, path));

    let max_chars = match max_chars {
        Some(0) => {
            return Err(CommandError::BadRequest(
                "maxChars must be greater than 0".to_string(),
            ));
        }
        Some(value) if value > DEFAULT_SKILL_READ_FALLBACK_MAX_CHARS => {
            return Err(CommandError::BadRequest(format!(
                "maxChars must be <= {DEFAULT_SKILL_READ_FALLBACK_MAX_CHARS} for api.skill.readFile; Agent skill.read uses Agent Profile budgets"
            )));
        }
        Some(value) => Some(value),
        None => Some(DEFAULT_SKILL_READ_FALLBACK_MAX_CHARS),
    };

    state
        .services
        .skill_service
        .read_skill_file(SkillReadRequest {
            scope: scope.unwrap_or_default(),
            name,
            path,
            start_line,
            line_count,
            start_char,
            max_chars,
        })
        .await
        .map_err(map_command_error("Failed to read Agent Skill file"))
}

pub async fn write_skill_file(
    name: String,
    path: String,
    content: String,
    scope: Option<SkillScope>,
    expected_sha256: Option<String>,
    state: Arc<AppState>,
) -> Result<SkillReadResult, CommandError> {
    log_command(format!("write_skill_file {}/{}", name, path));

    state
        .services
        .skill_service
        .write_skill_file(SkillWriteRequest {
            scope: scope.unwrap_or_default(),
            name,
            path,
            content,
            expected_sha256,
        })
        .await
        .map_err(map_command_error("Failed to write Agent Skill file"))
}

pub async fn export_skill(
    name: String,
    scope: Option<SkillScope>,
    state: Arc<AppState>,
) -> Result<SkillExportPayload, CommandError> {
    log_command(format!("export_skill {}", name));

    let exported = state
        .services
        .skill_service
        .export_skill(scope.unwrap_or_default(), &name)
        .await
        .map_err(map_command_error("Failed to export Agent Skill"))?;

    Ok(SkillExportPayload {
        file_name: exported.file_name,
        content_base64: BASE64_STANDARD.encode(exported.bytes),
        sha256: exported.sha256,
    })
}

pub async fn delete_skill(
    name: String,
    scope: Option<SkillScope>,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command(format!("delete_skill {}", name));

    state
        .services
        .skill_service
        .delete_skill(scope.unwrap_or_default(), &name)
        .await
        .map_err(map_command_error("Failed to delete Agent Skill"))
}

pub async fn move_skill(
    request: SkillMoveRequest,
    state: Arc<AppState>,
) -> Result<SkillInstallResult, CommandError> {
    log_command(format!("move_skill {}", request.name));

    state
        .services
        .skill_service
        .move_skill(request)
        .await
        .map_err(map_command_error("Failed to move Agent Skill"))
}

pub async fn retarget_skill_scope(
    request: SkillScopeRetargetRequest,
    state: Arc<AppState>,
) -> Result<SkillScopeRetargetResult, CommandError> {
    log_command(format!(
        "retarget_skill_scope {} -> {}",
        request.from_scope.label(),
        request.to_scope.label()
    ));

    state
        .services
        .skill_service
        .retarget_scope(request)
        .await
        .map_err(map_command_error("Failed to retarget Agent Skill scope"))
}
