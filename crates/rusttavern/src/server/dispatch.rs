//! Command dispatch table.
//!
//! Replaces Tauri's `generate_handler!` with a plain registry keyed by command
//! name. Commands keep their exact signatures (`state: Arc<AppState>` last)
//! but drop the `#[tauri::command]` attribute; the `register!` macros generate
//! the JSON-arg extraction (camelCase/snake_case compatible, matching the old
//! `withTauriArgumentAliases` behavior in host-bridge.js) and result
//! serialization.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::app::AppState;
use crate::presentation::errors::CommandError;

/// Command result: the serialized JSON body (serialized exactly once by
/// `serialize_result`) or a command error.
pub type CommandResult = Result<Vec<u8>, CommandError>;

/// Async command handler. Receives the shared app state and the JSON args
/// object, returns the serialized result.
pub type AsyncCommandHandler = fn(
    Arc<AppState>,
    &Value,
) -> Pin<Box<dyn Future<Output = CommandResult> + Send + '_>>;

/// Sync command handler (no async work needed).
pub type SyncCommandHandler = fn(Arc<AppState>, &Value) -> CommandResult;

enum CommandEntry {
    Async(AsyncCommandHandler),
    Sync(SyncCommandHandler),
}

/// The command registry. Built once at startup from `build_registry()`.
pub struct CommandRegistry {
    commands: HashMap<String, CommandEntry>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
        }
    }

    pub fn register_async(&mut self, name: &str, handler: AsyncCommandHandler) {
        self.commands.insert(name.to_string(), CommandEntry::Async(handler));
    }

    pub fn register_sync(&mut self, name: &str, handler: SyncCommandHandler) {
        self.commands.insert(name.to_string(), CommandEntry::Sync(handler));
    }

    /// Invoke a command with a JSON args object.
    pub async fn invoke(&self, name: &str, state: Arc<AppState>, args: &Value) -> CommandResult {
        match self.commands.get(name) {
            Some(CommandEntry::Async(handler)) => handler(state, args).await,
            Some(CommandEntry::Sync(handler)) => handler(state, args),
            None => Err(CommandError::NotFound(format!(
                "Command not found: {name}"
            ))),
        }
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract a command argument from the JSON args object. Keys are matched
/// camelCase first, then snake_case (and the exact key), mirroring the old
/// frontend alias behavior. A missing key deserializes as `null`, which lets
/// `Option<T>` parameters be omitted while required parameters fail loudly.
pub fn extract_arg<T: DeserializeOwned>(args: &Value, name: &str) -> Result<T, CommandError> {
    let value = lookup_arg(args, name);
    // Borrowed deserialization: `&Value` implements `Deserializer`, so no
    // deep clone of the argument subtree is needed.
    T::deserialize(value).map_err(|error| {
        CommandError::BadRequest(format!("Invalid argument '{name}': {error}"))
    })
}

fn lookup_arg<'a>(args: &'a Value, name: &str) -> &'a Value {
    let Value::Object(map) = args else {
        return &Value::Null;
    };

    if let Some(value) = map.get(name) {
        return value;
    }

    let camel_case = to_camel_case(name);
    if let Some(value) = map.get(&camel_case) {
        return value;
    }

    &Value::Null
}

fn to_camel_case(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut upper_next = false;
    for ch in name.chars() {
        if ch == '_' {
            upper_next = true;
        } else if upper_next {
            out.extend(ch.to_uppercase());
            upper_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}

/// Serialize a command result directly to JSON bytes (a single pass, no
/// intermediate `Value`), mapping serialization failures to a command error
/// instead of panicking at the boundary.
pub fn serialize_result<T: serde::Serialize>(value: T) -> CommandResult {
    serde_json::to_vec(&value).map_err(|error| {
        CommandError::InternalServerError(format!(
            "Failed to serialize command result: {error}"
        ))
    })
}

/// Unwrap a command's `Result<T, CommandError>` into the wire contract the
/// frontend expects: success serializes the bare payload (no `{"Ok": ...}`
/// envelope), failure propagates the `CommandError` so the router maps it to
/// the legacy HTTP status + `{"BadRequest": ...}` error body.
pub fn finish_result<T: serde::Serialize>(
    result: Result<T, CommandError>,
) -> CommandResult {
    match result {
        Ok(value) => serialize_result(value),
        Err(error) => Err(error),
    }
}

/// Async command registration.
///
/// ```ignore
/// register!(registry, get_character, state: name);        // with app state
/// register!(registry, devlog_append_frontend_logs, entries); // stateless
/// register!(registry, plugin_dialog_open, name: "plugin:dialog|open"); // explicit name
/// ```
///
/// Commands take `state: Arc<AppState>` as their LAST parameter. The `state:`
/// prefix marks it in the macro invocation (the token is unambiguous, unlike
/// a bare ident). Expands to: look up each argument in the JSON args
/// (camelCase/snake_case compatible), call the command, serialize the result.
#[macro_export]
macro_rules! register {
    ($registry:expr, $command:path, name: $name:expr, state: $($arg:ident),* $(,)?) => {{
        let handler: $crate::server::dispatch::AsyncCommandHandler =
            |state: std::sync::Arc<$crate::app::AppState>, args: &serde_json::Value| {
                Box::pin(async move {
                    $crate::server::dispatch::touch_args(args);
                    $(
                        let $arg = $crate::server::dispatch::extract_arg(args, stringify!($arg))?;
                    )*
                    let result = $command($($arg,)* state).await;
                    $crate::server::dispatch::finish_result(result)
                })
            };
        $registry.register_async($name, handler);
    }};
    ($registry:expr, $command:path, name: $name:expr $(, $arg:ident)* $(,)?) => {{
        let handler: $crate::server::dispatch::AsyncCommandHandler =
            |_state: std::sync::Arc<$crate::app::AppState>, args: &serde_json::Value| {
                Box::pin(async move {
                    $crate::server::dispatch::touch_args(args);
                    $(
                        let $arg = $crate::server::dispatch::extract_arg(args, stringify!($arg))?;
                    )*
                    let result = $command($($arg),*).await;
                    $crate::server::dispatch::finish_result(result)
                })
            };
        $registry.register_async($name, handler);
    }};
    ($registry:expr, $command:path, state: $($arg:ident),* $(,)?) => {{
        let name = $crate::server::dispatch::command_name(stringify!($command));
        let handler: $crate::server::dispatch::AsyncCommandHandler =
            |state: std::sync::Arc<$crate::app::AppState>, args: &serde_json::Value| {
                Box::pin(async move {
                    $crate::server::dispatch::touch_args(args);
                    $(
                        let $arg = $crate::server::dispatch::extract_arg(args, stringify!($arg))?;
                    )*
                    let result = $command($($arg,)* state).await;
                    $crate::server::dispatch::finish_result(result)
                })
            };
        $registry.register_async(&name, handler);
    }};
    ($registry:expr, $command:path $(, $arg:ident)* $(,)?) => {{
        let name = $crate::server::dispatch::command_name(stringify!($command));
        let handler: $crate::server::dispatch::AsyncCommandHandler =
            |_state: std::sync::Arc<$crate::app::AppState>, args: &serde_json::Value| {
                Box::pin(async move {
                    $crate::server::dispatch::touch_args(args);
                    $(
                        let $arg = $crate::server::dispatch::extract_arg(args, stringify!($arg))?;
                    )*
                    let result = $command($($arg),*).await;
                    $crate::server::dispatch::finish_result(result)
                })
            };
        $registry.register_async(&name, handler);
    }};
}

/// Sync command registration.
///
/// ```ignore
/// register_sync!(registry, get_version);                          // stateless
/// register_sync!(registry, get_runtime_paths, state:);            // state only
/// register_sync!(registry, emit_event, state: event_type, data);
/// ```
#[macro_export]
macro_rules! register_sync {
    ($registry:expr, $command:path, state: $($arg:ident),* $(,)?) => {{
        let name = $crate::server::dispatch::command_name(stringify!($command));
        let handler: $crate::server::dispatch::SyncCommandHandler =
            |state: std::sync::Arc<$crate::app::AppState>, args: &serde_json::Value| {
                $crate::server::dispatch::touch_args(args);
                $(
                    let $arg = $crate::server::dispatch::extract_arg(args, stringify!($arg))?;
                )*
                let result = $command($($arg,)* state);
                $crate::server::dispatch::finish_result(result)
            };
        $registry.register_sync(&name, handler);
    }};
    ($registry:expr, $command:path $(, $arg:ident)* $(,)?) => {{
        let name = $crate::server::dispatch::command_name(stringify!($command));
        let handler: $crate::server::dispatch::SyncCommandHandler =
            |_state: std::sync::Arc<$crate::app::AppState>, args: &serde_json::Value| {
                $crate::server::dispatch::touch_args(args);
                $(
                    let $arg = $crate::server::dispatch::extract_arg(args, stringify!($arg))?;
                )*
                let result = $command($($arg),*);
                $crate::server::dispatch::finish_result(result)
            };
        $registry.register_sync(&name, handler);
    }};
}

/// Suppress unused-parameter warnings for commands without JSON args.
pub fn touch_args(_args: &serde_json::Value) {}

/// Extract the bare function name from a `module::path::command` string.
pub fn command_name(path: &str) -> String {
    path.rsplit("::")
        .next()
        .unwrap_or(path)
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::errors::CommandError;

    #[test]
    fn command_name_strips_module_path() {
        assert_eq!(
            command_name("super::character_commands::get_all_characters"),
            "get_all_characters"
        );
        assert_eq!(command_name("get_version"), "get_version");
    }

    #[test]
    fn extract_arg_matches_camel_and_snake_case() {
        let args = serde_json::json!({
            "requestId": "abc",
            "snake_value": 42,
        });

        let request_id: String = extract_arg(&args, "request_id").expect("camelCase alias");
        assert_eq!(request_id, "abc");

        let snake_value: u64 = extract_arg(&args, "snake_value").expect("snake_case direct");
        assert_eq!(snake_value, 42);
    }

    #[test]
    fn extract_arg_missing_key_deserializes_null() {
        let args = serde_json::json!({});
        let optional: Option<String> = extract_arg(&args, "missing").expect("null option");
        assert!(optional.is_none());
    }

    #[test]
    fn extract_arg_invalid_value_fails() {
        let args = serde_json::json!({ "count": "not-a-number" });
        let error = extract_arg::<u64>(&args, "count").expect_err("invalid number");
        assert!(matches!(error, CommandError::BadRequest(_)));
    }

    #[test]
    fn to_camel_case_converts_snake() {
        assert_eq!(to_camel_case("file_path"), "filePath");
        assert_eq!(to_camel_case("alreadyCamel"), "alreadyCamel");
        assert_eq!(to_camel_case("a_b_c"), "aBC");
    }

    #[test]
    fn serialize_result_emits_json_bytes_directly() {
        let expected = serde_json::json!({ "ok": true, "n": 42, "list": [1, 2, 3] });
        let bytes = serialize_result(expected.clone()).expect("serialize");
        // Single-pass serialization: the result is already the final body,
        // and it parses back to the identical value.
        let parsed: serde_json::Value = serde_json::from_slice(&bytes).expect("parse body");
        assert_eq!(parsed, expected);
    }

    #[test]
    fn finish_result_unwraps_ok_without_envelope() {
        let result: Result<serde_json::Value, CommandError> =
            Ok(serde_json::json!({ "value": 7 }));
        let bytes = finish_result(result).expect("serialize ok");
        let parsed: serde_json::Value = serde_json::from_slice(&bytes).expect("parse body");
        // The wire contract is the bare payload — no `{"Ok": ...}` envelope.
        assert_eq!(parsed, serde_json::json!({ "value": 7 }));
    }

    #[test]
    fn finish_result_propagates_command_error() {
        let result: Result<serde_json::Value, CommandError> =
            Err(CommandError::NotFound("missing".to_string()));
        let error = finish_result(result).expect_err("propagate error");
        assert!(matches!(error, CommandError::NotFound(message) if message == "missing"));
    }
}
