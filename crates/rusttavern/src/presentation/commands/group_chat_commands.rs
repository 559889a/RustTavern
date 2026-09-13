use std::sync::Arc;

use crate::app::AppState;
use crate::presentation::commands::helpers::{log_command, log_command_lazy, map_command_error};
use crate::presentation::errors::CommandError;
use crate::server::fs_resources::validate_server_path;
use tt_application::dto::chat_dto::{
    ChatSearchResultDto, DeleteGroupChatDto, ImportGroupChatDto, PinnedGroupChatDto,
    RenameGroupChatDto, RestoreGroupChatBackupDto,
};
use tt_application::errors::ApplicationError;
use tt_ports::repositories::chat_types::{ChatPayloadChunk, ChatPayloadCursor, ChatPayloadTail};

pub async fn list_group_chat_summaries(
    chat_ids: Option<Vec<String>>,
    include_metadata: Option<bool>,
    state: Arc<AppState>,
) -> Result<Vec<ChatSearchResultDto>, CommandError> {
    log_command("list_group_chat_summaries");

    state
        .services
        .group_chat_service
        .list_group_chat_summaries(chat_ids.as_deref(), include_metadata.unwrap_or(false))
        .await
        .map_err(map_command_error("Failed to list group chat summaries"))
}

pub async fn list_recent_group_chat_summaries(
    chat_ids: Option<Vec<String>>,
    include_metadata: Option<bool>,
    max_entries: Option<usize>,
    pinned: Option<Vec<PinnedGroupChatDto>>,
    state: Arc<AppState>,
) -> Result<Vec<ChatSearchResultDto>, CommandError> {
    log_command("list_recent_group_chat_summaries");
    let pinned = pinned.unwrap_or_default();
    let pinned_refs = pinned.into_iter().map(Into::into).collect::<Vec<_>>();

    state
        .services
        .group_chat_service
        .list_recent_group_chat_summaries(
            chat_ids.as_deref(),
            include_metadata.unwrap_or(false),
            max_entries.unwrap_or(usize::MAX),
            &pinned_refs,
        )
        .await
        .map_err(map_command_error(
            "Failed to list recent group chat summaries",
        ))
}

pub async fn search_group_chats(
    query: String,
    chat_ids: Option<Vec<String>>,
    state: Arc<AppState>,
) -> Result<Vec<ChatSearchResultDto>, CommandError> {
    log_command(format!("search_group_chats {}", query));

    state
        .services
        .group_chat_service
        .search_group_chats(&query, chat_ids.as_deref())
        .await
        .map_err(map_command_error("Failed to search group chats"))
}

pub async fn get_group_chat_path(
    id: String,
    allow_not_found: Option<bool>,
    state: Arc<AppState>,
) -> Result<String, CommandError> {
    log_command(format!("get_group_chat_path {}", id));

    let allow_not_found = allow_not_found.unwrap_or(false);
    match state
        .services
        .group_chat_service
        .get_group_chat_payload_path(&id)
        .await
    {
        Ok(path) => Ok(path),
        Err(ApplicationError::NotFound(_)) if allow_not_found => Ok(String::new()),
        Err(error) => Err(map_command_error(format!(
            "Failed to get group chat payload path {}",
            id
        ))(error)),
    }
}

pub async fn get_group_chat_payload_tail(
    id: String,
    max_lines: usize,
    allow_not_found: Option<bool>,
    state: Arc<AppState>,
) -> Result<ChatPayloadTail, CommandError> {
    // Lazy detail: this command polls chat tails during chat switches.
    log_command_lazy("get_group_chat_payload_tail", || id.clone());

    let allow_not_found = allow_not_found.unwrap_or(false);
    match state
        .services
        .group_chat_service
        .get_group_chat_payload_tail_lines(&id, max_lines)
        .await
    {
        Ok(result) => Ok(result),
        Err(ApplicationError::NotFound(_)) if allow_not_found => Ok(ChatPayloadTail {
            header: String::new(),
            lines: Vec::new(),
            cursor: ChatPayloadCursor {
                offset: 0,
                size: 0,
                modified_millis: 0,
            },
            has_more_before: false,
        }),
        Err(error) => Err(map_command_error(format!(
            "Failed to get group chat payload tail {}",
            id
        ))(error)),
    }
}

pub async fn get_group_chat_payload_before(
    id: String,
    cursor: ChatPayloadCursor,
    max_lines: usize,
    state: Arc<AppState>,
) -> Result<ChatPayloadChunk, CommandError> {
    log_command(format!("get_group_chat_payload_before {}", id));

    state
        .services
        .group_chat_service
        .get_group_chat_payload_before_lines(&id, cursor, max_lines)
        .await
        .map_err(map_command_error(format!(
            "Failed to get group chat payload before {}",
            id
        )))
}

pub async fn get_group_chat_payload_before_pages(
    id: String,
    cursor: ChatPayloadCursor,
    max_lines: usize,
    max_pages: usize,
    state: Arc<AppState>,
) -> Result<Vec<ChatPayloadChunk>, CommandError> {
    log_command(format!("get_group_chat_payload_before_pages {}", id));

    state
        .services
        .group_chat_service
        .get_group_chat_payload_before_pages_lines(&id, cursor, max_lines, max_pages)
        .await
        .map_err(map_command_error(format!(
            "Failed to get group chat payload before pages {}",
            id
        )))
}

pub async fn delete_group_chat(
    dto: DeleteGroupChatDto,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command(format!("delete_group_chat {}", dto.id));

    state
        .services
        .group_chat_service
        .delete_group_chat(dto)
        .await
        .map_err(map_command_error("Failed to delete group chat payload"))
}

pub async fn rename_group_chat(
    dto: RenameGroupChatDto,
    state: Arc<AppState>,
) -> Result<String, CommandError> {
    log_command(format!(
        "rename_group_chat {} -> {}",
        dto.old_file_name, dto.new_file_name
    ));

    state
        .services
        .group_chat_service
        .rename_group_chat(dto)
        .await
        .map_err(map_command_error("Failed to rename group chat payload"))
}

pub async fn import_group_chat_payload(
    dto: ImportGroupChatDto,
    state: Arc<AppState>,
) -> Result<String, CommandError> {
    log_command("import_group_chat_payload");

    // Client-supplied path: must be a staged upload (see import_chat).
    validate_server_path(&dto.file_path, &state.host.data_root).await?;

    state
        .services
        .group_chat_service
        .import_group_chat(dto)
        .await
        .map_err(map_command_error("Failed to import group chat payload"))
}

pub async fn restore_group_chat_backup(
    dto: RestoreGroupChatBackupDto,
    state: Arc<AppState>,
) -> Result<String, CommandError> {
    log_command(format!("restore_group_chat_backup {}", dto.backup_name));

    state
        .services
        .group_chat_service
        .restore_group_chat_backup(dto)
        .await
        .map_err(map_command_error("Failed to restore group chat backup"))
}
