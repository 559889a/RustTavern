use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;

use crate::app::AppState;
use crate::presentation::commands::helpers::{
    log_command, log_user_visible_error, map_command_error,
};
use crate::presentation::errors::CommandError;
use crate::server::stream::StreamSink;
use tt_application::dto::chat_completion_dto::{
    ChatCompletionGenerateRequestDto, ChatCompletionStatusRequestDto,
};
use tt_application::services::chat_completion_service::ChatCompletionService;
use tt_domain::models::upstream_failure::UpstreamFailure;

/// Removes the generation registry entry when the handler future is dropped.
///
/// A client disconnect cancels the axum future before the explicit
/// `complete_generation` call runs; without this guard the registry entry
/// would linger until the server restarts.
struct GenerationGuard {
    service: Arc<ChatCompletionService>,
    request_id: String,
    done: bool,
}

impl GenerationGuard {
    fn new(service: Arc<ChatCompletionService>, request_id: String) -> Self {
        Self {
            service,
            request_id,
            done: false,
        }
    }

    fn finish(&mut self) {
        if !self.done {
            self.service.complete_generation(&self.request_id);
            self.done = true;
        }
    }
}

impl Drop for GenerationGuard {
    fn drop(&mut self) {
        self.finish();
    }
}

pub async fn get_chat_completions_status(
    dto: ChatCompletionStatusRequestDto,
    state: Arc<AppState>,
) -> Result<Value, CommandError> {
    log_command("get_chat_completions_status");

    state
        .services
        .chat_completion_service
        .get_status(dto)
        .await
        .map_err(map_command_error("Failed to get chat completions status"))
}

pub async fn generate_chat_completion(
    dto: ChatCompletionGenerateRequestDto,
    request_id: String,
    state: Arc<AppState>,
) -> Result<Value, CommandError> {
    let request_id = request_id.trim().to_string();
    validate_stream_id(&request_id)?;
    log_command(format!("generate_chat_completion {}", request_id));

    let service = state.services.chat_completion_service.clone();
    let cancel = service.register_generation(&request_id);
    let mut guard = GenerationGuard::new(service.clone(), request_id);
    let result = service.generate_with_cancel(dto, cancel).await;
    guard.finish();

    result.map_err(map_command_error("Failed to generate chat completion"))
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ChatCompletionStreamEvent {
    Chunk {
        data: String,
    },
    Done,
    Error {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        details: Option<UpstreamFailure>,
    },
}

/// Starts a streaming generation. Events are published to the global stream
/// registry under `stream_id` and consumed by `GET /__tt/stream/{stream_id}`
/// (SSE). The old `Channel` parameter is gone; the frontend subscribes to the
/// SSE endpoint instead.
pub async fn start_chat_completion_stream(
    stream_id: String,
    dto: ChatCompletionGenerateRequestDto,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    validate_stream_id(&stream_id)?;
    log_command(format!("start_chat_completion_stream {}", stream_id));

    let service = state.services.chat_completion_service.clone();
    let cancel = service.register_stream(&stream_id);
    let sink = state.host.streams.open(&stream_id).await;

    tokio::spawn(run_stream_generation(
        service,
        state.host.streams.clone(),
        stream_id,
        dto,
        cancel,
        sink,
    ));

    Ok(())
}

pub async fn cancel_chat_completion_stream(
    stream_id: String,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    validate_stream_id(&stream_id)?;
    log_command(format!("cancel_chat_completion_stream {}", stream_id));

    state
        .services
        .chat_completion_service
        .cancel_stream(&stream_id);
    Ok(())
}

pub async fn cancel_chat_completion_generation(
    request_id: String,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    validate_stream_id(&request_id)?;
    log_command(format!("cancel_chat_completion_generation {}", request_id));

    state
        .services
        .chat_completion_service
        .cancel_generation(&request_id);
    Ok(())
}

async fn run_stream_generation(
    service: Arc<ChatCompletionService>,
    streams: Arc<crate::server::stream::StreamRegistry>,
    stream_id: String,
    dto: ChatCompletionGenerateRequestDto,
    cancel: tokio::sync::watch::Receiver<bool>,
    on_event: StreamSink,
) {
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<String>();
    let generation_task = tokio::spawn({
        let service = service.clone();
        async move { service.generate_stream(dto, sender, cancel).await }
    });

    while let Some(chunk) = receiver.recv().await {
        if chunk.is_empty() {
            continue;
        }

        let emit_result = on_event.send(ChatCompletionStreamEvent::Chunk { data: chunk });

        if !emit_result {
            generation_task.abort();
            service.complete_stream(&stream_id);
            streams.close(&stream_id).await;
            return;
        }
    }

    let generation_result = match generation_task.await {
        Ok(result) => result,
        Err(error) => Err(tt_application::errors::ApplicationError::InternalError(
            format!("Streaming task join failed: {error}"),
        )),
    };

    service.complete_stream(&stream_id);

    match generation_result {
        Ok(()) => {
            let _ = on_event.send(ChatCompletionStreamEvent::Done);
        }
        Err(error) => {
            let command_error = CommandError::from(error);
            let details = command_error.upstream_failure().cloned();
            let message = command_error.to_string();
            if !on_event.send(ChatCompletionStreamEvent::Error {
                message,
                details,
            }) {
                // No stream subscriber attached (e.g. the request failed
                // before the frontend subscribed): without this the failure
                // is completely silent — the stream call already returned Ok.
                log_user_visible_error(&format!(
                    "Chat completion stream failed: {command_error}"
                ));
            }
        }
    }

    streams.close(&stream_id).await;
}

fn validate_stream_id(stream_id: &str) -> Result<(), CommandError> {
    let stream_id = stream_id.trim();
    if stream_id.is_empty() || stream_id.len() > 128 {
        return Err(CommandError::BadRequest(
            "Invalid stream id length".to_string(),
        ));
    }

    if !stream_id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err(CommandError::BadRequest(
            "Invalid stream id characters".to_string(),
        ));
    }

    Ok(())
}
