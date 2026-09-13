use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ttsync_client::{SyncDirection as ClientSyncDirection, SyncObserver, SyncProgress};

use tt_contracts::sync::{SyncJobContext, SyncJobEvent, SyncJobProgress, SyncJobProgressDirection};
use tt_ports::sync::SyncJobEventPublisher;

/// Last time a transfer reported observable progress.
///
/// The sync HTTP client has no request timeout (its options do not expose one), so a half-open
/// connection — peer sleeps or Wi-Fi drops mid-transfer, where no keepalive ever fires — would leave
/// the job awaiting a read forever. That job holds the shared local-mutation permit, so every later
/// sync and every extension install would keep failing until the process restarts. This clock lets
/// the executor turn that silent deadlock into a reported failure.
#[derive(Debug)]
pub struct SyncProgressClock {
    last: Mutex<Instant>,
}

impl SyncProgressClock {
    pub fn new() -> Self {
        Self {
            last: Mutex::new(Instant::now()),
        }
    }

    pub fn touch(&self) {
        *self.last.lock().expect("sync progress clock poisoned") = Instant::now();
    }

    pub fn idle_for(&self) -> Duration {
        self.last
            .lock()
            .expect("sync progress clock poisoned")
            .elapsed()
    }
}

impl Default for SyncProgressClock {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SyncJobProgressObserver {
    events: Arc<dyn SyncJobEventPublisher>,
    job: SyncJobContext,
    clock: Arc<SyncProgressClock>,
}

impl SyncJobProgressObserver {
    pub fn new(events: Arc<dyn SyncJobEventPublisher>, job: SyncJobContext) -> Self {
        Self {
            events,
            job,
            clock: Arc::new(SyncProgressClock::new()),
        }
    }

    pub fn clock(&self) -> Arc<SyncProgressClock> {
        Arc::clone(&self.clock)
    }
}

impl SyncObserver for SyncJobProgressObserver {
    fn on_progress(&self, progress: SyncProgress) {
        self.clock.touch();
        self.events.publish_sync_job(SyncJobEvent::progress(
            self.job.clone(),
            SyncJobProgress {
                direction: progress_direction(progress.direction),
                phase: progress.phase,
                files_done: progress.files_done,
                files_total: progress.files_total,
                bytes_done: progress.bytes_done,
                bytes_total: progress.bytes_total,
                current_path: progress.current_path,
            },
        ));
    }
}

fn progress_direction(direction: ClientSyncDirection) -> SyncJobProgressDirection {
    match direction {
        ClientSyncDirection::Pull => SyncJobProgressDirection::Pull,
        ClientSyncDirection::Push => SyncJobProgressDirection::Push,
    }
}
