mod adapters;
mod repositories;
mod services;

use std::path::Path;
use std::sync::Arc;

use tt_adapter_storage_core::file_system::DataDirectory;
use tt_domain::errors::DomainError;

use crate::app::context::AppContext;

use super::{AppServices, StartupProfile};

pub(super) async fn initialize_data_directory(
    data_root: &Path,
) -> Result<DataDirectory, DomainError> {
    let data_directory = DataDirectory::new(data_root.to_path_buf());
    data_directory.initialize().await?;
    Ok(data_directory)
}

pub(super) async fn build_services(
    app_context: &Arc<AppContext>,
    data_directory: &DataDirectory,
    startup_profile: &StartupProfile,
) -> Result<AppServices, DomainError> {
    services::build(app_context, data_directory, startup_profile).await
}
