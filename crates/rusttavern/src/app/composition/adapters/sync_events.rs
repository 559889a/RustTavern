use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use tokio::sync::{Mutex, oneshot};
use ttsync_contract::sync::SyncMode;

use crate::app::context::AppContext;
use crate::app::events::EventBus;
use tt_application::services::lan_sync_service::LanSyncService;
use tt_application::services::tt_sync_service::TtSyncService;
use tt_contracts::sync::SyncJobEvent;
use tt_contracts::sync_automation::{
    SyncAutomationStatus, SyncAutomationTarget, SyncAutomationToastEvent,
};
use tt_domain::errors::DomainError;
use tt_domain::models::lan_sync::LanSyncPairRequestEvent;
use tt_ports::lan_sync::{LanPairingApprovalRequest, LanServerErrorReporter, PairingApproval};
use tt_ports::sync::SyncJobEventPublisher;
use tt_ports::sync_automation::{
    SyncAutomationEndpointCatalog, SyncAutomationEventPublisher, SyncAutomationLanServerControl,
};

pub(in crate::app::composition) fn sync_automation_events(
    app_context: &Arc<AppContext>,
) -> Arc<dyn SyncAutomationEventPublisher> {
    Arc::new(EventBusSyncAutomationEventPublisher {
        events: app_context.events.clone(),
    })
}

pub(in crate::app::composition) fn sync_job_events(
    app_context: &Arc<AppContext>,
) -> Arc<dyn SyncJobEventPublisher> {
    Arc::new(EventBusSyncJobEventPublisher {
        events: app_context.events.clone(),
    })
}

pub(in crate::app::composition) fn pairing_approval(
    app_context: &Arc<AppContext>,
) -> Arc<dyn PairingApproval> {
    Arc::new(EventBusPairingApproval::new(app_context.events.clone()))
}

pub(in crate::app::composition) fn lan_server_errors() -> Arc<dyn LanServerErrorReporter> {
    Arc::new(ServerLanServerErrorReporter)
}

pub(in crate::app::composition) fn sync_automation_lan_server(
    lan_sync_service: Arc<LanSyncService>,
    lan_sync_allowed: bool,
) -> Arc<dyn SyncAutomationLanServerControl> {
    Arc::new(ServiceSyncAutomationLanServerControl {
        lan_sync_service,
        lan_sync_allowed,
    })
}

pub(in crate::app::composition) fn sync_automation_endpoint_catalog(
    lan_sync_service: Arc<LanSyncService>,
    tt_sync_service: Arc<TtSyncService>,
    lan_sync_allowed: bool,
) -> Arc<dyn SyncAutomationEndpointCatalog> {
    Arc::new(ServiceSyncAutomationEndpointCatalog {
        lan_sync_service,
        tt_sync_service,
        lan_sync_allowed,
    })
}

struct EventBusSyncAutomationEventPublisher {
    events: Arc<EventBus>,
}

impl SyncAutomationEventPublisher for EventBusSyncAutomationEventPublisher {
    fn publish_status(&self, status: SyncAutomationStatus) {
        self.events.publish("sync_auto:status", status);
    }

    fn publish_toast(&self, event: SyncAutomationToastEvent) {
        self.events.publish("sync_auto:toast", event);
    }
}

struct EventBusSyncJobEventPublisher {
    events: Arc<EventBus>,
}

impl SyncJobEventPublisher for EventBusSyncJobEventPublisher {
    fn publish_sync_job(&self, event: SyncJobEvent) {
        self.events.publish("sync:job", event);
    }
}

struct ServerLanServerErrorReporter;

impl LanServerErrorReporter for ServerLanServerErrorReporter {
    fn report_lan_server_error(&self, message: String) {
        tracing::error!(
            target: crate::observability_targets::USER_VISIBLE_ERROR,
            "{message}"
        );
    }
}

struct EventBusPairingApproval {
    events: Arc<EventBus>,
    pending: Mutex<HashMap<String, oneshot::Sender<bool>>>,
}

impl EventBusPairingApproval {
    fn new(events: Arc<EventBus>) -> Self {
        Self {
            events,
            pending: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl PairingApproval for EventBusPairingApproval {
    async fn request(&self, request: LanPairingApprovalRequest) -> Result<bool, DomainError> {
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            pending.insert(request.request_id.clone(), tx);
        }

        self.events.publish(
            "lan_sync:pair_request",
            LanSyncPairRequestEvent {
                request_id: request.request_id.clone(),
                peer_device_id: request.peer_device_id,
                peer_device_name: request.peer_device_name,
                peer_ip: request.peer_ip,
            },
        );

        let timeout = Duration::from_millis(request.expires_at_ms.saturating_sub(now_ms()));
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(accepted)) => Ok(accepted),
            Ok(Err(_)) => Err(DomainError::cancelled("Pairing request cancelled")),
            Err(_) => {
                self.pending.lock().await.remove(&request.request_id);
                Err(DomainError::AuthenticationError(
                    "Pairing expired".to_string(),
                ))
            }
        }
    }

    async fn confirm(&self, request_id: &str, accept: bool) -> Result<(), DomainError> {
        let tx = self
            .pending
            .lock()
            .await
            .remove(request_id)
            .ok_or_else(|| {
                DomainError::NotFound(format!("Pair request not found: {}", request_id))
            })?;

        tx.send(accept).map_err(|_| {
            DomainError::InternalError("Pairing decision receiver dropped".to_string())
        })
    }

    async fn cancel_all(&self) {
        self.pending.lock().await.clear();
    }
}

struct ServiceSyncAutomationLanServerControl {
    lan_sync_service: Arc<LanSyncService>,
    lan_sync_allowed: bool,
}

#[async_trait]
impl SyncAutomationLanServerControl for ServiceSyncAutomationLanServerControl {
    fn validate_allowed(&self) -> Result<(), DomainError> {
        if !self.lan_sync_allowed {
            return Err(DomainError::InvalidData(
                "LAN Sync is not allowed by the current platform policy".to_string(),
            ));
        }
        Ok(())
    }

    async fn start(&self) -> Result<(), DomainError> {
        self.validate_allowed()?;
        self.lan_sync_service.start_server().await.map(|_| ())
    }

    async fn ensure_running(&self) -> Result<(), DomainError> {
        self.validate_allowed()?;
        if !self.lan_sync_service.get_status().await?.running {
            return Err(DomainError::InvalidData(
                "LAN Sync server is not running. Start the sync port before using LAN auto upload."
                    .to_string(),
            ));
        }
        Ok(())
    }
}

struct ServiceSyncAutomationEndpointCatalog {
    lan_sync_service: Arc<LanSyncService>,
    tt_sync_service: Arc<TtSyncService>,
    lan_sync_allowed: bool,
}

#[async_trait]
impl SyncAutomationEndpointCatalog for ServiceSyncAutomationEndpointCatalog {
    async fn validate_target(
        &self,
        target: &SyncAutomationTarget,
        mode: SyncMode,
    ) -> Result<(), DomainError> {
        match target {
            SyncAutomationTarget::Lan { device_id } => {
                if !self.lan_sync_allowed {
                    return Err(DomainError::InvalidData(
                        "LAN Sync is not allowed by the current platform policy".to_string(),
                    ));
                }

                let devices = self.lan_sync_service.list_paired_devices().await?;
                let device = devices
                    .iter()
                    .find(|device| device.device_id == *device_id)
                    .ok_or_else(|| {
                        DomainError::NotFound(format!("LAN Sync device not found: {device_id}"))
                    })?;
                if device.last_known_address.is_none() {
                    return Err(DomainError::InvalidData(
                        "LAN auto upload requires a paired LAN Sync device with an address"
                            .to_string(),
                    ));
                }
            }
            SyncAutomationTarget::Tt { server_device_id } => {
                let servers = self.tt_sync_service.list_servers().await?;
                let server = servers
                    .iter()
                    .find(|server| server.server_device_id.as_str() == server_device_id.as_str())
                    .ok_or_else(|| {
                        DomainError::NotFound(format!(
                            "TT-Sync server not found: {server_device_id}"
                        ))
                    })?;
                if !server.permissions.write {
                    return Err(DomainError::AuthenticationError(
                        "TT-Sync server does not grant write permission".to_string(),
                    ));
                }
                if mode == SyncMode::Mirror && !server.permissions.mirror_delete {
                    return Err(DomainError::AuthenticationError(
                        "TT-Sync server does not grant mirror_delete permission".to_string(),
                    ));
                }
            }
        }
        Ok(())
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
