use std::sync::Arc;

use crate::app::context::AppContext;
use crate::app::host_handles::HostHandles;
use crate::infrastructure::paths::RuntimePaths;
use crate::server::router::AppStateSlot;

pub mod backend_errors;
mod backend_readiness;
pub(crate) mod composition;
pub(crate) mod context;
#[cfg(test)]
mod contract_tests;
pub mod dev_observability;
pub(crate) mod events;
pub(crate) mod host_handles;
mod startup_profile;
mod state;

pub(crate) use backend_readiness::BackendReadiness;
pub(crate) use startup_profile::StartupProfile;
pub(crate) use state::AppServices;
pub use state::AppState;

pub fn spawn_initialization(
    app_context: Arc<AppContext>,
    host_handles: HostHandles,
    app_state_slot: Arc<AppStateSlot>,
    runtime_paths: RuntimePaths,
    startup_profile: StartupProfile,
    backend_readiness: Arc<BackendReadiness>,
) {
    tokio::spawn(async move {
        match AppState::new(
            app_context,
            runtime_paths,
            startup_profile,
            host_handles,
        )
        .await
        {
            Ok(state) => {
                let state = Arc::new(state);
                if let Err(error) = state
                    .services
                    .content_service
                    .initialize_default_content("default-user")
                    .await
                {
                    let message = format!("Failed to initialize default content: {error}");
                    backend_readiness.mark_failed(message.clone());
                    tracing::error!(
                        target: crate::observability_targets::USER_VISIBLE_ERROR,
                        "{message}",
                    );
                    app_state_slot.set(Err(message));
                    return;
                }
                tracing::debug!("Successfully initialized default content");

                app_state_slot.set(Ok(state.clone()));
                backend_readiness.mark_ready();

                state
                    .services
                    .settings_service
                    .schedule_chat_backup_reconciliation();

                let sync_automation_service = state.services.sync_automation_service.clone();
                let sync_automation_cancel = state.lifecycle.sync_automation_cancel.clone();
                tokio::spawn(async move {
                    sync_automation_service.run(sync_automation_cancel).await;
                });

                let agent_run_retention_automation_service = state
                    .services
                    .agent_run_retention_automation_service
                    .clone();
                let agent_run_retention_automation_cancel = state
                    .lifecycle
                    .agent_run_retention_automation_cancel
                    .clone();
                tokio::spawn(async move {
                    agent_run_retention_automation_service
                        .run(agent_run_retention_automation_cancel)
                        .await;
                });

                let chat_history_coordinator = state.services.chat_history_coordinator.clone();
                let chat_history_cancel = state.lifecycle.chat_history_cancel.clone();
                tokio::spawn(async move {
                    chat_history_coordinator.run(chat_history_cancel).await;
                });

                tracing::debug!("Application is ready");
            }
            Err(error) => {
                let message = format!("Failed to initialize application state: {}", error);
                backend_readiness.mark_failed(message.clone());
                tracing::error!(
                    target: crate::observability_targets::USER_VISIBLE_ERROR,
                    "{message}",
                );
                app_state_slot.set(Err(message));
            }
        }
    });
}
