//! Application context: shared host services passed into composition.
//!
//! Replaces the old `AppHandle` + `app.manage()` state registry. Composition
//! and AppState construction read everything they need from this struct, and
//! commands reach the same services through `AppState::host`.

use std::sync::Arc;

use crate::app::events::EventBus;
use crate::infrastructure::assets::ResourceRoots;
use crate::infrastructure::logging::llm_api_logs::LlmApiLogStore;
use crate::infrastructure::paths::RuntimePaths;
use tt_adapter_http::HttpClientPool;

/// Everything the composition root needs that used to live on the Tauri
/// `AppHandle` managed state.
pub struct AppContext {
    pub events: Arc<EventBus>,
    pub http_client_pool: Arc<HttpClientPool>,
    pub llm_api_log_store: Arc<LlmApiLogStore>,
    pub resources: Arc<ResourceRoots>,
    pub runtime_paths: Arc<RuntimePaths>,
}
