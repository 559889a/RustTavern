use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use serde::Serialize;
use serde_json::Value;
use crate::app::AppState;
use crate::presentation::commands::helpers::{log_command, map_command_error};
use crate::presentation::errors::CommandError;

#[derive(Debug, Serialize)]
pub struct ExtensionStoreBlobPayload {
    pub content_base64: String,
    pub mime_type: String,
}

#[derive(Debug, Serialize)]
pub struct ExtensionStoreJsonLookupPayload {
    pub found: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
}

pub async fn get_extension_store_json(
    namespace: String,
    key: String,
    table: Option<String>,
    state: Arc<AppState>,
) -> Result<Value, CommandError> {
    log_command(format!(
        "get_extension_store_json {}:{}/{}",
        namespace,
        table.as_deref().unwrap_or("main"),
        key
    ));

    state
        .services
        .extension_store_service
        .get_json(&namespace, table.as_deref(), &key)
        .await
        .map_err(map_command_error(format!(
            "Failed to get extension store json {}:{}",
            namespace, key
        )))
}

pub async fn try_get_extension_store_json(
    namespace: String,
    key: String,
    table: Option<String>,
    state: Arc<AppState>,
) -> Result<ExtensionStoreJsonLookupPayload, CommandError> {
    log_command(format!(
        "try_get_extension_store_json {}:{}/{}",
        namespace,
        table.as_deref().unwrap_or("main"),
        key
    ));

    let value = state
        .services
        .extension_store_service
        .try_get_json(&namespace, table.as_deref(), &key)
        .await
        .map_err(map_command_error(format!(
            "Failed to try-get extension store json {}:{}",
            namespace, key
        )))?;

    Ok(ExtensionStoreJsonLookupPayload {
        found: value.is_some(),
        value,
    })
}

pub async fn set_extension_store_json(
    namespace: String,
    key: String,
    value: Value,
    table: Option<String>,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command(format!(
        "set_extension_store_json {}:{}/{}",
        namespace,
        table.as_deref().unwrap_or("main"),
        key
    ));

    state
        .services
        .extension_store_service
        .set_json(&namespace, table.as_deref(), &key, value)
        .await
        .map_err(map_command_error(format!(
            "Failed to set extension store json {}:{}",
            namespace, key
        )))
}

pub async fn update_extension_store_json(
    namespace: String,
    key: String,
    value: Value,
    table: Option<String>,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command(format!(
        "update_extension_store_json {}:{}/{}",
        namespace,
        table.as_deref().unwrap_or("main"),
        key
    ));

    state
        .services
        .extension_store_service
        .update_json(&namespace, table.as_deref(), &key, value)
        .await
        .map_err(map_command_error(format!(
            "Failed to update extension store json {}:{}",
            namespace, key
        )))
}

pub async fn rename_extension_store_key(
    namespace: String,
    key: String,
    new_key: String,
    table: Option<String>,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command(format!(
        "rename_extension_store_key {}:{}/{} -> {}",
        namespace,
        table.as_deref().unwrap_or("main"),
        key,
        new_key
    ));

    state
        .services
        .extension_store_service
        .rename_json_key(&namespace, table.as_deref(), &key, &new_key)
        .await
        .map_err(map_command_error(format!(
            "Failed to rename extension store key {}:{} -> {}",
            namespace, key, new_key
        )))
}

pub async fn delete_extension_store_json(
    namespace: String,
    key: String,
    table: Option<String>,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command(format!(
        "delete_extension_store_json {}:{}/{}",
        namespace,
        table.as_deref().unwrap_or("main"),
        key
    ));

    state
        .services
        .extension_store_service
        .delete_json(&namespace, table.as_deref(), &key)
        .await
        .map_err(map_command_error(format!(
            "Failed to delete extension store json {}:{}",
            namespace, key
        )))
}

pub async fn list_extension_store_keys(
    namespace: String,
    table: Option<String>,
    state: Arc<AppState>,
) -> Result<Vec<String>, CommandError> {
    log_command(format!(
        "list_extension_store_keys {}:{}",
        namespace,
        table.as_deref().unwrap_or("main")
    ));

    state
        .services
        .extension_store_service
        .list_json_keys(&namespace, table.as_deref())
        .await
        .map_err(map_command_error(format!(
            "Failed to list extension store keys {}",
            namespace
        )))
}

pub async fn list_extension_store_tables(
    namespace: String,
    state: Arc<AppState>,
) -> Result<Vec<String>, CommandError> {
    log_command(format!("list_extension_store_tables {}", namespace));

    state
        .services
        .extension_store_service
        .list_tables(&namespace)
        .await
        .map_err(map_command_error(format!(
            "Failed to list extension store tables {}",
            namespace
        )))
}

pub async fn delete_extension_store_table(
    namespace: String,
    table: String,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command(format!(
        "delete_extension_store_table {}:{}",
        namespace, table
    ));

    state
        .services
        .extension_store_service
        .delete_table(&namespace, &table)
        .await
        .map_err(map_command_error(format!(
            "Failed to delete extension store table {}:{}",
            namespace, table
        )))
}

pub async fn get_extension_store_blob(
    namespace: String,
    key: String,
    table: Option<String>,
    state: Arc<AppState>,
) -> Result<ExtensionStoreBlobPayload, CommandError> {
    log_command(format!(
        "get_extension_store_blob {}:{}/{}",
        namespace,
        table.as_deref().unwrap_or("main"),
        key
    ));

    let bytes = state
        .services
        .extension_store_service
        .get_blob(&namespace, table.as_deref(), &key)
        .await
        .map_err(map_command_error(format!(
            "Failed to get extension store blob {}:{}",
            namespace, key
        )))?;

    let mime_type = mime_guess::from_path(&key)
        .first_or_octet_stream()
        .essence_str()
        .to_string();

    Ok(ExtensionStoreBlobPayload {
        content_base64: BASE64_STANDARD.encode(bytes),
        mime_type,
    })
}

pub async fn set_extension_store_blob(
    namespace: String,
    key: String,
    data_base64: String,
    table: Option<String>,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command(format!(
        "set_extension_store_blob {}:{}/{}",
        namespace,
        table.as_deref().unwrap_or("main"),
        key
    ));

    let data_base64 = data_base64.trim().to_string();
    if data_base64.is_empty() {
        return Err(CommandError::BadRequest(
            "No blob data specified".to_string(),
        ));
    }

    let bytes = BASE64_STANDARD
        .decode(data_base64.as_bytes())
        .map_err(|error| CommandError::BadRequest(format!("Invalid base64: {}", error)))?;

    state
        .services
        .extension_store_service
        .set_blob(&namespace, table.as_deref(), &key, bytes)
        .await
        .map_err(map_command_error(format!(
            "Failed to set extension store blob {}:{}",
            namespace, key
        )))
}

pub async fn delete_extension_store_blob(
    namespace: String,
    key: String,
    table: Option<String>,
    state: Arc<AppState>,
) -> Result<(), CommandError> {
    log_command(format!(
        "delete_extension_store_blob {}:{}/{}",
        namespace,
        table.as_deref().unwrap_or("main"),
        key
    ));

    state
        .services
        .extension_store_service
        .delete_blob(&namespace, table.as_deref(), &key)
        .await
        .map_err(map_command_error(format!(
            "Failed to delete extension store blob {}:{}",
            namespace, key
        )))
}

pub async fn list_extension_store_blob_keys(
    namespace: String,
    table: Option<String>,
    state: Arc<AppState>,
) -> Result<Vec<String>, CommandError> {
    log_command(format!(
        "list_extension_store_blob_keys {}:{}",
        namespace,
        table.as_deref().unwrap_or("main")
    ));

    state
        .services
        .extension_store_service
        .list_blob_keys(&namespace, table.as_deref())
        .await
        .map_err(map_command_error(format!(
            "Failed to list extension store blob keys {}",
            namespace
        )))
}
