use std::sync::Arc;

use serde_json::Value;
use crate::app::AppState;
use crate::presentation::commands::helpers::{log_command, map_command_error};
use crate::presentation::errors::CommandError;
use tt_application::dto::chat_dto::ChatSearchResultDto;
use tt_ports::repositories::chat_repository::{
    ChatMessageSearchHit, ChatMessageSearchQuery, FindLastMessageQuery, LocatedChatMessage,
};

pub async fn get_character_chat_summary(
    character_name: String,
    file_name: String,
    include_metadata: Option<bool>,
    state: Arc<AppState>,
) -> Result<ChatSearchResultDto, CommandError> {
    log_command(format!(
        "get_character_chat_summary {}/{}",
        character_name, file_name
    ));

    state
        .services
        .chat_service
        .get_character_chat_summary(
            &character_name,
            &file_name,
            include_metadata.unwrap_or(false),
        )
        .await
        .map_err(map_command_error(format!(
            "Failed to get chat summary {}/{}",
            character_name, file_name
        )))
}

pub async fn get_character_chat_metadata(
    character_name: String,
    file_name: String,
    state: Arc<AppState>,
) -> Result<Value, CommandError> {
    log_command(format!(
        "get_character_chat_metadata {}/{}",
        character_name, file_name
    ));

    state
        .services
        .chat_service
        .get_character_chat_metadata(&character_name, &file_name)
        .await
        .map_err(map_command_error(format!(
            "Failed to get chat metadata {}/{}",
            character_name, file_name
        )))
}

pub async fn set_character_chat_metadata_extension(
    character_name: String,
    file_name: String,
    namespace: String,
    value: Value,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command(format!(
        "set_character_chat_metadata_extension {}/{}:{}",
        character_name, file_name, namespace
    ));

    state
        .services
        .chat_service
        .set_character_chat_metadata_extension(&character_name, &file_name, &namespace, value)
        .await
        .map_err(map_command_error(format!(
            "Failed to set chat metadata extension {}/{}:{}",
            character_name, file_name, namespace
        )))
}

pub async fn get_character_chat_store_json(
    character_name: String,
    file_name: String,
    namespace: String,
    key: String,
    state: Arc<AppState>,
) -> Result<Value, CommandError> {
    log_command(format!(
        "get_character_chat_store_json {}/{}:{}/{}",
        character_name, file_name, namespace, key
    ));

    state
        .services
        .chat_service
        .get_character_chat_store_json(&character_name, &file_name, &namespace, &key)
        .await
        .map_err(map_command_error(format!(
            "Failed to get chat store json {}/{}:{}/{}",
            character_name, file_name, namespace, key
        )))
}

pub async fn set_character_chat_store_json(
    character_name: String,
    file_name: String,
    namespace: String,
    key: String,
    value: Value,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command(format!(
        "set_character_chat_store_json {}/{}:{}/{}",
        character_name, file_name, namespace, key
    ));

    state
        .services
        .chat_service
        .set_character_chat_store_json(&character_name, &file_name, &namespace, &key, value)
        .await
        .map_err(map_command_error(format!(
            "Failed to set chat store json {}/{}:{}/{}",
            character_name, file_name, namespace, key
        )))
}

pub async fn update_character_chat_store_json(
    character_name: String,
    file_name: String,
    namespace: String,
    key: String,
    value: Value,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command(format!(
        "update_character_chat_store_json {}/{}:{}/{}",
        character_name, file_name, namespace, key
    ));

    state
        .services
        .chat_service
        .update_character_chat_store_json(&character_name, &file_name, &namespace, &key, value)
        .await
        .map_err(map_command_error(format!(
            "Failed to update chat store json {}/{}:{}/{}",
            character_name, file_name, namespace, key
        )))
}

pub async fn rename_character_chat_store_key(
    character_name: String,
    file_name: String,
    namespace: String,
    key: String,
    new_key: String,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command(format!(
        "rename_character_chat_store_key {}/{}:{}/{} -> {}",
        character_name, file_name, namespace, key, new_key
    ));

    state
        .services
        .chat_service
        .rename_character_chat_store_key(&character_name, &file_name, &namespace, &key, &new_key)
        .await
        .map_err(map_command_error(format!(
            "Failed to rename chat store key {}/{}:{}/{} -> {}",
            character_name, file_name, namespace, key, new_key
        )))
}

pub async fn delete_character_chat_store_json(
    character_name: String,
    file_name: String,
    namespace: String,
    key: String,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command(format!(
        "delete_character_chat_store_json {}/{}:{}/{}",
        character_name, file_name, namespace, key
    ));

    state
        .services
        .chat_service
        .delete_character_chat_store_json(&character_name, &file_name, &namespace, &key)
        .await
        .map_err(map_command_error(format!(
            "Failed to delete chat store json {}/{}:{}/{}",
            character_name, file_name, namespace, key
        )))
}

pub async fn list_character_chat_store_keys(
    character_name: String,
    file_name: String,
    namespace: String,
    state: Arc<AppState>,
) -> Result<Vec<String>, CommandError> {
    log_command(format!(
        "list_character_chat_store_keys {}/{}:{}",
        character_name, file_name, namespace
    ));

    state
        .services
        .chat_service
        .list_character_chat_store_keys(&character_name, &file_name, &namespace)
        .await
        .map_err(map_command_error(format!(
            "Failed to list chat store keys {}/{}:{}",
            character_name, file_name, namespace
        )))
}

pub async fn find_last_character_chat_message(
    character_name: String,
    file_name: String,
    query: FindLastMessageQuery,
    state: Arc<AppState>,
) -> Result<Option<LocatedChatMessage>, CommandError> {
    log_command(format!(
        "find_last_character_chat_message {}/{}",
        character_name, file_name
    ));

    state
        .services
        .chat_service
        .find_last_character_chat_message(&character_name, &file_name, query)
        .await
        .map_err(map_command_error(format!(
            "Failed to locate last chat message {}/{}",
            character_name, file_name
        )))
}

pub async fn search_character_chat_messages(
    character_name: String,
    file_name: String,
    query: ChatMessageSearchQuery,
    state: Arc<AppState>,
) -> Result<Vec<ChatMessageSearchHit>, CommandError> {
    log_command(format!(
        "search_character_chat_messages {}/{}",
        character_name, file_name
    ));

    state
        .services
        .chat_service
        .search_character_chat_messages(&character_name, &file_name, query)
        .await
        .map_err(map_command_error(format!(
            "Failed to search chat messages {}/{}",
            character_name, file_name
        )))
}
