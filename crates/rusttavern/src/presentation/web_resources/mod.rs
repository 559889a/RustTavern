//! Browser-visible resource services for the HTTP server.
//!
//! Replaces the old `app/host/resources.rs` Tauri managed-state installation:
//! host resources, user media, and bundled templates are constructed from
//! runtime paths and published through the server context instead of
//! `app.manage()`.

use std::sync::Arc;

use crate::app::StartupProfile;
use crate::infrastructure::assets::ResourceRoots;
use crate::infrastructure::bundled_resources::BundledResourceStore;
use crate::infrastructure::paths::RuntimePaths;
use crate::server::config::ServerConfig;
use tt_adapter_media::{FilesystemHostResourceStore, FilesystemUserMediaStore};
use tt_application::services::bundled_template_service::BundledTemplateService;
use tt_application::services::host_resource_service::HostResourceService;
use tt_application::services::user_media_service::UserMediaService;
use tt_domain::errors::DomainError;

/// Resource services that used to live on Tauri managed state.
pub struct WebResourceServices {
    pub host_resources: Arc<HostResourceService>,
    pub user_media_service: Arc<UserMediaService>,
    pub bundled_templates: Arc<BundledTemplateService>,
    pub resources: Arc<ResourceRoots>,
}

impl WebResourceServices {
    pub fn install(
        runtime_paths: &RuntimePaths,
        config: &ServerConfig,
        startup_profile: &StartupProfile,
    ) -> Result<Self, DomainError> {
        let resources = Arc::new(ResourceRoots::new(config.resources_root.clone()));

        let host_resource_store = Arc::new(FilesystemHostResourceStore::from_data_root(
            &runtime_paths.data_root,
        ));
        let host_resource_service = Arc::new(HostResourceService::new(
            startup_profile
                .rusttavern_settings
                .avatar_persona_original_images_enabled,
            host_resource_store,
        ));

        let user_media_store = Arc::new(FilesystemUserMediaStore::from_data_root(
            &runtime_paths.data_root,
        ));
        let user_media_service = Arc::new(UserMediaService::new(user_media_store));

        let bundled_templates = Arc::new(BundledTemplateService::new(Arc::new(
            BundledResourceStore::new((*resources).clone()),
        )));

        Ok(Self {
            host_resources: host_resource_service,
            user_media_service,
            bundled_templates,
            resources,
        })
    }
}
