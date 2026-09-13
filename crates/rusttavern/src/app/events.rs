//! Server-side event bus.
//!
//! The single backend→frontend event channel. Backend subsystems publish named
//! events; the HTTP layer exposes them to browsers through the `/__tt/events`
//! SSE endpoint (see `crate::server::events`).

use serde::{Deserialize, Serialize};

/// Maximum number of buffered events per subscriber before the slowest
/// subscriber starts dropping events (broadcast channel semantics).
const EVENT_BUS_CAPACITY: usize = 512;

/// Host-contract event: the server is shutting down gracefully; the frontend
/// should flush pending state (lifecycle flush service) before the process
/// exits.
pub const GRACEFUL_EXIT_EVENT: &str = "rusttavern-graceful-exit-requested";

/// One named event delivered to subscribers. The payload is pre-serialized
/// once at publish time so N subscribers never re-serialize the same value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMessage {
    pub event: String,
    pub payload: String,
}

/// A tokio broadcast-based event bus. Subscribers that are too slow will miss
/// events (broadcast `Lagged`), which is the documented trade-off for live
/// observability events; durable state always lives in repositories, never in
/// the bus.
#[derive(Clone)]
pub struct EventBus {
    sender: tokio::sync::broadcast::Sender<EventMessage>,
}

impl EventBus {
    pub fn new() -> Self {
        let (sender, _) = tokio::sync::broadcast::channel(EVENT_BUS_CAPACITY);
        Self { sender }
    }

    /// Publish an event to all current subscribers. The payload is serialized
    /// exactly once; every subscriber receives the pre-serialized text.
    pub fn publish(&self, event: impl Into<String>, payload: impl Serialize) {
        let payload = match serde_json::to_string(&payload) {
            Ok(text) => text,
            Err(error) => {
                tracing::error!(
                    "Failed to serialize event '{}' payload: {}",
                    event.into(),
                    error
                );
                return;
            }
        };
        let message = EventMessage {
            event: event.into(),
            payload,
        };
        let _ = self.sender.send(message);
    }

    /// Subscribe to the event stream. The returned receiver starts receiving
    /// events published after this call.
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<EventMessage> {
        self.sender.subscribe()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
