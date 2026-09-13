use std::collections::HashMap;
use std::time::{Duration, Instant};

use tt_domain::models::chat::Chat;

/// Approximate heap footprint of one chat: message text plus an allowance per
/// message for name/timestamp/`extra`/`additional` and `String` overhead.
///
/// The budget only needs to be right to within a factor, and message text
/// dominates, so this avoids walking every field on every write.
fn approximate_bytes(chat: &Chat) -> usize {
    const PER_MESSAGE_OVERHEAD: usize = 512;
    const CHAT_OVERHEAD: usize = 2048;

    CHAT_OVERHEAD
        + chat
            .messages
            .iter()
            .map(|message| message.mes.len() + message.name.len() + PER_MESSAGE_OVERHEAD)
            .sum::<usize>()
}

struct Entry {
    chat: Chat,
    inserted_at: Instant,
    bytes: usize,
}

/// Memory cache for chat data.
///
/// Bounded by **bytes**, not entry count: a `Chat` owns its whole message
/// history, so a count bound lets a hundred large chats pin tens of megabytes
/// permanently. Eviction is oldest-inserted-first.
pub(super) struct MemoryCache {
    chats: HashMap<String, Entry>,
    max_bytes: usize,
    max_entries: usize,
    total_bytes: usize,
    ttl: Duration,
}

impl MemoryCache {
    /// Create a new memory cache with the given byte/entry budget and TTL
    pub(super) fn new(max_bytes: usize, max_entries: usize, ttl: Duration) -> Self {
        Self {
            chats: HashMap::new(),
            max_bytes,
            max_entries,
            total_bytes: 0,
            ttl,
        }
    }

    /// Get a chat from the cache
    pub(super) fn get(&self, key: &str) -> Option<Chat> {
        if let Some(entry) = self.chats.get(key)
            && entry.inserted_at.elapsed() < self.ttl
        {
            return Some(entry.chat.clone());
        }
        None
    }

    /// Set a chat in the cache
    pub(super) fn set(&mut self, key: String, chat: Chat) {
        let bytes = approximate_bytes(&chat);
        if let Some(previous) = self.chats.insert(
            key,
            Entry {
                chat,
                inserted_at: Instant::now(),
                bytes,
            },
        ) {
            self.total_bytes -= previous.bytes;
        }
        self.total_bytes += bytes;

        while self.total_bytes > self.max_bytes || self.chats.len() > self.max_entries {
            let Some(oldest_key) = self
                .chats
                .iter()
                .min_by_key(|(_, entry)| entry.inserted_at)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            if let Some(evicted) = self.chats.remove(&oldest_key) {
                self.total_bytes -= evicted.bytes;
            }
        }
    }

    /// Remove a chat from the cache
    pub(super) fn remove(&mut self, key: &str) {
        if let Some(entry) = self.chats.remove(key) {
            self.total_bytes -= entry.bytes;
        }
    }

    /// Clear the cache
    pub(super) fn clear(&mut self) {
        self.chats.clear();
        self.total_bytes = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tt_domain::models::chat::ChatMessage;

    fn chat_with_message_bytes(len: usize) -> Chat {
        let mut chat = Chat::new("user", "character");
        chat.add_message(ChatMessage::user("user", &"x".repeat(len)));
        chat
    }

    #[test]
    fn evicts_by_byte_budget_not_entry_count() {
        let chat = chat_with_message_bytes(2048);
        // Room for one entry plus slack: nothing but the byte budget can evict.
        let mut cache = MemoryCache::new(approximate_bytes(&chat) + 1024, 64, Duration::from_secs(60));

        cache.set("a".to_string(), chat.clone());
        cache.set("b".to_string(), chat);

        assert!(cache.get("a").is_none());
        assert!(cache.get("b").is_some());
    }
}
