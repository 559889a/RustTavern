//! Command stream registry.
//!
//! Replaces Tauri's `ipc::Channel`: stream commands register a `StreamSink`
//! under a `stream_id`, and the `GET /__tt/stream/{stream_id}` SSE endpoint
//! consumes it. The sink interface mirrors Channel's `send` so command bodies
//! change minimally.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use tokio::sync::Mutex;
use tokio::sync::broadcast;
use tokio::sync::Notify;

/// Stream event buffer. Large enough for chat streaming token bursts; a
/// slow consumer may lag and drop events (documented trade-off, same as the
/// event bus).
const STREAM_BUFFER_CAPACITY: usize = 4096;

/// How long a closed-stream marker is kept, so a late SSE subscriber fails
/// fast instead of waiting out the full subscribe timeout.
const CLOSED_MARKER_TTL: Duration = Duration::from_secs(60);

/// Sink side of a command stream. `send` fails (returns false) only when no
/// consumer is subscribed, mirroring `Channel::send` error semantics. Events
/// are pre-serialized to text exactly once per `send`; the broadcast channel
/// carries the text so N SSE subscribers never re-serialize the same event.
#[derive(Clone)]
pub struct StreamSink {
    sender: broadcast::Sender<String>,
    /// Registry reference used by the drop guard to close an abandoned
    /// stream (task panic / client disconnect before the explicit close).
    registry: Option<Arc<StreamRegistry>>,
    stream_id: String,
    /// Unique registration token: the drop guard only closes the stream if
    /// this token is still the registered one, so a reopened stream with the
    /// same id is never closed by a stale sink.
    token: u64,
}

impl StreamSink {
    pub fn send(&self, event: impl Serialize) -> bool {
        let text = match serde_json::to_string(&event) {
            Ok(text) => text,
            Err(error) => {
                tracing::error!("Failed to serialize stream event: {error}");
                return false;
            }
        };
        self.sender.send(text).is_ok()
    }
}

impl Drop for StreamSink {
    fn drop(&mut self) {
        // Abnormal-path guard: a generating task that is dropped without the
        // explicit close (panic, client disconnect) leaves the stream open.
        // Close it, but only while this sink is still the registered sender,
        // so a reopened stream with the same id is never closed by a stale
        // sink.
        let Some(registry) = &self.registry else {
            return;
        };
        let registry = registry.clone();
        let stream_id = self.stream_id.clone();
        let token = self.token;
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                if registry.is_current_sender(&stream_id, token).await {
                    registry.close(&stream_id).await;
                }
            });
        }
    }
}

/// Global registry of active command streams.
pub struct StreamRegistry {
    streams: Mutex<HashMap<String, (u64, broadcast::Sender<String>)>>,
    /// Stream ids that were closed recently (with the close time). Lets a
    /// subscriber that connects after the stream already finished fail fast
    /// instead of polling until the subscribe timeout.
    closed: Mutex<HashMap<String, Instant>>,
    /// Wakes subscribers waiting in `wait_for_subscribe` when a stream is
    /// opened or closed, so registration races are bridged without a polling
    /// window (a fast-failing stream can finish in less than a poll cycle).
    changed: Notify,
    next_token: std::sync::atomic::AtomicU64,
}

impl StreamRegistry {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            streams: Mutex::new(HashMap::new()),
            closed: Mutex::new(HashMap::new()),
            changed: Notify::new(),
            next_token: std::sync::atomic::AtomicU64::new(1),
        })
    }

    /// Register a new stream. A previously registered stream with the same id
    /// is replaced (the old consumer is disconnected).
    pub async fn open(self: &Arc<Self>, stream_id: &str) -> StreamSink {
        let (sender, _) = broadcast::channel(STREAM_BUFFER_CAPACITY);
        let token = self.next_token.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut streams = self.streams.lock().await;
        let mut closed = self.closed.lock().await;
        streams.insert(stream_id.to_string(), (token, sender.clone()));
        closed.remove(stream_id);
        drop(streams);
        drop(closed);
        self.changed.notify_waiters();
        StreamSink {
            sender,
            registry: Some(self.clone()),
            stream_id: stream_id.to_string(),
            token,
        }
    }

    /// Remove a stream from the registry. The id is remembered briefly so
    /// late subscribers fail fast (see `wait_for_subscribe`). Expired markers
    /// are swept here so the marker table stays bounded.
    pub async fn close(&self, stream_id: &str) {
        self.streams.lock().await.remove(stream_id);
        let mut closed = self.closed.lock().await;
        closed.retain(|_, at| at.elapsed() <= CLOSED_MARKER_TTL);
        closed.insert(stream_id.to_string(), Instant::now());
        self.changed.notify_waiters();
    }

    /// True when `token` is still the registered token for `stream_id`.
    async fn is_current_sender(&self, stream_id: &str, token: u64) -> bool {
        self.streams
            .lock()
            .await
            .get(stream_id)
            .is_some_and(|(current, _)| *current == token)
    }

    /// Subscribe to an existing stream. Returns None when the stream is not
    /// registered (yet).
    pub async fn subscribe(
        &self,
        stream_id: &str,
    ) -> Option<broadcast::Receiver<String>> {
        self.streams
            .lock()
            .await
            .get(stream_id)
            .map(|(_, sender)| sender.subscribe())
    }

    /// Wait up to `timeout` for a stream to be registered, then subscribe.
    /// The frontend subscribes before invoking the command, so this bridges
    /// the registration race without dropping buffered events.
    ///
    /// Returns None immediately when the stream was already closed (the
    /// command finished before this subscriber connected), so the SSE
    /// endpoint can answer 404 instead of hanging for the full timeout.
    pub async fn wait_for_subscribe(
        &self,
        stream_id: &str,
        timeout: Duration,
    ) -> Option<broadcast::Receiver<String>> {
        let deadline = Instant::now() + timeout;
        loop {
            // Register for the wakeup *before* reading the registry. A
            // `Notified` future only joins the waiter list once polled, so
            // creating it after the check would let an `open()` that lands in
            // between go unnoticed and stall this subscriber until the
            // timeout. `enable()` joins the list immediately.
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();

            {
                let mut closed = self.closed.lock().await;
                if closed.contains_key(stream_id) {
                    return None;
                }
                if !closed.is_empty() {
                    closed.retain(|_, at| at.elapsed() <= CLOSED_MARKER_TTL);
                }
            }

            if let Some(receiver) = self.subscribe(stream_id).await {
                return Some(receiver);
            }

            tokio::select! {
                _ = changed => {},
                _ = tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)) => {
                    return None;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn wait_for_subscribe_returns_when_stream_registers() {
        let registry = StreamRegistry::new();
        let id = "stream-1";

        let wait = tokio::spawn({
            let registry = registry.clone();
            async move { registry.wait_for_subscribe(id, Duration::from_secs(5)).await }
        });
        tokio::time::sleep(Duration::from_millis(50)).await;
        let _sink = registry.open(id).await;

        let receiver = wait.await.expect("wait task").expect("stream found");
        assert!(receiver.is_empty());
    }

    #[tokio::test]
    async fn wait_for_subscribe_fails_fast_after_close() {
        let registry = StreamRegistry::new();
        let id = "stream-2";

        let sink = registry.open(id).await;
        registry.close(id).await;
        drop(sink);

        // The stream finished before any subscriber connected: fail fast
        // instead of waiting out the timeout.
        let start = Instant::now();
        let result = registry.wait_for_subscribe(id, Duration::from_secs(10)).await;
        assert!(result.is_none());
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[tokio::test]
    async fn open_clears_the_closed_marker() {
        let registry = StreamRegistry::new();
        let id = "stream-3";

        registry.open(id).await;
        registry.close(id).await;
        registry.open(id).await;

        let receiver = registry
            .wait_for_subscribe(id, Duration::from_millis(500))
            .await
            .expect("reopened stream is subscribable");
        assert!(receiver.is_empty());
    }

    #[tokio::test]
    async fn sink_sends_pre_serialized_text() {
        let registry = StreamRegistry::new();
        let id = "stream-text";
        let sink = registry.open(id).await;
        let mut receiver = registry
            .wait_for_subscribe(id, Duration::from_secs(1))
            .await
            .expect("subscribed receiver");

        assert!(sink.send(serde_json::json!({ "type": "chunk", "data": "hi" })));
        let text = tokio::time::timeout(Duration::from_secs(2), receiver.recv())
            .await
            .expect("event within timeout")
            .expect("event item");
        // Pre-serialized at `send` time: subscribers receive the JSON text,
        // never a `Value` to re-serialize.
        assert_eq!(text, r#"{"data":"hi","type":"chunk"}"#);
    }

    #[tokio::test]
    async fn dropped_sink_closes_an_abandoned_stream() {
        let registry = StreamRegistry::new();
        let id = "stream-abandoned";

        let sink = registry.open(id).await;
        // No explicit close: the drop guard must close the stream.
        drop(sink);
        tokio::time::sleep(Duration::from_millis(50)).await;

        let result = registry
            .wait_for_subscribe(id, Duration::from_millis(300))
            .await;
        assert!(result.is_none(), "abandoned stream must be closed by the sink guard");
    }

    #[tokio::test]
    async fn dropped_stale_sink_does_not_close_a_reopened_stream() {
        let registry = StreamRegistry::new();
        let id = "stream-reopen";

        let first_sink = registry.open(id).await;
        let second_sink = registry.open(id).await; // replaces the first
        drop(first_sink); // stale guard must not close the newer stream

        let mut receiver = registry
            .wait_for_subscribe(id, Duration::from_millis(300))
            .await
            .expect("reopened stream still registered");
        assert!(second_sink.send(serde_json::json!({ "type": "done" })));
        let text = tokio::time::timeout(Duration::from_secs(2), receiver.recv())
            .await
            .expect("event within timeout")
            .expect("event item");
        assert_eq!(text, r#"{"type":"done"}"#);
    }
}
