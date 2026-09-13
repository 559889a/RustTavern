use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, ReadBuf};
use ttsync_client::{ClientWorkspace, WorkspaceWriteError};
use ttsync_contract::manifest::ManifestV2;
use ttsync_contract::path::SyncPath;
use ttsync_core::dataset::ResolvedDatasetPolicy;
use ttsync_core::error::SyncError;

use crate::sync::http_client::domain_error_to_sync;
use crate::sync::observer::SyncProgressClock;
use crate::tt_sync::fs::scan_manifest_with_policy;
use crate::{sync_fs, sync_transfer};

#[derive(Debug)]
pub struct RustTavernSyncWorkspace {
    sync_root: PathBuf,
    /// Marked on every chunk of transferred payload so the executor can distinguish a slow transfer
    /// from a connection that has silently died (see `SyncProgressClock`).
    progress: Option<Arc<SyncProgressClock>>,
}

impl RustTavernSyncWorkspace {
    pub fn new(sync_root: PathBuf, progress: Option<Arc<SyncProgressClock>>) -> Self {
        Self {
            sync_root,
            progress,
        }
    }

    fn resolve(&self, path: &SyncPath) -> PathBuf {
        sync_transfer::resolve_to_local(&self.sync_root, path)
    }
}

/// Wraps a transfer stream so each delivered chunk marks the progress clock.
///
/// On pull the wrapped reader is the network response; on push it is the local file that the HTTP
/// body pulls from at network pace. Either way, a chunk means bytes are still moving.
struct ClockedReader<R> {
    inner: R,
    progress: Option<Arc<SyncProgressClock>>,
}

impl<R> ClockedReader<R> {
    fn new(inner: R, progress: Option<Arc<SyncProgressClock>>) -> Self {
        Self { inner, progress }
    }
}

impl<R> AsyncRead for ClockedReader<R>
where
    R: AsyncRead + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let before = buf.filled().len();
        let polled = Pin::new(&mut self.inner).poll_read(cx, buf);
        if let Poll::Ready(Ok(())) = &polled
            && buf.filled().len() > before
            && let Some(progress) = &self.progress
        {
            progress.touch();
        }
        polled
    }
}

impl ClientWorkspace for RustTavernSyncWorkspace {
    async fn scan(&self, policy: ResolvedDatasetPolicy) -> Result<ManifestV2, SyncError> {
        scan_manifest_with_policy(self.sync_root.clone(), policy)
            .await
            .map_err(domain_error_to_sync)
    }

    async fn read_file(
        &self,
        path: &SyncPath,
    ) -> Result<Box<dyn AsyncRead + Send + Unpin>, SyncError> {
        let file = tokio::fs::File::open(self.resolve(path))
            .await
            .map_err(|error| SyncError::Io(error.to_string()))?;
        Ok(Box::new(ClockedReader::new(file, self.progress.clone())))
    }

    async fn write_file(
        &self,
        path: &SyncPath,
        data: &mut (dyn AsyncRead + Send + Unpin),
        modified_ms: u64,
    ) -> Result<(), WorkspaceWriteError> {
        let mut data = ClockedReader::new(data, self.progress.clone());
        sync_fs::write_file_atomic(&self.resolve(path), &mut data, modified_ms)
            .await
            .map_err(workspace_write_error)
    }

    async fn delete_file(&self, path: &SyncPath) -> Result<(), WorkspaceWriteError> {
        if let Some(progress) = &self.progress {
            progress.touch();
        }
        sync_fs::delete_sync_file(&self.sync_root, path)
            .await
            .map_err(workspace_write_error)
    }
}

fn workspace_write_error(error: sync_fs::FileMutationError) -> WorkspaceWriteError {
    let target_changed = error.target_changed();
    let error = error.into_error();
    if target_changed {
        WorkspaceWriteError::changed(error)
    } else {
        WorkspaceWriteError::unchanged(error)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::path::PathBuf;

    use tokio::io::AsyncReadExt;
    use ttsync_client::ClientWorkspace;
    use ttsync_contract::path::SyncPath;
    use uuid::Uuid;

    use super::RustTavernSyncWorkspace;

    fn temp_root() -> PathBuf {
        std::env::temp_dir().join(format!("rusttavern-sync-workspace-{}", Uuid::new_v4()))
    }

    #[tokio::test]
    async fn workspace_round_trips_file_operations_and_prunes_empty_parents() {
        let root = temp_root();
        let workspace = RustTavernSyncWorkspace::new(root.clone(), None);
        let path = SyncPath::new("default-user/chats/thread/hello.json".to_string()).unwrap();
        let mut source = Cursor::new(br#"{"hello":true}"#.to_vec());

        workspace
            .write_file(&path, &mut source, 1_000)
            .await
            .unwrap();

        let mut reader = workspace.read_file(&path).await.unwrap();
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await.unwrap();
        assert_eq!(&bytes, br#"{"hello":true}"#);

        workspace.delete_file(&path).await.unwrap();
        assert!(!root.join("default-user/chats/thread").exists());
        assert!(root.join("default-user/chats").exists());

        let _ = tokio::fs::remove_dir_all(root).await;
    }
}
