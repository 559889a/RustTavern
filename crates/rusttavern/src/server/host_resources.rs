//! axum delivery for `HostResourceService`.
//!
//! Converts an axum request into the `http::Request<Vec<u8>>` shape the
//! service expects, delegates to `try_serve`, and converts the response back.
//! Preserves the existing 404/ETag/Cache-Control/Range semantics implemented
//! by the service (see `docs/CurrentState/HostResourceCaching.md` and
//! `MediaAssetContract.md`).

use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::extract::State;
use axum::http::{Request, StatusCode, header};
use axum::response::{IntoResponse, Response};
use tokio_util::io::ReaderStream;
use tt_application::services::host_resource_service::{
    HostResourceDeliveryCapabilities, HostResourcePayload, HostResourceService,
};

/// HTTP browser delivery: full 304 support, no webview range workaround.
const HTTP_DELIVERY: HostResourceDeliveryCapabilities =
    HostResourceDeliveryCapabilities::new(true, false);

/// Max body size for host resource requests (all are GET/HEAD).
const MAX_HOST_RESOURCE_BODY_BYTES: usize = 64 * 1024;

/// Fallback route handler for browser-visible host resources
/// (`/thumbnail`, `/characters/*`, `/User Avatars/*`, `/backgrounds/*`,
/// `/assets/*`, `/user/images/*`, `/user/files/*`,
/// `/scripts/extensions/third-party/*`, `/css/user.css`).
pub async fn serve_host_resource(
    State(host_resources): State<Arc<HostResourceService>>,
    request: Request<Body>,
) -> Response {
    let (parts, body) = request.into_parts();
    let bytes = match to_bytes(body, MAX_HOST_RESOURCE_BODY_BYTES).await {
        Ok(bytes) => bytes.to_vec(),
        Err(error) => {
            tracing::warn!("Failed to read host resource request body: {}", error);
            return (
                StatusCode::BAD_REQUEST,
                "Failed to read request body",
            )
                .into_response();
        }
    };

    let http_request = Request::from_parts(parts, bytes);
    // Thumbnail generation is CPU-bound image work; run it on the blocking
    // pool so it never stalls a runtime worker (and concurrent thumbnail
    // requests no longer serialize behind one worker thread).
    let response = tokio::task::spawn_blocking({
        let host_resources = host_resources.clone();
        move || {
            host_resources
                .try_serve(&http_request, HTTP_DELIVERY)
                .unwrap_or_else(not_found_response)
        }
    })
    .await
    .unwrap_or_else(|error| {
        tracing::error!("Host resource serving task failed: {error}");
        let mut response =
            axum::http::Response::new(HostResourcePayload::from(b"Internal Server Error".to_vec()));
        *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
        response
    });

    let (parts, body) = response.into_parts();
    let mut builder = Response::builder().status(parts.status);
    *builder.headers_mut().unwrap() = parts.headers;
    let body = match body {
        HostResourcePayload::Bytes(bytes) => Body::from(bytes),
        // Streamed straight from the file handle instead of being materialised, so a
        // large asset costs a fixed-size buffer rather than its own size in RSS.
        HostResourcePayload::Reader(reader) => Body::from_stream(ReaderStream::new(reader)),
    };
    builder
        .body(body)
        .unwrap_or_else(|error| {
            tracing::error!("Failed to build host resource response: {}", error);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        })
}

fn not_found_response() -> axum::http::Response<HostResourcePayload> {
    let mut response = axum::http::Response::new(HostResourcePayload::from(b"Not Found".to_vec()));
    *response.status_mut() = StatusCode::NOT_FOUND;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    response
}
