mod cache;
mod helpers;
mod importer;
mod repository;
mod shallow_index;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use std::time::Duration;

use tokio::sync::Mutex;

use self::cache::{CharacterShallowIndexCache, MemoryCache};
use tt_adapter_storage_core::{
    FileChatRepository,
    chat_directory_identity::{
        SharedChatAliasStore,
    },
};

/// File-based character repository implementation.
pub struct FileCharacterRepository {
    characters_dir: PathBuf,
    chats_dir: PathBuf,
    default_avatar_path: PathBuf,
    shallow_index_path: PathBuf,
    memory_cache: Arc<Mutex<MemoryCache>>,
    shallow_index_cache: Arc<Mutex<Option<CharacterShallowIndexCache>>>,
    chat_aliases: SharedChatAliasStore,
    chat_repository: Arc<FileChatRepository>,
    /// Per-card write locks, mirroring the chat repository's payload lock
    /// table: a character card save is a read-merge-write over the PNG, so
    /// concurrent writers to the same card need serialization. Weak entries
    /// let idle locks evaporate.
    path_write_locks: Arc<Mutex<HashMap<PathBuf, Weak<Mutex<()>>>>>,
}

impl FileCharacterRepository {
    /// Create an isolated character repository.
    ///
    /// This is a convenience wrapper for single-repository use. Runtime
    /// bootstrap constructs character and chat repositories together and must
    /// call `with_chat_repository` so both projections share one chat cache.
    /// Create a repository with the shared character/chat alias store.
    ///
    /// Use `with_chat_repository` whenever a runtime also exposes a chat
    /// repository, so character and chat list projections share one summary
    /// cache. This constructor is for isolated character-repository tests/tools.
    pub fn with_chat_aliases(
        characters_dir: PathBuf,
        chats_dir: PathBuf,
        default_avatar_path: PathBuf,
        chat_aliases: SharedChatAliasStore,
    ) -> Self {
        let default_user_dir = characters_dir
            .parent()
            .or_else(|| chats_dir.parent())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| chats_dir.clone());
        let chat_repository = Arc::new(FileChatRepository::with_chat_aliases(
            characters_dir.clone(),
            chats_dir.clone(),
            default_user_dir.join("group chats"),
            default_user_dir.join("backups"),
            chat_aliases.clone(),
        ));
        Self::with_chat_repository(
            characters_dir,
            chats_dir,
            default_avatar_path,
            chat_aliases,
            chat_repository,
        )
    }

    pub fn with_chat_repository(
        characters_dir: PathBuf,
        chats_dir: PathBuf,
        default_avatar_path: PathBuf,
        chat_aliases: SharedChatAliasStore,
        chat_repository: Arc<FileChatRepository>,
    ) -> Self {
        let shallow_index_path = character_shallow_index_path_for_characters_dir(&characters_dir);
        let memory_cache = Arc::new(Mutex::new(MemoryCache::new(
            100,
            Duration::from_secs(30 * 60),
        )));
        let shallow_index_cache = Arc::new(Mutex::new(None));
        let path_write_locks = Arc::new(Mutex::new(HashMap::new()));

        Self {
            characters_dir,
            chats_dir,
            default_avatar_path,
            shallow_index_path,
            memory_cache,
            shallow_index_cache,
            chat_aliases,
            chat_repository,
            path_write_locks,
        }
    }
}

fn character_shallow_index_path_for_characters_dir(characters_dir: &Path) -> PathBuf {
    characters_dir
        .parent()
        .map(|default_user_dir| {
            default_user_dir
                .join("user")
                .join("cache")
                .join("character_shallow_index_v1.json")
        })
        .unwrap_or_else(|| characters_dir.join("character_shallow_index_v1.json"))
}
