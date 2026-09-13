//! Host handles: the subset of host services commands reach through
//! `AppState::host`. Kept out of `AppServices` because these are host-bound
//! infrastructure (events, observability, resources), not application
//! services.

use std::path::PathBuf;
use std::sync::Arc;

use crate::app::backend_errors::BackendErrorHub;
use crate::app::dev_observability::DevObservabilityHub;
use crate::app::BackendReadiness;
use crate::server::fs_resources::FsResourceRegistry;
use crate::server::stream::StreamRegistry;
use tt_application::services::bundled_template_service::BundledTemplateService;
use tt_application::services::host_resource_service::HostResourceService;
use tt_application::services::runtime_paths_service::RuntimePathsService;
use tt_application::services::user_media_service::UserMediaService;

/// Host-bound handles shared with presentation commands.
pub struct HostHandles {
    pub backend_errors: Arc<BackendErrorHub>,
    pub backend_readiness: Arc<BackendReadiness>,
    pub observability: Arc<DevObservabilityHub>,
    pub host_resources: Arc<HostResourceService>,
    pub user_media: Arc<UserMediaService>,
    pub runtime_paths: Arc<RuntimePathsService>,
    pub templates: Arc<BundledTemplateService>,
    pub streams: Arc<StreamRegistry>,
    /// Canonical data root; used to validate server-side file access
    /// (`plugin:fs|*`, `GET /__tt/file`).
    pub data_root: PathBuf,
    /// Data-archive import staging root (`<app_root>/.data-archive/imports`).
    /// The archive-import command additionally accepts archives staged here
    /// (see `prepare_data_archive_import_target_path`).
    pub archive_imports_root: PathBuf,
    /// rid-based file resources for the `plugin:fs|open/read` compat.
    pub fs_resources: Arc<FsResourceRegistry>,
}
