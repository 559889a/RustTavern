//! Server configuration: CLI + config.yaml resolution.
//!
//! The server is configured through (in priority order):
//! 1. CLI flags (`--host`, `--port`, `--data-root`, `--config`, `--resources`)
//! 2. `config.yaml` (defaults to `<exe_dir>/config.yaml`, generated with a
//!    commented template when missing)
//! 3. Built-in defaults (127.0.0.1:8000, data root `<exe_dir>/data`)

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::infrastructure::paths::RuntimePaths;

pub const DEFAULT_HOST: &str = "127.0.0.1";
pub const DEFAULT_PORT: u16 = 8000;
pub const DEFAULT_CONFIG_FILE_NAME: &str = "config.yaml";

/// Fully resolved server configuration after merging CLI + config.yaml.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub data_root: PathBuf,
    pub auto_open_browser: bool,
    /// Directory that contains `default/` and `src/scripts/templates/`.
    pub resources_root: PathBuf,
    /// Directory that contains the frontend `index.html` (static web root).
    pub web_root: PathBuf,
    /// Security section: auth mode, credentials, IP whitelist.
    pub security: SecurityConfig,
}

/// Security section of config.yaml. Phase 4 fleshes this out with
/// authentication and IP whitelist enforcement.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityConfig {
    /// `none` (default) or `basic`.
    #[serde(default)]
    pub auth_mode: String,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    /// Peers allowed to connect: single IP, CIDR range, or trailing wildcard
    /// (`192.168.1.*`). Empty means allow all (subject to auth rules).
    #[serde(default)]
    pub whitelist: Vec<String>,
}

/// Raw config.yaml shape.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawConfig {
    #[serde(default)]
    pub listen: ListenConfig,
    /// Optional data root override. CLI `--data-root` wins over this.
    #[serde(default)]
    pub data_root: Option<PathBuf>,
    #[serde(default = "default_true")]
    pub auto_open_browser: bool,
    #[serde(default)]
    pub security: SecurityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListenConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

impl Default for ListenConfig {
    fn default() -> Self {
        Self {
            host: DEFAULT_HOST.to_string(),
            port: DEFAULT_PORT,
        }
    }
}

fn default_host() -> String {
    DEFAULT_HOST.to_string()
}

fn default_port() -> u16 {
    DEFAULT_PORT
}

fn default_true() -> bool {
    true
}

/// CLI overrides. `None` means "not provided on the command line".
#[derive(Debug, Clone, Default)]
pub struct CliOverrides {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub data_root: Option<PathBuf>,
    pub config_file: Option<PathBuf>,
    pub resources_root: Option<PathBuf>,
    pub no_open_browser: bool,
}

/// Resolve the final server configuration.
pub fn resolve_server_config(cli: &CliOverrides) -> Result<ServerConfig, String> {
    let exe_dir = resolve_executable_directory()?;

    let config_path = cli
        .config_file
        .clone()
        .unwrap_or_else(|| exe_dir.join(DEFAULT_CONFIG_FILE_NAME));

    let raw = load_or_generate_config(&config_path)?;

    let host = cli
        .host
        .clone()
        .or_else(|| non_empty(raw.listen.host.clone()))
        .unwrap_or_else(|| DEFAULT_HOST.to_string());
    let port = cli.port.unwrap_or(raw.listen.port);

    let data_root = cli
        .data_root
        .clone()
        .or(raw.data_root.clone())
        .or_else(|| load_runtime_config_data_root(&exe_dir))
        .unwrap_or_else(|| exe_dir.join("data"));
    // A relative `--data-root` would otherwise resolve against the process CWD at
    // every use site (and the CWD reported back to the frontend is not the one the
    // caller meant), so pin it to an absolute path here.
    let data_root = std::path::absolute(&data_root)
        .map_err(|error| format!("Failed to resolve data root {}: {error}", data_root.display()))?;

    let resources_root = cli
        .resources_root
        .clone()
        .unwrap_or_else(|| resolve_default_resource_root(&exe_dir));

    // `--resources` names the resource root (contains `default/` and `src/`),
    // so the static web root is its `src/` subtree. Without the flag both
    // resolve from the executable directory (release layout or dev checkout).
    let web_root = match &cli.resources_root {
        Some(root) => root.join("src"),
        None => resolve_default_web_root(&exe_dir),
    };

    let auto_open_browser = !cli.no_open_browser && raw.auto_open_browser;

    Ok(ServerConfig {
        host,
        port,
        data_root,
        auto_open_browser,
        resources_root,
        web_root,
        security: raw.security,
    })
}

pub fn resolve_executable_directory() -> Result<PathBuf, String> {
    let executable_path = std::env::current_exe().map_err(|error| {
        format!("Failed to resolve the executable directory: {error}")
    })?;
    executable_path.parent().map(Path::to_path_buf).ok_or_else(|| {
        "Failed to resolve the executable directory: missing parent".to_string()
    })
}

/// Load config.yaml, generating a commented template when it does not exist.
pub fn load_or_generate_config(config_path: &Path) -> Result<RawConfig, String> {
    if !config_path.is_file() {
        if let Some(parent) = config_path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "Failed to create config directory {}: {error}",
                    parent.display()
                )
            })?;
        }
        std::fs::write(config_path, config_template()).map_err(|error| {
            format!("Failed to write config template {}: {error}", config_path.display())
        })?;
        tracing::info!(
            "Generated default config file at {}",
            config_path.display()
        );
    }

    let raw = std::fs::read_to_string(config_path).map_err(|error| {
        format!("Failed to read config file {}: {error}", config_path.display())
    })?;

    serde_yaml::from_str(&raw).map_err(|error| {
        format!(
            "Failed to parse config file {}: {error}",
            config_path.display()
        )
    })
}

/// In server mode the legacy desktop runtime config (rusttavern-runtime.json)
/// is honored only when no explicit data root was provided, so existing
/// desktop installs can reuse their data directory unchanged.
fn load_runtime_config_data_root(exe_dir: &Path) -> Option<PathBuf> {
    crate::infrastructure::paths::load_runtime_config(exe_dir)
        .ok()
        .flatten()
        .map(|config| config.data_root)
}

/// Release layout: `<exe_dir>/default/` + `<exe_dir>/src/scripts/templates/`.
/// Dev fallback: repository root (cargo run from the workspace).
fn resolve_default_resource_root(exe_dir: &Path) -> PathBuf {
    if exe_dir.join("default").is_dir() && exe_dir.join("src").is_dir() {
        return exe_dir.to_path_buf();
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// Static web root. Release: `<exe_dir>/src`. Dev: repository `src/`.
fn resolve_default_web_root(exe_dir: &Path) -> PathBuf {
    if exe_dir.join("src").join("index.html").is_file() {
        return exe_dir.join("src");
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").join("src")
}

/// Resolve runtime paths from the merged config, ensuring required directories
/// exist. `--data-root` has priority over config.yaml (already applied).
pub fn resolve_runtime_paths(config: &ServerConfig) -> Result<RuntimePaths, String> {
    let exe_dir = resolve_executable_directory()?;
    let mut paths = RuntimePaths::new_for_server(exe_dir);
    paths.data_root = config.data_root.clone();
    crate::infrastructure::paths::ensure_startup_paths(&paths)
        .map_err(|error| format!("Failed to prepare runtime directories: {error}"))?;
    Ok(paths)
}

fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn config_template() -> String {
    [
        "# RustTavern server configuration.",
        "#",
        "# This file is generated on first start. Edit it and restart the server",
        "# for changes to take effect. Command-line flags override these values.",
        "#",
        "# Listen address and port.",
        "listen:",
        "  host: 127.0.0.1",
        "  port: 8000",
        "#",
        "# Optional data directory override. Defaults to <exe dir>/data.",
        "# Existing RustTavern data directories can be pointed to directly; the",
        "# data directory layout is unchanged.",
        "# dataRoot: C:/path/to/data",
        "#",
        "# Open the default browser after startup.",
        "autoOpenBrowser: true",
        "#",
        "# Security settings.",
        "security:",
        "  # none (default) or basic (HTTP Basic + HttpOnly session cookie).",
        "  #",
        "  # Binding to a non-loopback address (e.g. 0.0.0.0) needs at least one",
        "  # access control, otherwise startup is refused: either authMode: basic",
        "  # or a whitelist that names the addresses allowed to connect.",
        "  authMode: none",
        "  # username: admin",
        "  # password: change-me",
        "  #",
        "  # The password is stored in plain text here, matching the original",
        "  # SillyTavern basicAuth convention. Restrict file permissions on",
        "  # config.yaml (e.g. chmod 600) so other local users cannot read it.",
        "  #",
        "  # After a successful Basic login the server issues an HttpOnly",
        "  # session cookie (SameSite=Strict) so browsers do not re-prompt.",
        "  #",
        "  # Whitelist of peers allowed to connect: single IP, CIDR range, or",
        "  # trailing wildcard. Empty means allow all (subject to auth rules).",
        "  # whitelist:",
        "  #   - 127.0.0.1",
        "  #   - 192.168.1.*",
        "  #   - 192.168.1.0/24",
        "",
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_template_round_trips() {
        let parsed: RawConfig = serde_yaml::from_str(&config_template()).expect("template parses");
        assert_eq!(parsed.listen.host, DEFAULT_HOST);
        assert_eq!(parsed.listen.port, DEFAULT_PORT);
        assert!(parsed.auto_open_browser);
        assert_eq!(parsed.security.auth_mode, "none");
    }

    #[test]
    fn empty_config_uses_defaults() {
        let parsed: RawConfig = serde_yaml::from_str("").expect("empty config parses");
        assert_eq!(parsed.listen.host, DEFAULT_HOST);
        assert_eq!(parsed.listen.port, DEFAULT_PORT);
        assert!(parsed.auto_open_browser);
    }

    #[test]
    fn cli_overrides_win_over_config() {
        let config = RawConfig {
            listen: ListenConfig {
                host: "0.0.0.0".to_string(),
                port: 9000,
            },
            data_root: Some(PathBuf::from("/data/config")),
            auto_open_browser: true,
            security: SecurityConfig::default(),
        };

        let cli = CliOverrides {
            host: Some("127.0.0.1".to_string()),
            port: Some(8080),
            data_root: Some(PathBuf::from("/data/cli")),
            ..CliOverrides::default()
        };

        assert_eq!(cli.host.as_deref(), Some("127.0.0.1"));
        assert_eq!(cli.port, Some(8080));
        assert_eq!(cli.data_root.as_deref(), Some(PathBuf::from("/data/cli").as_path()));
        assert_eq!(config.listen.host, "0.0.0.0");
        assert_eq!(config.listen.port, 9000);
    }
}
