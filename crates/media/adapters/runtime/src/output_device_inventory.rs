//! Optional OS enumeration never runs on an HTTP executor or under a configuration edit lock.
use media_http::{OutputDeviceDiscoveryState, OutputDeviceInventory};
use std::{
    sync::{
        Arc, Mutex,
        mpsc::{self, SyncSender},
    },
    time::{Duration, Instant},
};

const REFRESH_INTERVAL: Duration = Duration::from_secs(30);
const STALLED_AFTER: Duration = Duration::from_secs(3);
type Discover = Arc<dyn Fn() -> Result<Vec<String>, String> + Send + Sync>;

struct State {
    snapshot: OutputDeviceInventory,
    pending_since: Option<Instant>,
    completed_at: Option<Instant>,
}

/// One bounded worker per process, even if CoreAudio never returns. Dropping the owner closes
/// its channel without joining an OS call; an in-flight worker exits after discovery returns.
#[derive(Clone)]
pub(super) struct Inventory {
    state: Arc<Mutex<State>>,
    refresh: SyncSender<()>,
}

impl Inventory {
    pub(super) fn start(discover: Discover) -> Self {
        let state = Arc::new(Mutex::new(State {
            snapshot: OutputDeviceInventory {
                devices: Vec::new(),
                state: OutputDeviceDiscoveryState::Loading,
                has_successful_snapshot: false,
                error: None,
            },
            pending_since: None,
            completed_at: None,
        }));
        let (refresh, requests) = mpsc::sync_channel(1);
        let owner = Self {
            state: state.clone(),
            refresh,
        };
        let worker = std::thread::Builder::new()
            .name("media-output-device-discovery".into())
            .spawn(move || {
                while requests.recv().is_ok() {
                    // No snapshot lock is held during any platform/device call.
                    let result =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| discover()))
                            .unwrap_or_else(|_| {
                                Err("audio output discovery stopped unexpectedly".into())
                            });
                    let mut state = state.lock().unwrap_or_else(|error| error.into_inner());
                    match result {
                        Ok(devices) => {
                            state.snapshot.devices = devices;
                            state.snapshot.state = OutputDeviceDiscoveryState::Ready;
                            state.snapshot.has_successful_snapshot = true;
                            state.snapshot.error = None;
                        }
                        Err(error) => {
                            state.snapshot.state = OutputDeviceDiscoveryState::Failed;
                            state.snapshot.error = Some(error);
                        }
                    }
                    state.pending_since = None;
                    state.completed_at = Some(Instant::now());
                }
            });
        if let Err(error) = worker {
            let mut state = owner
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            state.snapshot.state = OutputDeviceDiscoveryState::Failed;
            state.snapshot.error = Some(format!("audio output discovery could not start: {error}"));
            state.completed_at = Some(Instant::now());
        } else {
            owner.request_at(Instant::now());
        }
        owner
    }

    fn request_at(&self, now: Instant) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.pending_since.is_some()
            || state
                .completed_at
                .is_some_and(|at| now.saturating_duration_since(at) < REFRESH_INTERVAL)
        {
            return;
        }
        state.pending_since = Some(now);
        state.snapshot.state = OutputDeviceDiscoveryState::Loading;
        state.snapshot.error = None;
        if let Err(error) = self.refresh.try_send(()) {
            state.pending_since = None;
            state.completed_at = Some(now);
            state.snapshot.state = OutputDeviceDiscoveryState::Failed;
            state.snapshot.error = Some(format!("audio output discovery is unavailable: {error}"));
        }
    }

    pub(super) fn snapshot(&self) -> OutputDeviceInventory {
        self.snapshot_at(Instant::now())
    }

    fn snapshot_at(&self, now: Instant) -> OutputDeviceInventory {
        self.request_at(now);
        let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        let mut snapshot = state.snapshot.clone();
        if state
            .pending_since
            .is_some_and(|at| now.saturating_duration_since(at) >= STALLED_AFTER)
        {
            snapshot.state = OutputDeviceDiscoveryState::Stalled;
        }
        snapshot
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn blocked_discovery_is_single_flight_and_snapshot_and_drop_never_wait_for_it() {
        let (entered, entry) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let gate = Mutex::new(gate);
        let count = Arc::new(AtomicUsize::new(0));
        let calls = count.clone();
        let inventory = Inventory::start(Arc::new(move || {
            calls.fetch_add(1, Ordering::SeqCst);
            entered.send(()).unwrap();
            gate.lock().unwrap().recv().unwrap();
            Ok(vec!["Desk output".into()])
        }));
        entry.recv_timeout(Duration::from_secs(2)).unwrap();
        for _ in 0..100 {
            let snapshot = inventory.snapshot();
            assert_eq!(snapshot.state, OutputDeviceDiscoveryState::Loading);
            assert!(!snapshot.has_successful_snapshot);
        }
        assert_eq!(
            inventory.snapshot_at(Instant::now() + STALLED_AFTER).state,
            OutputDeviceDiscoveryState::Stalled
        );
        assert_eq!(count.load(Ordering::SeqCst), 1);
        let observer = inventory.clone();
        drop(inventory);
        release.send(()).unwrap();
        wait_for(&observer, OutputDeviceDiscoveryState::Ready);
        assert_eq!(observer.snapshot().devices, ["Desk output"]);
    }

    #[test]
    fn actual_failure_retains_last_successful_names_then_recovers_without_parallel_workers() {
        let count = Arc::new(AtomicUsize::new(0));
        let calls = count.clone();
        let inventory = Inventory::start(Arc::new(move || {
            match calls.fetch_add(1, Ordering::SeqCst) {
                0 => Ok(vec!["Existing".into()]),
                1 => Err("enumeration rejected".into()),
                _ => Ok(vec!["Replacement".into()]),
            }
        }));
        wait_for(&inventory, OutputDeviceDiscoveryState::Ready);
        inventory.request_at(Instant::now() + REFRESH_INTERVAL);
        wait_for(&inventory, OutputDeviceDiscoveryState::Failed);
        let failed = inventory.snapshot();
        assert_eq!(failed.devices, ["Existing"]);
        assert!(failed.has_successful_snapshot);
        assert_eq!(failed.error.as_deref(), Some("enumeration rejected"));
        inventory.request_at(Instant::now() + REFRESH_INTERVAL);
        wait_for(&inventory, OutputDeviceDiscoveryState::Ready);
        assert_eq!(inventory.snapshot().devices, ["Replacement"]);
        assert!(inventory.snapshot().error.is_none());
        assert_eq!(count.load(Ordering::SeqCst), 3);
    }

    fn wait_for(inventory: &Inventory, expected: OutputDeviceDiscoveryState) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while inventory.snapshot().state != expected {
            assert!(
                Instant::now() < deadline,
                "discovery did not publish {expected:?}"
            );
            std::thread::yield_now();
        }
    }
}
