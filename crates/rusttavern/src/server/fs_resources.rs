//! Server-side file resource registry (`plugin:fs|open/read` compat).
//!
//! The frontend `readable-file-stream-service` and `asset-io` (Android path)
//! call `plugin:fs|open` / `plugin:fs|read` / `plugin:resources|close` with
//! the Tauri resource-id (rid) semantics. This module implements the same
//! contract over plain HTTP JSON:
//!
//! - `open` validates the path against the data root (or the upload staging
//!   root) and returns a numeric rid.
//! - `read` returns a JSON number array: the bytes read followed by the same
//!   8-byte big-endian trailer the native bridge appends (bytes_read as u64),
//!   so the frontend `normalizeFsReadResponse` keeps working unchanged.
//! - `close` releases the rid.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};
use tokio::sync::Mutex;

use crate::presentation::errors::CommandError;

const UPLOAD_STAGING_ROOT_NAME: &str = "rusttavern-upload-staging";
/// Upper bound for a single `read` call. The frontend streams in 512 KiB
/// chunks, so anything larger is a protocol error rather than a real read.
const MAX_READ_LEN: usize = 1024 * 1024;
/// Idle timeout for open resources. Browser pages that vanish without
/// calling `close` (tab closed, navigation) would otherwise keep file
/// handles and table entries alive forever; entries idle for this long are
/// dropped lazily on the next `open`/`read`.
const RESOURCE_IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// Upper bound on concurrently open resources; when full, the least-recently
/// used entry is evicted on the next `open`.
const MAX_RESOURCES: usize = 256;

struct FileEntry {
    file: tokio::sync::Mutex<tokio::fs::File>,
    last_access: std::sync::Mutex<Instant>,
}

/// Global registry of open file resources.
pub struct FsResourceRegistry {
    resources: Mutex<HashMap<u64, Arc<FileEntry>>>,
    next_rid: AtomicU64,
}

/// Drop entries that have been idle past [`RESOURCE_IDLE_TIMEOUT`]. Call with
/// the registry lock already held; never awaits.
fn sweep_idle_locked(resources: &mut HashMap<u64, Arc<FileEntry>>) {
    let now = Instant::now();
    resources.retain(|_, entry| {
        *entry.last_access.lock().expect("entry poisoned") + RESOURCE_IDLE_TIMEOUT > now
    });
}

impl FsResourceRegistry {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            resources: Mutex::new(HashMap::new()),
            next_rid: AtomicU64::new(1),
        })
    }

    pub async fn open(&self, path: &Path, options: &FsOpenOptions) -> Result<u64, CommandError> {
        let mut file = tokio::fs::OpenOptions::new()
            .read(options.read)
            .write(options.write)
            .create(options.create)
            .append(options.append)
            .open(path)
            .await
            .map_err(|error| {
                CommandError::InternalServerError(format!(
                    "Failed to open {}: {}",
                    path.display(),
                    error
                ))
            })?;

        if options.append
            && let Err(error) = file.seek(SeekFrom::End(0)).await
        {
            return Err(CommandError::InternalServerError(format!(
                "Failed to seek {}: {}",
                path.display(),
                error
            )));
        }

        let rid = self.next_rid.fetch_add(1, Ordering::Relaxed);
        let entry = Arc::new(FileEntry {
            file: tokio::sync::Mutex::new(file),
            last_access: std::sync::Mutex::new(Instant::now()),
        });
        let mut resources = self.resources.lock().await;
        sweep_idle_locked(&mut resources);
        if resources.len() >= MAX_RESOURCES {
            // Evict the least-recently used entry to keep the table bounded.
            if let Some(oldest) = resources
                .iter()
                .min_by_key(|(_, entry)| *entry.last_access.lock().expect("entry poisoned"))
                .map(|(rid, _)| *rid)
            {
                resources.remove(&oldest);
            }
        }
        resources.insert(rid, entry);
        Ok(rid)
    }

    /// Read up to `len` bytes. Returns the bytes followed by the 8-byte
    /// big-endian trailer with the number of bytes actually read.
    pub async fn read(&self, rid: u64, len: usize) -> Result<Vec<u8>, CommandError> {
        if len > MAX_READ_LEN {
            return Err(CommandError::BadRequest(format!(
                "Read length {len} exceeds the {MAX_READ_LEN} byte limit"
            )));
        }
        // Take the entry pointer under the short registry lock, then read
        // outside it: concurrent reads of different resources no longer
        // serialize on one global lock.
        let entry = {
            let mut resources = self.resources.lock().await;
            sweep_idle_locked(&mut resources);
            resources.get(&rid).cloned().ok_or_else(|| {
                CommandError::BadRequest(format!("Unknown file resource id: {rid}"))
            })?
        };
        *entry.last_access.lock().expect("entry poisoned") = Instant::now();

        let mut file = entry.file.lock().await;
        let mut buffer = vec![0u8; len];
        let bytes_read = file.read(&mut buffer).await.map_err(|error| {
            CommandError::InternalServerError(format!("Failed to read file resource: {error}"))
        })?;
        buffer.truncate(bytes_read);

        // 8-byte big-endian trailer (u64), matching the native bridge contract.
        let mut trailer = [0u8; 8];
        let mut remaining = bytes_read as u64;
        for byte in trailer.iter_mut().rev() {
            *byte = (remaining & 0xff) as u8;
            remaining >>= 8;
        }
        buffer.extend_from_slice(&trailer);
        Ok(buffer)
    }

    pub async fn close(&self, rid: u64) -> Result<(), CommandError> {
        self.resources.lock().await.remove(&rid);
        Ok(())
    }
}

/// Open flags accepted by `plugin:fs|open`.
#[derive(Debug, Default, Clone, Copy)]
pub struct FsOpenOptions {
    pub read: bool,
    pub write: bool,
    pub create: bool,
    pub append: bool,
}

/// Parse the `options` argument of `plugin:fs|open` (loose: unknown fields
/// are ignored, mirroring the plugin's tolerant option handling).
pub fn parse_open_options(value: Option<&serde_json::Value>) -> FsOpenOptions {
    let mut options = FsOpenOptions::default();
    let Some(serde_json::Value::Object(map)) = value else {
        return options;
    };
    let flag = |key: &str| map.get(key).and_then(serde_json::Value::as_bool).unwrap_or(false);
    options.read = flag("read");
    options.write = flag("write");
    options.create = flag("create");
    options.append = flag("append");
    if !options.read && !options.write {
        // The frontend always opens with `{ read: true }`; default to read so
        // a missing flag does not produce an unreadable handle.
        options.read = true;
    }
    options
}

/// The upload staging root shared with `upload_staging_commands`.
pub fn upload_staging_root() -> PathBuf {
    std::env::temp_dir().join(UPLOAD_STAGING_ROOT_NAME)
}

/// Canonical forms of the two allowed roots, resolved once.
///
/// `validate_server_path` runs on every `/__tt/file` request (chat payloads,
/// extension assets — dozens to hundreds per page load) and both roots are
/// fixed for the process lifetime, so re-canonicalizing them per call was two
/// avoidable syscalls each time; canonicalize is comparatively expensive on
/// Windows.
async fn allowed_roots(data_root: &Path) -> &'static Vec<PathBuf> {
    static ROOTS: tokio::sync::OnceCell<Vec<PathBuf>> = tokio::sync::OnceCell::const_new();

    ROOTS
        .get_or_init(|| async {
            let mut roots = Vec::with_capacity(2);
            for root in [data_root.to_path_buf(), upload_staging_root()] {
                if let Ok(canonical) = canonicalize_with_missing_tail(&root).await {
                    roots.push(canonical);
                }
            }
            roots
        })
        .await
}

/// Validate that `path` resolves inside the data root or the upload staging
/// root, returning its canonical form. The path may point at a not-yet-existing
/// file (e.g. a mkdir/remove target): the nearest existing ancestor is
/// canonicalized and the remainder is re-appended.
pub async fn validate_server_path(path: &str, data_root: &Path) -> Result<PathBuf, CommandError> {
    let requested = PathBuf::from(path.trim());
    if !requested.is_absolute() {
        return Err(CommandError::BadRequest(
            "Server file path must be absolute".to_string(),
        ));
    }

    let canonical = canonicalize_with_missing_tail(&requested).await?;

    for canonical_root in allowed_roots(data_root).await {
        if canonical.starts_with(canonical_root) {
            return Ok(canonical);
        }
    }

    Err(CommandError::BadRequest(format!(
        "Path is outside the server data directory: {}",
        requested.display()
    )))
}

/// `validate_server_path` against a caller-supplied root set, for
/// low-frequency commands whose allowed roots vary (the roots are
/// canonicalized per call, so the hot `/__tt/file` path stays on
/// `validate_server_path` with its cached roots).
pub async fn validate_server_path_in_roots(
    path: &str,
    roots: &[PathBuf],
) -> Result<PathBuf, CommandError> {
    let requested = PathBuf::from(path.trim());
    if !requested.is_absolute() {
        return Err(CommandError::BadRequest(
            "Server file path must be absolute".to_string(),
        ));
    }

    let canonical = canonicalize_with_missing_tail(&requested).await?;

    for root in roots {
        let Ok(canonical_root) = canonicalize_with_missing_tail(root).await else {
            continue;
        };
        if canonical.starts_with(canonical_root) {
            return Ok(canonical);
        }
    }

    Err(CommandError::BadRequest(format!(
        "Path is outside the server data directory: {}",
        requested.display()
    )))
}

async fn canonicalize_with_missing_tail(path: &Path) -> Result<PathBuf, CommandError> {
    let mut existing = path;
    let mut tail: Vec<std::ffi::OsString> = Vec::new();

    loop {
        match tokio::fs::canonicalize(existing).await {
            Ok(canonical) => {
                let mut result = canonical;
                for component in tail.iter().rev() {
                    result.push(component);
                }
                return Ok(result);
            }
            Err(_) => {
                let Some(name) = existing.file_name() else {
                    return Err(CommandError::BadRequest(format!(
                        "Invalid server file path: {}",
                        path.display()
                    )));
                };
                tail.push(name.to_os_string());
                let Some(parent) = existing.parent() else {
                    return Err(CommandError::BadRequest(format!(
                        "Invalid server file path: {}",
                        path.display()
                    )));
                };
                existing = parent;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str, content: &[u8]) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "rusttavern-fs-resources-{name}-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&path, content).expect("write temp file");
        path
    }

    fn read_options() -> FsOpenOptions {
        FsOpenOptions {
            read: true,
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn read_beyond_max_len_is_rejected() {
        let path = temp_file("len-limit", b"abc");
        let registry = FsResourceRegistry::new();
        let rid = registry
            .open(&path, &read_options())
            .await
            .expect("open resource");
        let error = registry
            .read(rid, MAX_READ_LEN + 1)
            .await
            .expect_err("over-limit read must be rejected");
        assert!(matches!(error, CommandError::BadRequest(_)));
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn read_returns_bytes_with_big_endian_trailer() {
        let path = temp_file("trailer", b"hello");
        let registry = FsResourceRegistry::new();
        let rid = registry
            .open(&path, &read_options())
            .await
            .expect("open resource");
        let data = registry.read(rid, 16).await.expect("read resource");
        assert_eq!(&data[..5], b"hello");
        assert_eq!(
            u64::from_be_bytes(data[5..13].try_into().expect("trailer width")),
            5
        );
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn idle_resources_are_swept_on_next_use() {
        let path = temp_file("idle", b"data");
        let registry = FsResourceRegistry::new();
        let rid = registry
            .open(&path, &read_options())
            .await
            .expect("open resource");

        // Age the entry beyond the idle timeout, then any use sweeps it.
        {
            let resources = registry.resources.lock().await;
            let entry = resources.get(&rid).expect("entry present");
            *entry.last_access.lock().expect("entry lock") =
                Instant::now() - RESOURCE_IDLE_TIMEOUT - Duration::from_secs(1);
        }

        let error = registry.read(rid, 4).await.expect_err("swept entry is gone");
        assert!(matches!(error, CommandError::BadRequest(_)));
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn table_stays_bounded_by_evicting_lru_on_overflow() {
        let path = temp_file("lru", b"data");
        let registry = FsResourceRegistry::new();
        let mut rids = Vec::new();
        for _ in 0..MAX_RESOURCES {
            rids.push(
                registry
                    .open(&path, &read_options())
                    .await
                    .expect("open resource"),
            );
        }

        let extra = registry
            .open(&path, &read_options())
            .await
            .expect("open resource");
        assert!(registry.read(extra, 4).await.is_ok());

        // Exactly one of the older entries was evicted.
        let mut failed = 0;
        for rid in &rids {
            if registry.read(*rid, 4).await.is_err() {
                failed += 1;
            }
        }
        assert_eq!(failed, 1);
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn concurrent_reads_of_different_resources_succeed() {
        let path_a = temp_file("concurrent-a", b"aaaa");
        let path_b = temp_file("concurrent-b", b"bbbb");
        let registry = FsResourceRegistry::new();
        let rid_a = registry
            .open(&path_a, &read_options())
            .await
            .expect("open resource");
        let rid_b = registry
            .open(&path_b, &read_options())
            .await
            .expect("open resource");

        let (read_a, read_b) = tokio::join!(
            registry.read(rid_a, 8),
            registry.read(rid_b, 8),
        );
        assert!(read_a.is_ok() && read_b.is_ok());
        let _ = std::fs::remove_file(&path_a);
        let _ = std::fs::remove_file(&path_b);
    }
}
