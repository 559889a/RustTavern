use serde::de::DeserializeOwned;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tokio::fs;

use tt_domain::errors::DomainError;

static DEFAULT_CONTENT_MANIFEST: OnceLock<Vec<String>> = OnceLock::new();

/// Resolves virtual resource paths (`default/...`, `frontend-templates/...`)
/// against a physical resources root.
///
/// Physical layout:
/// - `default/<path>` → `<root>/default/<path>`
/// - `frontend-templates/<path>` → `<root>/src/scripts/templates/<path>`
///
/// In release zips the root is the executable directory; in development it is
/// the repository root.
#[derive(Debug, Clone)]
pub struct ResourceRoots {
    root: PathBuf,
}

impl ResourceRoots {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn resolve(&self, relative_path: &str) -> PathBuf {
        let normalized = normalize_resource_relative_path(relative_path);
        if let Some(rest) = normalized.strip_prefix("frontend-templates/") {
            self.root.join("src/scripts/templates").join(rest)
        } else {
            self.root.join(normalized)
        }
    }

    pub fn read_text(&self, relative_path: &str) -> Result<String, DomainError> {
        let bytes = self.read_bytes(relative_path)?;
        String::from_utf8(bytes).map_err(|e| {
            tracing::error!("Failed to decode resource text {:?}: {}", relative_path, e);
            DomainError::InvalidData(format!(
                "Resource '{}' is not valid UTF-8: {}",
                relative_path, e
            ))
        })
    }

    pub fn read_bytes(&self, relative_path: &str) -> Result<Vec<u8>, DomainError> {
        let normalized = normalize_resource_relative_path(relative_path);
        let path = self.resolve(&normalized);
        match std::fs::read(&path) {
            Ok(bytes) => Ok(bytes),
            Err(error) => {
                if error.kind() == std::io::ErrorKind::NotFound {
                    tracing::debug!("Resource {:?} not found at {:?}", normalized, path);
                    return Err(DomainError::NotFound(format!(
                        "Resource not found: {}",
                        normalized
                    )));
                }

                tracing::error!(
                    "Failed to read resource bytes {:?} (resolved to {:?}): {}",
                    normalized,
                    path,
                    error
                );
                Err(DomainError::InternalError(format!(
                    "Failed to read resource '{}': {}",
                    normalized, error
                )))
            }
        }
    }

    pub fn read_json<T: DeserializeOwned>(&self, relative_path: &str) -> Result<T, DomainError> {
        let text = self.read_text(relative_path)?;
        serde_json::from_str(&text).map_err(|e| {
            tracing::error!("Failed to parse JSON resource {:?}: {}", relative_path, e);
            DomainError::InvalidData(format!("Invalid JSON resource '{}': {}", relative_path, e))
        })
    }

    pub async fn copy_resource_to_file(
        &self,
        resource_relative_path: &str,
        destination: &Path,
    ) -> Result<(), DomainError> {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                tracing::error!("Failed to create destination directory {:?}: {}", parent, e);
                DomainError::InternalError(format!("Failed to create directory: {}", e))
            })?;
        }

        // The store read is synchronous file IO; run it on the blocking pool
        // so an async caller never stalls a runtime worker.
        let source = self.clone();
        let path_for_task = resource_relative_path.to_string();
        let bytes = tokio::task::spawn_blocking(move || source.read_bytes(&path_for_task))
            .await
            .map_err(|error| {
                DomainError::InternalError(format!("Resource read task failed: {error}"))
            })??;
        fs::write(destination, bytes).await.map_err(|e| {
            tracing::error!(
                "Failed to write copied resource {:?} to {:?}: {}",
                resource_relative_path,
                destination,
                e
            );
            DomainError::InternalError(format!("Failed to write resource file: {}", e))
        })
    }
}

pub fn list_default_content_files_under(prefix: &str) -> Vec<String> {
    let normalized_prefix = prefix.trim_matches('/').replace('\\', "/");
    let query = if normalized_prefix.is_empty() {
        String::new()
    } else {
        format!("{}/", normalized_prefix)
    };

    let files = DEFAULT_CONTENT_MANIFEST.get_or_init(load_default_content_manifest);
    if query.is_empty() {
        return files.to_vec();
    }

    files
        .iter()
        .filter(|path| path.starts_with(&query))
        .cloned()
        .collect()
}

fn load_default_content_manifest() -> Vec<String> {
    let raw = include_str!(concat!(env!("OUT_DIR"), "/default_content_manifest.json"));
    match serde_json::from_str::<Vec<String>>(raw) {
        Ok(mut entries) => {
            entries.sort();
            entries
        }
        Err(error) => {
            tracing::error!(
                target: crate::observability_targets::USER_VISIBLE_ERROR,
                "Failed to load generated default content manifest: {}",
                error
            );
            Vec::new()
        }
    }
}

fn normalize_resource_relative_path(relative_path: &str) -> String {
    relative_path
        .trim()
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_string()
}
