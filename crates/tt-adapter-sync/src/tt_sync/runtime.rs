use std::collections::HashMap;
use std::path::PathBuf;

use tokio::sync::Mutex;

use ttsync_contract::peer::DeviceId;

use crate::tt_sync::store::TtSyncStore;
use tt_domain::errors::DomainError;
use tt_domain::models::tt_sync::{TtSyncIdentity, TtSyncPairedServer};
use tt_ports::sync::TtSyncRepository;

pub struct TtSyncRuntime {
    pub sync_root: PathBuf,
    pub store: TtSyncStore,
    paired_servers_cache: Mutex<Option<HashMap<String, TtSyncPairedServer>>>,
}

#[async_trait::async_trait]
impl TtSyncRepository for TtSyncRuntime {
    async fn load_or_create_identity(&self) -> Result<TtSyncIdentity, DomainError> {
        self.store.load_or_create_identity().await
    }

    async fn load_paired_servers(&self) -> Result<Vec<TtSyncPairedServer>, DomainError> {
        TtSyncRuntime::load_paired_servers(self).await
    }

    async fn upsert_paired_server(&self, server: TtSyncPairedServer) -> Result<(), DomainError> {
        TtSyncRuntime::upsert_paired_server(self, server).await
    }

    async fn remove_paired_server(&self, server_device_id: &DeviceId) -> Result<(), DomainError> {
        TtSyncRuntime::remove_paired_server(self, server_device_id).await
    }
}

impl TtSyncRuntime {
    pub fn new(sync_root: PathBuf, store_root: PathBuf) -> Self {
        Self {
            sync_root,
            store: TtSyncStore::new(store_root),
            paired_servers_cache: Mutex::new(None),
        }
    }

    pub async fn load_paired_servers(&self) -> Result<Vec<TtSyncPairedServer>, DomainError> {
        // Hold the cache lock across the disk read. Releasing it in between let a mutation land
        // (and find the cache still empty, so its own update was dropped) before the pre-write
        // snapshot got installed — permanently resurrecting an unpaired server or hiding a new one.
        let mut cache = self.paired_servers_cache.lock().await;
        if let Some(servers) = cache.as_ref() {
            return Ok(servers.values().cloned().collect());
        }

        let servers = self.store.load_paired_servers().await?;
        *cache = Some(
            servers
                .iter()
                .cloned()
                .map(|server| (server.server_device_id.to_string(), server))
                .collect(),
        );

        Ok(servers)
    }

    pub async fn get_paired_server(
        &self,
        server_device_id: &DeviceId,
    ) -> Result<TtSyncPairedServer, DomainError> {
        let mut cache = self.paired_servers_cache.lock().await;
        if let Some(map) = cache.as_ref() {
            return map.get(server_device_id.as_str()).cloned().ok_or_else(|| {
                DomainError::NotFound(format!(
                    "Paired TT-Sync server not found: {}",
                    server_device_id
                ))
            });
        }

        let map = self
            .store
            .load_paired_servers()
            .await?
            .into_iter()
            .map(|server| (server.server_device_id.to_string(), server))
            .collect::<HashMap<_, _>>();

        let result = map.get(server_device_id.as_str()).cloned().ok_or_else(|| {
            DomainError::NotFound(format!(
                "Paired TT-Sync server not found: {}",
                server_device_id
            ))
        });
        *cache = Some(map);
        result
    }

    pub async fn upsert_paired_server(
        &self,
        server: TtSyncPairedServer,
    ) -> Result<(), DomainError> {
        let mut cache = self.paired_servers_cache.lock().await;
        self.store.upsert_paired_server(server.clone()).await?;

        if let Some(map) = cache.as_mut() {
            map.insert(server.server_device_id.to_string(), server);
        }

        Ok(())
    }

    pub async fn remove_paired_server(
        &self,
        server_device_id: &DeviceId,
    ) -> Result<(), DomainError> {
        let mut cache = self.paired_servers_cache.lock().await;
        self.store.remove_paired_server(server_device_id).await?;

        if let Some(map) = cache.as_mut() {
            map.remove(server_device_id.as_str());
        }

        Ok(())
    }
}
