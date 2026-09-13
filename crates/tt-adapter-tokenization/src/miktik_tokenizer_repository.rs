use std::borrow::Cow;
use std::collections::HashSet;
use std::io::ErrorKind;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use flate2::read::GzDecoder;
use miktik::{TokenizerError, TokenizerRegistry};
use serde_json::Value;
use tokio::sync::{Mutex, RwLock};

use tt_domain::errors::DomainError;
use tt_ports::repositories::tokenizer_repository::{
    TokenizerRepository, has_reached_openai_text_token_limit,
};

const CLAUDE_JSON_GZIP_BYTES: &[u8] =
    include_bytes!("../../../resources/tokenizers/claude.json.gz");
const CLAUDE_CACHE_FILE_NAME: &str = "claude.json";
/// The one canonical tokenizer this repository serves. See `canonical_model`.
const CLAUDE_CANONICAL: &str = "claude";

pub struct MiktikTokenizerRepository {
    registry: Arc<TokenizerRegistry>,
    cache_dir: PathBuf,
    ready_models: RwLock<HashSet<&'static str>>,
    registration_guard: Mutex<()>,
}

impl MiktikTokenizerRepository {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self {
            registry: Arc::new(TokenizerRegistry::new()),
            cache_dir,
            ready_models: RwLock::new(HashSet::new()),
            registration_guard: Mutex::new(()),
        }
    }

    /// Every model name resolves to the Claude tokenizer.
    ///
    /// The other families miktik offers are not worth their resident cost here.
    /// Each one is a separate trie built in memory and kept for the whole
    /// process lifetime: measured on the phone's own data, loading gemma costs
    /// +167 MB, o200k +43 MB, cl100k +24 MB, against +19 MB for Claude — to
    /// produce a token count that only ever approximates the prompt size for a
    /// model whose real tokenizer RustTavern cannot know anyway. Picking one
    /// family keeps the count consistent everywhere and caps the cost at a
    /// single trie.
    ///
    /// ponytail: the ceiling is that a request for, say, an OpenAI model now
    /// gets Claude's approximation instead of o200k's. The upgrade path, if the
    /// accuracy is ever worth the memory, is to bring back a per-family lookup
    /// here together with an eviction bound on the registry.
    fn canonical_model(_requested_model: &str) -> &'static str {
        CLAUDE_CANONICAL
    }

    async fn ensure_model_ready_canonical(
        &self,
        canonical: &'static str,
    ) -> Result<(), DomainError> {
        if self.is_model_ready(canonical).await {
            return Ok(());
        }

        let _guard = self.registration_guard.lock().await;

        if self.is_model_ready(canonical).await {
            return Ok(());
        }

        let model_path = self.ensure_model_file().await?;
        self.register_model_file(canonical, &model_path)?;

        if self.warm_model(canonical).await.is_err() {
            self.remove_model_file(&model_path).await?;
            self.write_claude_model_file(&model_path).await?;
            self.register_model_file(canonical, &model_path)?;
            self.warm_model(canonical).await?;
        }

        self.mark_model_ready(canonical).await;
        Ok(())
    }

    fn register_model_file(
        &self,
        canonical: &'static str,
        model_path: &Path,
    ) -> Result<(), DomainError> {
        self.registry
            .register_model_file(canonical, model_path)
            .map_err(|error| Self::map_tokenizer_error("register model resource", canonical, error))
    }

    async fn warm_model(&self, canonical: &'static str) -> Result<(), DomainError> {
        let registry = Arc::clone(&self.registry);

        tokio::task::spawn_blocking(move || registry.get_canonical(canonical))
            .await
            .map_err(|error| {
                DomainError::InternalError(format!(
                    "Tokenizer warm-up task failed for '{canonical}': {error}"
                ))
            })?
            .map_err(|error| Self::map_tokenizer_error("load tokenizer", canonical, error))?;

        Ok(())
    }

    async fn ensure_model_file(&self) -> Result<PathBuf, DomainError> {
        let path = self.cache_dir.join(CLAUDE_CACHE_FILE_NAME);
        if !path.exists() {
            self.write_claude_model_file(&path).await?;
        }
        Ok(path)
    }

    async fn write_claude_model_file(&self, path: &Path) -> Result<(), DomainError> {
        let bytes = Self::decode_claude_payload(CLAUDE_JSON_GZIP_BYTES)?;
        self.write_bytes(path, &bytes).await
    }

    fn decode_claude_payload(payload: &[u8]) -> Result<Vec<u8>, DomainError> {
        let mut decoder = GzDecoder::new(Cursor::new(payload));
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed).map_err(|error| {
            DomainError::InternalError(format!(
                "Failed to decompress tokenizer payload '{}': {}",
                CLAUDE_CACHE_FILE_NAME, error
            ))
        })?;
        Ok(decompressed)
    }

    async fn write_bytes(&self, path: &Path, bytes: &[u8]) -> Result<(), DomainError> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|error| {
                DomainError::InternalError(format!(
                    "Failed to create tokenizer cache directory '{}': {}",
                    parent.display(),
                    error
                ))
            })?;
        }

        let temp_path = Self::temp_cache_path(path);
        tokio::fs::write(&temp_path, bytes).await.map_err(|error| {
            DomainError::InternalError(format!(
                "Failed to persist tokenizer resource to '{}': {}",
                temp_path.display(),
                error
            ))
        })?;

        if let Err(error) = tokio::fs::rename(&temp_path, path).await {
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(DomainError::InternalError(format!(
                "Failed to publish tokenizer resource to '{}': {}",
                path.display(),
                error
            )));
        }

        Ok(())
    }

    fn temp_cache_path(path: &Path) -> PathBuf {
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("tokenizer");
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        path.with_file_name(format!(".{file_name}.{}.{}.tmp", std::process::id(), nonce))
    }

    async fn remove_model_file(&self, path: &Path) -> Result<(), DomainError> {
        match tokio::fs::remove_file(path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(error) => Err(DomainError::InternalError(format!(
                "Failed to remove invalid tokenizer resource '{}': {}",
                path.display(),
                error
            ))),
        }
    }

    async fn is_model_ready(&self, canonical: &'static str) -> bool {
        self.ready_models.read().await.contains(canonical)
    }

    async fn mark_model_ready(&self, canonical: &'static str) {
        self.ready_models.write().await.insert(canonical);
    }

    fn map_tokenizer_error(action: &str, model: &str, error: TokenizerError) -> DomainError {
        match error {
            TokenizerError::ModelNotFound(message) => {
                DomainError::NotFound(format!("Failed to {} for '{}': {}", action, model, message))
            }
            TokenizerError::LoadError(message)
            | TokenizerError::EncodeError(message)
            | TokenizerError::DecodeError(message) => DomainError::InternalError(format!(
                "Failed to {} for '{}': {}",
                action, model, message
            )),
        }
    }

    fn value_to_text(value: &Value) -> Cow<'_, str> {
        match value {
            Value::String(text) => Cow::Borrowed(text),
            _ => Cow::Owned(value.to_string()),
        }
    }

    fn to_web_tokenizer_prompt(messages: &[Value]) -> String {
        #[derive(Clone)]
        struct PromptMessage {
            role: String,
            name: Option<String>,
            content: String,
        }

        let mut mapped = messages
            .iter()
            .map(|value| match value {
                Value::Object(map) => {
                    let role = map
                        .get("role")
                        .and_then(Value::as_str)
                        .unwrap_or("system")
                        .to_string();
                    let name = map.get("name").and_then(Value::as_str).map(str::to_string);
                    let mut content = map
                        .get("content")
                        .map(Self::value_to_text)
                        .map(Cow::into_owned)
                        .unwrap_or_default();
                    if let Some(tool_calls) = map.get("tool_calls") {
                        content.push_str(&tool_calls.to_string());
                    }
                    PromptMessage {
                        role,
                        name,
                        content,
                    }
                }
                _ => PromptMessage {
                    role: "system".to_string(),
                    name: None,
                    content: Self::value_to_text(value).into_owned(),
                },
            })
            .collect::<Vec<_>>();

        if !mapped.is_empty() {
            mapped[0].role = "system".to_string();

            let mut first_assistant_index = None;
            for (index, message) in mapped.iter().enumerate() {
                if index > 0 && message.role == "assistant" {
                    first_assistant_index = Some(index);
                    break;
                }
            }

            // Mirrors SillyTavern's convertClaudePrompt fixed-parameter path used in token counting.
            mapped[0].role = "user".to_string();
            if let Some(index) = first_assistant_index {
                let candidate_index = index.saturating_sub(1);
                if candidate_index != 0 && mapped[candidate_index].role == "user" {
                    mapped[candidate_index].role = "FixHumMsg".to_string();
                }
            }
        }

        let mut prompt = String::new();
        for (index, message) in mapped.iter().enumerate() {
            let prefix = match message.role.as_str() {
                "assistant" => "\n\nAssistant: ",
                "user" => "\n\nHuman: ",
                "system" => {
                    if index == 0 {
                        ""
                    } else if message.name.as_deref() == Some("example_assistant") {
                        "\n\nA: "
                    } else if message.name.as_deref() == Some("example_user") {
                        "\n\nH: "
                    } else {
                        "\n\n"
                    }
                }
                "FixHumMsg" => "\n\nFirst message: ",
                _ => "",
            };

            prompt.push_str(prefix);

            if message.role != "system"
                && let Some(name) = message.name.as_deref()
                && !name.is_empty()
            {
                prompt.push_str(name);
                prompt.push_str(": ");
            }

            prompt.push_str(&message.content);
        }

        prompt
    }
}

#[async_trait::async_trait]
impl TokenizerRepository for MiktikTokenizerRepository {
    async fn ensure_model_ready(&self, model: &str) -> Result<(), DomainError> {
        let canonical = Self::canonical_model(model);
        self.ensure_model_ready_canonical(canonical).await
    }

    fn encode(&self, model: &str, text: &str) -> Result<Vec<u32>, DomainError> {
        let canonical = Self::canonical_model(model);
        let tokenizer = self
            .registry
            .get_canonical(canonical)
            .map_err(|error| Self::map_tokenizer_error("load tokenizer", canonical, error))?;

        tokenizer
            .encode(text)
            .map_err(|error| Self::map_tokenizer_error("encode text", canonical, error))
    }

    fn decode(&self, model: &str, token_ids: &[u32]) -> Result<String, DomainError> {
        let canonical = Self::canonical_model(model);
        let tokenizer = self
            .registry
            .get_canonical(canonical)
            .map_err(|error| Self::map_tokenizer_error("load tokenizer", canonical, error))?;

        tokenizer
            .decode(token_ids)
            .map_err(|error| Self::map_tokenizer_error("decode token ids", canonical, error))
    }

    fn count_messages(&self, model: &str, messages: &[Value]) -> Result<usize, DomainError> {
        let canonical = Self::canonical_model(model);
        let prompt = Self::to_web_tokenizer_prompt(messages);

        self.registry
            .count_tokens_canonical(canonical, &prompt)
            .map_err(|error| {
                Self::map_tokenizer_error("count web-tokenizer messages", canonical, error)
            })
    }

    fn count_system_message_prefixes(
        &self,
        model: &str,
        base: &str,
        suffixes: &[String],
        stop_at: Option<usize>,
    ) -> Result<Vec<usize>, DomainError> {
        if suffixes.is_empty() {
            return Ok(Vec::new());
        }

        let canonical = Self::canonical_model(model);
        let additions = suffixes.iter().map(String::as_str).collect::<Vec<_>>();

        let mut token_counts = self
            .registry
            .estimate_cumulative_token_counts_canonical(canonical, base, &additions)
            .map_err(|error| {
                Self::map_tokenizer_error("estimate cumulative token counts", canonical, error)
            })?;
        if token_counts.len() != suffixes.len() {
            return Err(DomainError::InternalError(format!(
                "cumulative token estimate returned {} counts for {} suffixes on '{canonical}'",
                token_counts.len(),
                suffixes.len()
            )));
        }

        let empty_text_count = self
            .registry
            .count_tokens_canonical(canonical, "")
            .map_err(|error| Self::map_tokenizer_error("count empty text", canonical, error))?;
        let empty_message_count = TokenizerRepository::count_messages(
            self,
            canonical,
            &[serde_json::json!({
                "role": "system",
                "content": "",
            })],
        )?;
        let wrapper_tokens = empty_message_count
            .checked_sub(empty_text_count)
            .ok_or_else(|| {
                DomainError::InternalError(format!(
                    "system-message wrapper reduced the token count for '{canonical}'"
                ))
            })?;

        for count in &mut token_counts {
            *count = count.checked_add(wrapper_tokens).ok_or_else(|| {
                DomainError::InternalError(format!(
                    "cumulative token estimate overflowed for '{canonical}'"
                ))
            })?;
        }

        if let Some(index) = token_counts
            .iter()
            .position(|&count| has_reached_openai_text_token_limit(count, stop_at))
        {
            let terminal_count = token_counts[index];
            token_counts[index..].fill(terminal_count);
        }

        Ok(token_counts)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::json;

    use super::MiktikTokenizerRepository;
    use tt_ports::repositories::tokenizer_repository::{
        TokenizerRepository, openai_text_token_count,
    };

    static NEXT_TEMP_CACHE_DIR_ID: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn every_model_name_resolves_to_the_claude_tokenizer() {
        for model in [
            "claude-3-7-sonnet",
            "gpt-4o",
            "gpt-4.1-mini",
            "gpt-3.5-turbo-0301",
            "o4-mini",
            "gemini-2.0-flash",
            "key/bypass-gemini-3.8-flash",
            "deepseek-chat",
            "llama-3-70b",
            "mistral-large",
            "yi-34b",
            "command-r-plus",
            "",
            "   ",
        ] {
            assert_eq!(
                MiktikTokenizerRepository::canonical_model(model),
                "claude",
                "model '{model}' should use the claude tokenizer"
            );
        }
    }

    #[tokio::test]
    async fn every_model_name_produces_identical_counts() {
        let cache_dir = unique_temp_cache_dir();
        let repository = MiktikTokenizerRepository::new(cache_dir.clone());
        let messages = vec![
            json!({"role": "system", "content": "sys"}),
            json!({"role": "user", "content": "hello 世界"}),
        ];

        let mut counts = Vec::new();
        for model in ["claude-3-7-sonnet", "gpt-4o", "gemini-2.0-flash"] {
            TokenizerRepository::ensure_model_ready(&repository, model)
                .await
                .expect("tokenizer should prepare");
            counts.push(
                TokenizerRepository::count_messages(&repository, model, &messages)
                    .expect("token count should succeed"),
            );
        }

        let _ = std::fs::remove_dir_all(cache_dir);
        assert!(counts[0] > 0);
        assert_eq!(counts[0], counts[1]);
        assert_eq!(counts[1], counts[2]);
    }

    #[test]
    fn bundled_claude_payload_is_gzip_compressed() {
        let decoded =
            MiktikTokenizerRepository::decode_claude_payload(super::CLAUDE_JSON_GZIP_BYTES)
                .expect("bundled claude payload should decompress");
        assert!(!decoded.is_empty());
        assert!(decoded.len() > super::CLAUDE_JSON_GZIP_BYTES.len());
    }

    #[test]
    fn web_tokenizer_prompt_uses_claude_prefixes() {
        let messages = vec![
            json!({"role": "system", "content": "sys"}),
            json!({"role": "user", "content": "hello"}),
            json!({"role": "assistant", "content": "world"}),
        ];
        let prompt = MiktikTokenizerRepository::to_web_tokenizer_prompt(&messages);
        assert!(prompt.contains("\n\nHuman: sys"));
        assert!(prompt.contains("\n\nFirst message: hello"));
        assert!(prompt.contains("\n\nAssistant: world"));
    }

    #[tokio::test]
    async fn new_does_not_eagerly_register_a_tokenizer() {
        let cache_dir = unique_temp_cache_dir();
        let repository = MiktikTokenizerRepository::new(cache_dir.clone());

        assert!(!repository.is_model_ready("claude").await);
        let _ = std::fs::remove_dir_all(cache_dir);
    }

    #[tokio::test]
    async fn tokenizer_is_usable_without_network() {
        let cache_dir = unique_temp_cache_dir();
        let repository = MiktikTokenizerRepository::new(cache_dir.clone());
        let messages = vec![json!({"role": "user", "content": "hello world"})];

        TokenizerRepository::ensure_model_ready(&repository, "gemini-2.0-flash")
            .await
            .expect("claude tokenizer should prepare for a gemini model name");
        let count =
            TokenizerRepository::count_messages(&repository, "gemini-2.0-flash", &messages)
                .expect("claude tokenizer should count");

        let _ = std::fs::remove_dir_all(cache_dir);
        assert!(count > 0);
    }

    #[tokio::test]
    async fn tokenizer_materializes_the_cache_file_on_first_use() {
        let cache_dir = unique_temp_cache_dir();
        let repository = MiktikTokenizerRepository::new(cache_dir.clone());
        let messages = vec![json!({"role": "user", "content": "hello world"})];

        TokenizerRepository::ensure_model_ready(&repository, "gpt-4o")
            .await
            .expect("tokenizer should prepare");
        TokenizerRepository::count_messages(&repository, "gpt-4o", &messages)
            .expect("tokenizer should count");

        let cache_path = cache_dir.join(super::CLAUDE_CACHE_FILE_NAME);
        assert!(
            cache_path.exists(),
            "claude tokenizer should be materialized to cache"
        );
        let cached = std::fs::read(&cache_path).expect("cache file should be readable");
        assert_eq!(cached, claude_model_bytes());
        assert!(cache_temp_files(&cache_dir).is_empty());

        let _ = std::fs::remove_dir_all(cache_dir);
    }

    #[tokio::test]
    async fn corrupt_bundled_cache_is_rebuilt() {
        let cache_dir = unique_temp_cache_dir();
        std::fs::create_dir_all(&cache_dir).expect("cache dir should be created");
        std::fs::write(cache_dir.join(super::CLAUDE_CACHE_FILE_NAME), b"not a tokenizer")
            .expect("corrupt cache should be written");

        let repository = MiktikTokenizerRepository::new(cache_dir.clone());

        TokenizerRepository::ensure_model_ready(&repository, "claude")
            .await
            .expect("corrupt bundled cache should be rebuilt");

        let cached = std::fs::read(cache_dir.join(super::CLAUDE_CACHE_FILE_NAME))
            .expect("rebuilt cache should be readable");
        assert_eq!(cached, claude_model_bytes());

        let _ = std::fs::remove_dir_all(cache_dir);
    }

    #[test]
    fn cumulative_prefix_counts_return_empty_without_loading_a_model() {
        let repository = MiktikTokenizerRepository::new(unique_temp_cache_dir());

        let counts = TokenizerRepository::count_system_message_prefixes(
            &repository,
            "missing-model",
            "ignored",
            &[],
            Some(1),
        )
        .expect("empty suffixes should not load a tokenizer");

        assert!(counts.is_empty());
    }

    #[tokio::test]
    async fn cumulative_prefix_estimates_track_individual_messages() {
        let cache_dir = unique_temp_cache_dir();
        let repository = MiktikTokenizerRepository::new(cache_dir.clone());
        let base = "世界设定\n";
        let suffixes = vec![
            "First entry with punctuation!\n".to_string(),
            "第二条目，包含中文。\n".to_string(),
            "  whitespace and emoji: \u{1f642}\n".to_string(),
        ];

        for model in ["claude", "gemini-2.0-flash"] {
            TokenizerRepository::ensure_model_ready(&repository, model)
                .await
                .expect("tokenizer should prepare");
            let actual = TokenizerRepository::count_system_message_prefixes(
                &repository,
                model,
                base,
                &suffixes,
                None,
            )
            .expect("optimized prefix counts should succeed");

            let mut content = base.to_string();
            let expected = suffixes
                .iter()
                .map(|suffix| {
                    content.push_str(suffix);
                    let messages = vec![json!({ "role": "system", "content": content })];
                    TokenizerRepository::count_messages(&repository, model, &messages)
                        .expect("individual system message count should succeed")
                })
                .collect::<Vec<_>>();

            for (estimate, exact) in actual.iter().zip(&expected) {
                let tolerance = exact.div_ceil(20).max(1);
                assert!(
                    estimate.abs_diff(*exact) <= tolerance,
                    "prefix estimate drifted for {model}: estimate={estimate}, exact={exact}"
                );
            }
        }

        let _ = std::fs::remove_dir_all(cache_dir);
    }

    #[tokio::test]
    async fn cumulative_prefix_estimates_track_boundary_corpus() {
        let cache_dir = unique_temp_cache_dir();
        let repository = MiktikTokenizerRepository::new(cache_dir.clone());
        let fragments = [
            "",
            "a",
            "bc",
            "  ",
            "\n",
            " \n ",
            "punctuation!?",
            "世界",
            "设定。",
            "\u{1f642}",
            "e\u{301}",
            "<|not-a-special-token|>",
        ];

        for base_index in 0..fragments.len() {
            let base = fragments[..=base_index].concat();
            let suffixes = (0..32)
                .map(|index| fragments[(base_index + index + 1) % fragments.len()].to_string())
                .collect::<Vec<_>>();
            let actual = TokenizerRepository::count_system_message_prefixes(
                &repository,
                "claude",
                &base,
                &suffixes,
                None,
            )
            .expect("incremental prefix counts should succeed");

            let mut content = base.clone();
            let expected = suffixes
                .iter()
                .map(|suffix| {
                    content.push_str(suffix);
                    TokenizerRepository::count_messages(
                        &repository,
                        "claude",
                        &[json!({ "role": "system", "content": content })],
                    )
                    .expect("complete prefix count should succeed")
                })
                .collect::<Vec<_>>();

            for (estimate, exact) in actual.iter().zip(&expected) {
                assert!(
                    estimate.abs_diff(*exact) <= 1,
                    "prefix estimate drifted at base {base_index}: estimate={estimate}, exact={exact}"
                );
            }
        }

        let _ = std::fs::remove_dir_all(cache_dir);
    }

    #[tokio::test]
    async fn cumulative_prefix_counts_preserve_stop_at_fill() {
        let cache_dir = unique_temp_cache_dir();
        let repository = MiktikTokenizerRepository::new(cache_dir.clone());
        let suffixes = vec![
            "short\n".to_string(),
            "a considerably longer second entry\n".to_string(),
            "this entry must not need to be tokenized\n".to_string(),
        ];

        TokenizerRepository::ensure_model_ready(&repository, "claude")
            .await
            .expect("tokenizer should prepare");
        let full_counts = TokenizerRepository::count_system_message_prefixes(
            &repository,
            "claude",
            "base\n",
            &suffixes,
            None,
        )
        .expect("full prefix counts should succeed");
        let stop_at = openai_text_token_count(full_counts[1]);

        let stopped_counts = TokenizerRepository::count_system_message_prefixes(
            &repository,
            "claude",
            "base\n",
            &suffixes,
            Some(stop_at),
        )
        .expect("stopped prefix counts should succeed");

        let _ = std::fs::remove_dir_all(cache_dir);
        assert_eq!(
            stopped_counts,
            vec![full_counts[0], full_counts[1], full_counts[1]],
            "stop/fill semantics changed"
        );
    }

    #[tokio::test]
    async fn cumulative_prefix_estimates_stay_close_on_large_world_info() {
        let cache_dir = unique_temp_cache_dir();
        let repository = MiktikTokenizerRepository::new(cache_dir.clone());
        let base =
            "Stable world context with ordinary text, punctuation, and spacing.\n".repeat(120);
        let suffixes = (0..32)
            .map(|index| format!("Entry {index}: 世界设定 with details, spaces, and emoji 🙂.\n"))
            .collect::<Vec<_>>();

        TokenizerRepository::ensure_model_ready(&repository, "claude")
            .await
            .expect("tokenizer should prepare");

        let mut content = base.clone();
        let exact_counts = suffixes
            .iter()
            .map(|suffix| {
                content.push_str(suffix);
                TokenizerRepository::count_messages(
                    &repository,
                    "claude",
                    &[json!({ "role": "system", "content": content })],
                )
                .expect("complete prefix count should succeed")
            })
            .collect::<Vec<_>>();
        let estimated_counts = TokenizerRepository::count_system_message_prefixes(
            &repository,
            "claude",
            &base,
            &suffixes,
            None,
        )
        .expect("cumulative prefix estimates should succeed");

        for (estimate, exact) in estimated_counts.iter().zip(&exact_counts) {
            let tolerance = exact.div_ceil(20).max(1);
            assert!(
                estimate.abs_diff(*exact) <= tolerance,
                "large world-info estimate drifted: estimate={estimate}, exact={exact}"
            );
        }

        let stop_at = openai_text_token_count(estimated_counts[20]);
        let stop_index = estimated_counts
            .iter()
            .position(|&count| openai_text_token_count(count) >= stop_at)
            .expect("estimates should reach the selected threshold");

        let stopped_counts = TokenizerRepository::count_system_message_prefixes(
            &repository,
            "claude",
            &base,
            &suffixes,
            Some(stop_at),
        )
        .expect("stopped prefix estimates should succeed");

        let mut expected_stopped_counts = estimated_counts;
        let terminal_count = expected_stopped_counts[stop_index];
        expected_stopped_counts[stop_index..].fill(terminal_count);

        let _ = std::fs::remove_dir_all(cache_dir);
        assert_eq!(stopped_counts, expected_stopped_counts);
    }

    fn unique_temp_cache_dir() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let sequence = NEXT_TEMP_CACHE_DIR_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "rusttavern-tokenizer-test-{}-{nonce}-{sequence}",
            std::process::id()
        ))
    }

    fn claude_model_bytes() -> Vec<u8> {
        MiktikTokenizerRepository::decode_claude_payload(super::CLAUDE_JSON_GZIP_BYTES)
            .expect("bundled claude payload should decode")
    }

    fn cache_temp_files(cache_dir: &std::path::Path) -> Vec<PathBuf> {
        std::fs::read_dir(cache_dir)
            .expect("cache dir should be readable")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.ends_with(".tmp"))
            })
            .collect()
    }
}
