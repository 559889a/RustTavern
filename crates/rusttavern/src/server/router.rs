//! HTTP router assembly.
//!
//! Route layout:
//! - `POST /__tt/invoke/{command}` — generic command dispatch
//! - `GET  /__tt/stream/{stream_id}` — SSE command stream
//! - `GET  /__tt/events` — SSE global event stream
//! - `GET  /__tt/file?path=...` — validated file access (`convertFileSrc`)
//! - `GET  /__tt/health` — liveness probe
//! - Host resource routes → `HostResourceService`
//! - Everything else → static files from the web root

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::Router;
use axum::body::Body;
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::{Method, Request, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};

use crate::app::AppState;
use crate::app::events::EventBus;
use crate::presentation::errors::CommandError;
use crate::server::dispatch::CommandRegistry;
use crate::server::security::SecurityState;
use crate::server::stream::StreamRegistry;
use tt_application::services::host_resource_service::HostResourceService;

/// Upper bound for an invoke request body. Chat save frames are 4 MiB
/// (about 5.3 MiB after base64), upload chunks are 512 KiB, and a 27 MB
/// base64 image fits within 64 MiB with room to spare. The old 512 MiB limit
/// allowed a single request to exhaust a 1 GB machine.
const MAX_INVOKE_BODY_BYTES: u64 = 64 * 1024 * 1024;

/// Shared router state.
pub struct ServerState {
    pub events: Arc<EventBus>,
    pub registry: Arc<CommandRegistry>,
    pub app_state: Arc<AppStateSlot>,
    pub host_resources: Arc<HostResourceService>,
    pub streams: Arc<StreamRegistry>,
    /// Set when a graceful shutdown is announced; SSE handlers watch this to
    /// terminate their connections so `axum::serve` can finish draining.
    pub shutdown: tokio::sync::watch::Sender<bool>,
    pub web_root: PathBuf,
    /// Canonical data root; `GET /__tt/file` only serves paths inside it (or
    /// the upload staging root).
    pub data_root: PathBuf,
}

/// Holds the AppState once backend initialization completes. Commands invoked
/// before that point wait for initialization; a failed initialization resolves
/// every pending command with an error.
pub struct AppStateSlot {
    inner: tokio::sync::watch::Sender<Option<Result<Arc<AppState>, String>>>,
}

/// How long a command waits for backend initialization before failing fast.
const INIT_WAIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

impl AppStateSlot {
    pub fn new() -> Arc<Self> {
        let (inner, _) = tokio::sync::watch::channel(None);
        Arc::new(Self { inner })
    }

    pub fn set(&self, value: Result<Arc<AppState>, String>) {
        self.inner.send_replace(Some(value));
    }

    pub async fn get(&self) -> Result<Arc<AppState>, CommandError> {
        let mut receiver = self.inner.subscribe();
        let wait = tokio::time::timeout(INIT_WAIT_TIMEOUT, async {
            loop {
                match receiver.borrow_and_update().clone() {
                    Some(Ok(state)) => return Ok(state),
                    Some(Err(message)) => {
                        return Err(CommandError::InternalServerError(format!(
                            "Backend initialization failed: {message}"
                        )));
                    }
                    None => {}
                }
                if receiver.changed().await.is_err() {
                    return Err(CommandError::InternalServerError(
                        "Backend initialization channel closed".to_string(),
                    ));
                }
            }
        })
        .await;
        match wait {
            Ok(result) => result,
            Err(_) => Err(CommandError::InternalServerError(
                "Backend initialization timed out".to_string(),
            )),
        }
    }
}

pub fn build_router(state: Arc<ServerState>, security: Arc<SecurityState>) -> Router {
    let host_resource_routes = Router::new()
        .route("/thumbnail", axum::routing::get(serve_host_resource_any))
        .route("/characters/{*path}", axum::routing::get(serve_host_resource_any))
        .route(
            "/User Avatars/{*path}",
            axum::routing::get(serve_host_resource_any),
        )
        .route(
            "/User%20Avatars/{*path}",
            axum::routing::get(serve_host_resource_any),
        )
        .route(
            "/backgrounds/{*path}",
            axum::routing::get(serve_host_resource_any),
        )
        .route("/assets/{*path}", axum::routing::get(serve_host_resource_any))
        .route(
            "/user/images/{*path}",
            axum::routing::get(serve_host_resource_any),
        )
        .route("/user/files/{*path}", axum::routing::get(serve_host_resource_any))
        .route(
            "/scripts/extensions/third-party/{*path}",
            axum::routing::get(serve_host_resource_any),
        )
        .route("/css/user.css", axum::routing::get(serve_host_resource_any))
        .with_state(state.host_resources.clone());

    Router::new()
        .route(
            "/__tt/invoke/{command}",
            axum::routing::post(invoke_command),
        )
        .route("/__tt/events", axum::routing::get(events_sse_handler))
        .route(
            "/__tt/stream/{stream_id}",
            axum::routing::get(stream_sse_handler),
        )
        .route("/__tt/health", axum::routing::get(health))
        .route("/__tt/file", axum::routing::get(serve_tt_file))
        .route("/api/{*path}", axum::routing::any(api_route_not_found))
        .merge(host_resource_routes)
        .fallback(static_files)
        // Security chain: IP whitelist -> CSRF header -> Basic auth + session
        // cookie. Applied to every route including the static fallback.
        .layer(axum::middleware::from_fn_with_state(
            security,
            crate::server::security::auth_middleware,
        ))
        .with_state(state)
}

async fn events_sse_handler(State(state): State<Arc<ServerState>>) -> Response {
    crate::server::events::events_sse(axum::extract::State(state)).await
}

async fn stream_sse_handler(
    State(state): State<Arc<ServerState>>,
    AxumPath(stream_id): AxumPath<String>,
) -> Response {
    crate::server::events::stream_sse(axum::extract::State(state), AxumPath(stream_id)).await
}

async fn serve_host_resource_any(
    State(host_resources): State<Arc<HostResourceService>>,
    request: Request<Body>,
) -> Response {
    crate::server::host_resources::serve_host_resource(State(host_resources), request).await
}

async fn health() -> &'static str {
    "ok"
}

async fn invoke_command(
    State(state): State<Arc<ServerState>>,
    AxumPath(command): AxumPath<String>,
    request: Request<Body>,
) -> Response {
    // Content-Length pre-check: reject oversized bodies before buffering
    // anything. Chunked requests without a Content-Length are still capped by
    // the `to_bytes` limit below.
    if let Some(content_length) = request.headers().get(header::CONTENT_LENGTH)
        && let Some(length) = content_length
            .to_str()
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
        && length > MAX_INVOKE_BODY_BYTES
    {
        return payload_too_large_response(&command);
    }

    let body_bytes =
        match axum::body::to_bytes(request.into_body(), MAX_INVOKE_BODY_BYTES as usize).await {
        Ok(bytes) => bytes,
        Err(error) => {
            let message = error.to_string();
            if error.into_inner().is::<http_body_util::LengthLimitError>() {
                return payload_too_large_response(&command);
            }
            tracing::warn!("Failed to read invoke body for {command}: {message}");
            return command_error_response(&CommandError::BadRequest(
                "Failed to read invoke body".to_string(),
            ));
        }
    };

    let args: serde_json::Value = if body_bytes.is_empty() {
        serde_json::Value::Null
    } else {
        match serde_json::from_slice(&body_bytes) {
            Ok(value) => value,
            Err(error) => {
                return command_error_response(&CommandError::BadRequest(format!(
                    "Invalid invoke arguments: {error}"
                )));
            }
        }
    };

    let app_state = match state.app_state.get().await {
        Ok(app_state) => app_state,
        Err(error) => return command_error_response(&error),
    };

    match state.registry.invoke(&command, app_state, &args).await {
        Ok(value) => json_response(StatusCode::OK, value),
        Err(error) => command_error_response(&error),
    }
}

/// 413 response for invoke bodies over [`MAX_INVOKE_BODY_BYTES`]. Uses the
/// same JSON error shape as `command_error_response` so frontend error
/// normalization keeps working.
fn payload_too_large_response(command: &str) -> Response {
    tracing::warn!("Invoke body for {command} exceeds the size limit");
    Response::builder()
        .status(StatusCode::PAYLOAD_TOO_LARGE)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({"BadRequest": "Invoke body too large"}).to_string(),
        ))
        .unwrap_or_else(|_| StatusCode::PAYLOAD_TOO_LARGE.into_response())
}

pub fn command_error_response(error: &CommandError) -> Response {
    let status = command_error_status(error);
    // Match the legacy Tauri error contract: the body is the serialized
    // CommandError (e.g. `{"BadRequest":"..."}` or `{"UpstreamFailure":{...}}`),
    // which the frontend error normalization understands.
    let body = serde_json::to_vec(error).unwrap_or_else(|_| {
        serde_json::json!({"InternalServerError": "Failed to serialize error"})
            .to_string()
            .into_bytes()
    });
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap_or_else(|builder_error| {
            tracing::error!("Failed to build error response: {builder_error}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        })
}

fn command_error_status(error: &CommandError) -> StatusCode {
    match error {
        CommandError::BadRequest(_) => StatusCode::BAD_REQUEST,
        CommandError::Conflict(_) => StatusCode::CONFLICT,
        CommandError::NotFound(_) => StatusCode::NOT_FOUND,
        CommandError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
        CommandError::Cancelled(_) => StatusCode::BAD_REQUEST,
        CommandError::TooManyRequests(_) => StatusCode::TOO_MANY_REQUESTS,
        CommandError::UpstreamFailure(_) => StatusCode::BAD_GATEWAY,
        CommandError::InternalServerError(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn json_response(status: StatusCode, body: Vec<u8>) -> Response {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap_or_else(|error| {
            tracing::error!("Failed to build JSON response: {error}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        })
}

/// Extracts a header value as an owned string (None when absent or not
/// valid UTF-8). Used to avoid borrowing the request across `.await`.
fn request_header(request: &axum::extract::Request, name: header::HeaderName) -> Option<String> {
    request
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
}

/// Joins a decoded request path onto `root`, rejecting anything that could
/// escape it.
///
/// Only plain components are accepted. `Path::join` replaces the whole base
/// path when the argument is absolute or carries a Windows drive/UNC prefix,
/// so a request for `/%2Fetc%2Fpasswd` or `/C:/Windows/win.ini` would
/// otherwise read an arbitrary file instead of a web-root asset.
fn resolve_under_root(root: &Path, relative: &str) -> Option<PathBuf> {
    if relative.is_empty() || relative.contains('\\') {
        return None;
    }

    let mut resolved = root.to_path_buf();
    let mut pushed = false;
    for component in Path::new(relative).components() {
        match component {
            std::path::Component::Normal(segment) => {
                resolved.push(segment);
                pushed = true;
            }
            _ => return None,
        }
    }

    pushed.then_some(resolved)
}

/// True when the last path segment carries a file extension, i.e. the request
/// is for a concrete asset rather than an app deep link.
fn looks_like_asset_path(relative: &str) -> bool {
    relative
        .rsplit('/')
        .next()
        .is_some_and(|segment| segment.contains('.'))
}

/// Static file serving with SPA index.html fallback.
async fn static_files(State(state): State<Arc<ServerState>>, request: Request<Body>) -> Response {
    if request.method() != Method::GET && request.method() != Method::HEAD {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }

    let path = request.uri().path();
    let trimmed = path.trim_start_matches('/');
    let relative = if trimmed.is_empty() {
        "index.html".to_string()
    } else {
        percent_decode_path(trimmed)
    };

    let head_only = request.method() == Method::HEAD;
    let if_modified_since = request_header(&request, header::IF_MODIFIED_SINCE);
    let range_header = request_header(&request, header::RANGE);
    let accept_encoding = request_header(&request, header::ACCEPT_ENCODING);

    if let Some(file_path) = resolve_under_root(&state.web_root, &relative)
        && let Some(metadata) = file_metadata(&file_path).await
    {
        return serve_file(
            file_path,
            metadata,
            head_only,
            if_modified_since,
            range_header,
            true,
            accept_encoding,
        )
        .await;
    }

    // SPA fallback: unknown deep links serve index.html so the shell loads and
    // the frontend routes internally. A missing *asset* must not: answering a
    // `.js`/`.css`/`.json` request with the HTML shell under status 200 turns a
    // missing file into a MIME-type or JSON parse error far from its cause.
    if looks_like_asset_path(&relative) {
        tracing::debug!("Static asset not found: {relative}");
        return StatusCode::NOT_FOUND.into_response();
    }

    let index_path = state.web_root.join("index.html");
    if let Some(metadata) = file_metadata(&index_path).await {
        return serve_file(
            index_path,
            metadata,
            head_only,
            if_modified_since,
            range_header,
            true,
            accept_encoding,
        )
        .await;
    }

    StatusCode::NOT_FOUND.into_response()
}

/// `/api/*` is served entirely by the frontend fetch interceptor
/// (`src/host/main/routes/`), which rewrites those calls into
/// `POST /__tt/invoke/{command}`. A request that reaches the server means the
/// interceptor has no route for it, so answer with the command-error shape the
/// frontend already normalizes instead of letting the static fallback reply
/// `200 text/html`.
async fn api_route_not_found(request: Request<Body>) -> Response {
    let path = request.uri().path().to_string();
    tracing::warn!(
        "Unhandled {} {path}: no frontend interceptor route (server has no /api surface)",
        request.method()
    );
    command_error_response(&CommandError::NotFound(format!(
        "No API route: {} {path}",
        request.method()
    )))
}

/// Metadata for `path` when it is a regular file.
///
/// The caller passes the result on to [`serve_file`], so existence and the
/// size/mtime it needs come from one `stat` instead of two per request. With
/// `Cache-Control: no-cache` the browser revalidates every module on reload,
/// which made the duplicate stat a per-asset cost on every page load.
async fn file_metadata(path: &Path) -> Option<std::fs::Metadata> {
    tokio::fs::metadata(path)
        .await
        .ok()
        .filter(|metadata| metadata.is_file())
}

/// Formats a system time as an RFC 7231 HTTP date (second precision).
fn http_date(time: SystemTime) -> String {
    httpdate::fmt_http_date(time)
}

fn parse_http_date(value: &str) -> Option<SystemTime> {
    httpdate::parse_http_date(value).ok()
}

/// Seconds since the UNIX epoch for an HTTP-date comparison (HTTP dates
/// truncate to whole seconds).
fn epoch_secs(time: SystemTime) -> Option<u64> {
    time.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs())
}

/// Parses a single `Range: bytes=start-end` request. Supports open-ended
/// (`bytes=start-`), suffix (`bytes=-N`) and closed forms. Returns None for
/// malformed or unsatisfiable ranges (caller responds 416); multi-range
/// requests fall back to the full representation.
fn parse_single_range(value: &str, total: u64) -> Option<(u64, u64)> {
    let spec = value.trim().strip_prefix("bytes=")?;
    if spec.contains(',') {
        return None; // multi-range: serve the full representation
    }
    let (start, end) = spec.split_once('-')?;
    let start = start.trim();
    let end = end.trim();

    if start.is_empty() {
        // Suffix range: the last N bytes.
        let n: u64 = end.parse().ok()?;
        if n == 0 {
            return None;
        }
        let from = total.saturating_sub(n);
        return Some((from, total.saturating_sub(1)));
    }

    let from: u64 = start.parse().ok()?;
    if from >= total {
        return None;
    }
    let to = if end.is_empty() {
        total - 1
    } else {
        end.parse::<u64>().ok()?.min(total - 1)
    };
    if to < from {
        return None;
    }
    Some((from, to))
}

fn range_not_satisfiable_response(total: u64) -> Response {
    Response::builder()
        .status(StatusCode::RANGE_NOT_SATISFIABLE)
        .header(header::CONTENT_RANGE, format!("bytes */{total}"))
        .header(header::ACCEPT_RANGES, "bytes")
        .body(Body::empty())
        .unwrap_or_else(|_| StatusCode::RANGE_NOT_SATISFIABLE.into_response())
}

/// Smallest asset worth gzipping: below this the gain does not cover the framing.
const GZIP_MIN_BYTES: u64 = 1024;
/// Largest asset worth gzipping: without a ceiling a huge user file pulled in
/// through the static root would be materialised and compressed on a request.
const GZIP_MAX_BYTES: u64 = 16 * 1024 * 1024;
// ponytail: the cache is cleared wholesale if it ever exceeds these bounds, which
// keeps the policy at six lines. The real frontend is ~600 text assets totalling a
// few MB compressed, so a dev checkout that edits modules for hours stays far
// below both; if that ever stops holding, switch to FIFO eviction like the
// thumbnail and tokenizer caches.
const GZIP_CACHE_MAX_ENTRIES: usize = 4096;
const GZIP_CACHE_MAX_BYTES: usize = 32 * 1024 * 1024;

struct CompressedAsset {
    size: u64,
    modified_millis: i64,
    bytes: bytes::Bytes,
}

/// Gzipped static assets, keyed by path.
///
/// A given (size, mtime) is immutable, so re-gzipping `index.html` (757KB of
/// script tags) on every reload would burn a worker-thread slice each time on this
/// CPU. Entries are replaced whenever the pair changes, which is what makes
/// "edit a module, reload" still serve fresh bytes.
static COMPRESSED_ASSETS: LazyLock<Mutex<HashMap<PathBuf, CompressedAsset>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Whether the client would accept a gzip-encoded response.
fn accepts_gzip(accept_encoding: Option<&str>) -> bool {
    accept_encoding.is_some_and(|value| {
        value.split(',').any(|coding| {
            let mut parameters = coding.split(';');
            let name = parameters.next().unwrap_or("").trim();
            if !name.eq_ignore_ascii_case("gzip") {
                return false;
            }
            // `gzip;q=0` is an explicit refusal: honouring it as acceptance would
            // send a body the client has said it cannot decode.
            !parameters.any(|parameter| {
                ["q=", "Q="].iter().any(|prefix| {
                    parameter
                        .trim()
                        .strip_prefix(prefix)
                        .is_some_and(|value| value.trim().parse::<f32>() == Ok(0.0))
                })
            })
        })
    })
}

/// Whether this payload is text-like. Compressing an already-compressed image or
/// archive costs CPU for nothing.
fn is_compressible(content_type: &str) -> bool {
    content_type.starts_with("text/")
        || content_type.ends_with("javascript")
        || content_type.ends_with("json")
        || content_type.ends_with("xml")
}

/// A whole static asset answered with `Content-Encoding: gzip`.
///
/// `None` means "serve it identity" -- the caller then follows the normal path.
async fn compressed_asset_response(
    file_path: &Path,
    metadata: &std::fs::Metadata,
    content_type: &str,
    total: u64,
    accept_encoding: Option<&str>,
    last_modified: Option<&str>,
) -> Option<Response> {
    if !accepts_gzip(accept_encoding)
        || !is_compressible(content_type)
        || !(GZIP_MIN_BYTES..=GZIP_MAX_BYTES).contains(&total)
    {
        return None;
    }

    let modified_millis = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0);

    let bytes = match cached_compressed(file_path, total, modified_millis) {
        Some(bytes) => bytes,
        None => {
            let raw = tokio::fs::read(file_path).await.ok()?;
            // The file changed between the caller's stat and this read: fall back
            // to the identity path rather than advertise a length we do not have.
            if raw.len() as u64 != total {
                return None;
            }
            let compressed = tokio::task::spawn_blocking(move || gzip(&raw))
                .await
                .ok()
                .flatten()?;
            store_compressed(file_path, total, modified_millis, compressed)
        }
    };

    let mut builder = Response::builder();
    builder = builder
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_ENCODING, "gzip")
        .header(header::VARY, "Accept-Encoding")
        .header(header::CACHE_CONTROL, "no-cache")
        // No `Accept-Ranges`: ranges are always answered from the identity bytes,
        // so advertising them for this encoding would promise offsets into a
        // representation the client is not holding.
        .header(header::CONTENT_LENGTH, bytes.len().to_string());
    if let Some(last_modified) = last_modified {
        builder = builder.header(header::LAST_MODIFIED, last_modified);
    }
    Some(
        builder
            .body(Body::from(bytes))
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()),
    )
}

fn cached_compressed(file_path: &Path, size: u64, modified_millis: i64) -> Option<bytes::Bytes> {
    let cache = COMPRESSED_ASSETS.lock().ok()?;
    let entry = cache.get(file_path)?;
    (entry.size == size && entry.modified_millis == modified_millis).then(|| entry.bytes.clone())
}

fn store_compressed(
    file_path: &Path,
    size: u64,
    modified_millis: i64,
    bytes: bytes::Bytes,
) -> bytes::Bytes {
    let Ok(mut cache) = COMPRESSED_ASSETS.lock() else {
        return bytes;
    };
    let cached_bytes: usize = cache.values().map(|entry| entry.bytes.len()).sum();
    if cache.len() >= GZIP_CACHE_MAX_ENTRIES || cached_bytes + bytes.len() > GZIP_CACHE_MAX_BYTES {
        cache.clear();
    }
    cache.insert(
        file_path.to_path_buf(),
        CompressedAsset {
            size,
            modified_millis,
            bytes: bytes.clone(),
        },
    );
    bytes
}

fn gzip(raw: &[u8]) -> Option<bytes::Bytes> {
    use std::io::Write as _;

    // Level 1: on text this still lands around 3x smaller, and static assets are
    // compressed on the request path, on a 2.5GHz CPU.
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder.write_all(raw).ok()?;
    encoder.finish().ok().map(bytes::Bytes::from)
}

/// Serves a file with Last-Modified conditional requests (304), single-range
/// byte serving (206/416), and chunked streaming. HEAD requests return the
/// metadata without a body.
///
/// `metadata` comes from the caller's existence check ([`file_metadata`]) so the
/// file is stat'd once per request.
///
/// `compress` is set for the static frontend assets `state.web_root` serves (the
/// first page load is ~8MB of JavaScript) and left off for `/__tt/file`, whose
/// user-data bytes must stay exact for `convertFileSrc` consumers.
///
/// Header values arrive pre-extracted as owned strings: `axum::body::Body`
/// wraps an `UnsyncBoxBody`, so borrowing the request across `.await` would
/// make the handler future non-`Send` (the handler trait requires `Send`).
async fn serve_file(
    file_path: PathBuf,
    metadata: std::fs::Metadata,
    head_only: bool,
    if_modified_since: Option<String>,
    range_header: Option<String>,
    compress: bool,
    accept_encoding: Option<String>,
) -> Response {
    let total = metadata.len();
    let last_modified = metadata
        .modified()
        .ok()
        .filter(|time| epoch_secs(*time).is_some())
        .map(http_date);

    // Conditional request: when the client's copy is at least as fresh as the
    // file, answer 304 without reading the file.
    if let Some(modified_since) = if_modified_since
        .as_deref()
        .and_then(parse_http_date)
        && let (Some(modified), Some(since)) = (
            metadata.modified().ok().and_then(epoch_secs),
            epoch_secs(modified_since),
        )
        && modified <= since
    {
        let mut builder = Response::builder().status(StatusCode::NOT_MODIFIED);
        builder = builder.header(header::CACHE_CONTROL, "no-cache");
        if let Some(last_modified) = &last_modified {
            builder = builder.header(header::LAST_MODIFIED, last_modified);
        }
        return builder
            .body(Body::empty())
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response());
    }

    let file = match tokio::fs::File::open(&file_path).await {
        Ok(file) => file,
        Err(error) => {
            tracing::debug!("Failed to open {}: {}", file_path.display(), error);
            return StatusCode::NOT_FOUND.into_response();
        }
    };

    let content_type = mime_guess::from_path(&file_path)
        .first_or_octet_stream()
        .to_string();

    // Single-range byte serving (206) when a valid Range header is present.
    //
    // An empty file has no satisfiable byte range, so it is handled before the
    // range math: deriving the window from `total - 1` would advertise
    // `Content-Length: 1` for a body that is never sent, and the browser fails
    // the request with a content-length mismatch instead of loading the file.
    let (start, length) = if total == 0 {
        (0u64, 0u64)
    } else if let Some(range_value) = range_header.as_deref().filter(|_| !head_only) {
        match parse_single_range(range_value, total) {
            Some((from, to)) => (from, to - from + 1),
            None => return range_not_satisfiable_response(total),
        }
    } else {
        (0, total)
    };
    let partial = length < total;

    // Whole-file GET only: a ranged response has to stay byte-exact, HEAD returns
    // no body, and the range math above has already had its say (an invalid range
    // must still produce 416 rather than a compressed 200).
    if compress
        && !partial
        && !head_only
        && let Some(response) = compressed_asset_response(
            &file_path,
            &metadata,
            &content_type,
            total,
            accept_encoding.as_deref(),
            last_modified.as_deref(),
        )
        .await
    {
        return response;
    }

    let mut file = file;
    if start > 0 && file.seek(SeekFrom::Start(start)).await.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    let mut builder = Response::builder();
    builder = builder
        .status(if partial {
            StatusCode::PARTIAL_CONTENT
        } else {
            StatusCode::OK
        })
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "no-cache")
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_LENGTH, length.to_string());
    if compress {
        // Same URL, two encodings: without this a shared cache can hand the gzipped
        // bytes to a client that never asked for them.
        builder = builder.header(header::VARY, "Accept-Encoding");
    }
    if let Some(last_modified) = &last_modified {
        builder = builder.header(header::LAST_MODIFIED, last_modified);
    }
    if partial {
        let end = start + length - 1;
        builder = builder.header(
            header::CONTENT_RANGE,
            format!("bytes {start}-{end}/{total}"),
        );
    }

    if head_only {
        return builder
            .body(Body::empty())
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response());
    }

    // Stream the requested window in chunks instead of buffering the whole
    // file in memory (a large data file would otherwise double the working
    // set). `take` bounds the stream to the selected range.
    let stream = tokio_util::io::ReaderStream::new(file.take(length));
    builder
        .body(Body::from_stream(stream))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

fn percent_decode_path(value: &str) -> String {
    percent_encoding::percent_decode_str(value)
        .decode_utf8_lossy()
        .into_owned()
}

#[derive(Debug, Deserialize)]
struct TtFileQuery {
    path: String,
}

/// GET /__tt/file?path=... — validated file access for `convertFileSrc`.
/// Only files inside the data root (or the upload staging root) are served.
/// Supports Last-Modified conditional requests (304) and single-range byte
/// serving, like the static file path.
async fn serve_tt_file(
    State(state): State<Arc<ServerState>>,
    Query(query): Query<TtFileQuery>,
    request: axum::extract::Request,
) -> Response {
    let canonical = match crate::server::fs_resources::validate_server_path(
        &query.path,
        &state.data_root,
    )
    .await
    {
        Ok(path) => path,
        Err(error) => return command_error_response(&error),
    };

    let Some(metadata) = file_metadata(&canonical).await else {
        return command_error_response(&CommandError::NotFound(format!(
            "File not found: {}",
            canonical.display()
        )));
    };

    // Reuse the shared conditional/range/stream implementation; the only
    // difference is that 404s carry the command error shape.
    let head_only = request.method() == Method::HEAD;
    let if_modified_since = request_header(&request, header::IF_MODIFIED_SINCE);
    let range_header = request_header(&request, header::RANGE);
    serve_file(
        canonical,
        metadata,
        head_only,
        if_modified_since,
        range_header,
        // `/__tt/file` serves user data through `convertFileSrc`: those bytes are
        // consumed verbatim (and ranged), so they stay identity.
        false,
        None,
    )
    .await
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{looks_like_asset_path, parse_single_range, resolve_under_root};

    fn web_root() -> PathBuf {
        PathBuf::from(if cfg!(windows) { "C:/srv/web" } else { "/srv/web" })
    }

    #[test]
    fn resolve_under_root_accepts_plain_relative_paths() {
        let root = web_root();
        assert_eq!(
            resolve_under_root(&root, "scripts/extensions/foo.js"),
            Some(root.join("scripts").join("extensions").join("foo.js"))
        );
    }

    #[test]
    fn resolve_under_root_rejects_escapes() {
        let root = web_root();
        // Absolute paths: `Path::join` would replace the root entirely.
        assert_eq!(resolve_under_root(&root, "/etc/passwd"), None);
        assert_eq!(resolve_under_root(&root, "//host/share/file"), None);
        // Traversal and separator smuggling.
        assert_eq!(resolve_under_root(&root, "../secret"), None);
        assert_eq!(resolve_under_root(&root, "a/../../secret"), None);
        assert_eq!(resolve_under_root(&root, "..\\secret"), None);
        assert_eq!(resolve_under_root(&root, ""), None);
    }

    #[cfg(windows)]
    #[test]
    fn resolve_under_root_rejects_windows_drive_prefix() {
        // `web_root.join("C:/Windows/win.ini")` resolves to the drive path, so
        // the prefix component has to be refused, not joined.
        assert_eq!(resolve_under_root(&web_root(), "C:/Windows/win.ini"), None);
        assert_eq!(resolve_under_root(&web_root(), "C:Windows/win.ini"), None);
    }

    #[test]
    fn resolved_paths_stay_inside_the_root() {
        let root = web_root();
        let resolved = resolve_under_root(&root, "css/user.css").expect("resolved");
        assert!(resolved.starts_with(&root));
    }

    #[test]
    fn asset_paths_are_distinguished_from_deep_links() {
        assert!(looks_like_asset_path("scripts/foo.js"));
        assert!(looks_like_asset_path("index.html"));
        assert!(!looks_like_asset_path("characters"));
        assert!(!looks_like_asset_path("some/deep/link"));
        // A dot in a directory name must not make the last segment an asset.
        assert!(!looks_like_asset_path("dir.with.dots/page"));
    }

    #[test]
    fn range_open_ended() {
        assert_eq!(parse_single_range("bytes=10-", 100), Some((10, 99)));
    }

    #[test]
    fn range_closed() {
        assert_eq!(parse_single_range("bytes=10-20", 100), Some((10, 20)));
    }

    #[test]
    fn range_suffix() {
        assert_eq!(parse_single_range("bytes=-10", 100), Some((90, 99)));
        assert_eq!(parse_single_range("bytes=-0", 100), None);
    }

    #[test]
    fn range_clamped_to_total() {
        assert_eq!(parse_single_range("bytes=10-999", 100), Some((10, 99)));
    }

    #[test]
    fn range_unsatisfiable_or_malformed_is_none() {
        assert_eq!(parse_single_range("bytes=100-", 100), None);
        assert_eq!(parse_single_range("bytes=20-10", 100), None);
        assert_eq!(parse_single_range("bytes=abc", 100), None);
        assert_eq!(parse_single_range("items=0-1", 100), None);
    }

    #[test]
    fn multi_range_falls_back_to_full_representation() {
        assert_eq!(parse_single_range("bytes=0-1,3-4", 100), None);
    }
}
