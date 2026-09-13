use serde_json::{Map, Value};

use super::content_parts::openai_chat_content_to_lossy_text;
use super::prompt_post_processing::PromptNames;

/// Label a message's `name` should contribute to the prompt text.
///
/// Providers with no `name` field need the label folded into the text. `example_user` /
/// `example_assistant` are internal role markers rather than display names, so they must be replaced
/// by the persona / character name; forwarding them verbatim leaks `example_user:` to the model.
/// Returns `None` when nothing should be prefixed.
pub(super) fn resolve_prompt_name_label<'a>(
    name: Option<&'a str>,
    names: &'a PromptNames,
) -> Option<PromptNameLabel<'a>> {
    let name = name?.trim();
    if name.is_empty() {
        return None;
    }
    match name {
        "example_user" => Some(names.user_name.as_str())
            .filter(|label| !label.is_empty())
            .map(|label| PromptNameLabel {
                label,
                skip_on_group_name: false,
            }),
        "example_assistant" => Some(names.char_name.as_str())
            .filter(|label| !label.is_empty())
            .map(|label| PromptNameLabel {
                label,
                // Upstream skips the character label when the text already opens with a group
                // member's name.
                skip_on_group_name: true,
            }),
        other => Some(PromptNameLabel {
            label: other,
            skip_on_group_name: false,
        }),
    }
}

#[derive(Clone, Copy)]
pub(super) struct PromptNameLabel<'a> {
    pub(super) label: &'a str,
    pub(super) skip_on_group_name: bool,
}

impl PromptNameLabel<'_> {
    pub(super) fn apply(&self, text: &str, names: &PromptNames) -> String {
        let label = self.label.trim();
        if label.is_empty() {
            return text.to_string();
        }
        let prefix = format!("{label}: ");
        if text.starts_with(&prefix)
            || (self.skip_on_group_name && names.starts_with_group_name(text))
        {
            return text.to_string();
        }
        format!("{prefix}{text}")
    }
}

/// Fold `name` into `text`, keeping it unchanged when there is nothing to prefix.
pub(super) fn prefix_prompt_name(text: &str, name: Option<&str>, names: &PromptNames) -> String {
    match resolve_prompt_name_label(name, names) {
        Some(label) => label.apply(text, names),
        None => text.to_string(),
    }
}

/// Same as [`prefix_prompt_name`], but only substitutes the `example_*` role markers.
///
/// Used where upstream folds the example labels but leaves an ordinary `name` alone, such as the
/// leading system run that becomes a provider `system_instruction`.
pub(super) fn prefix_prompt_example_name(
    text: &str,
    name: Option<&str>,
    names: &PromptNames,
) -> String {
    let name = name.filter(|value| matches!(*value, "example_user" | "example_assistant"));
    prefix_prompt_name(text, name, names)
}

pub(super) fn message_name(message: &Map<String, Value>) -> Option<&str> {
    message
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub(super) fn add_assistant_prefix(messages: &mut [Value], property: &str) {
    let Some(last_message) = messages.last_mut().and_then(Value::as_object_mut) else {
        return;
    };

    if last_message.get("role").and_then(Value::as_str) != Some("assistant") {
        return;
    }

    last_message.insert(property.to_string(), Value::Bool(true));
}

pub(super) fn insert_if_present(dst: &mut Map<String, Value>, src: &Map<String, Value>, key: &str) {
    if let Some(value) = src.get(key).filter(|value| !value.is_null()) {
        dst.insert(key.to_string(), value.clone());
    }
}

/// Same as [`insert_if_present`], but moves the value out of the owned source map.
///
/// Used for bulk entries (`messages`, `tools`) so building a request does not keep a second full
/// copy of the prompt — including every base64 image — alive at the same time.
pub(super) fn move_if_present(
    dst: &mut Map<String, Value>,
    src: &mut Map<String, Value>,
    key: &str,
) {
    if let Some(value) = src.remove(key).filter(|value| !value.is_null()) {
        dst.insert(key.to_string(), value);
    }
}

/// Copy of a request payload that keeps only the scalar / provider-option entries.
///
/// Providers that post-process an already OpenAI-shaped body (OpenRouter, NanoGPT) must read the
/// original options *after* `openai::build` consumed the payload. Cloning the whole map for that
/// would duplicate the entire prompt for the sake of a handful of scalars.
pub(super) fn provider_option_source(payload: &Map<String, Value>) -> Map<String, Value> {
    payload
        .iter()
        .filter(|(key, _)| !matches!(key.as_str(), "messages" | "tools" | "logit_bias" | "prompt"))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

pub(super) fn message_content_to_text(content: Option<&Value>) -> String {
    openai_chat_content_to_lossy_text(content)
}

/// Split a base64 `data:` URL into its MIME type and payload.
///
/// Real data URLs may carry extra parameters between the MIME type and the encoding
/// (`data:image/png;charset=utf-8;base64,...`), so scan the parameter list for `base64` instead of
/// requiring it to be the only one. A URL with no `base64` marker is rejected: the payload would be
/// percent-encoded text, and forwarding it as base64 would corrupt the attachment.
pub(super) fn parse_data_url(value: &str) -> Option<(String, String)> {
    let trimmed = value.trim();
    let body = trimmed.strip_prefix("data:")?;
    let (metadata, data) = body.split_once(',')?;

    let mut parameters = metadata.split(';');
    let mime_type = parameters.next()?.trim();
    if mime_type.is_empty() || !parameters.any(|parameter| parameter.trim().eq_ignore_ascii_case("base64")) {
        return None;
    }

    let data = data.trim();
    if data.is_empty() {
        return None;
    }

    Some((mime_type.to_string(), data.to_string()))
}
