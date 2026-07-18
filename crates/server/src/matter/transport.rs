//! Commissionable Matter-over-IP transport for the playback bridge.
//!
//! `MatterTransport` owns a dedicated thread because `rs-matter` deliberately supports
//! single-threaded executors and its data-model futures do not need to be `Send`. The server
//! communicates with that thread through bounded, non-blocking commands: reconciled playback
//! values flow into Matter subscriptions and controller writes flow back to the server's normal
//! playback dispatcher.

#[path = "transport/bridge/mod.rs"]
mod bridge;
#[path = "transport/commissioning.rs"]
mod commissioning;
#[path = "transport/mdns.rs"]
mod mdns;
#[path = "transport/model.rs"]
mod model;
#[path = "transport/node.rs"]
mod node;
#[path = "transport/runtime.rs"]
mod runtime;

#[cfg(test)]
#[path = "transport/tests.rs"]
mod tests;

use self::commissioning::{load_or_create_identity, pairing_data};
use self::model::{
    ControlCommand, EndpointShape, RuntimeHandle, TransportLight, endpoint_shape, validate_lights,
};
use self::runtime::run_transport;
use super::MatterPlaybackLight;
use parking_lot::{Mutex, RwLock};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

pub use self::model::{
    MatterPairingData, MatterRemoteWrite, MatterTransportLifecycle, MatterTransportSnapshot,
};

const START_TIMEOUT: Duration = Duration::from_secs(5);

struct TransportState {
    runtime: Option<RuntimeHandle>,
}

/// Lifecycle manager used by the HTTP/server layer.
///
/// Call [`Self::reconcile`] whenever the adapter snapshot changes. Changes limited to OnOff and
/// CurrentLevel are published in place. Adding, removing, or renaming an endpoint restarts only
/// this Matter service, preserving fabrics in `<data-dir>/matter/kv`.
pub struct MatterTransport {
    storage_dir: PathBuf,
    state: Mutex<TransportState>,
    snapshot: Arc<RwLock<MatterTransportSnapshot>>,
    remote_writes_rx: Mutex<Receiver<MatterRemoteWrite>>,
    remote_writes_tx: Sender<MatterRemoteWrite>,
}

impl std::fmt::Debug for MatterTransport {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MatterTransport")
            .field("storage_dir", &self.storage_dir)
            .field("snapshot", &self.snapshot())
            .finish_non_exhaustive()
    }
}

impl MatterTransport {
    /// Create a transport rooted below the server's desk data directory.
    pub fn new(data_dir: impl AsRef<Path>) -> Self {
        let (remote_writes_tx, remote_writes_rx) = mpsc::channel();
        Self {
            storage_dir: data_dir.as_ref().join("matter"),
            state: Mutex::new(TransportState { runtime: None }),
            snapshot: Arc::new(RwLock::new(MatterTransportSnapshot::default())),
            remote_writes_rx: Mutex::new(remote_writes_rx),
            remote_writes_tx,
        }
    }

    /// Reconcile enabled state, endpoint composition, and mirrored values.
    pub fn reconcile(
        &self,
        enabled: bool,
        lights: &[MatterPlaybackLight],
    ) -> MatterTransportSnapshot {
        let transport_lights = match validate_lights(lights) {
            Ok(lights) => lights,
            Err(error) => {
                self.stop();
                self.set_failed(error);
                return self.snapshot();
            }
        };
        if !enabled {
            self.stop();
            return self.snapshot();
        }

        let shape = endpoint_shape(&transport_lights);
        let mut state = self.state.lock();
        if runtime_needs_restart(state.runtime.as_ref(), &shape) {
            stop_runtime(&mut state.runtime);
            self.start_locked(&mut state, shape, transport_lights);
        } else {
            self.reconcile_running(&mut state, transport_lights);
        }
        drop(state);
        self.snapshot()
    }

    pub fn snapshot(&self) -> MatterTransportSnapshot {
        self.snapshot.read().clone()
    }

    /// Drain controller writes without blocking the server's output loop.
    pub fn drain_remote_writes(&self) -> Vec<MatterRemoteWrite> {
        let receiver = self.remote_writes_rx.lock();
        let mut writes = Vec::new();
        while let Ok(write) = receiver.try_recv() {
            writes.push(write);
        }
        writes
    }

    pub fn stop(&self) {
        let mut state = self.state.lock();
        stop_runtime(&mut state.runtime);
        *self.snapshot.write() = MatterTransportSnapshot::default();
    }

    fn reconcile_running(&self, state: &mut TransportState, lights: Vec<TransportLight>) {
        let channel_closed = state.runtime.as_ref().is_some_and(|runtime| {
            runtime
                .control
                .send(ControlCommand::Reconcile(lights))
                .is_err()
        });
        if channel_closed {
            stop_runtime(&mut state.runtime);
            self.set_failed("Matter transport control channel closed".into());
        }
    }

    fn start_locked(
        &self,
        state: &mut TransportState,
        shape: Vec<EndpointShape>,
        lights: Vec<TransportLight>,
    ) {
        let identity = match load_or_create_identity(&self.storage_dir) {
            Ok(identity) => identity,
            Err(error) => {
                self.set_failed(format!("Matter identity persistence failed: {error}"));
                return;
            }
        };
        let pairing = match pairing_data(&identity) {
            Ok(pairing) => pairing,
            Err(error) => {
                self.set_failed(format!("Matter pairing payload failed: {error}"));
                return;
            }
        };
        *self.snapshot.write() = MatterTransportSnapshot {
            lifecycle: MatterTransportLifecycle::Starting,
            endpoint_count: shape.len(),
            pairing: Some(pairing),
            ..MatterTransportSnapshot::default()
        };

        let (control_tx, control_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let storage_dir = self.storage_dir.clone();
        let shared_snapshot = self.snapshot.clone();
        let remote_writes = self.remote_writes_tx.clone();
        let thread_shape = shape.clone();
        let startup_error_tx = ready_tx.clone();
        let join = match thread::Builder::new()
            .name("tosklight-matter".into())
            .spawn(move || {
                let result = run_transport(
                    &storage_dir,
                    &identity,
                    &thread_shape,
                    lights,
                    remote_writes,
                    control_rx,
                    ready_tx,
                    &shared_snapshot,
                );
                if let Err(error) = result {
                    report_runtime_failure(error.to_string(), &startup_error_tx, &shared_snapshot);
                }
            }) {
            Ok(join) => join,
            Err(error) => {
                self.set_failed(format!("could not spawn Matter transport thread: {error}"));
                return;
            }
        };
        self.finish_start(state, shape, control_tx, ready_rx, join);
    }

    fn finish_start(
        &self,
        state: &mut TransportState,
        shape: Vec<EndpointShape>,
        control: Sender<ControlCommand>,
        ready: Receiver<Result<model::StartupReady, String>>,
        join: thread::JoinHandle<()>,
    ) {
        match ready.recv_timeout(START_TIMEOUT) {
            Ok(Ok(ready)) => {
                self.set_running(&ready);
                state.runtime = Some(RuntimeHandle {
                    shape,
                    control,
                    join,
                });
            }
            Ok(Err(error)) => {
                let _ = join.join();
                self.set_failed(error);
            }
            Err(_) => {
                let _ = control.send(ControlCommand::Shutdown);
                let _ = join.join();
                self.set_failed("Matter transport startup timed out".into());
            }
        }
    }

    fn set_running(&self, ready: &model::StartupReady) {
        let mut snapshot = self.snapshot.write();
        snapshot.lifecycle = MatterTransportLifecycle::Running;
        snapshot.network_running = true;
        snapshot.commissioned = ready.commissioned;
        snapshot.commissioning_window_open = ready.commissioning_window_open;
        snapshot.commissionable = ready.commissioning_window_open;
        snapshot.last_error = None;
    }

    fn set_failed(&self, error: String) {
        set_failed_snapshot(&mut self.snapshot.write(), error);
    }
}

impl Drop for MatterTransport {
    fn drop(&mut self) {
        stop_runtime(&mut self.state.get_mut().runtime);
    }
}

fn runtime_needs_restart(runtime: Option<&RuntimeHandle>, shape: &[EndpointShape]) -> bool {
    runtime.is_none_or(|runtime| runtime.shape != shape || runtime.join.is_finished())
}

fn stop_runtime(runtime: &mut Option<RuntimeHandle>) {
    if let Some(runtime) = runtime.take() {
        let _ = runtime.control.send(ControlCommand::Shutdown);
        let _ = runtime.join.join();
    }
}

fn report_runtime_failure(
    message: String,
    startup_error: &mpsc::SyncSender<Result<model::StartupReady, String>>,
    snapshot: &RwLock<MatterTransportSnapshot>,
) {
    let _ = startup_error.try_send(Err(message.clone()));
    set_failed_snapshot(&mut snapshot.write(), message);
}

fn set_failed_snapshot(snapshot: &mut MatterTransportSnapshot, error: String) {
    snapshot.lifecycle = MatterTransportLifecycle::Failed;
    snapshot.network_running = false;
    snapshot.commissionable = false;
    snapshot.commissioning_window_open = false;
    snapshot.last_error = Some(error);
}
