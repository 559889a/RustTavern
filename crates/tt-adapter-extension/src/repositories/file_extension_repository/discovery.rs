use std::fs;
use std::path::{Path, PathBuf};

use tt_domain::errors::DomainError;
use tt_domain::models::extension::{Extension, ExtensionType};

use super::FileExtensionRepository;
use super::git_remote::open_embedded;
use super::git_worktree::{has_standard_embedded_git, read_managed_state};
use super::source_store::ExtensionStoreScope;

pub(super) async fn discover_extensions(
    repository: &FileExtensionRepository,
) -> Result<Vec<Extension>, DomainError> {
    tracing::info!("Discovering extensions");

    let mut extensions = Vec::new();
    for &name in super::ENABLED_SYSTEM_EXTENSIONS {
        extensions.push(Extension {
            name: name.to_string(),
            extension_type: ExtensionType::System,
            managed: true,
            path: PathBuf::from(format!("scripts/extensions/{}", name)),
            remote_url: None,
            commit_hash: None,
            branch_name: None,
            is_up_to_date: None,
        });
    }

    discover_scoped_extensions(repository, ExtensionStoreScope::Local, &mut extensions).await?;
    discover_scoped_extensions(repository, ExtensionStoreScope::Global, &mut extensions).await?;

    tracing::debug!("Discovered {} extensions", extensions.len());
    Ok(extensions)
}

async fn discover_scoped_extensions(
    repository: &FileExtensionRepository,
    scope: ExtensionStoreScope,
    extensions: &mut Vec<Extension>,
) -> Result<(), DomainError> {
    let extensions_dir = repository.extension_dir_for_scope(scope).to_path_buf();

    // The whole scan is synchronous (`read_dir` plus, per extension, `canonicalize` × 4 and gix
    // config/ref parsing). Running it directly on a 2-worker tokio runtime made `get_extensions`
    // occupy half the server for its duration, so keep it on the blocking pool.
    let scanned = tokio::task::spawn_blocking(move || scan_extension_directory(&extensions_dir))
        .await
        .map_err(|error| {
            DomainError::InternalError(format!("Extension discovery task failed: {error}"))
        })??;

    for entry in scanned {
        let extension_name = format!("third-party/{}", entry.folder_name);
        if scope == ExtensionStoreScope::Global
            && extensions
                .iter()
                .any(|extension| extension.name == extension_name)
        {
            continue;
        }

        let (managed, remote_url, commit_hash, branch_name) = match entry.projection {
            Some(projection) => projection,
            None => match repository
                .source_store
                .read(scope, &entry.folder_name, &entry.path)
                .await
            {
                Ok(Some(source)) => (
                    true,
                    Some(source.remote_url),
                    Some(source.installed_commit),
                    Some(source.reference),
                ),
                Ok(None) => (false, None, None, None),
                Err(error) => {
                    tracing::warn!(
                        "Ignoring invalid source state for '{}' at '{}': {}",
                        entry.folder_name,
                        entry.path.display(),
                        error
                    );
                    (false, None, None, None)
                }
            },
        };

        extensions.push(Extension {
            name: extension_name,
            extension_type: match scope {
                ExtensionStoreScope::Local => ExtensionType::Local,
                ExtensionStoreScope::Global => ExtensionType::Global,
            },
            managed,
            path: entry.path,
            remote_url,
            commit_hash,
            branch_name,
            is_up_to_date: None,
        });
    }

    Ok(())
}

struct ScannedExtension {
    folder_name: String,
    path: PathBuf,
    /// `None` when the directory has no standard embedded Git repository, or when projecting it
    /// failed (already logged); the caller then falls back to stored source metadata.
    projection: Option<GitProjection>,
}

fn scan_extension_directory(extensions_dir: &Path) -> Result<Vec<ScannedExtension>, DomainError> {
    if !extensions_dir.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(extensions_dir).map_err(|error| {
        DomainError::InternalError(format!(
            "Failed to read extensions directory '{}': {}",
            extensions_dir.display(),
            error
        ))
    })?;

    let mut scanned = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            DomainError::InternalError(format!(
                "Failed to read extension directory entry in '{}': {}",
                extensions_dir.display(),
                error
            ))
        })?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(file_name) = path.file_name() else {
            continue;
        };
        let folder_name = file_name.to_string_lossy().to_string();
        if folder_name.starts_with('.') {
            continue;
        }

        let projection = match git_projection(&path) {
            Ok(projection) => projection,
            Err(error) => {
                tracing::warn!(
                    "Failed to project embedded Git state for '{}' at '{}': {}",
                    folder_name,
                    path.display(),
                    error
                );
                None
            }
        };

        scanned.push(ScannedExtension {
            folder_name,
            path,
            projection,
        });
    }

    Ok(scanned)
}

type GitProjection = (bool, Option<String>, Option<String>, Option<String>);

fn git_projection(path: &Path) -> Result<Option<GitProjection>, DomainError> {
    if !has_standard_embedded_git(path)? {
        return Ok(None);
    }
    let repo = open_embedded(path)?;
    let state = read_managed_state(&repo)?;
    Ok(Some((
        true,
        Some(state.remote_url),
        Some(state.deployed.to_string()),
        Some(state.selected.display_name().to_string()),
    )))
}
