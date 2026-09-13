use async_trait::async_trait;
use tt_domain::errors::DomainError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChatPayloadTarget {
    Character {
        character_id: String,
        file_name: String,
    },
    Group {
        chat_id: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChatPayloadCommitBegin {
    pub session_id: String,
    pub max_frame_bytes: u64,
}

/// The (size, mtime-seconds) of a chat file as one client last saw it.
/// Supplied by the loader at `begin`; `finish` rejects the commit when the
/// file on disk no longer matches. The integrity slug never rotates (it is
/// the frontend's stable store-directory key), so it cannot detect two
/// clients editing concurrently from the same baseline — this can.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChatPayloadBaseline {
    pub size: u64,
    pub mtime_sec: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommittedChatPayload {
    pub target: ChatPayloadTarget,
    pub size: u64,
    /// mtime of the published file in whole seconds, so the committing client
    /// can adopt it as its next baseline without a re-read.
    pub mtime_sec: u64,
}

/// Streams a complete chat payload into private, target-volume staging and
/// publishes it atomically when the session is finished.
#[async_trait]
pub trait ChatPayloadCommitRepository: Send + Sync {
    async fn begin(
        &self,
        target: ChatPayloadTarget,
        force: bool,
        baseline: Option<ChatPayloadBaseline>,
    ) -> Result<ChatPayloadCommitBegin, DomainError>;

    async fn append(&self, session_id: &str, offset: u64, bytes: &[u8])
    -> Result<u64, DomainError>;

    async fn finish(
        &self,
        session_id: &str,
        expected_size: u64,
    ) -> Result<CommittedChatPayload, DomainError>;

    /// Aborting an absent or already-consumed session is a successful no-op.
    async fn abort(&self, session_id: &str) -> Result<(), DomainError>;
}
