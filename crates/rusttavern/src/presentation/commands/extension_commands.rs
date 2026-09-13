use std::sync::Arc;

use crate::app::AppState;
use crate::presentation::commands::helpers::{
    ensure_ios_policy_allows, log_command, map_command_error,
};
use crate::presentation::errors::CommandError;
use tt_domain::ios_policy::IosPolicyScope;
use tt_domain::models::extension::{
    Extension, ExtensionBranch, ExtensionInstallResult, ExtensionUpdateResult, ExtensionVersion,
};

pub async fn get_extensions(
    state: Arc<AppState>,
) -> Result<Vec<Extension>, CommandError> {
    log_command("get_extensions");

    let mut extensions = state
        .services
        .extension_service
        .get_extensions()
        .await
        .map_err(map_command_error("Failed to get extensions"))?;

    if state.ios_policy.scope == IosPolicyScope::Ios {
        let allow_third_party = state
            .ios_policy
            .capabilities
            .extensions
            .third_party_execution;
        let system_allowlist = &state
            .ios_policy
            .capabilities
            .extensions
            .system_allowlist;

        let before = extensions.len();
        extensions.retain(|extension| match extension.extension_type {
            tt_domain::models::extension::ExtensionType::System => {
                system_allowlist.allows(&extension.name)
            }
            tt_domain::models::extension::ExtensionType::Local
            | tt_domain::models::extension::ExtensionType::Global => allow_third_party,
        });

        let filtered = before.saturating_sub(extensions.len());
        if filtered > 0 {
            tracing::info!(
                filtered,
                before,
                profile = ?state.ios_policy.profile,
                "iOS policy filtered extensions during discovery"
            );
        }
    }

    Ok(extensions)
}

pub async fn install_extension(
    url: String,
    global: bool,
    branch: Option<String>,
    state: Arc<AppState>,
) -> Result<ExtensionInstallResult, CommandError> {
    log_command("install_extension");

    ensure_ios_policy_allows(
        &state.ios_policy,
        state
            .ios_policy
            .capabilities
            .extensions
            .third_party_management,
        "extensions.third_party_management",
    )?;

    state
        .services
        .extension_service
        .install_extension(&url, global, branch)
        .await
        .map_err(map_command_error("Failed to install extension"))
}

pub async fn update_extension(
    extension_name: String,
    global: bool,
    state: Arc<AppState>,
) -> Result<ExtensionUpdateResult, CommandError> {
    log_command(format!("update_extension {}", extension_name));

    ensure_ios_policy_allows(
        &state.ios_policy,
        state
            .ios_policy
            .capabilities
            .extensions
            .third_party_management,
        "extensions.third_party_management",
    )?;

    state
        .services
        .extension_service
        .update_extension(&extension_name, global)
        .await
        .map_err(map_command_error("Failed to update extension"))
}

pub async fn delete_extension(
    extension_name: String,
    global: bool,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command(format!("delete_extension {}", extension_name));

    ensure_ios_policy_allows(
        &state.ios_policy,
        state
            .ios_policy
            .capabilities
            .extensions
            .third_party_management,
        "extensions.third_party_management",
    )?;

    state
        .services
        .extension_service
        .delete_extension(&extension_name, global)
        .await
        .map_err(map_command_error("Failed to delete extension"))
}

pub async fn get_extension_version(
    extension_name: String,
    global: bool,
    state: Arc<AppState>,
) -> Result<ExtensionVersion, CommandError> {
    log_command(format!("get_extension_version {}", extension_name));

    ensure_ios_policy_allows(
        &state.ios_policy,
        state
            .ios_policy
            .capabilities
            .extensions
            .third_party_management,
        "extensions.third_party_management",
    )?;

    state
        .services
        .extension_service
        .get_extension_version(&extension_name, global)
        .await
        .map_err(map_command_error("Failed to get extension version"))
}

pub async fn get_extension_branches(
    extension_name: String,
    global: bool,
    state: Arc<AppState>,
) -> Result<Vec<ExtensionBranch>, CommandError> {
    log_command(format!("get_extension_branches {}", extension_name));

    ensure_ios_policy_allows(
        &state.ios_policy,
        state
            .ios_policy
            .capabilities
            .extensions
            .third_party_management,
        "extensions.third_party_management",
    )?;

    state
        .services
        .extension_service
        .get_extension_branches(&extension_name, global)
        .await
        .map_err(map_command_error("Failed to get extension branches"))
}

pub async fn switch_extension_branch(
    extension_name: String,
    branch: String,
    global: bool,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command(format!(
        "switch_extension_branch {} to {}",
        extension_name, branch
    ));

    ensure_ios_policy_allows(
        &state.ios_policy,
        state
            .ios_policy
            .capabilities
            .extensions
            .third_party_management,
        "extensions.third_party_management",
    )?;

    state
        .services
        .extension_service
        .switch_extension_branch(&extension_name, &branch, global)
        .await
        .map_err(map_command_error("Failed to switch extension branch"))
}

pub async fn move_extension(
    extension_name: String,
    source: String,
    destination: String,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command(format!(
        "move_extension {} from {} to {}",
        extension_name, source, destination
    ));

    ensure_ios_policy_allows(
        &state.ios_policy,
        state
            .ios_policy
            .capabilities
            .extensions
            .third_party_management,
        "extensions.third_party_management",
    )?;

    state
        .services
        .extension_service
        .move_extension(&extension_name, &source, &destination)
        .await
        .map_err(map_command_error("Failed to move extension"))
}
