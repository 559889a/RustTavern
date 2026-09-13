use serde::{Deserialize, Serialize};

use crate::presentation::commands::helpers::log_command;
use crate::presentation::errors::CommandError;

const SILLYTAVERN_COMPAT_VERSION: &str = "1.18.0";
use tt_domain::models::update::UpdateChannel;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub agent: String,
    #[serde(rename = "pkgVersion")]
    pub pkg_version: String,
    #[serde(rename = "productVersion")]
    pub product_version: String,
    #[serde(rename = "gitRevision")]
    pub git_revision: Option<String>,
    #[serde(rename = "gitBranch")]
    pub git_branch: Option<String>,
    #[serde(rename = "defaultUpdateChannel")]
    pub default_update_channel: UpdateChannel,
}

pub fn get_version() -> Result<String, CommandError> {
    Ok(crate::product::VERSION.to_string())
}

pub fn get_client_version() -> Result<VersionInfo, CommandError> {
    log_command("get_client_version");

    let version_info = VersionInfo {
        // Keep the upstream client-agent shape for extension compatibility checks.
        agent: format!("SillyTavern:{}:RustTavern", SILLYTAVERN_COMPAT_VERSION),
        // Most upstream extensions parse pkgVersion as the SillyTavern SemVer.
        // Keep it aligned with the embedded frontend baseline to preserve plugin behavior.
        pkg_version: SILLYTAVERN_COMPAT_VERSION.to_string(),
        product_version: crate::product::VERSION.to_string(),
        git_revision: crate::product::optional_build_value(crate::product::GIT_REVISION)
            .map(str::to_string),
        git_branch: crate::product::optional_build_value(crate::product::GIT_BRANCH)
            .map(str::to_string),
        default_update_channel: crate::product::default_update_channel(),
    };

    Ok(version_info)
}

pub fn is_ready() -> Result<bool, CommandError> {
    Ok(true)
}
