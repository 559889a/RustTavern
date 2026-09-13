use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex;
use ttsync_core::crypto::random_base64url;
use uuid::Uuid;

use crate::json_file::{read_json_file, write_json_file};
use tt_domain::errors::DomainError;
use tt_domain::models::tt_sync::{TtSyncIdentity, TtSyncPairedServer};

pub struct TtSyncStore {
    tt_sync_dir: PathBuf,
    /// Serializes read-modify-write of `paired-servers.json` and first-run identity creation.
    ///
    /// Sync jobs write a paired server's `last_sync_ms`/permissions at the same time the user can
    /// pair or unpair one; without this, one of the two records is silently lost (an unpair is
    /// reverted, or a new server disappears). Mirrors `LanPeerStore::paired_devices_lock`.
    state_lock: Arc<Mutex<()>>,
}

impl TtSyncStore {
    pub fn new(default_user_dir: PathBuf) -> Self {
        Self {
            tt_sync_dir: default_user_dir
                .join("user")
                .join("lan-sync")
                .join("tt-sync-v2"),
            state_lock: Arc::new(Mutex::new(())),
        }
    }

    fn identity_path(&self) -> PathBuf {
        self.tt_sync_dir.join("identity.json")
    }

    fn paired_servers_path(&self) -> PathBuf {
        self.tt_sync_dir.join("paired-servers.json")
    }

    pub async fn load_or_create_identity(&self) -> Result<TtSyncIdentity, DomainError> {
        let path = self.identity_path();
        if fs_is_file(&path).await {
            return read_json_file(&path).await;
        }

        // Two concurrent first-run callers must not each generate an identity: the loser would
        // return a seed that is not on disk, and a pairing completed with it would be rejected by
        // the peer from then on.
        let _guard = self.state_lock.lock().await;
        if fs_is_file(&path).await {
            return read_json_file(&path).await;
        }

        let identity = TtSyncIdentity {
            device_id: ttsync_contract::peer::DeviceId::new(Uuid::new_v4().to_string())
                .expect("generated uuid must be valid"),
            device_name: "RustTavern".to_string(),
            ed25519_seed: random_base64url(32),
        };
        write_json_file(&path, &identity).await?;
        Ok(identity)
    }

    pub async fn load_paired_servers(&self) -> Result<Vec<TtSyncPairedServer>, DomainError> {
        let _guard = self.state_lock.lock().await;
        self.load_paired_servers_unlocked().await
    }

    async fn load_paired_servers_unlocked(&self) -> Result<Vec<TtSyncPairedServer>, DomainError> {
        let path = self.paired_servers_path();
        if !fs_is_file(&path).await {
            return Ok(Vec::new());
        }
        read_json_file(&path).await
    }

    async fn save_paired_servers_unlocked(
        &self,
        servers: &[TtSyncPairedServer],
    ) -> Result<(), DomainError> {
        write_json_file(&self.paired_servers_path(), servers).await
    }

    pub async fn upsert_paired_server(
        &self,
        server: TtSyncPairedServer,
    ) -> Result<(), DomainError> {
        let _guard = self.state_lock.lock().await;
        let mut servers = self.load_paired_servers_unlocked().await?;

        if let Some(existing) = servers
            .iter_mut()
            .find(|item| item.server_device_id == server.server_device_id)
        {
            *existing = server;
        } else {
            servers.push(server);
        }

        self.save_paired_servers_unlocked(&servers).await
    }

    pub async fn remove_paired_server(
        &self,
        server_device_id: &ttsync_contract::peer::DeviceId,
    ) -> Result<(), DomainError> {
        let _guard = self.state_lock.lock().await;
        let servers = self.load_paired_servers_unlocked().await?;
        let filtered = servers
            .into_iter()
            .filter(|server| &server.server_device_id != server_device_id)
            .collect::<Vec<_>>();

        self.save_paired_servers_unlocked(&filtered).await
    }
}

async fn fs_is_file(path: &std::path::Path) -> bool {
    tokio::fs::metadata(path)
        .await
        .is_ok_and(|metadata| metadata.is_file())
}
