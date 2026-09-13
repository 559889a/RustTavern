use std::error::Error;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const DATA_ARCHIVE_ROOT_DIR: &str = ".data-archive";
const DATA_ARCHIVE_IMPORTS_DIR: &str = "imports";
const DATA_ARCHIVE_EXPORTS_DIR: &str = "exports";
const RUNTIME_CONFIG_FILE: &str = "rusttavern-runtime.json";
const EFFECTIVELY_EMPTY_DIRECTORY_ENTRIES: &[&str] = &[
    ".ds_store",
    ".localized",
    "desktop.ini",
    "thumbs.db",
    "icon\r",
];

/// Server runtime paths. `app_root` is the executable directory; `data_root`
/// is resolved from CLI/config (default `<app_root>/data`).
#[derive(Debug, Clone)]
pub struct RuntimePaths {
    pub app_root: PathBuf,
    pub data_root: PathBuf,
    pub log_root: PathBuf,
    pub archive_imports_root: PathBuf,
    pub archive_exports_root: PathBuf,
}

impl RuntimePaths {
    fn new(app_root: PathBuf) -> Self {
        let data_root = app_root.join("data");
        let log_root = app_root.join("logs");
        let archive_root = app_root.join(DATA_ARCHIVE_ROOT_DIR);
        let archive_imports_root = archive_root.join(DATA_ARCHIVE_IMPORTS_DIR);
        let archive_exports_root = archive_root.join(DATA_ARCHIVE_EXPORTS_DIR);

        Self {
            app_root,
            data_root,
            log_root,
            archive_imports_root,
            archive_exports_root,
        }
    }

    /// Server-mode constructor: app root is the executable directory; the
    /// caller may override `data_root` afterwards (CLI/config resolution).
    pub fn new_for_server(app_root: PathBuf) -> Self {
        Self::new(app_root)
    }
}

pub fn ensure_startup_paths(paths: &RuntimePaths) -> Result<(), Box<dyn Error>> {
    std::fs::create_dir_all(&paths.data_root)?;
    std::fs::create_dir_all(&paths.log_root)?;
    Ok(())
}

pub(crate) fn is_effectively_empty_directory(path: &Path) -> Result<bool, std::io::Error> {
    Ok(collect_ignorable_effectively_empty_entries(path)?.is_some())
}

fn collect_ignorable_effectively_empty_entries(
    path: &Path,
) -> Result<Option<Vec<PathBuf>>, std::io::Error> {
    let mut ignorable_entries = Vec::new();

    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        if is_ignorable_effectively_empty_entry(&entry)? {
            ignorable_entries.push(entry.path());
            continue;
        }

        return Ok(None);
    }

    Ok(Some(ignorable_entries))
}

fn is_ignorable_effectively_empty_entry(entry: &std::fs::DirEntry) -> Result<bool, std::io::Error> {
    if !entry.file_type()?.is_file() {
        return Ok(false);
    }

    let normalized = entry
        .file_name()
        .to_string_lossy()
        .trim()
        .to_ascii_lowercase();
    Ok(EFFECTIVELY_EMPTY_DIRECTORY_ENTRIES.contains(&normalized.as_str()))
}

pub(crate) async fn request_runtime_data_root_change(
    app_root: &Path,
    current_data_root: &Path,
    raw_target: &str,
) -> Result<(), tt_domain::errors::DomainError> {
    let raw = raw_target.trim();
    if raw.is_empty() {
        return Err(tt_domain::errors::DomainError::InvalidData(
            "data_root is required".to_string(),
        ));
    }

    let target = PathBuf::from(raw);
    if !target.is_absolute() {
        return Err(tt_domain::errors::DomainError::InvalidData(
            "data_root must be an absolute path".to_string(),
        ));
    }

    if !target.is_dir() {
        return Err(tt_domain::errors::DomainError::InvalidData(format!(
            "data_root must be an existing directory: {}",
            target.display()
        )));
    }

    if !is_effectively_empty_directory(&target).map_err(|error| {
        tt_domain::errors::DomainError::InternalError(format!(
            "Failed to inspect data_root: {error}"
        ))
    })? {
        return Err(tt_domain::errors::DomainError::InvalidData(format!(
            "data_root must be an empty directory: {}",
            target.display()
        )));
    }

    let canonical_target = dunce::canonicalize(&target).map_err(|error| {
        tt_domain::errors::DomainError::InternalError(format!(
            "Failed to canonicalize path: {error}"
        ))
    })?;
    let canonical_current = dunce::canonicalize(current_data_root).map_err(|error| {
        tt_domain::errors::DomainError::InternalError(format!(
            "Failed to canonicalize current data root: {error}"
        ))
    })?;

    if canonical_target == canonical_current {
        return Err(tt_domain::errors::DomainError::InvalidData(
            "data_root is already the current data directory".to_string(),
        ));
    }

    if canonical_target.starts_with(&canonical_current) {
        return Err(tt_domain::errors::DomainError::InvalidData(
            "data_root cannot be inside the current data directory".to_string(),
        ));
    }

    let config = RustTavernRuntimeConfig {
        version: RUSTTAVERN_RUNTIME_CONFIG_VERSION,
        data_root: canonical_target,
    };

    tt_adapter_storage_core::file_system::write_json_file(&runtime_config_path(app_root), &config)
        .await
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RustTavernRuntimeConfig {
    #[serde(default = "runtime_config_version")]
    pub version: u32,
    pub data_root: PathBuf,
}

fn runtime_config_version() -> u32 {
    RUSTTAVERN_RUNTIME_CONFIG_VERSION
}

pub const RUSTTAVERN_RUNTIME_CONFIG_VERSION: u32 = 1;

pub fn runtime_config_path(app_root: impl AsRef<std::path::Path>) -> PathBuf {
    app_root.as_ref().join(RUNTIME_CONFIG_FILE)
}

pub(crate) fn load_runtime_config(
    app_root: &std::path::Path,
) -> Result<Option<RustTavernRuntimeConfig>, Box<dyn Error>> {
    let path = runtime_config_path(app_root);
    if !path.is_file() {
        return Ok(None);
    }

    let raw = std::fs::read_to_string(&path)?;
    let mut config: RustTavernRuntimeConfig = serde_json::from_str(&raw)?;
    config.data_root = dunce::simplified(&config.data_root).to_path_buf();

    if config.version != runtime_config_version() {
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "Unsupported runtime config version {}, expected {}",
                config.version,
                runtime_config_version()
            ),
        )));
    }

    if !config.data_root.is_absolute() {
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Runtime config data_root must be an absolute path",
        )));
    }

    Ok(Some(config))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use uuid::Uuid;

    fn is_initialized_data_root(path: &Path) -> bool {
        path.join("default-user").is_dir()
    }

    struct TempDirGuard {
        root: PathBuf,
    }

    impl TempDirGuard {
        fn new(prefix: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "rusttavern-runtime-paths-{}-{}",
                prefix,
                Uuid::new_v4()
            ));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).expect("create temp root");
            Self { root }
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn effectively_empty_directory_ignores_known_metadata_files() {
        let temp = TempDirGuard::new("effectively-empty");

        fs::write(temp.root.join("desktop.ini"), "").expect("write desktop.ini");
        fs::write(temp.root.join(".DS_Store"), "").expect("write .DS_Store");

        assert!(
            is_effectively_empty_directory(&temp.root).expect("inspect temp root"),
            "expected metadata-only directory to be treated as empty"
        );

        fs::write(temp.root.join("actual.txt"), "content").expect("write actual file");
        assert!(
            !is_effectively_empty_directory(&temp.root).expect("inspect non-empty directory"),
            "expected non-metadata file to make directory non-empty"
        );
    }

    #[tokio::test]
    async fn request_runtime_data_root_change_writes_pending_config() {
        let temp = TempDirGuard::new("request-change");
        let app_root = temp.root.join("app");
        let current = temp.root.join("current");
        let target = temp.root.join("target");
        fs::create_dir_all(&app_root).expect("create app root");
        fs::create_dir_all(&current).expect("create current root");
        fs::create_dir_all(&target).expect("create target root");

        request_runtime_data_root_change(&app_root, &current, &format!("  {}  ", target.display()))
            .await
            .expect("request data root change");

        let persisted = load_runtime_config(&app_root)
            .expect("load runtime config")
            .expect("runtime config should exist");
        assert_eq!(
            persisted.data_root,
            dunce::canonicalize(&target).expect("canonical target")
        );
    }

    #[tokio::test]
    async fn request_runtime_data_root_change_rejects_current_child() {
        let temp = TempDirGuard::new("request-change-child");
        let app_root = temp.root.join("app");
        let current = temp.root.join("current");
        let target = current.join("child");
        fs::create_dir_all(&app_root).expect("create app root");
        fs::create_dir_all(&target).expect("create target root");

        let error =
            request_runtime_data_root_change(&app_root, &current, &target.to_string_lossy())
                .await
                .expect_err("expected current child to be rejected");

        assert!(matches!(
            error,
            tt_domain::errors::DomainError::InvalidData(message)
                if message == "data_root cannot be inside the current data directory"
        ));
        assert!(
            !runtime_config_path(&app_root).exists(),
            "rejected request must not write runtime config"
        );
    }

    #[test]
    fn is_initialized_data_root_detects_default_user() {
        let temp = TempDirGuard::new("initialized-root");
        assert!(!is_initialized_data_root(&temp.root));

        fs::create_dir_all(temp.root.join("default-user")).expect("create default user");
        assert!(is_initialized_data_root(&temp.root));
    }
}
