use std::path::Path;

use async_trait::async_trait;

use crate::infrastructure::paths::{load_runtime_config, request_runtime_data_root_change};
use tt_domain::errors::DomainError;
use tt_ports::runtime_paths::{RuntimePathConfigInfo, RuntimePathConfigStore};

pub(crate) struct FilesystemRuntimePathConfigStore;

#[async_trait]
impl RuntimePathConfigStore for FilesystemRuntimePathConfigStore {
    fn load_config(&self, app_root: &Path) -> Result<Option<RuntimePathConfigInfo>, DomainError> {
        load_runtime_config(app_root)
            .map(|config| {
                config.map(|config| RuntimePathConfigInfo {
                    data_root: config.data_root,
                    // Server mode has no pending data-root migration flow.
                    migration_pending: false,
                    migration_error: None,
                })
            })
            .map_err(|error| {
                DomainError::InternalError(format!(
                    "Failed to load rusttavern-runtime.json: {error}"
                ))
            })
    }

    async fn request_data_root_change(
        &self,
        app_root: &Path,
        current_data_root: &Path,
        raw_target: &str,
    ) -> Result<(), DomainError> {
        request_runtime_data_root_change(app_root, current_data_root, raw_target).await
    }
}
