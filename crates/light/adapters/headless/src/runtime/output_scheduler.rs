//! Network-output scheduling and safe shutdown for the server runtime.

use super::capability_resources::{
    ActiveShowCoordinator, ActiveShowProjection, OutputControlCapability,
    OutputPersistenceResource, PlaybackRenderCapability,
};
use super::visualization_frame::{
    FrameDynamicSources, RenderedSemanticFrame, VisualizationFrameHub,
};
use super::{
    ActionTimingResource, ApiError, AppState, DynamicSnapshotPublication, OutputControl,
    PersistedOutputRuntime, playback_service,
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
        atomic::{AtomicU16, Ordering},
    },
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

mod cold_preset_materialization;
mod dynamic_projection;
#[cfg(test)]
pub(super) use dynamic_projection::physical_adapter::{
    PhysicalAdapterLane,
    position::{PositionAdapter, tests as position_test_support},
};
pub(in crate::runtime) mod change_lead;
mod dynamic_reconciliation;
mod slow_phase_trace;
use slow_phase_trace::trace_slow_output_phases;
mod live_output_bench;
pub use live_output_bench::{
    LiveOutputBench, LiveOutputFrame, LiveOutputWork, ReadoutConsumerReport, ReadoutConsumers,
};
mod restored_dynamic_candidate;
mod start_path_timing;
mod timecode_audio;
pub(in crate::runtime) use restored_dynamic_candidate::{
    RestoredDynamicCandidate, prepare_restored_dynamic_candidate,
};
pub use start_path_timing::StartPathPhases;
use timecode_audio::timecode_audio_contributions;

pub(in crate::runtime) use cold_preset_materialization::{
    materialize_cold_preset_dependencies, materialize_pending_preset_dependencies,
};

pub(in crate::runtime) use dynamic_reconciliation::{
    ColdDynamicReconciliationInputs, dynamic_playback_owner, reconcile_cold_dynamic_candidate,
};

/// Pending (Preload) readout identity and the accepted-pair payload, for Position readouts.
pub(in crate::runtime) use dynamic_projection::pending_publication::{
    PendingAttemptTicket, PendingEpisodeIdentity, PendingLaneValues, PendingNativeReadout,
    PendingPairBinding, PendingReadoutError,
};
/// TL-554: native Color descriptors, adoption and Direct status for routes and edit capture.
pub(in crate::runtime) use dynamic_projection::physical_adapter as physical_adapters;
pub(in crate::runtime) use dynamic_projection::reconcile_dynamic_controllers;
pub(in crate::runtime) use dynamic_projection::retained_preload_hybrid::PendingHybridResult;
/// TL-548 C4 hook: the desk's Pending (Preload) episode executor and its lifecycle triggers.
pub(in crate::runtime) use dynamic_projection::retained_preload_hybrid::pending_executor::{
    PendingEpisodeResource, PendingEpisodeStatus, PendingTrigger,
};
#[cfg(test)]
pub(in crate::runtime) use dynamic_projection::retained_preload_hybrid::tests::position::RealPositionPendingRig;
#[cfg(test)]
pub(super) use dynamic_projection::{
    DynamicPlaybackControl, dynamic_transition_events, fully_controlled_dynamic_playbacks,
};
pub(in crate::runtime) use dynamic_projection::{
    LiveFamilyAdapters, ProgrammerReconciliationCache, dynamic_output_frame,
};
pub(super) use dynamic_projection::{dynamic_contributions, dynamic_projection};
use dynamic_reconciliation::{
    reconcile_cue_dynamics, reconcile_dynamic_playbacks, reconcile_programmer_dynamics,
};

type OutputSequences = HashMap<(Protocol, Universe), u8>;
type SharedSequences = Arc<tokio::sync::Mutex<OutputSequences>>;
pub(super) type OutputTask = Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'static>>;

/// Process authority for physical delivery; it never comes from editable show settings.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::runtime) enum PhysicalDeliveryPolicy {
    #[default]
    Live,
    VisualizerPreview,
}

impl PhysicalDeliveryPolicy {
    pub(in crate::runtime) async fn send(
        self,
        network: &NetworkOutput,
        usb: &UsbOutputFanout,
        routes: &[light_output::OutputRoute],
        frames: &HashMap<Universe, DmxFrame>,
        patched_slots: &HashMap<Universe, u16>,
        sequences: &mut OutputSequences,
    ) -> io::Result<u64> {
        if self == Self::VisualizerPreview {
            return Ok(0);
        }
        let network = network
            .send_routes(routes, frames, patched_slots, sequences)
            .await;
        let usb = usb.enqueue_routes(routes, frames);
        combined_delivery_result(network, usb)
    }

    pub(in crate::runtime) fn configure_usb(
        self,
        usb: &UsbOutputFanout,
        document: &light_output::UsbEndpointDocument,
    ) -> Result<(), String> {
        if self == Self::VisualizerPreview {
            return document.validate();
        }
        usb.configure(document)
    }

    async fn finish_scheduler_output(
        self,
        network: &NetworkOutput,
        usb: &UsbOutputFanout,
        routes: &[light_output::OutputRoute],
        sequences: &mut OutputSequences,
    ) {
        // Preserve the scheduler's computed USB safe frame. Explicit route removal is the
        // separate operation that queues USB zeros; it must not run during scheduler shutdown.
        if self == Self::Live {
            let _ = network.terminate_routes(routes, sequences).await;
        }
        usb.shutdown();
    }

    pub(in crate::runtime) async fn terminate(
        self,
        network: &NetworkOutput,
        usb: &UsbOutputFanout,
        routes: &[light_output::OutputRoute],
        sequences: &mut OutputSequences,
    ) {
        if self == Self::VisualizerPreview {
            return;
        }
        let _ = network.terminate_routes(routes, sequences).await;
        usb.terminate_routes(routes);
    }
}

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
    pub visualizer_preview: bool,
    pub dynamics: Arc<Mutex<light_dynamics::DynamicRuntime>>,
    pub dynamic_snapshot: Arc<DynamicSnapshotPublication>,
    pub dynamic_source_origins: super::dynamic_source_origins::SharedDynamicSourceOrigins,
    pub speed_groups: Arc<Mutex<[light_control::speed::SpeedGroupController; 5]>>,
    pub dynamic_auto_offs: Arc<Mutex<Vec<PlaybackIdentity>>>,
    pub visualization_frames: Arc<VisualizationFrameHub>,
    pub action_timing: ActionTimingResource,
    pub data_dir: std::path::PathBuf,
    pub internal_audio: Arc<Mutex<super::internal_audio::InternalAudioRuntime>>,
}

pub(super) struct OutputScheduler {
    delivery_policy: PhysicalDeliveryPolicy,
    pub(super) output: Arc<NetworkOutput>,
    pub(super) sequences: SharedSequences,
    pub(super) control: Arc<Mutex<OutputControl>>,
    pub(super) usb: Arc<UsbOutputFanout>,
    family_adapters: Arc<LiveFamilyAdapters>,
    start: Option<tokio::sync::oneshot::Sender<()>>,
    task: OutputTask,
}

struct SharedResources {
    delivery_policy: PhysicalDeliveryPolicy,
    pub(super) output: Arc<NetworkOutput>,
    pub(super) sequences: SharedSequences,
    pub(super) control: Arc<Mutex<OutputControl>>,
    pub(super) usb: Arc<UsbOutputFanout>,
    persistence: OutputPersistenceResource,
    programmer_reconciliation_cache: Arc<ProgrammerReconciliationCache>,
    family_adapters: Arc<LiveFamilyAdapters>,
}

#[derive(Clone)]
struct Runtime {
    delivery_policy: PhysicalDeliveryPolicy,
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
    pub(super) dynamic_snapshot: Arc<DynamicSnapshotPublication>,
    pub(super) dynamic_source_origins: super::dynamic_source_origins::SharedDynamicSourceOrigins,
    pub(super) speed_groups: Arc<Mutex<[light_control::speed::SpeedGroupController; 5]>>,
    pub(super) rate: Arc<AtomicU16>,
    pub(super) dynamic_auto_offs: Arc<Mutex<Vec<PlaybackIdentity>>>,
    pub(super) visualization_frames: Arc<VisualizationFrameHub>,
    pub(super) action_timing: ActionTimingResource,
    pub(super) programmer_reconciliation_cache: Arc<ProgrammerReconciliationCache>,
    pub(super) family_adapters: Arc<LiveFamilyAdapters>,
    pub(super) persistence: OutputPersistenceResource,
    pub(super) internal_audio: Arc<Mutex<super::internal_audio::InternalAudioRuntime>>,
    change_lead: Arc<light_output::ChangeLeadTime>,
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

async fn bind_output(bind_ip: IpAddr, test_bench: bool) -> anyhow::Result<Arc<NetworkOutput>> {
    let cid = *Uuid::new_v4().as_bytes();
    let mut output = NetworkOutput::bind(bind_ip, cid, "Light").await?;
    if test_bench {
        // Loopback has no broadcast or multicast, so the bench hears peers on loopback listeners.
        // The desk's reply address is a documentation address, so bench peers on loopback are
        // never mistaken for the desk's own frames.
        let loopback = std::net::SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, 0));
        output = output
            .listen_for_art_polls(loopback, std::net::Ipv4Addr::new(192, 0, 2, 1))?
            .listen_for_sacn(loopback)?;
    }
    Ok(Arc::new(output))
}

fn create_control(runtime: &PersistedOutputRuntime) -> Arc<Mutex<OutputControl>> {
    Arc::new(Mutex::new(OutputControl {
        options: RenderOptions {
            grand_master: runtime.grand_master,
            blackout: runtime.blackout,
            control_loss_progress: None,
            ..Default::default()
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
    let (rendered, visualization_scope, dynamic, engine) = {
        let Ok(_activation) = runtime.activation.try_acquire() else {
            return send_retained_output(&runtime).await;
        };
        runtime.timecodes.reconcile_cue_lists(&runtime.engine);
        let visualization_scope = VisualizationScope {
            show_id: runtime.active_show.current().map(|show| show.id.0),
        };
        let completed = ordered_output_operation(&runtime.playback, || {
            render_ordered_output_frame(&runtime, options)
        });
        let (rendered, dynamic, engine) = match completed {
            Ok(completed) => completed,
            Err(EngineError::StalePreparedFrame) => {
                drop(_activation);
                return send_retained_output(&runtime).await;
            }
            Err(error) => {
                return Err(io::Error::other(error));
            }
        };
        (rendered, visualization_scope, dynamic, engine)
    };
    let change_lead_start = rendered.change_lead_start();
    runtime.internal_audio.lock().reconcile(
        rendered.rendered.source_snapshot.fixtures.as_ref(),
        rendered.rendered.profile_visualization_values.as_ref(),
        &rendered.rendered.resolved_values,
    );
    let publish_started = Instant::now();
    let (routes, frames, patched_slots) = {
        let mut control = runtime.control.lock();
        if !control.effective_hold() {
            runtime.visualization_frames.publish(
                &rendered,
                visualization_scope,
                runtime.engine.output_pool().as_deref(),
            );
        }
        output_payload(
            &mut control,
            rendered.rendered.routes,
            rendered.rendered.universes,
            rendered.rendered.patched_slots,
        )
    };
    let publish = publish_started.elapsed();
    let send_started = Instant::now();
    let result = runtime
        .delivery_policy
        .send(
            &runtime.output,
            &runtime.usb,
            &routes,
            &frames,
            &patched_slots,
            &mut *runtime.sequences.lock().await,
        )
        .await;
    let send = send_started.elapsed();
    trace_slow_output_phases(tick_started.elapsed(), dynamic, engine, publish, send);
    let delivered = result.is_ok();
    change_lead::record_change_lead(
        &runtime.change_lead,
        &runtime.engine,
        change_lead_start,
        delivered,
    );
    if delivered {
        runtime.action_timing.complete_output_render(action_timing);
    }
    result
}

/// The ordered part of one output tick: captures the prepared frame, projects Dynamics into it
/// and renders it, returning the rendered frame with its Dynamic and engine phase durations.
fn render_ordered_output_frame(
    runtime: &Runtime,
    options: RenderOptions,
) -> PlaybackOperation<Result<(RenderedSemanticFrame, Duration, Duration), EngineError>> {
    let dynamic_started = Instant::now();
    let Some(prepared) = runtime.engine.try_prepare_output_frame(options) else {
        return PlaybackOperation::new(Err(EngineError::StalePreparedFrame));
    };
    let prepared = crate::runtime::dynamic_snapshot_publication::RetainedFrameCapture::select(
        prepared,
        &runtime.dynamic_snapshot,
        dynamic_started,
    );
    let timecode_audio = timecode_audio_contributions(
        &runtime.timecodes,
        prepared.snapshot().fixtures.as_ref(),
        prepared.sampled_at(),
    );
    let baseline_samples = if timecode_audio.is_empty() {
        Vec::new()
    } else {
        vec![timecode_audio]
    };
    let mut events = Vec::new();
    let completed = dynamic_output_frame(
        &runtime.engine,
        &prepared,
        prepared.retained(),
        &baseline_samples,
        &runtime.dynamics,
        &runtime.dynamic_snapshot,
        &runtime.dynamic_source_origins,
        &runtime.speed_groups,
        &runtime.rate,
        &runtime.programmer_reconciliation_cache,
        &runtime.family_adapters,
        |source| {
            let dynamic = dynamic_started.elapsed();
            let engine_started = Instant::now();
            let operation = source.playback_operation(
                &runtime.engine,
                &runtime.active_show,
                &runtime.playback,
                &prepared,
                Some(&runtime.persistence),
            );
            // These transitions already happened during capture, including when
            // final render rejects a stale continuity token.
            events.extend(operation.events);
            operation
                .output
                .map(|rendered| (rendered, dynamic, engine_started.elapsed()))
        },
    );
    if completed.is_err() {
        change_lead::carry_uncommitted_capture(&runtime.change_lead, &prepared);
        events.extend(captured_playback_events(
            &runtime.engine,
            &runtime.active_show,
            &runtime.playback,
            &prepared,
            None,
            Some(&runtime.persistence),
        ));
    }
    dispatch_automatic_cue_actions(
        &prepared,
        &runtime.engine,
        &runtime.timecodes,
        runtime.active_show.current().as_ref(),
    );
    let result = completed.map(|completed| {
        runtime.dynamic_auto_offs.lock().extend(completed.auto_offs);
        events.extend(completed.events);
        let dynamics = Arc::new(FrameDynamicSources {
            sample_boundary: completed.sample_boundary,
            runtime: completed.runtime,
            samples: completed.samples,
            origins: completed.origins,
            programmer_values: Arc::clone(prepared.dynamic_programmer_values()),
            cue_values: prepared.cue_dynamic_values().into(),
            ordinary: completed.ordinary,
            change_lead_start: completed.change_lead_start,
        });
        let (rendered, dynamic, engine) = completed.output;
        (
            RenderedSemanticFrame {
                rendered,
                options,
                dynamics: Some(dynamics),
            },
            dynamic,
            engine,
        )
    });
    PlaybackOperation::with_events(result, events)
}

/// A player patched from the TL-367 profile revision carries the canonical Media attributes.
fn dispatch_automatic_cue_actions(
    frame: &light_engine::PreparedOutputFrame,
    engine: &Engine,
    timecodes: &light_application::timeline::TimecodeRuntimeService,
    show: Option<&light_show::ShowEntry>,
) {
    let Some(show) = show else {
        return;
    };
    for (cue_list_id, actions) in claim_automatic_cue_action_batches(frame) {
        let mut completion = 0;
        for action in &actions {
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

/// Resolves only final-state transitions captured by this authoritative tick. The token is
/// independent of event publication, so a failed DMX frame still gets one action dispatch.
pub(in crate::runtime) fn claim_automatic_cue_action_batches(
    frame: &light_engine::PreparedOutputFrame,
) -> Vec<(light_core::CueListId, Vec<light_playback::CueAction>)> {
    let snapshot = frame.snapshot();
    let mut batches = Vec::new();
    for transition in frame.claim_automatic_cue_action_transitions() {
        let Some(playback) = frame.captured_active_playbacks().iter().find(|playback| {
            playback.cue_list_id == transition.cue_list_id
                && playback.playback_number == transition.playback_number
                && playback.current_cue_id == Some(transition.current.id)
                && playback.transition_ordinal == transition.transition_ordinal
        }) else {
            continue;
        };
        if playback.transition_timing_bypassed || playback.discrete_cue_actions_suppressed {
            continue;
        }
        let cue_list_id = transition.cue_list_id;
        let Some(cue) = snapshot
            .cue_lists
            .iter()
            .find(|cue_list| cue_list.id == cue_list_id)
            .and_then(|cue_list| {
                cue_list
                    .cues
                    .iter()
                    .find(|cue| cue.id == transition.current.id)
            })
        else {
            continue;
        };
        batches.push((cue_list_id, cue.actions.clone()));
    }
    batches
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
    let result = runtime
        .delivery_policy
        .send(
            &runtime.output,
            &runtime.usb,
            &routes,
            &frames,
            &patched_slots,
            &mut *runtime.sequences.lock().await,
        )
        .await;
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
    if state.output.uses_internal_timecode_clock() {
        state.timecodes.tick();
    }
    let action_timing = state.action_timing.begin_output_render();
    let (rendered, semantic_timing, visualization_scope) = {
        let _activation = state.active_show.acquire_shared().await;
        state.timecodes.reconcile_cue_lists(state.output.engine());
        let visualization_scope = VisualizationScope {
            show_id: state.active_show.current().map(|show| show.id.0),
        };
        let playback = state.playback.render_capability();
        let active_show = state.active_show.output_projection();
        let result = state.output.render_with_playback_events_timed_with_capture(
            &active_show,
            &playback,
            state.output.render_options(),
            |prepared| {
                dispatch_automatic_cue_actions(
                    prepared,
                    state.output.engine(),
                    &state.timecodes,
                    active_show.current().as_ref(),
                );
            },
        );
        let (rendered, semantic_timing) = match result {
            Ok(rendered) => rendered,
            Err(error) => {
                drop(_activation);
                return match error {
                    EngineError::StalePreparedFrame => state.output.send_retained_output().await,
                    error => Err(io::Error::other(error)),
                };
            }
        };
        (rendered, semantic_timing, visualization_scope)
    };
    let publish_started = Instant::now();
    state
        .output
        .render_frames_and_publish(&rendered, visualization_scope);
    let publish = publish_started.elapsed();
    let send_started = Instant::now();
    let result = state.output.send_retained_output().await;
    let send = send_started.elapsed();
    let change_lead_start = rendered.change_lead_start();
    state
        .output
        .record_change_lead(change_lead_start, result.is_ok());
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
            source: AutomaticRenderSource::Live(options),
            playback,
            sampled,
            persistence,
        })
        .output
}

pub(in crate::runtime) fn render_prepared_with_playback_events(
    engine: &Engine,
    active_show: &ActiveShowProjection,
    playback: &PlaybackRenderCapability,
    frame: &light_engine::PreparedOutputFrame,
    sampled: &[ContributionBatch],
    persistence: Option<&OutputPersistenceResource>,
) -> Result<RenderResult, EngineError> {
    playback
        .run_unit_of_work(AutomaticRender {
            engine,
            active_show,
            source: AutomaticRenderSource::Prepared(frame),
            playback,
            sampled,
            persistence,
        })
        .output
}

/// Authoritative capture advances Playback, so the entire capture/sample/render operation must
/// share the same ordering boundary as an operator command. The closure returns drafts for this
/// unit of work; it must not call another locking Playback operation.
pub(in crate::runtime) fn ordered_output_operation<O>(
    playback: &PlaybackRenderCapability,
    operation: impl FnOnce() -> PlaybackOperation<O>,
) -> O {
    playback
        .run_unit_of_work(OrderedOutputOperation(operation))
        .output
}

struct OrderedOutputOperation<F>(F);

impl<F, O> PlaybackUnitOfWork for OrderedOutputOperation<F>
where
    F: FnOnce() -> PlaybackOperation<O>,
{
    type Output = O;

    fn execute(self) -> PlaybackOperation<O> {
        (self.0)()
    }
}

/// Render/checkpoint/event assembly for a caller already inside ordered_output_operation.
pub(in crate::runtime) fn prepared_playback_operation(
    engine: &Engine,
    active_show: &ActiveShowProjection,
    playback: &PlaybackRenderCapability,
    frame: &light_engine::PreparedOutputFrame,
    sampled: &[ContributionBatch],
    persistence: Option<&OutputPersistenceResource>,
) -> PlaybackOperation<Result<RenderResult, EngineError>> {
    AutomaticRender {
        engine,
        active_show,
        source: AutomaticRenderSource::Prepared(frame),
        playback,
        sampled,
        persistence,
    }
    .execute()
}

enum AutomaticRenderSource<'a> {
    Live(RenderOptions),
    Prepared(&'a light_engine::PreparedOutputFrame),
}

struct AutomaticRender<'a> {
    engine: &'a Engine,
    active_show: &'a ActiveShowProjection,
    source: AutomaticRenderSource<'a>,
    playback: &'a PlaybackRenderCapability,
    sampled: &'a [ContributionBatch],
    persistence: Option<&'a OutputPersistenceResource>,
}

impl PlaybackUnitOfWork for AutomaticRender<'_> {
    type Output = Result<RenderResult, EngineError>;

    fn execute(self) -> PlaybackOperation<Self::Output> {
        let captured;
        let frame = match self.source {
            AutomaticRenderSource::Live(options) => {
                captured = self.engine.prepare_output_frame(options);
                &captured
            }
            AutomaticRenderSource::Prepared(frame) => frame,
        };
        completed_prepared_playback_operation(
            self.engine,
            self.active_show,
            self.playback,
            frame,
            self.engine.render_prepared(frame, self.sampled),
            self.persistence,
        )
    }
}

/// Finish capture-owned Playback events after either renderer, or an earlier sampling failure.
/// This function never renders or commits engine continuity. A staged family producer can
/// therefore consume its prepared token once and pass the result through this same boundary.
pub(in crate::runtime) fn completed_prepared_playback_operation(
    engine: &Engine,
    active_show: &ActiveShowProjection,
    playback: &PlaybackRenderCapability,
    frame: &light_engine::PreparedOutputFrame,
    mut result: Result<RenderResult, EngineError>,
    persistence: Option<&OutputPersistenceResource>,
) -> PlaybackOperation<Result<RenderResult, EngineError>> {
    let events = captured_playback_events(
        engine,
        active_show,
        playback,
        frame,
        result.as_mut().ok(),
        persistence,
    );
    PlaybackOperation::with_events(result, events)
}

/// Capture transitions have already happened even when Dynamic evaluation fails before the
/// renderer runs. Claim them once; a failed frame must not publish completed-frame telemetry.
pub(in crate::runtime) fn captured_playback_events(
    engine: &Engine,
    active_show: &ActiveShowProjection,
    playback: &PlaybackRenderCapability,
    frame: &light_engine::PreparedOutputFrame,
    mut rendered: Option<&mut RenderResult>,
    persistence: Option<&OutputPersistenceResource>,
) -> Vec<light_application::EventDraft> {
    // Playback advanced during capture. Its real automatic transitions must still be
    // persisted and announced if an unrelated continuity edit makes DMX retain a frame.
    let transitions = frame.claim_automatic_playback_transitions().to_vec();
    if let Some(rendered) = rendered.as_deref_mut() {
        rendered.automatic_playback_transitions.clear();
    }
    let show_id = active_show.current().as_ref().map(|show| show.id.0);
    if !transitions.is_empty()
        && let (Some(show_id), Some(persistence)) = (show_id, persistence)
        && let Err(error) = checkpoint_automatic_playback_runtime(engine, persistence, show_id)
    {
        tracing::warn!(error = %error.message, "automatic Playback runtime persistence is pending");
    }
    let mut events = show_id
        .map(|show_id| {
            playback_service::automatic_projection_changes(
                engine,
                PlaybackShowScope {
                    show_id,
                    show_revision: frame.snapshot().revision,
                },
                transitions,
            )
        })
        .map(automatic_playback_events)
        .unwrap_or_default();
    if let Some(rendered) = rendered
        && let Some(show_id) = show_id
        && let Some(draft) =
            playback.completed_frame(engine, show_id, rendered.revision, rendered.sampled_at)
    {
        events.push(draft);
    }
    events
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
            if let Err(error) = runtime
                .timecodes
                .handle_source_loss(snapshot.timecode_id, action)
            {
                tracing::warn!(timecode_id = %snapshot.timecode_id.0, error = %error.message, "Timecode source-loss transport failed");
            }
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
    mut rendered: light_engine::Pooled<HashMap<Universe, DmxFrame>>,
) -> light_engine::Pooled<HashMap<Universe, DmxFrame>> {
    if control.effective_hold() {
        // Hold republishes what was last sent, in the borrowed frame this render already has.
        rendered.clone_from(&control.last_frames);
        return rendered;
    }
    apply_raw_overrides(&mut rendered, &control.raw_overrides);
    control.last_frames.clone_from(&rendered);
    rendered
}

fn output_payload(
    control: &mut OutputControl,
    routes: Arc<[light_output::OutputRoute]>,
    mut rendered: light_engine::Pooled<HashMap<Universe, DmxFrame>>,
    mut patched_slots: light_engine::Pooled<HashMap<Universe, u16>>,
) -> (
    Arc<[light_output::OutputRoute]>,
    light_engine::Pooled<HashMap<Universe, DmxFrame>>,
    light_engine::Pooled<HashMap<Universe, u16>>,
) {
    if control.effective_hold() {
        rendered.clone_from(&control.last_frames);
        patched_slots.clone_from(&control.last_patched_slots);
        return (Arc::clone(&control.last_routes), rendered, patched_slots);
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
    runtime
        .delivery_policy
        .finish_scheduler_output(
            &runtime.output,
            &runtime.usb,
            &routes,
            &mut *runtime.sequences.lock().await,
        )
        .await;
}

async fn send_safe_frame(runtime: &Runtime) -> Option<Arc<[light_output::OutputRoute]>> {
    let options = safe_shutdown_options(&runtime.control);
    let safe = runtime.engine.render(options).ok()?;
    let _ = runtime
        .delivery_policy
        .send(
            &runtime.output,
            &runtime.usb,
            &safe.routes,
            &safe.universes,
            &safe.patched_slots,
            &mut *runtime.sequences.lock().await,
        )
        .await;
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
            .with_delivery_policy(self.delivery_policy)
    }

    pub(super) fn usb_output(&self) -> Arc<UsbOutputFanout> {
        Arc::clone(&self.usb)
    }

    pub(super) fn family_adapters(&self) -> Arc<LiveFamilyAdapters> {
        Arc::clone(&self.family_adapters)
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
        let delivery_policy = if config.visualizer_preview {
            PhysicalDeliveryPolicy::VisualizerPreview
        } else {
            PhysicalDeliveryPolicy::Live
        };
        delivery_policy
            .configure_usb(&usb, &usb_document)
            .map_err(anyhow::Error::msg)?;
        Ok(Self {
            delivery_policy,
            output: bind_output(
                if config.visualizer_preview {
                    std::net::Ipv4Addr::LOCALHOST.into()
                } else {
                    config.bind_ip
                },
                config.test_bench,
            )
            .await?,
            sequences: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            control: create_control(&config.persisted_runtime),
            usb,
            persistence,
            programmer_reconciliation_cache: Arc::new(ProgrammerReconciliationCache::default()),
            // TL-548 C3 / TL-552: production is opted in to the all-family Live path; `engaged`
            // still requires a contract-1 engine (see `e2e_semantic_contract`).
            family_adapters: Arc::new(LiveFamilyAdapters::new(
                super::e2e_semantic_contract::live_family_adapters_opted_in(),
            )),
        })
    }

    fn runtime(&self, config: &Config) -> Runtime {
        Runtime {
            delivery_policy: self.delivery_policy,
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
            dynamic_snapshot: Arc::clone(&config.dynamic_snapshot),
            dynamic_source_origins: Arc::clone(&config.dynamic_source_origins),
            speed_groups: Arc::clone(&config.speed_groups),
            rate: Arc::clone(&config.rate),
            dynamic_auto_offs: Arc::clone(&config.dynamic_auto_offs),
            visualization_frames: Arc::clone(&config.visualization_frames),
            action_timing: config.action_timing.clone(),
            programmer_reconciliation_cache: Arc::clone(&self.programmer_reconciliation_cache),
            family_adapters: Arc::clone(&self.family_adapters),
            persistence: self.persistence.clone(),
            internal_audio: Arc::clone(&config.internal_audio),
            change_lead: change_lead::change_lead_recorder(&config.health),
        }
    }

    fn scheduler(
        self,
        start: tokio::sync::oneshot::Sender<()>,
        task: OutputTask,
    ) -> OutputScheduler {
        OutputScheduler {
            delivery_policy: self.delivery_policy,
            output: self.output,
            sequences: self.sequences,
            control: self.control,
            usb: self.usb,
            family_adapters: self.family_adapters,
            start: Some(start),
            task,
        }
    }
}

#[cfg(test)]
#[path = "output_scheduler_tests.rs"]
mod tests;
