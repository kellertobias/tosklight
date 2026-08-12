//! Network-output scheduling and safe shutdown for the server runtime.

use super::capability_resources::{
    ActiveShowCoordinator, ActiveShowProjection, OutputControlCapability,
    OutputPersistenceResource, PlaybackRenderCapability,
};
use super::visualization_frame::VisualizationFrameHub;
use super::{
    ActionTimingResource, ApiError, AppState, OutputControl, PersistedOutputRuntime,
    playback_service,
};
use light_application::{
    PlaybackOperation, PlaybackShowScope, PlaybackUnitOfWork, automatic_playback_events,
};
use light_control::{
    ExternalTimecodeLossPolicy, SmpteTimecode, TimecodeRouter, TimecodeSourceTransition,
};
use light_core::{AttributeKey, AttributeValue, FixtureId, MergeMode, TimedValue, Universe};
use light_dynamics::ScalarSourceResolver;
use light_engine::{
    ContributionBatch, ContributionSample, Engine, EngineError, RenderOptions, RenderResult,
};
use light_output::{
    DmxFrame, NetworkOutput, OutputHealth, Protocol, UsbEndpointDocument, UsbOutputFanout,
    run_scheduler_dynamic_wakeable,
};
use light_playback::{PlaybackIdentity, TimecodeTransportAction, TimecodeTransportState};
use light_wire::v2::visualization::VisualizationScope;
use parking_lot::Mutex;
use std::{
    collections::{HashMap, HashSet},
    future::Future,
    io,
    net::IpAddr,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicU16, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

mod dynamic_projection;
mod dynamic_reconciliation;

use dynamic_projection::dynamic_contributions_with_auto_off;
#[cfg(test)]
pub(super) use dynamic_projection::{
    DynamicPlaybackControl, dynamic_transition_events, fully_controlled_dynamic_playbacks,
};
pub(in crate::runtime) use dynamic_projection::{
    ProgrammerReconciliationCache, dynamic_contributions_cached,
};
pub(super) use dynamic_projection::{dynamic_contributions, dynamic_projection};
use dynamic_reconciliation::{
    reconcile_cue_dynamics, reconcile_dynamic_playbacks, reconcile_programmer_dynamics,
};

type OutputSequences = HashMap<(Protocol, Universe), u8>;
type SharedSequences = Arc<tokio::sync::Mutex<OutputSequences>>;
pub(super) type OutputTask = Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'static>>;
const SLOW_OUTPUT_PHASE_THRESHOLD: Duration = Duration::from_millis(20);
const SLOW_OUTPUT_PHASE_SAMPLE_LIMIT: u64 = 128;
static SLOW_OUTPUT_PHASE_SAMPLES: AtomicU64 = AtomicU64::new(0);

pub(super) struct Config {
    pub bind_ip: IpAddr,
    pub engine: Arc<Engine>,
    pub health: Arc<std::sync::Mutex<OutputHealth>>,
    pub rate: Arc<AtomicU16>,
    pub timecode: Arc<Mutex<TimecodeRouter>>,
    pub timecodes: light_application::timeline::TimecodeRuntimeService,
    pub cancellation: CancellationToken,
    pub persisted_runtime: PersistedOutputRuntime,
    pub playback: PlaybackRenderCapability,
    pub active_show: ActiveShowProjection,
    pub activation: ActiveShowCoordinator,
    pub test_bench: bool,
    pub dynamics: Arc<Mutex<light_dynamics::DynamicRuntime>>,
    pub speed_groups: Arc<Mutex<[light_control::speed::SpeedGroupController; 5]>>,
    pub dynamic_auto_offs: Arc<Mutex<Vec<PlaybackIdentity>>>,
    pub visualization_frames: Arc<VisualizationFrameHub>,
    pub action_timing: ActionTimingResource,
    pub data_dir: std::path::PathBuf,
    pub internal_audio: Arc<Mutex<super::internal_audio::InternalAudioRuntime>>,
}

pub(super) struct OutputScheduler {
    pub(super) output: Arc<NetworkOutput>,
    pub(super) sequences: SharedSequences,
    pub(super) control: Arc<Mutex<OutputControl>>,
    pub(super) usb: Arc<UsbOutputFanout>,
    start: Option<tokio::sync::oneshot::Sender<()>>,
    task: OutputTask,
}

struct SharedResources {
    pub(super) output: Arc<NetworkOutput>,
    pub(super) sequences: SharedSequences,
    pub(super) control: Arc<Mutex<OutputControl>>,
    pub(super) usb: Arc<UsbOutputFanout>,
    persistence: OutputPersistenceResource,
    programmer_reconciliation_cache: Arc<ProgrammerReconciliationCache>,
}

#[derive(Clone)]
struct Runtime {
    pub(super) engine: Arc<Engine>,
    pub(super) output: Arc<NetworkOutput>,
    pub(super) sequences: SharedSequences,
    pub(super) control: Arc<Mutex<OutputControl>>,
    pub(super) usb: Arc<UsbOutputFanout>,
    pub(super) timecode: Arc<Mutex<TimecodeRouter>>,
    pub(super) timecodes: light_application::timeline::TimecodeRuntimeService,
    pub(super) playback: PlaybackRenderCapability,
    pub(super) active_show: ActiveShowProjection,
    pub(super) activation: ActiveShowCoordinator,
    pub(super) cancellation: CancellationToken,
    pub(super) dynamics: Arc<Mutex<light_dynamics::DynamicRuntime>>,
    pub(super) speed_groups: Arc<Mutex<[light_control::speed::SpeedGroupController; 5]>>,
    pub(super) rate: Arc<AtomicU16>,
    pub(super) dynamic_auto_offs: Arc<Mutex<Vec<PlaybackIdentity>>>,
    pub(super) visualization_frames: Arc<VisualizationFrameHub>,
    pub(super) action_timing: ActionTimingResource,
    pub(super) programmer_reconciliation_cache: Arc<ProgrammerReconciliationCache>,
    pub(super) persistence: OutputPersistenceResource,
    pub(super) internal_audio: Arc<Mutex<super::internal_audio::InternalAudioRuntime>>,
}

pub(super) async fn start(config: Config) -> anyhow::Result<OutputScheduler> {
    let resources = SharedResources::create(&config).await?;
    let runtime = resources.runtime(&config);
    let (start, ready) = tokio::sync::oneshot::channel();
    let task = task(
        runtime,
        config.rate,
        config.health,
        config.test_bench,
        ready,
    );
    Ok(resources.scheduler(start, task))
}

async fn bind_output(bind_ip: IpAddr) -> anyhow::Result<Arc<NetworkOutput>> {
    let cid = *Uuid::new_v4().as_bytes();
    Ok(Arc::new(NetworkOutput::bind(bind_ip, cid, "Light").await?))
}

fn create_control(runtime: &PersistedOutputRuntime) -> Arc<Mutex<OutputControl>> {
    Arc::new(Mutex::new(OutputControl {
        options: RenderOptions {
            grand_master: runtime.grand_master,
            blackout: runtime.blackout,
            control_loss_progress: None,
        },
        revision: runtime.revision,
        ..OutputControl::default()
    }))
}

fn task(
    runtime: Runtime,
    rate: Arc<AtomicU16>,
    health: Arc<std::sync::Mutex<OutputHealth>>,
    test_bench: bool,
    ready: tokio::sync::oneshot::Receiver<()>,
) -> OutputTask {
    Box::pin(async move {
        if !await_start(ready, &runtime.cancellation).await {
            return Ok(());
        }
        run(&runtime, rate, health, test_bench).await;
        shut_down_safely(&runtime).await;
        Ok(())
    })
}

async fn await_start(
    ready: tokio::sync::oneshot::Receiver<()>,
    cancellation: &CancellationToken,
) -> bool {
    tokio::select! {
        result = ready => result.is_ok(),
        _ = cancellation.cancelled() => false,
    }
}

async fn run(
    runtime: &Runtime,
    rate: Arc<AtomicU16>,
    health: Arc<std::sync::Mutex<OutputHealth>>,
    test_bench: bool,
) {
    if test_bench {
        runtime.cancellation.cancelled().await;
        return;
    }
    let cancellation = runtime.cancellation.clone();
    run_scheduler_dynamic_wakeable(
        rate,
        cancellation,
        health,
        runtime.action_timing.output_wake(),
        || render_tick(runtime.clone()),
    )
    .await;
}

// @tour one-action-end-to-end:30 Render semantic state into routed frames
// A scheduler tick advances timecode, renders authoritative engine state, maps universes into
// frames, and sends configured routes. Network I/O starts only after rendering completes.
async fn render_tick(runtime: Runtime) -> io::Result<u64> {
    let tick_started = Instant::now();
    let action_timing = runtime.action_timing.begin_output_render();
    if update_timecode(&runtime) {
        runtime.timecodes.tick();
    }
    let options = runtime.control.lock().render_options();
    let before_cues = runtime
        .engine
        .playback_runtime_status()
        .into_iter()
        .map(|status| (status.playback.cue_list_id, status.playback.current_cue_id))
        .collect::<HashMap<_, _>>();
    let (rendered, visualization_scope, dynamic, engine) = {
        let Ok(_activation) = runtime.activation.try_acquire() else {
            return send_retained_output(&runtime).await;
        };
        let visualization_scope = VisualizationScope {
            show_id: runtime.active_show.current().map(|show| show.id.0),
        };
        let dynamic_started = Instant::now();
        let (mut sampled, auto_offs, dynamic_events, _, _) = dynamic_contributions_with_auto_off(
            &runtime.engine,
            &runtime.dynamics,
            &runtime.speed_groups,
            &runtime.rate,
            &[],
            Some(&runtime.programmer_reconciliation_cache),
            true,
        );
        if !auto_offs.is_empty() {
            runtime.dynamic_auto_offs.lock().extend(auto_offs);
        }
        for event in dynamic_events {
            runtime.playback.publish(event);
        }
        let timecode_audio = timecode_audio_contributions(
            &runtime.timecodes,
            runtime.engine.snapshot().fixtures.as_ref(),
        );
        if !timecode_audio.is_empty() {
            sampled.push(timecode_audio);
        }
        let dynamic = dynamic_started.elapsed();
        let engine_started = Instant::now();
        let rendered = render_with_playback_events(
            &runtime.engine,
            &runtime.active_show,
            &runtime.playback,
            options,
            &sampled,
            Some(&runtime.persistence),
        )
        .map_err(io::Error::other)?;
        (
            rendered,
            visualization_scope,
            dynamic,
            engine_started.elapsed(),
        )
    };
    dispatch_automatic_cue_actions(
        &runtime.engine,
        &runtime.timecodes,
        &runtime.active_show,
        &before_cues,
    );
    runtime.internal_audio.lock().reconcile(
        runtime.engine.snapshot().fixtures.as_ref(),
        rendered.profile_visualization_values.as_ref(),
        rendered.resolved_changed_at.as_ref(),
    );
    let publish_started = Instant::now();
    let (routes, frames, patched_slots) = {
        let mut control = runtime.control.lock();
        if !control.hold {
            runtime
                .visualization_frames
                .publish(&rendered, options, visualization_scope);
        }
        output_payload(
            &mut control,
            rendered.routes,
            rendered.universes,
            rendered.patched_slots,
        )
    };
    let publish = publish_started.elapsed();
    let send_started = Instant::now();
    let network = runtime
        .output
        .send_routes(
            &routes,
            &frames,
            &patched_slots,
            &mut *runtime.sequences.lock().await,
        )
        .await;
    let usb = runtime.usb.enqueue_routes(&routes, &frames);
    let result = combined_delivery_result(network, usb);
    let send = send_started.elapsed();
    trace_slow_output_phases(tick_started.elapsed(), dynamic, engine, publish, send);
    if result.is_ok() {
        runtime.action_timing.complete_output_render(action_timing);
    }
    result
}

fn timecode_audio_contributions(
    timecodes: &light_application::timeline::TimecodeRuntimeService,
    fixtures: &[light_fixture::PatchedFixture],
) -> ContributionBatch {
    let rate = timecodes.frame_rate();
    let changed_at = chrono::Utc::now();
    let mut order = 0_u64;
    ContributionBatch::new(timecodes.snapshots().into_iter().flat_map(|snapshot| {
        let transport = match snapshot.transport {
            TimecodeTransportState::Stopped => return Vec::new(),
            TimecodeTransportState::Paused => 64_u32,
            TimecodeTransportState::Playing => 128_u32,
        };
        snapshot
            .reconstructed
            .audio_players
            .into_iter()
            .flat_map(|player| {
                let fixture_id = fixtures
                    .iter()
                    .find(|fixture| fixture.fixture_id == player.fixture_id)
                    .and_then(|fixture| fixture.logical_heads.first())
                    .map_or(player.fixture_id, |head| head.fixture_id);
                let cursor_millis = u32::try_from(
                    u128::from(player.cursor_frame.0)
                        .saturating_mul(u128::from(rate.denominator()))
                        .saturating_mul(1_000)
                        / u128::from(rate.numerator()),
                )
                .unwrap_or(u32::MAX);
                [
                    (
                        "audio.folder",
                        AttributeValue::RawDmxExact(u32::from(player.folder)),
                    ),
                    (
                        "audio.file",
                        AttributeValue::RawDmxExact(u32::from(player.file)),
                    ),
                    ("audio.transport", AttributeValue::RawDmxExact(transport)),
                    (
                        "audio.repeat",
                        AttributeValue::RawDmxExact(if player.repeat { 255 } else { 0 }),
                    ),
                    ("audio.volume", AttributeValue::Normalized(player.volume)),
                    (
                        "audio.cursor_millis",
                        AttributeValue::RawDmxExact(cursor_millis),
                    ),
                ]
                .into_iter()
                .map(|(attribute, value)| {
                    order = order.saturating_add(1);
                    ContributionSample::independent(TimedValue {
                        fixture_id,
                        attribute: AttributeKey(attribute.into()),
                        value,
                        priority: 75,
                        changed_at,
                        programmer_order: order,
                        merge_mode: MergeMode::Ltp,
                        fade: false,
                        fade_millis: None,
                        delay_millis: None,
                    })
                })
                .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    }))
}

fn dispatch_automatic_cue_actions(
    engine: &Engine,
    timecodes: &light_application::timeline::TimecodeRuntimeService,
    active_show: &ActiveShowProjection,
    before: &HashMap<light_core::CueListId, Option<Uuid>>,
) {
    let Some(show) = active_show.current() else {
        return;
    };
    let snapshot = engine.snapshot();
    for status in engine.playback_runtime_status() {
        let cue_list_id = status.playback.cue_list_id;
        let Some(current) = status
            .playback
            .current_cue_id
            .filter(|current| before.get(&cue_list_id).copied().flatten() != Some(*current))
        else {
            continue;
        };
        let Some(cue) = snapshot
            .cue_lists
            .iter()
            .find(|cue_list| cue_list.id == cue_list_id)
            .and_then(|cue_list| cue_list.cues.iter().find(|cue| cue.id == current))
        else {
            continue;
        };
        let mut completion = 0;
        for action in &cue.actions {
            match super::timecode_v2::apply_installed_cue_action(timecodes, &show.path, action) {
                Ok(value) => completion = completion.max(value.unwrap_or(0)),
                Err(error) => {
                    tracing::warn!(error = %error.message, "automatic Cue Timecode action failed")
                }
            }
        }
        engine.set_cue_external_completion_millis(cue_list_id, completion);
    }
}

async fn send_retained_output(runtime: &Runtime) -> io::Result<u64> {
    let tick_started = Instant::now();
    let (routes, frames, patched_slots) = {
        let control = runtime.control.lock();
        (
            Arc::clone(&control.last_routes),
            control.last_frames.clone(),
            control.last_patched_slots.clone(),
        )
    };
    let send_started = Instant::now();
    let network = runtime
        .output
        .send_routes(
            &routes,
            &frames,
            &patched_slots,
            &mut *runtime.sequences.lock().await,
        )
        .await;
    let usb = runtime.usb.enqueue_routes(&routes, &frames);
    let result = combined_delivery_result(network, usb);
    trace_slow_output_phases(
        tick_started.elapsed(),
        Duration::ZERO,
        Duration::ZERO,
        Duration::ZERO,
        send_started.elapsed(),
    );
    result
}

pub(in crate::runtime) fn combined_delivery_result(
    network: io::Result<u64>,
    usb: u64,
) -> io::Result<u64> {
    match (network, usb) {
        (Ok(network), usb) => Ok(network + usb),
        (Err(_), usb) if usb > 0 => Ok(usb),
        (Err(error), _) => Err(error),
    }
}

/// Runs one test-bench frame through the same render, routing, sequence, health-facing output
/// boundary as the production scheduler.
pub(super) async fn render_test_tick(state: AppState) -> io::Result<u64> {
    let tick_started = Instant::now();
    let action_timing = state.action_timing.begin_output_render();
    let before_cues = state
        .output
        .playback_runtime_status()
        .into_iter()
        .map(|status| (status.playback.cue_list_id, status.playback.current_cue_id))
        .collect::<HashMap<_, _>>();
    let (rendered, semantic_timing, visualization_scope) = {
        let _activation = state.active_show.acquire_shared().await;
        let visualization_scope = VisualizationScope {
            show_id: state.active_show.current().map(|show| show.id.0),
        };
        let playback = state.playback.render_capability();
        let (rendered, semantic_timing) = state
            .output
            .render_with_playback_events_timed(
                &state.active_show.output_projection(),
                &playback,
                state.output.render_options(),
            )
            .map_err(io::Error::other)?;
        (rendered, semantic_timing, visualization_scope)
    };
    dispatch_automatic_cue_actions(
        state.output.engine(),
        &state.timecodes,
        &state.active_show.output_projection(),
        &before_cues,
    );
    let publish_started = Instant::now();
    let frames = state
        .output
        .render_frames_and_publish(&rendered, visualization_scope);
    let publish = publish_started.elapsed();
    let send_started = Instant::now();
    let result = state
        .output
        .send_network_routes(&rendered.routes, &frames, &rendered.patched_slots)
        .await;
    let send = send_started.elapsed();
    trace_slow_output_phases(
        tick_started.elapsed(),
        semantic_timing.dynamic,
        semantic_timing.engine,
        publish,
        send,
    );
    if result.is_ok() {
        state.action_timing.complete_output_render(action_timing);
    }
    result
}

fn trace_slow_output_phases(
    total: Duration,
    dynamic: Duration,
    engine: Duration,
    publish: Duration,
    send: Duration,
) {
    if total < SLOW_OUTPUT_PHASE_THRESHOLD {
        return;
    }
    let sample = SLOW_OUTPUT_PHASE_SAMPLES.fetch_add(1, Ordering::Relaxed);
    if sample >= SLOW_OUTPUT_PHASE_SAMPLE_LIMIT {
        return;
    }
    tracing::info!(
        sample = sample + 1,
        total_micros = duration_micros(total),
        dynamic_micros = duration_micros(dynamic),
        engine_micros = duration_micros(engine),
        publish_micros = duration_micros(publish),
        send_micros = duration_micros(send),
        "slow output tick phase sample"
    );
}

fn duration_micros(duration: Duration) -> u64 {
    u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
}

pub(super) fn render_with_playback_events(
    engine: &Engine,
    active_show: &ActiveShowProjection,
    playback: &PlaybackRenderCapability,
    options: RenderOptions,
    sampled: &[ContributionBatch],
    persistence: Option<&OutputPersistenceResource>,
) -> Result<RenderResult, EngineError> {
    playback
        .run_unit_of_work(AutomaticRender {
            engine,
            active_show,
            options,
            playback,
            sampled,
            persistence,
        })
        .output
}

struct AutomaticRender<'a> {
    engine: &'a Engine,
    active_show: &'a ActiveShowProjection,
    options: RenderOptions,
    playback: &'a PlaybackRenderCapability,
    sampled: &'a [ContributionBatch],
    persistence: Option<&'a OutputPersistenceResource>,
}

impl PlaybackUnitOfWork for AutomaticRender<'_> {
    type Output = Result<RenderResult, EngineError>;

    fn execute(self) -> PlaybackOperation<Self::Output> {
        let mut rendered = match self
            .engine
            .render_with_contribution_batches(self.options, self.sampled)
        {
            Ok(rendered) => rendered,
            Err(error) => return PlaybackOperation::new(Err(error)),
        };
        let transitions = std::mem::take(&mut rendered.automatic_playback_transitions);
        let show_id = self.active_show.current().as_ref().map(|show| show.id.0);
        if !transitions.is_empty()
            && let (Some(show_id), Some(persistence)) = (show_id, self.persistence)
            && let Err(error) =
                checkpoint_automatic_playback_runtime(self.engine, persistence, show_id)
        {
            tracing::warn!(error = %error.message, "automatic Playback runtime persistence is pending");
        }
        let mut events = show_id
            .map(|show_id| {
                playback_service::automatic_projection_changes(
                    self.engine,
                    PlaybackShowScope {
                        show_id,
                        show_revision: rendered.revision,
                    },
                    transitions,
                )
            })
            .map(automatic_playback_events)
            .unwrap_or_default();
        if let Some(show_id) = show_id
            && let Some(draft) = self.playback.completed_frame(
                self.engine,
                show_id,
                rendered.revision,
                self.engine.application_time(),
            )
        {
            events.push(draft);
        }
        PlaybackOperation::with_events(Ok(rendered), events)
    }
}

fn checkpoint_automatic_playback_runtime(
    engine: &Engine,
    persistence: &OutputPersistenceResource,
    show_id: Uuid,
) -> Result<(), ApiError> {
    let serialized = super::serialize_active_playbacks(&engine.playback_runtime())?;
    persistence
        .checkpoint_active_playbacks(light_core::ShowId(show_id), &serialized)
        .map_err(ApiError::store)
}

fn update_timecode(runtime: &Runtime) -> bool {
    let (current, transition, uses_internal_clock) = {
        let mut router = runtime.timecode.lock();
        let current = router.poll_loss().cloned();
        let transition = router.take_transition();
        (current, transition, router.uses_internal_clock())
    };
    runtime
        .engine
        .set_timecode_frame(current.as_ref().map(timecode_frame));
    if let Some(TimecodeSourceTransition::ExternalLost { policy }) = transition {
        apply_external_timecode_loss(runtime, policy);
    }
    uses_internal_clock
}

fn apply_external_timecode_loss(runtime: &Runtime, policy: ExternalTimecodeLossPolicy) {
    let action = match policy {
        ExternalTimecodeLossPolicy::ContinueInternal => return,
        ExternalTimecodeLossPolicy::Pause => TimecodeTransportAction::Pause,
        ExternalTimecodeLossPolicy::Stop => TimecodeTransportAction::Stop,
    };
    for snapshot in runtime.timecodes.snapshots() {
        let eligible = match policy {
            ExternalTimecodeLossPolicy::Pause => {
                snapshot.transport == TimecodeTransportState::Playing
            }
            ExternalTimecodeLossPolicy::Stop => {
                snapshot.transport != TimecodeTransportState::Stopped
            }
            ExternalTimecodeLossPolicy::ContinueInternal => false,
        };
        if eligible {
            let _ = runtime.timecodes.handle(snapshot.timecode_id, action);
        }
    }
}

fn timecode_frame(timecode: &SmpteTimecode) -> u64 {
    let fps = u64::from(timecode.rate.nominal_frames());
    let seconds = u64::from(timecode.hours) * 3600
        + u64::from(timecode.minutes) * 60
        + u64::from(timecode.seconds);
    seconds * fps + u64::from(timecode.frames)
}

fn output_frames(
    control: &mut OutputControl,
    mut rendered: HashMap<Universe, DmxFrame>,
) -> HashMap<Universe, DmxFrame> {
    if control.hold {
        return control.last_frames.clone();
    }
    apply_raw_overrides(&mut rendered, &control.raw_overrides);
    control.last_frames.clone_from(&rendered);
    rendered
}

fn output_payload(
    control: &mut OutputControl,
    routes: Arc<[light_output::OutputRoute]>,
    rendered: HashMap<Universe, DmxFrame>,
    patched_slots: HashMap<Universe, u16>,
) -> (
    Arc<[light_output::OutputRoute]>,
    HashMap<Universe, DmxFrame>,
    HashMap<Universe, u16>,
) {
    if control.hold {
        return (
            Arc::clone(&control.last_routes),
            control.last_frames.clone(),
            control.last_patched_slots.clone(),
        );
    }
    let frames = output_frames(control, rendered);
    control.last_routes = Arc::clone(&routes);
    control.last_patched_slots.clone_from(&patched_slots);
    (routes, frames, patched_slots)
}

fn apply_raw_overrides(
    frames: &mut HashMap<Universe, DmxFrame>,
    overrides: &HashMap<(Universe, light_core::DmxAddress), u8>,
) {
    for (&(universe, address), &value) in overrides {
        if let Some(frame) = frames.get_mut(&universe) {
            frame[usize::from(address - 1)] = value;
        }
    }
}

async fn shut_down_safely(runtime: &Runtime) {
    let routes = send_safe_frame(runtime)
        .await
        .unwrap_or_else(|| runtime.engine.output_routes());
    let _ = runtime
        .output
        .terminate_routes(&routes, &mut *runtime.sequences.lock().await)
        .await;
    runtime.usb.shutdown();
}

async fn send_safe_frame(runtime: &Runtime) -> Option<Arc<[light_output::OutputRoute]>> {
    let options = safe_shutdown_options(&runtime.control);
    let safe = runtime.engine.render(options).ok()?;
    let _ = runtime
        .output
        .send_routes(
            &safe.routes,
            &safe.universes,
            &safe.patched_slots,
            &mut *runtime.sequences.lock().await,
        )
        .await;
    runtime.usb.enqueue_routes(&safe.routes, &safe.universes);
    Some(safe.routes)
}

fn safe_shutdown_options(control: &Mutex<OutputControl>) -> RenderOptions {
    let mut options = control.lock().options;
    options.control_loss_progress = Some(1.0);
    options
}

impl OutputScheduler {
    pub(super) fn start_rendering(&mut self) -> anyhow::Result<()> {
        self.start
            .take()
            .ok_or_else(|| anyhow::anyhow!("output scheduler was already started"))?
            .send(())
            .map_err(|_| anyhow::anyhow!("output scheduler stopped before startup completed"))
    }

    pub(super) fn network_output(&self) -> Arc<NetworkOutput> {
        Arc::clone(&self.output)
    }

    pub(super) fn sequences(&self) -> SharedSequences {
        Arc::clone(&self.sequences)
    }

    pub(super) fn control_capability(&self) -> OutputControlCapability {
        OutputControlCapability::new(Arc::clone(&self.control))
    }

    pub(super) fn usb_output(&self) -> Arc<UsbOutputFanout> {
        Arc::clone(&self.usb)
    }

    pub(super) fn into_task(mut self) -> OutputTask {
        self.start.take();
        self.task
    }
}

impl SharedResources {
    async fn create(config: &Config) -> anyhow::Result<Self> {
        let persistence = OutputPersistenceResource::open(&config.data_dir)?;
        let usb_document = persistence
            .setting(super::usb_output::USB_ENDPOINTS_SETTING)
            .ok()
            .flatten()
            .and_then(|value| match serde_json::from_str::<UsbEndpointDocument>(&value) {
                Ok(document) if document.validate().is_ok() => Some(document),
                Ok(_) | Err(_) => {
                    tracing::warn!("ignoring malformed USB endpoint installation document; original setting is preserved");
                    None
                }
            })
            .unwrap_or_default();
        let usb = Arc::new(UsbOutputFanout::new(Arc::new(
            light_usb_dmx_serial::SerialUsbDriverFactory,
        )));
        usb.configure(&usb_document).map_err(anyhow::Error::msg)?;
        Ok(Self {
            output: bind_output(config.bind_ip).await?,
            sequences: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            control: create_control(&config.persisted_runtime),
            usb,
            persistence,
            programmer_reconciliation_cache: Arc::new(ProgrammerReconciliationCache::default()),
        })
    }

    fn runtime(&self, config: &Config) -> Runtime {
        Runtime {
            engine: Arc::clone(&config.engine),
            output: Arc::clone(&self.output),
            sequences: Arc::clone(&self.sequences),
            control: Arc::clone(&self.control),
            usb: Arc::clone(&self.usb),
            timecode: Arc::clone(&config.timecode),
            timecodes: config.timecodes.clone(),
            playback: config.playback.clone(),
            active_show: config.active_show.clone(),
            activation: config.activation.clone(),
            cancellation: config.cancellation.clone(),
            dynamics: Arc::clone(&config.dynamics),
            speed_groups: Arc::clone(&config.speed_groups),
            rate: Arc::clone(&config.rate),
            dynamic_auto_offs: Arc::clone(&config.dynamic_auto_offs),
            visualization_frames: Arc::clone(&config.visualization_frames),
            action_timing: config.action_timing.clone(),
            programmer_reconciliation_cache: Arc::clone(&self.programmer_reconciliation_cache),
            persistence: self.persistence.clone(),
            internal_audio: Arc::clone(&config.internal_audio),
        }
    }

    fn scheduler(
        self,
        start: tokio::sync::oneshot::Sender<()>,
        task: OutputTask,
    ) -> OutputScheduler {
        OutputScheduler {
            output: self.output,
            sequences: self.sequences,
            control: self.control,
            usb: self.usb,
            start: Some(start),
            task,
        }
    }
}

#[cfg(test)]
#[path = "output_scheduler_tests.rs"]
mod tests;
