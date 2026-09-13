//! `/__tt/events` and `/__tt/stream/{stream_id}` SSE endpoints.
//!
//! - `/__tt/events` streams every EventBus message to connected browsers.
//!   The frontend `host-bridge.listen()` subscribes here and filters by
//!   event name.
//! - `/__tt/stream/{stream_id}` streams command events (chat completion etc.)
//!   registered through `StreamRegistry`. The endpoint waits briefly for the
//!   stream to appear so the frontend can subscribe before invoking the
//!   command without losing the first events.
//!
//! Both endpoints terminate their connections when the server announces a
//! graceful shutdown (`ServerState::shutdown`), so `axum::serve` can finish
//! draining instead of waiting on the long-lived SSE connections forever.

use std::convert::Infallible;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

use crate::app::events::EventMessage;
use crate::server::router::ServerState;

// An interval tick wakes the process even when an event frame went out in the
// same window, so this period *is* the idle wake-up rate for every open
// connection — 4/min at 15s on a phone that should be sleeping. A minute is
// still far below any proxy or EventSource idle timeout, and the stream is
// only a liveness ping: real events travel as they arrive.
const SSE_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(60);
const STREAM_WAIT_TIMEOUT: Duration = Duration::from_secs(30);

/// GET /__tt/events — SSE stream of all backend events.
pub async fn events_sse(State(state): State<Arc<ServerState>>) -> Response {
    let receiver = state.events.subscribe();
    let last_activity = Arc::new(AtomicU64::new(0));
    let activity = last_activity.clone();
    let events = BroadcastStream::new(receiver).filter_map(move |result| match result {
        Ok(message) => {
            activity.fetch_add(1, Ordering::Relaxed);
            Some(format_sse_event(&message))
        }
        // Slow subscribers drop events; that is the documented trade-off for
        // live observability events (durable state lives in repositories).
        Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(_)) => None,
    });

    let body = Body::from_stream(
        // UFCS keeps `take_until` unambiguous: `filter_map`/`merge` come from
        // `tokio_stream::StreamExt`, `take_until` from `futures_util::StreamExt`.
        futures_util::StreamExt::take_until(
            events
                .map(Ok::<_, Infallible>)
                .merge(keepalive_stream(last_activity)),
            shutdown_future(state.shutdown.subscribe()),
        ),
    );

    sse_response(body)
}

/// GET /__tt/stream/{stream_id} — SSE stream of command events.
pub async fn stream_sse(
    State(state): State<Arc<ServerState>>,
    Path(stream_id): Path<String>,
) -> Response {
    let receiver = tokio::select! {
        maybe = state.streams.wait_for_subscribe(&stream_id, STREAM_WAIT_TIMEOUT) => maybe,
        _ = shutdown_future(state.shutdown.subscribe()) => return shutdown_response(),
    };

    let Some(receiver) = receiver else {
        return (
            StatusCode::NOT_FOUND,
            format!("Stream not found: {stream_id}"),
        )
            .into_response();
    };

    // A slow consumer that falls behind gets one explicit termination frame
    // (matching the `error` shape of command stream events) and the connection
    // closes, so the frontend can tell the stream was truncated instead of
    // silently missing tokens.
    let mut terminated = false;
    let last_activity = Arc::new(AtomicU64::new(0));
    let activity = last_activity.clone();
    let events = BroadcastStream::new(receiver).filter_map(move |result| {
        let frame = stream_frame(result, &mut terminated);
        if frame.is_some() {
            activity.fetch_add(1, Ordering::Relaxed);
        }
        frame
    });

    let body = Body::from_stream(
        futures_util::StreamExt::take_until(
            events
                .map(Ok::<_, Infallible>)
                .merge(keepalive_stream(last_activity)),
            shutdown_future(state.shutdown.subscribe()),
        ),
    );

    sse_response(body)
}

/// Resolves when the server announces a graceful shutdown.
async fn shutdown_future(mut receiver: tokio::sync::watch::Receiver<bool>) {
    // Also resolves when the channel closes (server state dropped).
    let _ = receiver.changed().await;
}

fn shutdown_response() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        "Server is shutting down",
    )
        .into_response()
}

/// Idle keepalive: emits a ping only when no event frame was produced since
/// the previous tick. The tick itself still fires either way — what is saved
/// on an active connection is the payload, not the wake-up; the wake-up rate
/// is bounded by `SSE_KEEPALIVE_INTERVAL`.
fn keepalive_stream(
    last_activity: Arc<AtomicU64>,
) -> impl tokio_stream::Stream<Item = Result<String, Infallible>> {
    tokio_stream::wrappers::IntervalStream::new(tokio::time::interval(SSE_KEEPALIVE_INTERVAL))
        .map(move |_| {
            let had_activity = last_activity.swap(0, Ordering::Relaxed) > 0;
            Ok::<_, Infallible>(if had_activity {
                String::new()
            } else {
                ": ping\n\n".to_string()
            })
        })
}

fn sse_response(body: Body) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("X-Accel-Buffering", "no")
        .body(body)
        .unwrap_or_else(|error| {
            tracing::error!("Failed to build SSE response: {error}");
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .expect("static error response")
        })
}

/// Map one broadcast item of a command stream to an SSE frame. On `Lagged`
/// (consumer fell behind) emits one explicit termination frame and flips
/// `terminated`, after which the stream yields nothing and the connection
/// closes — the frontend sees the truncation instead of silent missing
/// tokens.
fn stream_frame(
    result: Result<String, tokio_stream::wrappers::errors::BroadcastStreamRecvError>,
    terminated: &mut bool,
) -> Option<String> {
    if *terminated {
        return None;
    }
    match result {
        Ok(value) => Some(format!("data: {value}\n\n")),
        Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(_)) => {
            *terminated = true;
            Some(
                "data: {\"type\":\"error\",\"message\":\"Stream interrupted: consumer fell behind\"}\n\n"
                    .to_string(),
            )
        }
    }
}

fn format_sse_event(message: &EventMessage) -> String {
    // The payload is pre-serialized once at publish time (single-line JSON);
    // escape stray newlines defensively.
    let payload = message.payload.replace('\n', "\\n");
    format!("event: {}\ndata: {}\n\n", message.event, payload)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::app::events::EventBus;

    #[tokio::test]
    async fn shutdown_future_resolves_when_signaled() {
        let (tx, rx) = tokio::sync::watch::channel(false);
        let future = shutdown_future(rx);
        tokio::pin!(future);
        assert!(futures_util::poll!(&mut future).is_pending());
        let _ = tx.send(true);
        assert!(futures_util::poll!(&mut future).is_ready());
    }

    #[tokio::test]
    async fn shutdown_future_resolves_when_channel_closed() {
        let (tx, rx) = tokio::sync::watch::channel(false);
        drop(tx);
        tokio::time::timeout(Duration::from_secs(1), shutdown_future(rx))
            .await
            .expect("shutdown future must resolve when the channel closes");
    }

    #[test]
    fn stream_frame_wraps_pre_serialized_payload() {
        let mut terminated = false;
        let frame = stream_frame(Ok("{\"type\":\"done\"}".to_string()), &mut terminated)
            .expect("frame");
        assert_eq!(frame, "data: {\"type\":\"done\"}\n\n");
        assert!(!terminated);
    }

    #[test]
    fn lagged_subscriber_gets_termination_frame_then_nothing() {
        let mut terminated = false;
        let frame = stream_frame(
            Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(1)),
            &mut terminated,
        )
        .expect("termination frame");
        assert!(frame.contains("Stream interrupted"));
        assert!(terminated);
        assert!(
            stream_frame(Ok("{\"type\":\"chunk\"}".to_string()), &mut terminated).is_none()
        );
    }

    #[tokio::test]
    async fn sse_stream_terminates_on_shutdown_signal() {
        let bus = Arc::new(EventBus::new());
        let (shutdown_tx, _) = tokio::sync::watch::channel(false);

        // Same stream assembly as `events_sse`.
        let receiver = bus.subscribe();
        let events = BroadcastStream::new(receiver).filter_map(|result| match result {
            Ok(message) => Some(format_sse_event(&message)),
            Err(tokio_stream::wrappers::errors::BroadcastStreamRecvError::Lagged(_)) => None,
        });
        let mut stream = Box::pin(futures_util::StreamExt::take_until(
            events
                .map(Ok::<_, Infallible>)
                .merge(keepalive_stream(Arc::new(AtomicU64::new(0)))),
            shutdown_future(shutdown_tx.subscribe()),
        ));

        // A published event is delivered before the shutdown signal.
        bus.publish("test", 1u32);
        let first = tokio::time::timeout(
            Duration::from_secs(2),
            futures_util::StreamExt::next(&mut stream),
        )
        .await
        .expect("stream should yield the published event")
        .expect("stream item")
        .expect("infallible item");
        assert!(first.contains("event: test"));
        assert!(first.contains("data: 1"));

        // After the shutdown signal the stream terminates instead of hanging
        // on the keepalive interval.
        let _ = shutdown_tx.send(true);
        let next = tokio::time::timeout(
            Duration::from_secs(2),
            futures_util::StreamExt::next(&mut stream),
        )
        .await
        .expect("stream must terminate after the shutdown signal");
        assert!(next.is_none());
    }
}
