use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use light_extensions_contract::{
    CanonicalControlIntent, Configure, DRAFT_PROTOCOL_V1, DeviceActionDeclaration,
    DeviceActionRequest, DeviceActionResult, ExtensionCapability, ExtensionHello, FeedbackDelta,
    HandshakeError, HandshakeExpectations, HealthReport, HostHello, Message, Shutdown,
    ShutdownReason, TelemetryChannelDeclaration, TelemetryQuality, negotiate, validate_capability,
    validate_control_input, validate_device_action_request, validate_device_action_result,
    validate_telemetry_sample, validate_timecode_sample,
};

use crate::bounded::BoundedLog;
use crate::manifest::FeedbackFeature;
use crate::ports::{
    BoundControlInput, ExtensionApplicationPorts, HostControlContext, TelemetryEnvelope,
    TimecodeEnvelope,
};
use crate::session::{ReaderEvent, WriterCommand, reader_loop, stderr_loop, writer_loop};

const SOURCE: &str = "native_extension";

#[derive(Clone, Debug)]
pub struct ExtensionSpec {
    pub program: PathBuf,
    pub extension_id: String,
    pub extension_instance_id: String,
    pub extension_version: String,
    pub approved_package_digest: String,
    pub desk_id: String,
    pub channel_credential: String,
    pub requested_capabilities: BTreeSet<ExtensionCapability>,
    pub feedback_features: BTreeSet<FeedbackFeature>,
    pub telemetry_channels: Vec<TelemetryChannelDeclaration>,
    pub maximum_telemetry_rate_hz: u32,
    pub device_actions: Vec<DeviceActionDeclaration>,
    pub settings: BTreeMap<String, serde_json::Value>,
    pub control_bindings: BTreeMap<String, CanonicalControlIntent>,
    /// Test/package-owned variables. Identity and credential variables below override collisions.
    pub environment: BTreeMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct ExtensionLimits {
    pub handshake_timeout: Duration,
    pub shutdown_timeout: Duration,
    pub first_restart_backoff: Duration,
    pub maximum_restart_backoff: Duration,
    pub maximum_failures: u32,
    pub command_queue: usize,
    pub inbound_queue: usize,
    pub wire_queue: usize,
    pub stderr_queue: usize,
    pub log_bytes: usize,
    pub log_lines: usize,
    pub telemetry_history_samples: usize,
}

impl Default for ExtensionLimits {
    fn default() -> Self {
        Self {
            handshake_timeout: Duration::from_secs(2),
            shutdown_timeout: Duration::from_millis(500),
            first_restart_backoff: Duration::from_millis(250),
            maximum_restart_backoff: Duration::from_secs(5),
            maximum_failures: 5,
            command_queue: 64,
            inbound_queue: 64,
            wire_queue: 16,
            stderr_queue: 64,
            log_bytes: 64 * 1024,
            log_lines: 512,
            telemetry_history_samples: 128,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExtensionState {
    Starting,
    Handshaking,
    Running,
    Restarting { failures: u32, delay: Duration },
    Terminal { failures: u32 },
    Stopped,
}

#[derive(Clone, Debug)]
pub struct HostHealth {
    pub state: ExtensionState,
    pub launches: u64,
    pub crashes: u64,
    pub protocol_errors: u64,
    pub inbound_drops: u64,
    pub outbound_drops: u64,
    pub log_drops: u64,
    pub last_error: Option<String>,
    pub extension_health: Option<HealthReport>,
    pub log_lines: Vec<String>,
    pub log_bytes: usize,
    pub telemetry: BTreeMap<String, TelemetryChannelHealth>,
    pub device_action_results: Vec<DeviceActionResult>,
}

#[derive(Clone, Debug)]
pub struct TelemetryChannelHealth {
    pub latest: Option<TelemetryEnvelope>,
    pub history: Vec<TelemetryEnvelope>,
    pub accepted_samples: u64,
    pub lost_samples: u64,
    pub invalid_samples: u64,
    pub stale_samples: u64,
    pub excess_rate_samples: u64,
    pub stale: bool,
    pub last_sequence: Option<u64>,
    expected_interval_micros: Option<u64>,
}

struct SharedHealth {
    value: Mutex<HostHealth>,
    inbound_drops: Arc<AtomicU64>,
    stderr_transport_drops: Arc<AtomicU64>,
}

impl SharedHealth {
    fn new(channels: &[TelemetryChannelDeclaration]) -> Self {
        let telemetry = channels
            .iter()
            .map(|channel| {
                (
                    channel.channel_id.clone(),
                    TelemetryChannelHealth {
                        latest: None,
                        history: Vec::new(),
                        accepted_samples: 0,
                        lost_samples: 0,
                        invalid_samples: 0,
                        stale_samples: 0,
                        excess_rate_samples: 0,
                        stale: false,
                        last_sequence: None,
                        expected_interval_micros: channel.expected_interval_micros,
                    },
                )
            })
            .collect();
        Self {
            value: Mutex::new(HostHealth {
                state: ExtensionState::Starting,
                launches: 0,
                crashes: 0,
                protocol_errors: 0,
                inbound_drops: 0,
                outbound_drops: 0,
                log_drops: 0,
                last_error: None,
                extension_health: None,
                log_lines: Vec::new(),
                log_bytes: 0,
                telemetry,
                device_action_results: Vec::new(),
            }),
            inbound_drops: Arc::new(AtomicU64::new(0)),
            stderr_transport_drops: Arc::new(AtomicU64::new(0)),
        }
    }

    fn snapshot(&self) -> HostHealth {
        let mut value = self.value.lock().expect("extension health mutex").clone();
        value.inbound_drops = self.inbound_drops.load(Ordering::Relaxed);
        value.log_drops += self.stderr_transport_drops.load(Ordering::Relaxed);
        let now = unix_time_micros();
        for channel in value.telemetry.values_mut() {
            channel.stale = channel
                .latest
                .as_ref()
                .zip(channel.expected_interval_micros)
                .is_some_and(|(latest, expected)| {
                    now.saturating_sub(latest.received_at_micros) > expected.saturating_mul(2)
                });
        }
        value
    }
}

enum SupervisorCommand {
    Feedback(FeedbackDelta),
    DeviceAction(DeviceActionRequest),
    RefreshSnapshot,
    Restart,
    Stop,
}

struct ActiveSession {
    child: Child,
    writer_tx: SyncSender<WriterCommand>,
    writer: JoinHandle<()>,
    reader: JoinHandle<()>,
    logger: JoinHandle<()>,
    reader_rx: Receiver<ReaderEvent>,
    log_rx: Receiver<String>,
    logs: BoundedLog,
    inbound_drops_before: u64,
    capabilities: BTreeSet<ExtensionCapability>,
    context: HostControlContext,
    last_input_id: Option<u64>,
    held_controls: BTreeSet<String>,
    last_timecode_id: Option<u64>,
    pending_device_actions: BTreeMap<u64, String>,
}

#[derive(Clone, Copy)]
enum RequestedSessionExit {
    Restart,
    Stop,
    WriterStopped,
}

impl ActiveSession {
    fn requested(mut self, exit: RequestedSessionExit, timeout: Duration) -> SessionResult {
        match exit {
            RequestedSessionExit::Restart | RequestedSessionExit::Stop => {
                graceful_stop(&mut self.child, &self.writer_tx, timeout);
                close_threads(self.writer_tx, self.writer, self.reader, self.logger);
                match exit {
                    RequestedSessionExit::Restart => SessionResult::ExplicitRestart,
                    RequestedSessionExit::Stop => SessionResult::Stopped,
                    RequestedSessionExit::WriterStopped => unreachable!(),
                }
            }
            RequestedSessionExit::WriterStopped => {
                self.failed("child writer stopped".into(), (true, false))
            }
        }
    }

    fn failed(self, detail: String, failure: (bool, bool)) -> SessionResult {
        finish_failed(
            self.child,
            self.writer_tx,
            self.writer,
            self.reader,
            self.logger,
            detail,
            failure,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedbackEnqueueError {
    QueueFull,
    SnapshotRepairPending,
    Stopped,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeviceActionEnqueueError {
    UndeclaredOrDenied,
    InvalidParameters,
    QueueFull,
    Stopped,
}

pub struct ExtensionHost<P: ExtensionApplicationPorts> {
    spec: ExtensionSpec,
    limits: ExtensionLimits,
    ports: Arc<P>,
}

impl<P: ExtensionApplicationPorts> ExtensionHost<P> {
    pub fn new(spec: ExtensionSpec, limits: ExtensionLimits, ports: Arc<P>) -> Self {
        Self {
            spec,
            limits,
            ports,
        }
    }

    pub fn start(self) -> RunningExtension {
        let shared = Arc::new(SharedHealth::new(&self.spec.telemetry_channels));
        let repair = Arc::new(AtomicBool::new(false));
        let (commands, receiver) = std::sync::mpsc::sync_channel(self.limits.command_queue.max(1));
        let thread_shared = Arc::clone(&shared);
        let thread_repair = Arc::clone(&repair);
        let device_actions = self.spec.device_actions.clone();
        let worker = std::thread::Builder::new()
            .name(format!("extension-{}", self.spec.extension_id))
            .spawn(move || self.supervise(receiver, thread_shared, thread_repair))
            .expect("extension supervisor thread starts");
        RunningExtension {
            commands: Some(commands),
            shared,
            repair,
            device_actions,
            worker: Some(worker),
        }
    }

    fn supervise(
        self,
        commands: Receiver<SupervisorCommand>,
        shared: Arc<SharedHealth>,
        repair: Arc<AtomicBool>,
    ) {
        let mut failures = 0_u32;
        let mut attempt = 0_u64;
        loop {
            attempt += 1;
            set_state(&shared, ExtensionState::Starting);
            let result = self.run_one(attempt, &commands, &shared, &repair);
            match result {
                SessionResult::Stopped => {
                    set_state(&shared, ExtensionState::Stopped);
                    return;
                }
                SessionResult::ExplicitRestart => {
                    failures = 0;
                    continue;
                }
                SessionResult::Failed {
                    detail,
                    crashed,
                    protocol,
                } => {
                    failures += 1;
                    let mut health = shared.value.lock().expect("extension health mutex");
                    if crashed {
                        health.crashes += 1;
                    }
                    if protocol {
                        health.protocol_errors += 1;
                    }
                    health.last_error = Some(detail);
                    if failures >= self.limits.maximum_failures.max(1) {
                        health.state = ExtensionState::Terminal { failures };
                        drop(health);
                        loop {
                            match commands.recv() {
                                Ok(SupervisorCommand::Restart) => {
                                    failures = 0;
                                    break;
                                }
                                Ok(SupervisorCommand::Feedback(_))
                                | Ok(SupervisorCommand::DeviceAction(_)) => {
                                    repair.store(true, Ordering::Release);
                                    shared
                                        .value
                                        .lock()
                                        .expect("extension health mutex")
                                        .outbound_drops += 1;
                                }
                                Ok(SupervisorCommand::RefreshSnapshot) => {
                                    repair.store(true, Ordering::Release)
                                }
                                Ok(SupervisorCommand::Stop) | Err(_) => {
                                    set_state(&shared, ExtensionState::Stopped);
                                    return;
                                }
                            }
                        }
                        continue;
                    }
                    let delay = backoff(&self.limits, failures);
                    health.state = ExtensionState::Restarting { failures, delay };
                    drop(health);
                    match commands.recv_timeout(delay) {
                        Ok(SupervisorCommand::Stop) | Err(RecvTimeoutError::Disconnected) => {
                            set_state(&shared, ExtensionState::Stopped);
                            return;
                        }
                        Ok(SupervisorCommand::Restart) => failures = 0,
                        Ok(SupervisorCommand::Feedback(_))
                        | Ok(SupervisorCommand::DeviceAction(_)) => {
                            repair.store(true, Ordering::Release);
                            shared
                                .value
                                .lock()
                                .expect("extension health mutex")
                                .outbound_drops += 1;
                        }
                        Ok(SupervisorCommand::RefreshSnapshot) => {
                            repair.store(true, Ordering::Release)
                        }
                        Err(RecvTimeoutError::Timeout) => {}
                    }
                }
            }
        }
    }

    fn run_one(
        &self,
        attempt: u64,
        commands: &Receiver<SupervisorCommand>,
        shared: &Arc<SharedHealth>,
        repair: &Arc<AtomicBool>,
    ) -> SessionResult {
        let mut session = match self.prepare_session(attempt, shared, repair) {
            Ok(session) => session,
            Err(result) => return result,
        };

        loop {
            drain_logs(&session.log_rx, &mut session.logs, shared);
            if repair.load(Ordering::Acquire)
                && let Err(exit) = self.repair_feedback(&mut session, commands, shared, repair)
            {
                return session.requested(exit, self.limits.shutdown_timeout);
            }
            match session.child.try_wait() {
                Ok(Some(status)) => {
                    let detail = exit_detail(status);
                    return session.failed(detail, (true, false));
                }
                Err(error) => {
                    let detail = error.to_string();
                    return session.failed(detail, (true, false));
                }
                Ok(None) => {}
            }
            if let Err(exit) = self.forward_command(&mut session, commands, shared, repair) {
                return session.requested(exit, self.limits.shutdown_timeout);
            }
            match session.reader_rx.recv_timeout(Duration::from_millis(2)) {
                Ok(ReaderEvent::Frame(frame)) => {
                    if let Err(detail) = self.handle_message(
                        frame.message,
                        &session.capabilities,
                        &session.context,
                        &session.writer_tx,
                        shared,
                        &mut session.last_input_id,
                        &mut session.held_controls,
                        &mut session.last_timecode_id,
                        &mut session.pending_device_actions,
                    ) {
                        return session.failed(detail, (false, true));
                    }
                }
                Ok(ReaderEvent::Protocol(detail)) => {
                    return session.failed(detail, (false, true));
                }
                Ok(ReaderEvent::Closed) => {
                    return session.failed("extension stdout closed".into(), (true, false));
                }
                Err(RecvTimeoutError::Disconnected) => {
                    let overflowed =
                        shared.inbound_drops.load(Ordering::Relaxed) > session.inbound_drops_before;
                    return session.failed(
                        if overflowed {
                            "extension inbound queue overflowed; reconnect requires a full snapshot"
                                .into()
                        } else {
                            "extension reader stopped".into()
                        },
                        if overflowed {
                            (false, true)
                        } else {
                            (true, false)
                        },
                    );
                }
                Err(RecvTimeoutError::Timeout) => {}
            }
        }
    }

    fn spawn(&self, attempt: u64) -> Result<Child, String> {
        let mut command = Command::new(&self.spec.program);
        command
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in &self.spec.environment {
            command.env(key, value);
        }
        command
            .env("TOSKLIGHT_EXTENSION_ID", &self.spec.extension_id)
            .env(
                "TOSKLIGHT_EXTENSION_INSTANCE_ID",
                &self.spec.extension_instance_id,
            )
            .env(
                "TOSKLIGHT_EXTENSION_PACKAGE_DIGEST",
                &self.spec.approved_package_digest,
            )
            .env(
                "TOSKLIGHT_EXTENSION_CHANNEL_CREDENTIAL",
                &self.spec.channel_credential,
            )
            .env("TOSKLIGHT_EXTENSION_LAUNCH_ATTEMPT", attempt.to_string());
        command
            .spawn()
            .map_err(|error| format!("could not start {}: {error}", self.spec.program.display()))
    }

    fn context(&self) -> HostControlContext {
        HostControlContext {
            extension_id: self.spec.extension_id.clone(),
            extension_instance_id: self.spec.extension_instance_id.clone(),
            desk_id: self.spec.desk_id.clone(),
            source: SOURCE,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_message(
        &self,
        message: Message,
        capabilities: &BTreeSet<ExtensionCapability>,
        context: &HostControlContext,
        writer: &SyncSender<WriterCommand>,
        shared: &Arc<SharedHealth>,
        last_input_id: &mut Option<u64>,
        held_controls: &mut BTreeSet<String>,
        last_timecode_id: &mut Option<u64>,
        pending_device_actions: &mut BTreeMap<u64, String>,
    ) -> Result<(), String> {
        validate_capability(&message, capabilities).map_err(|error| error.to_string())?;
        match message {
            Message::ControlInput(input) => {
                self.handle_control_input(input, context, writer, last_input_id, held_controls)?
            }
            Message::TelemetrySample(sample) => self.handle_telemetry_sample(sample, shared)?,
            Message::Health(report) => {
                shared
                    .value
                    .lock()
                    .expect("extension health mutex")
                    .extension_health = Some(report);
            }
            Message::TimecodeSample(sample) => {
                validate_timecode_sample(&sample).map_err(|error| error.to_string())?;
                if last_timecode_id.is_some_and(|previous| sample.sample_id <= previous) {
                    return Err(format!(
                        "timecode sample_id {} is stale or duplicated",
                        sample.sample_id
                    ));
                }
                *last_timecode_id = Some(sample.sample_id);
                self.ports
                    .publish_timecode(TimecodeEnvelope {
                        extension_id: self.spec.extension_id.clone(),
                        extension_instance_id: self.spec.extension_instance_id.clone(),
                        received_at_micros: unix_time_micros(),
                        sample,
                    })
                    .map_err(|error| error.to_string())?;
            }
            Message::DeviceActionResult(result) => {
                self.handle_device_action_result(result, pending_device_actions, shared)?
            }
            Message::Shutdown(shutdown) => {
                return Err(format!(
                    "extension requested shutdown: {:?}",
                    shutdown.reason
                ));
            }
            Message::ProtocolError(error) => return Err(error.detail),
            Message::HostHello(_)
            | Message::ExtensionHello(_)
            | Message::Configure(_)
            | Message::DeviceActionRequest(_)
            | Message::FeedbackSnapshot(_)
            | Message::FeedbackDelta(_) => {
                return Err(
                    "extension sent a message reserved for the host or a later phase".into(),
                );
            }
        }
        Ok(())
    }
}

include!("supervisor/message_handlers.rs");
include!("supervisor/session_runtime.rs");

fn unix_time_micros() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros()
        .try_into()
        .unwrap_or(u64::MAX)
}

pub struct RunningExtension {
    commands: Option<SyncSender<SupervisorCommand>>,
    shared: Arc<SharedHealth>,
    repair: Arc<AtomicBool>,
    device_actions: Vec<DeviceActionDeclaration>,
    worker: Option<JoinHandle<()>>,
}

impl RunningExtension {
    pub fn health(&self) -> HostHealth {
        self.shared.snapshot()
    }

    pub fn feedback_delta(&self, delta: FeedbackDelta) -> Result<(), FeedbackEnqueueError> {
        if self.repair.load(Ordering::Acquire) {
            self.shared
                .value
                .lock()
                .expect("extension health mutex")
                .outbound_drops += 1;
            return Err(FeedbackEnqueueError::SnapshotRepairPending);
        }
        let Some(commands) = &self.commands else {
            return Err(FeedbackEnqueueError::Stopped);
        };
        match commands.try_send(SupervisorCommand::Feedback(delta)) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => {
                self.repair.store(true, Ordering::Release);
                self.shared
                    .value
                    .lock()
                    .expect("extension health mutex")
                    .outbound_drops += 1;
                Err(FeedbackEnqueueError::QueueFull)
            }
            Err(TrySendError::Disconnected(_)) => Err(FeedbackEnqueueError::Stopped),
        }
    }

    pub fn device_action(
        &self,
        request: DeviceActionRequest,
    ) -> Result<(), DeviceActionEnqueueError> {
        match validate_device_action_request(&request, &self.device_actions) {
            Ok(()) => {}
            Err(HandshakeError::UndeclaredDeviceAction(_)) => {
                return Err(DeviceActionEnqueueError::UndeclaredOrDenied);
            }
            Err(_) => return Err(DeviceActionEnqueueError::InvalidParameters),
        }
        let Some(commands) = &self.commands else {
            return Err(DeviceActionEnqueueError::Stopped);
        };
        match commands.try_send(SupervisorCommand::DeviceAction(request)) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => Err(DeviceActionEnqueueError::QueueFull),
            Err(TrySendError::Disconnected(_)) => Err(DeviceActionEnqueueError::Stopped),
        }
    }

    pub fn restart(&self) -> Result<(), FeedbackEnqueueError> {
        self.commands
            .as_ref()
            .ok_or(FeedbackEnqueueError::Stopped)?
            .send(SupervisorCommand::Restart)
            .map_err(|_| FeedbackEnqueueError::Stopped)
    }

    pub fn refresh_snapshot(&self) -> Result<(), FeedbackEnqueueError> {
        self.commands
            .as_ref()
            .ok_or(FeedbackEnqueueError::Stopped)?
            .try_send(SupervisorCommand::RefreshSnapshot)
            .map_err(|error| match error {
                TrySendError::Full(_) => FeedbackEnqueueError::QueueFull,
                TrySendError::Disconnected(_) => FeedbackEnqueueError::Stopped,
            })
    }

    pub fn stop(&mut self) {
        if let Some(commands) = self.commands.take() {
            let _ = commands.send(SupervisorCommand::Stop);
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for RunningExtension {
    fn drop(&mut self) {
        self.stop();
    }
}

enum SessionResult {
    Stopped,
    ExplicitRestart,
    Failed {
        detail: String,
        crashed: bool,
        protocol: bool,
    },
}

impl SessionResult {
    fn failed(detail: String, crashed: bool, protocol: bool) -> Self {
        Self::Failed {
            detail,
            crashed,
            protocol,
        }
    }
}

fn wait_for_hello(
    child: &mut Child,
    reader: &Receiver<ReaderEvent>,
    logs: &Receiver<String>,
    bounded_logs: &mut BoundedLog,
    shared: &Arc<SharedHealth>,
    timeout: Duration,
) -> Result<ExtensionHello, (String, bool, bool)> {
    let deadline = Instant::now() + timeout;
    loop {
        drain_logs(logs, bounded_logs, shared);
        match child.try_wait() {
            Ok(Some(status)) => return Err((exit_detail(status), true, false)),
            Err(error) => return Err((error.to_string(), true, false)),
            Ok(None) => {}
        }
        let now = Instant::now();
        if now >= deadline {
            return Err(("extension handshake deadline exceeded".into(), false, true));
        }
        match reader.recv_timeout((deadline - now).min(Duration::from_millis(10))) {
            Ok(ReaderEvent::Frame(frame)) => match frame.message {
                Message::ExtensionHello(hello) => return Ok(hello),
                other => {
                    return Err((
                        format!("expected extension hello, received {other:?}"),
                        false,
                        true,
                    ));
                }
            },
            Ok(ReaderEvent::Protocol(detail)) => return Err((detail, false, true)),
            Ok(ReaderEvent::Closed) => return Err(("extension stdout closed".into(), true, false)),
            Err(RecvTimeoutError::Disconnected) => {
                return Err(("extension reader stopped".into(), true, false));
            }
            Err(RecvTimeoutError::Timeout) => {}
        }
    }
}

fn finish_failed(
    mut child: Child,
    writer_tx: SyncSender<WriterCommand>,
    writer: JoinHandle<()>,
    reader: JoinHandle<()>,
    logger: JoinHandle<()>,
    detail: String,
    failure: (bool, bool),
) -> SessionResult {
    let (crashed, protocol) = failure;
    let _ = child.kill();
    let _ = child.wait();
    close_threads(writer_tx, writer, reader, logger);
    SessionResult::failed(detail, crashed, protocol)
}

fn graceful_stop(child: &mut Child, writer: &SyncSender<WriterCommand>, timeout: Duration) {
    let _ = writer.try_send(WriterCommand::Message(Box::new(Message::Shutdown(
        Shutdown {
            reason: ShutdownReason::HostRequested,
            detail: None,
        },
    ))));
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(_)) | Err(_) => return,
            Ok(None) => std::thread::sleep(Duration::from_millis(2)),
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn close_threads(
    writer_tx: SyncSender<WriterCommand>,
    writer: JoinHandle<()>,
    reader: JoinHandle<()>,
    logger: JoinHandle<()>,
) {
    let _ = writer_tx.try_send(WriterCommand::Close);
    drop(writer_tx);
    let _ = writer.join();
    let _ = reader.join();
    let _ = logger.join();
}

fn drain_logs(logs: &Receiver<String>, bounded: &mut BoundedLog, shared: &Arc<SharedHealth>) {
    while let Ok(line) = logs.try_recv() {
        let dropped = bounded.push(line);
        let (lines, bytes) = bounded.snapshot();
        let mut health = shared.value.lock().expect("extension health mutex");
        health.log_drops += dropped;
        health.log_lines = lines;
        health.log_bytes = bytes;
    }
}

fn set_state(shared: &Arc<SharedHealth>, state: ExtensionState) {
    shared.value.lock().expect("extension health mutex").state = state;
}

fn exit_detail(status: ExitStatus) -> String {
    match status.code() {
        Some(code) => format!("extension exited with status {code}"),
        None => "extension process terminated".to_owned(),
    }
}

fn backoff(limits: &ExtensionLimits, failures: u32) -> Duration {
    let multiplier = 2_u32.saturating_pow(failures.saturating_sub(1).min(16));
    (limits.first_restart_backoff * multiplier).min(limits.maximum_restart_backoff)
}

fn fresh_channel_challenge() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn degrade_feedback_snapshot(
    snapshot: &mut light_extensions_contract::FeedbackSnapshot,
    features: &BTreeSet<FeedbackFeature>,
) {
    for value in snapshot.controls.values_mut() {
        degrade_feedback_value(value, features);
    }
}

fn degrade_feedback_delta(
    delta: &mut light_extensions_contract::FeedbackDelta,
    features: &BTreeSet<FeedbackFeature>,
) {
    for change in &mut delta.changes {
        if let Some(value) = &mut change.value {
            degrade_feedback_value(value, features);
        }
    }
}

fn degrade_feedback_value(
    value: &mut light_extensions_contract::FeedbackValue,
    features: &BTreeSet<FeedbackFeature>,
) {
    use light_extensions_contract::{FeedbackValue, LampState};
    match value {
        FeedbackValue::Rgb { red, green, blue }
            if !features.contains(&FeedbackFeature::RgbColor) =>
        {
            *value = FeedbackValue::Boolean(*red != 0 || *green != 0 || *blue != 0);
        }
        FeedbackValue::Control(state) => {
            if !features.contains(&FeedbackFeature::RgbColor) {
                state.resolved_rgb = None;
            }
            if !features.contains(&FeedbackFeature::SemanticColor) {
                state.semantic_color = None;
            }
            if !features.contains(&FeedbackFeature::Blink)
                && matches!(state.lamp, LampState::BlinkSlow | LampState::BlinkFast)
            {
                state.lamp = LampState::On;
            }
            if !features.contains(&FeedbackFeature::Lamp) {
                state.lamp = LampState::Off;
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restart_backoff_grows_and_caps() {
        let limits = ExtensionLimits {
            first_restart_backoff: Duration::from_millis(10),
            maximum_restart_backoff: Duration::from_millis(25),
            ..ExtensionLimits::default()
        };
        assert_eq!(backoff(&limits, 1), Duration::from_millis(10));
        assert_eq!(backoff(&limits, 2), Duration::from_millis(20));
        assert_eq!(backoff(&limits, 3), Duration::from_millis(25));
    }

    #[test]
    fn every_process_launch_gets_a_fresh_unpredictable_challenge() {
        let first = fresh_channel_challenge();
        let second = fresh_channel_challenge();
        assert_ne!(first, second);
        assert!(uuid::Uuid::parse_str(&first).is_ok());
        assert!(uuid::Uuid::parse_str(&second).is_ok());
    }

    #[test]
    fn feedback_color_and_blink_degrade_deterministically_to_device_features() {
        use light_extensions_contract::{FeedbackControlState, FeedbackValue, LampState};
        let source = FeedbackValue::Control(FeedbackControlState {
            selected: true,
            lamp: LampState::BlinkSlow,
            semantic_color: Some("warning".into()),
            resolved_rgb: Some([255, 128, 0]),
            ..FeedbackControlState::default()
        });
        let project = |features: BTreeSet<FeedbackFeature>| {
            let mut value = source.clone();
            degrade_feedback_value(&mut value, &features);
            value
        };

        let FeedbackValue::Control(rgb) = project(BTreeSet::from([
            FeedbackFeature::Lamp,
            FeedbackFeature::Blink,
            FeedbackFeature::RgbColor,
        ])) else {
            panic!("RGB projection must remain structured");
        };
        assert_eq!(rgb.resolved_rgb, Some([255, 128, 0]));
        assert_eq!(rgb.semantic_color, None);
        assert_eq!(rgb.lamp, LampState::BlinkSlow);

        let FeedbackValue::Control(indexed) = project(BTreeSet::from([
            FeedbackFeature::Lamp,
            FeedbackFeature::Blink,
            FeedbackFeature::SemanticColor,
        ])) else {
            panic!("indexed projection must remain structured");
        };
        assert_eq!(indexed.resolved_rgb, None);
        assert_eq!(indexed.semantic_color.as_deref(), Some("warning"));
        assert_eq!(indexed.lamp, LampState::BlinkSlow);

        let FeedbackValue::Control(monochrome) = project(BTreeSet::from([FeedbackFeature::Lamp]))
        else {
            panic!("monochrome projection must remain structured");
        };
        assert_eq!(monochrome.resolved_rgb, None);
        assert_eq!(monochrome.semantic_color, None);
        assert_eq!(monochrome.lamp, LampState::On);
    }
}
