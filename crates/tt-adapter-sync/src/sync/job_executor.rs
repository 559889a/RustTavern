use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use ttsync_client::{
    ClientSyncEngine, ClientSyncFailure, ClientSyncOptions, ClientSyncReport, ClientSyncTarget,
    LocalChangeSummary,
};
use ttsync_contract::peer::DeviceId;
use ttsync_contract::sync::SyncMode;

use crate::sync::http_client::{new_sync_client, sync_error_to_domain};
use crate::sync::lan::client::request_peer_pull as request_lan_peer_pull;
use crate::sync::lan::store::LanPeerStore;
use crate::sync::observer::{SyncJobProgressObserver, SyncProgressClock};
use crate::sync::workspace::RustTavernSyncWorkspace;
use crate::sync_transfer;
use crate::tt_sync::runtime::TtSyncRuntime;
use tt_contracts::sync::{
    LocalAppliedChangeSummary, ResolvedSyncPolicy, SyncEndpointRef, SyncExecutionFailure,
    SyncExecutionKind, SyncExecutionReport, SyncJob, SyncJobSummary, SyncOperationOptions,
};
use tt_domain::errors::DomainError;
use tt_ports::sync::{SyncJobEventPublisher, SyncJobExecutor};

/// A transfer that reports no progress for this long is treated as failed.
///
/// The sync HTTP client exposes no request timeout, so a half-open connection (sleeping peer,
/// dropped Wi-Fi) would otherwise leave the job awaiting a read forever while holding the shared
/// local-mutation permit — blocking every later sync and every extension install until restart.
/// The window is generous because a large single file legitimately reports progress only on
/// completion, and the clock also ticks on every transferred chunk.
const SYNC_STALL_TIMEOUT: Duration = Duration::from_secs(120);

/// Run a sync transfer, failing it if it stops making observable progress.
async fn with_stall_guard<F, T>(
    clock: Arc<SyncProgressClock>,
    endpoint_label: &str,
    transfer: F,
) -> Result<T, SyncExecutionFailure>
where
    F: Future<Output = T>,
{
    let watchdog = async {
        loop {
            let idle = clock.idle_for();
            if idle >= SYNC_STALL_TIMEOUT {
                return;
            }
            // Sleep exactly until the clock would reach the timeout instead of re-reading it on a
            // fixed short tick; a touch during the sleep simply shortens the next wait.
            tokio::time::sleep(SYNC_STALL_TIMEOUT - idle).await;
        }
    };

    tokio::select! {
        biased;
        result = transfer => Ok(result),
        () = watchdog => Err(SyncExecutionFailure::new(
            DomainError::Transient(format!(
                "{endpoint_label} stopped responding: no sync progress for {} seconds",
                SYNC_STALL_TIMEOUT.as_secs()
            )),
            // The engine's own counters are gone with the dropped future, so report the
            // conservative "target may have changed" shape rather than claiming a clean abort.
            LocalAppliedChangeSummary {
                files_written: 0,
                bytes_written: 0,
                files_deleted: 0,
                target_changed: true,
            },
        )),
    }
}

pub struct InfrastructureSyncJobExecutor {
    lan_sync_root: std::path::PathBuf,
    events: Arc<dyn SyncJobEventPublisher>,
    lan_peer_store: LanPeerStore,
    tt_runtime: Arc<TtSyncRuntime>,
    product_user_agent: String,
}

impl InfrastructureSyncJobExecutor {
    pub fn new(
        lan_sync_root: std::path::PathBuf,
        events: Arc<dyn SyncJobEventPublisher>,
        lan_peer_store: LanPeerStore,
        tt_runtime: Arc<TtSyncRuntime>,
        product_user_agent: impl Into<String>,
    ) -> Self {
        let product_user_agent = product_user_agent.into();
        assert!(
            !product_user_agent.trim().is_empty(),
            "sync product user agent must not be empty"
        );

        Self {
            lan_sync_root,
            events,
            lan_peer_store,
            tt_runtime,
            product_user_agent,
        }
    }

    async fn execute_lan_pull(
        &self,
        job: SyncJob,
        device_id: DeviceId,
        mode: SyncMode,
        options: SyncOperationOptions,
    ) -> Result<SyncExecutionReport, SyncExecutionFailure> {
        let peer = self.lan_peer_store.get_paired_device(&device_id).await?;
        let identity = self.lan_peer_store.load_or_create_identity().await?;
        let client = new_sync_client(
            peer.base_url.clone(),
            peer.spki_sha256.clone(),
            &self.product_user_agent,
        )?;
        let observer = SyncJobProgressObserver::new(self.events.clone(), job.context());
        let clock = observer.clock();
        let workspace = Arc::new(RustTavernSyncWorkspace::new(
            self.lan_sync_root.clone(),
            Some(Arc::clone(&clock)),
        ));
        let engine = ClientSyncEngine::new(
            client,
            workspace,
            ClientSyncTarget {
                device_id: identity.device_id,
                ed25519_seed_b64url: identity.ed25519_seed,
            },
            "LAN Sync peer",
        );
        let result = with_stall_guard(clock, "LAN Sync peer", async {
            engine
                .pull(
                    client_options(mode, options, sync_transfer::default_transfer_concurrency()),
                    &observer,
                )
                .await
        })
        .await?;

        match result {
            Ok(report) => {
                let local_applied = execution_local_applied(&report);
                let permissions = report.granted_permissions;
                let last_sync_ms = sync_transfer::now_ms();
                self.lan_peer_store
                    .update_paired_device(&device_id, |peer| {
                        peer.grant.permissions = permissions;
                        peer.grant.last_sync_ms = Some(last_sync_ms);
                    })
                    .await
                    .map_err(|error| SyncExecutionFailure::new(error, local_applied))?;
                Ok(execution_report(report))
            }
            Err(failure) => {
                if let Some(permissions) = failure.granted_permissions {
                    // Never let a bookkeeping write replace the real transfer failure.
                    if let Err(error) = self
                        .lan_peer_store
                        .update_paired_device(&device_id, |peer| {
                            peer.grant.permissions = permissions;
                        })
                        .await
                    {
                        tracing::warn!(
                            "Failed to persist LAN Sync peer permissions after a transfer failure: {error}"
                        );
                    }
                }
                Err(execution_failure(failure))
            }
        }
    }

    async fn execute_tt_pull(
        &self,
        job: SyncJob,
        server_device_id: DeviceId,
        mode: SyncMode,
        options: SyncOperationOptions,
    ) -> Result<SyncExecutionReport, SyncExecutionFailure> {
        let mut server = self.tt_runtime.get_paired_server(&server_device_id).await?;
        let identity = self.tt_runtime.store.load_or_create_identity().await?;
        let client = new_sync_client(
            server.base_url.clone(),
            server.spki_sha256.clone(),
            &self.product_user_agent,
        )?;
        let observer = SyncJobProgressObserver::new(self.events.clone(), job.context());
        let clock = observer.clock();
        let workspace = Arc::new(RustTavernSyncWorkspace::new(
            self.tt_runtime.sync_root.clone(),
            Some(Arc::clone(&clock)),
        ));
        let engine = ClientSyncEngine::new(
            client,
            workspace,
            ClientSyncTarget {
                device_id: identity.device_id,
                ed25519_seed_b64url: identity.ed25519_seed,
            },
            "TT-Sync server",
        );
        let result = with_stall_guard(clock, "TT-Sync server", async {
            engine
                .pull(
                    client_options(mode, options, tt_sync_transfer_concurrency()),
                    &observer,
                )
                .await
        })
        .await?;

        match result {
            Ok(report) => {
                let local_applied = execution_local_applied(&report);
                server.permissions = report.granted_permissions;
                server.last_sync_ms = Some(sync_transfer::now_ms());
                self.tt_runtime
                    .upsert_paired_server(server)
                    .await
                    .map_err(|error| SyncExecutionFailure::new(error, local_applied))?;
                Ok(execution_report(report))
            }
            Err(failure) => {
                if let Some(permissions) = failure.granted_permissions {
                    server.permissions = permissions;
                    // Never let a bookkeeping write replace the real transfer failure.
                    if let Err(error) = self.tt_runtime.upsert_paired_server(server).await {
                        tracing::warn!(
                            "Failed to persist TT-Sync server permissions after a transfer failure: {error}"
                        );
                    }
                }
                Err(execution_failure(failure))
            }
        }
    }

    async fn execute_tt_push(
        &self,
        job: SyncJob,
        server_device_id: DeviceId,
        mode: SyncMode,
        options: SyncOperationOptions,
    ) -> Result<SyncExecutionReport, SyncExecutionFailure> {
        let mut server = self.tt_runtime.get_paired_server(&server_device_id).await?;
        let identity = self.tt_runtime.store.load_or_create_identity().await?;
        let client = new_sync_client(
            server.base_url.clone(),
            server.spki_sha256.clone(),
            &self.product_user_agent,
        )?;
        let observer = SyncJobProgressObserver::new(self.events.clone(), job.context());
        let clock = observer.clock();
        let workspace = Arc::new(RustTavernSyncWorkspace::new(
            self.tt_runtime.sync_root.clone(),
            Some(Arc::clone(&clock)),
        ));
        let engine = ClientSyncEngine::new(
            client,
            workspace,
            ClientSyncTarget {
                device_id: identity.device_id,
                ed25519_seed_b64url: identity.ed25519_seed,
            },
            "TT-Sync server",
        );
        let result = with_stall_guard(clock, "TT-Sync server", async {
            engine
                .direct_push(
                    client_options(mode, options, tt_sync_transfer_concurrency()),
                    &observer,
                )
                .await
        })
        .await?;

        match result {
            Ok(report) => {
                let local_applied = execution_local_applied(&report);
                server.permissions = report.granted_permissions;
                server.last_sync_ms = Some(sync_transfer::now_ms());
                self.tt_runtime
                    .upsert_paired_server(server)
                    .await
                    .map_err(|error| SyncExecutionFailure::new(error, local_applied))?;
                Ok(execution_report(report))
            }
            Err(failure) => {
                if let Some(permissions) = failure.granted_permissions {
                    server.permissions = permissions;
                    // Never let a bookkeeping write replace the real transfer failure.
                    if let Err(error) = self.tt_runtime.upsert_paired_server(server).await {
                        tracing::warn!(
                            "Failed to persist TT-Sync server permissions after a transfer failure: {error}"
                        );
                    }
                }
                Err(execution_failure(failure))
            }
        }
    }
}

#[async_trait]
impl SyncJobExecutor for InfrastructureSyncJobExecutor {
    async fn execute(&self, job: SyncJob) -> Result<SyncExecutionReport, SyncExecutionFailure> {
        match (&job.endpoint, job.execution, &job.policy) {
            (
                SyncEndpointRef::LanPeer { device_id },
                SyncExecutionKind::Pull,
                ResolvedSyncPolicy::Transfer { mode, options },
            ) => {
                self.execute_lan_pull(job.clone(), device_id.clone(), *mode, options.clone())
                    .await
            }
            (
                SyncEndpointRef::LanPeer { device_id },
                SyncExecutionKind::RequestRemotePull,
                ResolvedSyncPolicy::RemotePullRequest { options },
            ) => {
                request_lan_peer_pull(
                    self.lan_peer_store.clone(),
                    device_id,
                    options.clone(),
                    &self.product_user_agent,
                )
                .await?;
                Ok(SyncExecutionReport::remote_request_accepted())
            }
            (
                SyncEndpointRef::RemoteServer { server_device_id },
                SyncExecutionKind::Pull,
                ResolvedSyncPolicy::Transfer { mode, options },
            ) => {
                self.execute_tt_pull(
                    job.clone(),
                    server_device_id.clone(),
                    *mode,
                    options.clone(),
                )
                .await
            }
            (
                SyncEndpointRef::RemoteServer { server_device_id },
                SyncExecutionKind::DirectPush,
                ResolvedSyncPolicy::Transfer { mode, options },
            ) => {
                self.execute_tt_push(
                    job.clone(),
                    server_device_id.clone(),
                    *mode,
                    options.clone(),
                )
                .await
            }
            _ => Err(SyncExecutionFailure::without_local_mutation(
                DomainError::InvalidData(
                    "Sync job endpoint does not support the requested execution".to_string(),
                ),
            )),
        }
    }
}

fn client_options(
    mode: SyncMode,
    options: SyncOperationOptions,
    file_concurrency: usize,
) -> ClientSyncOptions {
    let mut client_options = ClientSyncOptions::new(mode, options.selection);
    client_options.overwrite_policy = options.overwrite_policy;
    client_options.require_bundle_zstd = options.require_bundle_zstd;
    client_options.file_concurrency = file_concurrency;
    client_options
}

fn tt_sync_transfer_concurrency() -> usize {
    if cfg!(any(target_os = "android", target_os = "ios")) {
        8
    } else {
        16
    }
}

fn execution_report(report: ClientSyncReport) -> SyncExecutionReport {
    SyncExecutionReport::completed(
        SyncJobSummary::new(
            report.summary.files_total,
            report.summary.bytes_total,
            report.summary.files_deleted,
        ),
        local_applied(report.local_applied, false),
    )
}

fn execution_failure(failure: ClientSyncFailure) -> SyncExecutionFailure {
    let local_applied = failure_local_applied(&failure);
    SyncExecutionFailure::new(sync_error_to_domain(failure.error), local_applied)
}

fn execution_local_applied(report: &ClientSyncReport) -> LocalAppliedChangeSummary {
    local_applied(report.local_applied, false)
}

fn failure_local_applied(failure: &ClientSyncFailure) -> LocalAppliedChangeSummary {
    local_applied(failure.local_applied, failure.local_target_changed)
}

fn local_applied(summary: LocalChangeSummary, target_changed: bool) -> LocalAppliedChangeSummary {
    LocalAppliedChangeSummary {
        files_written: summary.files_written,
        bytes_written: summary.bytes_written,
        files_deleted: summary.files_deleted,
        target_changed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use ttsync_core::error::SyncError;

    #[test]
    fn failure_summary_preserves_target_changed_with_counted_changes() {
        let failure = ClientSyncFailure {
            error: SyncError::Io("rename failed".to_string()),
            local_applied: LocalChangeSummary {
                files_written: 1,
                bytes_written: 7,
                files_deleted: 0,
            },
            local_target_changed: true,
            remote_maybe_changed: false,
            granted_permissions: None,
        };

        let summary = failure_local_applied(&failure);

        assert_eq!(summary.files_written, 1);
        assert_eq!(summary.bytes_written, 7);
        assert!(summary.target_changed);
        assert!(summary.changed());
    }

    #[test]
    fn failure_summary_ignores_remote_only_changes() {
        let failure = ClientSyncFailure {
            error: SyncError::Io("upload failed".to_string()),
            local_applied: LocalChangeSummary::default(),
            local_target_changed: false,
            remote_maybe_changed: true,
            granted_permissions: None,
        };

        let summary = failure_local_applied(&failure);

        assert_eq!(summary.files_written, 0);
        assert!(!summary.target_changed);
        assert!(!summary.changed());
    }
}
