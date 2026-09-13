use std::path::Path;

use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use super::FileAgentRepository;
use tt_domain::errors::DomainError;
use tt_domain::models::agent::AgentRunEvent;

/// Initial tail window used to locate the last journal line. Grown on demand when a single
/// event line (model responses can be large) does not fit.
const EVENT_TAIL_PROBE_BYTES: u64 = 16 * 1024;

impl FileAgentRepository {
    pub(super) async fn read_all_events(
        &self,
        run_id: &str,
    ) -> Result<Vec<AgentRunEvent>, DomainError> {
        let events_path = self.load_run_dir(run_id).await?.join("events.jsonl");
        let contents = match fs::read_to_string(&events_path).await {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Vec::new());
            }
            Err(error) => {
                return Err(DomainError::InternalError(format!(
                    "Failed to read agent event journal {}: {}",
                    events_path.display(),
                    error
                )));
            }
        };

        contents
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                serde_json::from_str::<AgentRunEvent>(line).map_err(|error| {
                    DomainError::InvalidData(format!(
                        "Invalid agent event in {}: {}",
                        events_path.display(),
                        error
                    ))
                })
            })
            .collect()
    }

    pub(super) async fn last_event_seq(&self, run_id: &str) -> Result<Option<u64>, DomainError> {
        let events_path = self.load_run_dir(run_id).await?.join("events.jsonl");
        last_event_seq_in_file(&events_path).await
    }
}

/// Read the highest `seq` already stored in a run journal by parsing only its last line.
///
/// The journal is append-only, so the tail carries the sequence high-water mark. Appending an
/// event used to read and deserialize the whole file, which made a long run quadratic in the
/// number of events it emits.
pub(super) async fn last_event_seq_in_file(events_path: &Path) -> Result<Option<u64>, DomainError> {
    let mut file = match fs::File::open(events_path).await {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(io_failure(events_path, "open", error));
        }
    };

    let total = file
        .metadata()
        .await
        .map_err(|error| io_failure(events_path, "stat", error))?
        .len();
    if total == 0 {
        return Ok(None);
    }

    let mut window = EVENT_TAIL_PROBE_BYTES;
    loop {
        let start = total.saturating_sub(window);
        file.seek(std::io::SeekFrom::Start(start))
            .await
            .map_err(|error| io_failure(events_path, "seek", error))?;

        let mut buffer = Vec::with_capacity(usize::try_from(total - start).unwrap_or(0));
        file.read_to_end(&mut buffer)
            .await
            .map_err(|error| io_failure(events_path, "read tail of", error))?;

        let trimmed = trim_trailing_newlines(&buffer);
        if trimmed.is_empty() {
            if start == 0 {
                return Ok(None);
            }
        } else if let Some(index) = trimmed.iter().rposition(|byte| *byte == b'\n') {
            return parse_seq(events_path, &trimmed[index + 1..]).map(Some);
        } else if start == 0 {
            return parse_seq(events_path, trimmed).map(Some);
        }

        if start == 0 {
            return Ok(None);
        }
        window = window.saturating_mul(2);
    }
}

fn trim_trailing_newlines(buffer: &[u8]) -> &[u8] {
    let mut end = buffer.len();
    while end > 0 && matches!(buffer[end - 1], b'\n' | b'\r' | b' ' | b'\t') {
        end -= 1;
    }
    &buffer[..end]
}

fn parse_seq(events_path: &Path, line: &[u8]) -> Result<u64, DomainError> {
    let line = std::str::from_utf8(line).map_err(|error| {
        DomainError::InvalidData(format!(
            "Invalid agent event encoding in {}: {}",
            events_path.display(),
            error
        ))
    })?;
    serde_json::from_str::<AgentRunEvent>(line)
        .map(|event| event.seq)
        .map_err(|error| {
            DomainError::InvalidData(format!(
                "Invalid agent event in {}: {}",
                events_path.display(),
                error
            ))
        })
}

fn io_failure(events_path: &Path, action: &str, error: std::io::Error) -> DomainError {
    DomainError::InternalError(format!(
        "Failed to {} agent event journal {}: {}",
        action,
        events_path.display(),
        error
    ))
}

