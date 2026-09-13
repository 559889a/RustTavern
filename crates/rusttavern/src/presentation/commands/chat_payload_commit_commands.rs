use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::app::AppState;
use crate::presentation::errors::CommandError;
use tt_application::dto::chat_history_dto::{ChatHistoryLocator, CurrentCommitReason};
use tt_ports::repositories::chat_payload_commit_repository::ChatPayloadBaseline;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BeginChatCommitResult {
    session_id: String,
    max_frame_bytes: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatPayloadBaselineArgs {
    size: u64,
    mtime_sec: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FinishChatCommitResult {
    size: u64,
    mtime_sec: u64,
}

pub async fn begin_chat_commit(
    target: ChatHistoryLocator,
    force: bool,
    baseline: Option<ChatPayloadBaselineArgs>,
    state: Arc<AppState>,
) -> Result<BeginChatCommitResult, CommandError> {
    let baseline = baseline.map(|args| ChatPayloadBaseline {
        size: args.size,
        mtime_sec: args.mtime_sec,
    });
    let session = state
        .services
        .chat_payload_commit_service
        .begin(target, force, baseline)
        .await?;

    Ok(BeginChatCommitResult {
        session_id: session.session_id,
        max_frame_bytes: session.max_frame_bytes,
    })
}

/// Append a base64-encoded JSON chunk. The `data` field is base64-decoded by
/// the generic dispatch path (see `chunk_body::decode_base64_chunk`).
pub async fn append_chat_commit_chunk(
    session_id: String,
    offset: u64,
    data: String,
    state: Arc<AppState>,
) -> Result<u64, CommandError> {
    let bytes = crate::presentation::commands::chunk_body::decode_base64_chunk(&data)?;

    state
        .services
        .chat_payload_commit_service
        .append(&session_id, offset, &bytes)
        .await
        .map_err(Into::into)
}

pub async fn finish_chat_commit(
    session_id: String,
    expected_size: u64,
    commit_reason: CurrentCommitReason,
    state: Arc<AppState>,
) -> Result<FinishChatCommitResult, CommandError> {
    let committed = state
        .services
        .chat_payload_commit_service
        .finish(&session_id, expected_size, commit_reason)
        .await?;

    Ok(FinishChatCommitResult {
        size: committed.size,
        mtime_sec: committed.mtime_sec,
    })
}

pub async fn abort_chat_commit(
    session_id: String,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    state
        .services
        .chat_payload_commit_service
        .abort(&session_id)
        .await
        .map_err(Into::into)
}
