//! Network-output scheduling and safe shutdown for the server runtime.

use super::capability_resources::{
    ActiveShowCoordinator, ActiveShowProjection, OutputControlCapability, PlaybackRenderCapability,
};
use super::{AppState, OutputControl, PersistedOutputRuntime, playback_service};
use light_application::{
    PlaybackOperation, PlaybackShowScope, PlaybackUnitOfWork, automatic_playback_events,
};
use light_control::{SmpteTimecode, TimecodeRouter};
use light_core::{AttributeKey, AttributeValue, FixtureId, MergeMode, TimedValue, Universe};
use light_dynamics::ScalarSourceResolver;
use light_engine::{
    ContributionBatch, ContributionSample, Engine, EngineError, RenderOptions, RenderResult,
};
use light_output::{DmxFrame, NetworkOutput, OutputHealth, Protocol, run_scheduler_dynamic};
use parking_lot::Mutex;
use std::{
    collections::{HashMap, HashSet},
    future::Future,
    io,
    net::IpAddr,
    pin::Pin,
    sync::{Arc, atomic::AtomicU16},
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

type OutputSequences = HashMap<(Protocol, Universe), u8>;
type SharedSequences = Arc<tokio::sync::Mutex<OutputSequences>>;
pub(super) type OutputTask = Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'static>>;

pub(super) struct Config {
    pub bind_ip: IpAddr,
    pub engine: Arc<Engine>,
    pub health: Arc<std::sync::Mutex<OutputHealth>>,
    pub rate: Arc<AtomicU16>,
    pub timecode: Arc<Mutex<TimecodeRouter>>,
    pub cancellation: CancellationToken,
    pub persisted_runtime: PersistedOutputRuntime,
    pub playback: PlaybackRenderCapability,
    pub active_show: ActiveShowProjection,
    pub activation: ActiveShowCoordinator,
    pub test_bench: bool,
    pub dynamics: Arc<Mutex<light_dynamics::DynamicRuntime>>,
    pub speed_groups: Arc<Mutex<[light_control::speed::SpeedGroupController; 5]>>,
    pub dynamic_auto_offs: Arc<Mutex<Vec<u16>>>,
}

pub(super) struct OutputScheduler {
    pub(super) output: Arc<NetworkOutput>,
    pub(super) sequences: SharedSequences,
    pub(super) control: Arc<Mutex<OutputControl>>,
    start: Option<tokio::sync::oneshot::Sender<()>>,
    task: OutputTask,
}

struct SharedResources {
    pub(super) output: Arc<NetworkOutput>,
    pub(super) sequences: SharedSequences,
    pub(super) control: Arc<Mutex<OutputControl>>,
}

#[derive(Clone)]
struct Runtime {
    pub(super) engine: Arc<Engine>,
    pub(super) output: Arc<NetworkOutput>,
    pub(super) sequences: SharedSequences,
    pub(super) control: Arc<Mutex<OutputControl>>,
    pub(super) timecode: Arc<Mutex<TimecodeRouter>>,
    pub(super) playback: PlaybackRenderCapability,
    pub(super) active_show: ActiveShowProjection,
    pub(super) activation: ActiveShowCoordinator,
    pub(super) cancellation: CancellationToken,
    pub(super) dynamics: Arc<Mutex<light_dynamics::DynamicRuntime>>,
    pub(super) speed_groups: Arc<Mutex<[light_control::speed::SpeedGroupController; 5]>>,
    pub(super) rate: Arc<AtomicU16>,
    pub(super) dynamic_auto_offs: Arc<Mutex<Vec<u16>>>,
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
    run_scheduler_dynamic(rate, cancellation, health, || render_tick(runtime.clone())).await;
}

// @tour one-action-end-to-end:30 Render semantic state into routed frames
// A scheduler tick advances timecode, renders authoritative engine state, maps universes into
// frames, and sends configured routes. Network I/O starts only after rendering completes.
async fn render_tick(runtime: Runtime) -> io::Result<u64> {
    update_timecode(&runtime);
    let options = runtime.control.lock().render_options();
    let rendered = {
        let _activation = runtime.activation.acquire().await;
        let (sampled, auto_offs, dynamic_events, _) = dynamic_contributions_with_auto_off(
            &runtime.engine,
            &runtime.dynamics,
            &runtime.speed_groups,
            &runtime.rate,
            &[],
            true,
        );
        if !auto_offs.is_empty() {
            runtime.dynamic_auto_offs.lock().extend(auto_offs);
        }
        for event in dynamic_events {
            runtime.playback.publish(event);
        }
        render_with_playback_events(
            &runtime.engine,
            &runtime.active_show,
            &runtime.playback,
            options,
            &sampled,
        )
        .map_err(io::Error::other)?
    };
    let frames = output_frames(&mut runtime.control.lock(), rendered.universes);
    runtime
        .output
        .send_routes(
            &rendered.routes,
            &frames,
            &rendered.patched_slots,
            &mut *runtime.sequences.lock().await,
        )
        .await
}

/// Runs one test-bench frame through the same render, routing, sequence, health-facing output
/// boundary as the production scheduler.
pub(super) async fn render_test_tick(state: AppState) -> io::Result<u64> {
    let rendered = {
        let _activation = state.active_show.acquire().await;
        let playback = state.playback.render_capability();
        state
            .output
            .render_with_playback_events(
                &state.active_show.output_projection(),
                &playback,
                state.output.render_options(),
            )
            .map_err(io::Error::other)?
    };
    let frames = state.output.render_frames(rendered.universes);
    state
        .output
        .send_network_routes(&rendered.routes, &frames, &rendered.patched_slots)
        .await
}

pub(super) fn render_with_playback_events(
    engine: &Engine,
    active_show: &ActiveShowProjection,
    playback: &PlaybackRenderCapability,
    options: RenderOptions,
    sampled: &[ContributionBatch],
) -> Result<RenderResult, EngineError> {
    playback
        .run_unit_of_work(AutomaticRender {
            engine,
            active_show,
            options,
            playback,
            sampled,
        })
        .output
}

struct AutomaticRender<'a> {
    engine: &'a Engine,
    active_show: &'a ActiveShowProjection,
    options: RenderOptions,
    playback: &'a PlaybackRenderCapability,
    sampled: &'a [ContributionBatch],
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

struct TickSources {
    values: HashMap<(FixtureId, AttributeKey), AttributeValue>,
}

impl light_dynamics::ScalarSourceResolver for TickSources {
    fn current(&self, target: FixtureId, attribute: &AttributeKey) -> Option<f32> {
        self.values
            .get(&(target, attribute.clone()))
            .and_then(AttributeValue::normalized)
    }

    fn preset(
        &self,
        _preset_id: &str,
        _target: FixtureId,
        _attribute: &AttributeKey,
    ) -> Option<f32> {
        None
    }
}

pub(super) fn dynamic_contributions(
    engine: &Engine,
    dynamics: &Mutex<light_dynamics::DynamicRuntime>,
    speed_groups: &Mutex<[light_control::speed::SpeedGroupController; 5]>,
    rate: &AtomicU16,
    extra_programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    apply_auto_off: bool,
) -> Vec<ContributionBatch> {
    dynamic_contributions_with_auto_off(
        engine,
        dynamics,
        speed_groups,
        rate,
        extra_programmer_values,
        apply_auto_off,
    )
    .0
}

pub(super) fn dynamic_projection(
    engine: &Engine,
    dynamics: &Mutex<light_dynamics::DynamicRuntime>,
    speed_groups: &Mutex<[light_control::speed::SpeedGroupController; 5]>,
    rate: &AtomicU16,
    extra_programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
) -> (
    Vec<ContributionBatch>,
    Vec<light_dynamics::DynamicRuntimeSample>,
) {
    let (batches, _, _, samples) = dynamic_contributions_with_auto_off(
        engine,
        dynamics,
        speed_groups,
        rate,
        extra_programmer_values,
        false,
    );
    (batches, samples)
}

fn dynamic_contributions_with_auto_off(
    engine: &Engine,
    dynamics: &Mutex<light_dynamics::DynamicRuntime>,
    speed_groups: &Mutex<[light_control::speed::SpeedGroupController; 5]>,
    rate: &AtomicU16,
    extra_programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    apply_auto_off: bool,
) -> (
    Vec<ContributionBatch>,
    Vec<u16>,
    Vec<light_application::EventDraft>,
    Vec<light_dynamics::DynamicRuntimeSample>,
) {
    let now = engine.application_time();
    let now_millis = u64::try_from(now.timestamp_millis()).unwrap_or_default();
    let sources = TickSources {
        values: engine.resolved_values(),
    };
    let speed_groups = speed_groups.lock();
    let speed_transports = std::array::from_fn(|index| {
        let snapshot = speed_groups[index].snapshot(now_millis);
        light_dynamics::DynamicSpeedTransport {
            effective_bpm: snapshot.effective_bpm,
            phase_origin_millis: snapshot.phase_origin_millis,
            phase_reference_millis: speed_groups[index].phase_reference_millis(now_millis),
            beat_phase: snapshot.beat_phase,
            phase_advancing: snapshot.phase_advancing,
        }
    });
    drop(speed_groups);
    let rate = u64::from(rate.load(std::sync::atomic::Ordering::Relaxed).max(1));
    let interval = (1_000 / rate).max(1);
    let mut dynamics = dynamics.lock();
    let before_runtime = dynamics.snapshot();
    dynamics.set_global_paused(engine.playback_dynamics().paused, now_millis);
    reconcile_programmer_dynamics(engine, &mut dynamics, now_millis, extra_programmer_values);
    reconcile_cue_dynamics(engine, &mut dynamics, now_millis);
    let playback_controls = reconcile_dynamic_playbacks(engine, &mut dynamics, now_millis);
    let samples = dynamics.sample_all(now_millis, interval, &speed_transports, &sources);
    let after_runtime = dynamics.snapshot();
    drop(dynamics);
    let dynamic_events = dynamic_transition_events(&before_runtime, &after_runtime, now_millis);
    let auto_offs = if apply_auto_off {
        let fully_controlled = fully_controlled_dynamic_playbacks(
            engine,
            &samples,
            &playback_controls,
            &after_runtime,
            now,
        );
        engine.auto_off_fully_controlled_dynamic_playbacks(fully_controlled)
    } else {
        Vec::new()
    };

    struct Candidate {
        value: AttributeValue,
        priority: i16,
        changed_at_millis: u64,
        stable_order: u128,
        activation_mix: f32,
        dynamic: bool,
    }
    let mut candidates = HashMap::<(FixtureId, AttributeKey), Vec<Candidate>>::new();
    let mut consider = |key: (FixtureId, AttributeKey), candidate: Candidate| {
        candidates.entry(key).or_default().push(candidate);
    };
    for sample in &samples {
        let dynamic_value =
            playback_controls
                .get(&sample.controller_id)
                .map_or(sample.value, |control| {
                    if sample.attribute.is_intensity() {
                        sample.value * control.master
                    } else if control.crossfade_non_intensity {
                        sources
                            .current(sample.target, &sample.attribute)
                            .map_or(sample.value, |base| {
                                base + (sample.value - base) * control.master
                            })
                    } else {
                        sample.value
                    }
                });
        if playback_controls
            .get(&sample.controller_id)
            .is_some_and(|control| {
                control.master == 0.0
                    && !sample.attribute.is_intensity()
                    && !control.crossfade_non_intensity
            })
        {
            continue;
        }
        consider(
            (sample.target, sample.attribute.clone()),
            Candidate {
                value: AttributeValue::Normalized(dynamic_value),
                priority: sample.priority,
                changed_at_millis: sample.activated_at_millis,
                stable_order: sample.controller_id.as_u128(),
                activation_mix: sample.activation_mix,
                dynamic: true,
            },
        );
    }
    for (_, priority, stored) in engine
        .dynamic_programmer_values()
        .into_iter()
        .chain(extra_programmer_values.iter().cloned())
    {
        let (value, timing) = match stored.value {
            light_dynamics::DynamicSemanticValue::Static { value, timing } => (value, timing),
            light_dynamics::DynamicSemanticValue::FixAt { value, timing } => {
                (AttributeValue::Normalized(value), timing)
            }
            light_dynamics::DynamicSemanticValue::DynamicOn { .. }
            | light_dynamics::DynamicSemanticValue::DynamicOff { .. }
            | light_dynamics::DynamicSemanticValue::Release => continue,
        };
        consider(
            (stored.fixture_id, stored.attribute),
            Candidate {
                value,
                priority,
                changed_at_millis: stored.changed_at_millis,
                stable_order: u128::from(stored.programmer_order),
                activation_mix: authored_activation_mix(
                    stored.changed_at_millis,
                    timing,
                    now_millis,
                ),
                dynamic: false,
            },
        );
    }
    for stored in engine.active_cue_dynamic_values() {
        let (value, timing) = match stored.value {
            light_dynamics::DynamicSemanticValue::Static { value, timing } => (value, timing),
            light_dynamics::DynamicSemanticValue::FixAt { value, timing } => {
                (AttributeValue::Normalized(value), timing)
            }
            light_dynamics::DynamicSemanticValue::DynamicOn { .. }
            | light_dynamics::DynamicSemanticValue::DynamicOff { .. }
            | light_dynamics::DynamicSemanticValue::Release => continue,
        };
        consider(
            (stored.fixture_id, stored.attribute),
            Candidate {
                value,
                priority: stored.priority,
                changed_at_millis: stored.changed_at_millis,
                stable_order: stored.current_cue_id.as_u128(),
                activation_mix: authored_activation_mix(
                    stored.changed_at_millis,
                    timing,
                    now_millis,
                ),
                dynamic: false,
            },
        );
    }
    if candidates.is_empty() {
        return (Vec::new(), auto_offs, dynamic_events, samples);
    }
    (
        vec![ContributionBatch::new(candidates.into_iter().map(
            |((fixture_id, attribute), mut stack)| {
                stack.sort_by_key(|candidate| {
                    (
                        candidate.priority,
                        candidate.changed_at_millis,
                        candidate.stable_order,
                    )
                });
                let has_dynamic = stack.iter().any(|candidate| candidate.dynamic);
                let mut resolved = sources
                    .values
                    .get(&(fixture_id, attribute.clone()))
                    .cloned()
                    .unwrap_or_else(|| stack[0].value.clone());
                for candidate in &stack {
                    resolved = blend_attribute_value(
                        resolved,
                        candidate.value.clone(),
                        candidate.activation_mix,
                    );
                }
                let candidate = stack
                    .last()
                    .expect("one Dynamic/FAT candidate exists for every stack");
                let merge_mode = if attribute.is_intensity() && !has_dynamic {
                    MergeMode::Htp
                } else {
                    MergeMode::Ltp
                };
                ContributionSample::independent(TimedValue {
                    fixture_id,
                    attribute,
                    value: resolved,
                    priority: candidate.priority,
                    changed_at: chrono::DateTime::from_timestamp_millis(
                        i64::try_from(candidate.changed_at_millis).unwrap_or(i64::MAX),
                    )
                    .unwrap_or(now),
                    programmer_order: candidate.stable_order.min(u128::from(u64::MAX)) as u64,
                    merge_mode,
                    fade: false,
                    fade_millis: None,
                    delay_millis: None,
                })
            },
        ))],
        auto_offs,
        dynamic_events,
        samples,
    )
}

fn dynamic_transition_events(
    before: &light_dynamics::DynamicRuntimeSnapshot,
    after: &light_dynamics::DynamicRuntimeSnapshot,
    now_millis: u64,
) -> Vec<light_application::EventDraft> {
    let mut events = Vec::new();
    if before.global_paused != after.global_paused {
        events.push(light_application::EventDraft::dynamic_runtime_changed(
            None,
            light_application::DynamicRuntimeChange {
                kind: if after.global_paused {
                    light_application::DynamicRuntimeEventKind::Paused
                } else {
                    light_application::DynamicRuntimeEventKind::Resumed
                },
                dynamic_id: None,
                runtime_instance_id: None,
                controller_id: None,
                winning_controller_id: None,
                occurred_at_millis: now_millis,
                message: Some("global Dynamic transport".into()),
            },
        ));
    }
    let before_instances = before
        .instances
        .iter()
        .map(|instance| (instance.id, instance))
        .collect::<HashMap<_, _>>();
    let after_instances = after
        .instances
        .iter()
        .map(|instance| (instance.id, instance))
        .collect::<HashMap<_, _>>();
    for instance in &after.instances {
        let previous = before_instances.get(&instance.id).copied();
        if instance.completed && !previous.is_some_and(|previous| previous.completed) {
            events.push(light_application::EventDraft::dynamic_runtime_changed(
                None,
                light_application::DynamicRuntimeChange {
                    kind: light_application::DynamicRuntimeEventKind::InstanceOff,
                    dynamic_id: Some(instance.definition.id),
                    runtime_instance_id: Some(instance.id),
                    controller_id: winning_controller(previous),
                    winning_controller_id: None,
                    occurred_at_millis: now_millis,
                    message: Some("one-shot completed".into()),
                },
            ));
        }
        let previous_controllers = previous
            .map(|instance| {
                instance
                    .controllers
                    .iter()
                    .map(|controller| controller.id)
                    .collect::<HashSet<_>>()
            })
            .unwrap_or_default();
        if previous.is_some_and(|previous| previous.completed) && !instance.completed {
            let controller_id = winning_controller(Some(instance));
            for kind in [
                light_application::DynamicRuntimeEventKind::InstanceStarted,
                if instance
                    .pending_until_millis
                    .is_some_and(|boundary| now_millis < boundary)
                {
                    light_application::DynamicRuntimeEventKind::InstancePending
                } else {
                    light_application::DynamicRuntimeEventKind::InstanceActive
                },
            ] {
                events.push(light_application::EventDraft::dynamic_runtime_changed(
                    None,
                    light_application::DynamicRuntimeChange {
                        kind,
                        dynamic_id: Some(instance.definition.id),
                        runtime_instance_id: Some(instance.id),
                        controller_id,
                        winning_controller_id: controller_id,
                        occurred_at_millis: now_millis,
                        message: Some("one-shot retriggered".into()),
                    },
                ));
            }
        }
        for controller in &instance.controllers {
            if instance.completed || previous.is_some_and(|previous| previous.completed) {
                continue;
            }
            if !previous_controllers.contains(&controller.id) {
                for kind in [
                    light_application::DynamicRuntimeEventKind::InstanceStarted,
                    if instance
                        .pending_until_millis
                        .is_some_and(|boundary| now_millis < boundary)
                    {
                        light_application::DynamicRuntimeEventKind::InstancePending
                    } else {
                        light_application::DynamicRuntimeEventKind::InstanceActive
                    },
                ] {
                    events.push(light_application::EventDraft::dynamic_runtime_changed(
                        None,
                        light_application::DynamicRuntimeChange {
                            kind,
                            dynamic_id: Some(instance.definition.id),
                            runtime_instance_id: Some(instance.id),
                            controller_id: Some(controller.id),
                            winning_controller_id: winning_controller(Some(instance)),
                            occurred_at_millis: now_millis,
                            message: None,
                        },
                    ));
                }
            }
        }
        if !instance.completed
            && previous.is_some_and(|previous| previous.pending_until_millis.is_some())
            && instance
                .pending_until_millis
                .is_none_or(|boundary| now_millis >= boundary)
        {
            events.push(light_application::EventDraft::dynamic_runtime_changed(
                None,
                light_application::DynamicRuntimeChange {
                    kind: light_application::DynamicRuntimeEventKind::InstanceActive,
                    dynamic_id: Some(instance.definition.id),
                    runtime_instance_id: Some(instance.id),
                    controller_id: winning_controller(Some(instance)),
                    winning_controller_id: winning_controller(Some(instance)),
                    occurred_at_millis: now_millis,
                    message: None,
                },
            ));
        }
    }
    for instance in &before.instances {
        let after_instance = after_instances.get(&instance.id).copied();
        let after_controllers = after_instance
            .map(|instance| {
                instance
                    .controllers
                    .iter()
                    .map(|controller| controller.id)
                    .collect::<HashSet<_>>()
            })
            .unwrap_or_default();
        for transition in &instance.controller_transitions {
            if transition.release_started_at_millis.is_some()
                && !after_controllers.contains(&transition.controller_id)
            {
                events.push(light_application::EventDraft::dynamic_runtime_changed(
                    None,
                    light_application::DynamicRuntimeChange {
                        kind: light_application::DynamicRuntimeEventKind::TransitionCompleted,
                        dynamic_id: Some(instance.definition.id),
                        runtime_instance_id: Some(instance.id),
                        controller_id: Some(transition.controller_id),
                        winning_controller_id: winning_controller(after_instance),
                        occurred_at_millis: now_millis,
                        message: None,
                    },
                ));
            }
        }
        let before_winner = winning_controller(Some(instance));
        let after_winner = winning_controller(after_instance);
        if before_winner != after_winner && after_winner.is_some() {
            events.push(light_application::EventDraft::dynamic_runtime_changed(
                None,
                light_application::DynamicRuntimeChange {
                    kind: light_application::DynamicRuntimeEventKind::ControllerWinnerChanged,
                    dynamic_id: Some(instance.definition.id),
                    runtime_instance_id: Some(instance.id),
                    controller_id: after_winner,
                    winning_controller_id: after_winner,
                    occurred_at_millis: now_millis,
                    message: None,
                },
            ));
        }
    }
    events
}

fn winning_controller(instance: Option<&light_dynamics::DynamicInstanceSnapshot>) -> Option<Uuid> {
    instance
        .filter(|instance| !instance.completed)
        .and_then(|instance| {
            instance
                .controllers
                .iter()
                .max_by_key(|controller| {
                    (
                        controller.priority,
                        controller.activated_at_millis,
                        controller.id,
                    )
                })
                .map(|controller| controller.id)
        })
}

fn authored_activation_mix(
    changed_at_millis: u64,
    timing: light_dynamics::DynamicValueTiming,
    now_millis: u64,
) -> f32 {
    let delay = timing.delay_millis.unwrap_or_default();
    if now_millis < changed_at_millis.saturating_add(delay) {
        return 0.0;
    }
    let fade = timing.fade_millis.unwrap_or_default();
    if fade == 0 {
        return 1.0;
    }
    (now_millis
        .saturating_sub(changed_at_millis)
        .saturating_sub(delay) as f32
        / fade as f32)
        .clamp(0.0, 1.0)
}

fn blend_attribute_value(
    underlying: AttributeValue,
    contribution: AttributeValue,
    mix: f32,
) -> AttributeValue {
    match (underlying.normalized(), contribution.normalized()) {
        (Some(underlying), Some(contribution)) => AttributeValue::Normalized(
            underlying + (contribution - underlying) * mix.clamp(0.0, 1.0),
        ),
        _ if mix >= 0.5 => contribution,
        _ => underlying,
    }
}

struct DynamicPlaybackControl {
    playback_number: u16,
    master: f32,
    crossfade_non_intensity: bool,
    auto_off_full_control: bool,
    temporary_only: bool,
}

fn fully_controlled_dynamic_playbacks(
    engine: &Engine,
    samples: &[light_dynamics::DynamicRuntimeSample],
    controls: &HashMap<Uuid, DynamicPlaybackControl>,
    runtime: &light_dynamics::DynamicRuntimeSnapshot,
    now: chrono::DateTime<chrono::Utc>,
) -> Vec<u16> {
    let persistent = engine
        .playback_contributions_at(now)
        .into_iter()
        .filter(|contribution| !contribution.source.temporary)
        .collect::<Vec<_>>();
    let mut addresses = HashMap::<
        u16,
        Vec<(
            &light_dynamics::DynamicRuntimeSample,
            &DynamicPlaybackControl,
        )>,
    >::new();
    let controller_sources = runtime
        .instances
        .iter()
        .flat_map(|instance| {
            instance
                .controllers
                .iter()
                .map(|controller| (controller.id, controller.source.clone()))
        })
        .collect::<HashMap<_, _>>();
    let persistent_fat = persistent_fat_values(engine);
    for sample in samples {
        if let Some(control) = controls.get(&sample.controller_id)
            && control.auto_off_full_control
        {
            addresses
                .entry(control.playback_number)
                .or_default()
                .push((sample, control));
        }
    }
    addresses
        .into_iter()
        .filter_map(|(number, target_samples)| {
            (!target_samples.is_empty()
                && target_samples.iter().all(|(sample, control)| {
                    let dynamic_value = if sample.attribute.is_intensity() {
                        sample.value * control.master
                    } else {
                        sample.value
                    };
                    persistent.iter().any(|candidate| {
                        candidate
                            .source
                            .playback_number
                            .is_some_and(|other| other != number)
                            && candidate.value.fixture_id == sample.target
                            && candidate.value.attribute == sample.attribute
                            && persistent_playback_wins_dynamic(candidate, sample, dynamic_value)
                    }) || samples.iter().any(|candidate| {
                        candidate.controller_id != sample.controller_id
                            && candidate.target == sample.target
                            && candidate.attribute == sample.attribute
                            && candidate.activation_mix >= 1.0
                            && controller_sources
                                .get(&candidate.controller_id)
                                .is_some_and(|source| match source {
                                    light_dynamics::DynamicControllerSource::Playback {
                                        playback_number,
                                    } => {
                                        *playback_number != number
                                            && controls
                                                .get(&candidate.controller_id)
                                                .is_some_and(|control| !control.temporary_only)
                                    }
                                    light_dynamics::DynamicControllerSource::Programmer {
                                        ..
                                    }
                                    | light_dynamics::DynamicControllerSource::Cue { .. } => true,
                                })
                            && persistent_dynamic_wins_dynamic(candidate, sample)
                    }) || persistent_fat.iter().any(|candidate| {
                        candidate.fixture_id == sample.target
                            && candidate.attribute == sample.attribute
                            && persistent_semantic_wins_dynamic(
                                candidate.priority,
                                candidate.changed_at_millis,
                                sample,
                            )
                    })
                }))
            .then_some(number)
        })
        .collect()
}

#[derive(Clone)]
struct PersistentFatValue {
    fixture_id: FixtureId,
    attribute: AttributeKey,
    priority: i16,
    changed_at_millis: u64,
}

fn persistent_fat_values(engine: &Engine) -> Vec<PersistentFatValue> {
    let programmer =
        engine
            .dynamic_programmer_values()
            .into_iter()
            .filter_map(|(_, priority, stored)| {
                matches!(
                    stored.value,
                    light_dynamics::DynamicSemanticValue::FixAt { .. }
                )
                .then_some(PersistentFatValue {
                    fixture_id: stored.fixture_id,
                    attribute: stored.attribute,
                    priority,
                    changed_at_millis: stored.changed_at_millis,
                })
            });
    let cues = engine
        .active_cue_dynamic_values()
        .into_iter()
        .filter_map(|stored| {
            matches!(
                stored.value,
                light_dynamics::DynamicSemanticValue::FixAt { .. }
            )
            .then_some(PersistentFatValue {
                fixture_id: stored.fixture_id,
                attribute: stored.attribute,
                priority: stored.priority,
                changed_at_millis: stored.changed_at_millis,
            })
        });
    programmer.chain(cues).collect()
}

fn persistent_dynamic_wins_dynamic(
    candidate: &light_dynamics::DynamicRuntimeSample,
    dynamic: &light_dynamics::DynamicRuntimeSample,
) -> bool {
    (
        candidate.priority,
        candidate.activated_at_millis,
        candidate.controller_id,
    ) > (
        dynamic.priority,
        dynamic.activated_at_millis,
        dynamic.controller_id,
    )
}

fn persistent_semantic_wins_dynamic(
    priority: i16,
    changed_at_millis: u64,
    dynamic: &light_dynamics::DynamicRuntimeSample,
) -> bool {
    (priority, changed_at_millis) > (dynamic.priority, dynamic.activated_at_millis)
}

fn persistent_playback_wins_dynamic(
    candidate: &light_playback::PlaybackContribution,
    dynamic: &light_dynamics::DynamicRuntimeSample,
    dynamic_value: f32,
) -> bool {
    if candidate.value.priority != dynamic.priority {
        return candidate.value.priority > dynamic.priority;
    }
    if candidate.value.merge_mode == MergeMode::Htp {
        return candidate.value.value.normalized().unwrap_or(0.0) > dynamic_value;
    }
    u64::try_from(candidate.value.changed_at.timestamp_millis()).unwrap_or_default()
        > dynamic.activated_at_millis
}

fn reconcile_dynamic_playbacks(
    engine: &Engine,
    dynamics: &mut light_dynamics::DynamicRuntime,
    now_millis: u64,
) -> HashMap<Uuid, DynamicPlaybackControl> {
    let snapshot = engine.snapshot();
    let active = engine
        .active_dynamic_playbacks()
        .into_iter()
        .filter(|playback| playback.enabled)
        .collect::<Vec<_>>();
    let desired_ids = active
        .iter()
        .map(|playback| dynamic_playback_controller_id(playback.playback_number))
        .collect::<HashSet<_>>();
    for (instance_id, controller) in dynamics.controllers() {
        if matches!(
            controller.source,
            light_dynamics::DynamicControllerSource::Playback { .. }
        ) && !desired_ids.contains(&controller.id)
        {
            let release_millis = match controller.source {
                light_dynamics::DynamicControllerSource::Playback { playback_number } => snapshot
                    .playbacks
                    .iter()
                    .find(|playback| playback.number == playback_number)
                    .map_or(0, |playback| playback.xfade_millis),
                _ => 0,
            };
            let _ =
                dynamics.off_controller(instance_id, controller.id, now_millis, 0, release_millis);
        }
    }

    let definitions = snapshot
        .dynamics
        .iter()
        .map(|definition| (definition.id, definition))
        .collect::<HashMap<_, _>>();
    let groups = snapshot
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect::<HashMap<_, _>>();
    let mut controls = HashMap::new();
    for active in active {
        let Some(playback) = snapshot
            .playbacks
            .iter()
            .find(|playback| playback.number == active.playback_number)
        else {
            continue;
        };
        let light_playback::PlaybackTarget::Dynamic { assignment } = &playback.target else {
            continue;
        };
        let controller_id = dynamic_playback_controller_id(active.playback_number);
        controls.insert(
            controller_id,
            DynamicPlaybackControl {
                playback_number: active.playback_number,
                master: active.master,
                crossfade_non_intensity: assignment.crossfade_non_intensity,
                auto_off_full_control: assignment.auto_off_full_control,
                temporary_only: active.flash && active.flash_restore_off,
            },
        );
        let definition = assignment
            .dynamic
            .dynamic_id
            .and_then(|id| definitions.get(&id).copied())
            .cloned()
            .unwrap_or_else(|| (*assignment.dynamic.embedded_fallback.definition).clone());
        let speed_multiplier = effective_dynamic_playback_speed(&definition, &active);
        if let Some((instance_id, _)) = dynamics.controller(controller_id) {
            let _ = dynamics.update_controller(
                controller_id,
                Some(active.size),
                Some(speed_multiplier),
                None,
            );
            let resume_policy = match assignment.resume_policy {
                light_playback::DynamicPlaybackResumePolicy::FollowDynamic => None,
                light_playback::DynamicPlaybackResumePolicy::ResumeFrozenPhase => {
                    Some(light_dynamics::ActivationPolicy::StartNow)
                }
                light_playback::DynamicPlaybackResumePolicy::RejoinSynchronizedPosition => {
                    Some(light_dynamics::ActivationPolicy::JoinSyncNow)
                }
                light_playback::DynamicPlaybackResumePolicy::ResumeOnNextBoundary => {
                    Some(light_dynamics::ActivationPolicy::NextBoundary)
                }
            };
            let _ = dynamics.set_controller_paused_with_resume(
                instance_id,
                controller_id,
                active.paused,
                now_millis,
                resume_policy,
            );
            continue;
        }
        if !definitions.contains_key(&definition.id)
            && dynamics
                .install_fallback_definition(definition.clone())
                .is_err()
        {
            continue;
        }
        let targets = match &definition.target_binding {
            light_dynamics::DynamicTargetBinding::LiveGroup { group_id } => {
                light_programmer::resolve_group(group_id, &groups).unwrap_or_default()
            }
            light_dynamics::DynamicTargetBinding::FrozenTargets { targets } => targets.clone(),
            light_dynamics::DynamicTargetBinding::Targetless => match &assignment.target_scope {
                Some(light_playback::DynamicPlaybackTargetScope::LiveGroup { group_id }) => {
                    light_programmer::resolve_group(group_id, &groups).unwrap_or_default()
                }
                Some(light_playback::DynamicPlaybackTargetScope::FrozenTargets { targets }) => {
                    targets.clone()
                }
                None => Vec::new(),
            },
        };
        let activated_at_millis =
            u64::try_from(active.activated_at.timestamp_millis()).unwrap_or_default();
        if dynamics
            .start(light_dynamics::DynamicStartRequest {
                definition_id: definition.id,
                controller: light_dynamics::DynamicController {
                    id: controller_id,
                    source: light_dynamics::DynamicControllerSource::Playback {
                        playback_number: active.playback_number,
                    },
                    priority: assignment.priority,
                    activated_at_millis,
                    size: active.size,
                    speed_multiplier,
                    phase_offset_degrees: 0.0,
                    paused: active.paused,
                },
                target_scope: light_dynamics::DynamicTargetScope {
                    ordered_targets: targets,
                },
                stage_positions: (*snapshot.dynamic_stage_positions).clone(),
                now_millis: activated_at_millis.min(now_millis),
                activation_delay_millis: 0,
                activation_duration_millis: playback.xfade_millis,
                activation_policy_override: assignment.activation_override,
                reuse_matching_targetless: false,
            })
            .is_err()
        {
            controls.remove(&controller_id);
        }
    }
    controls
}

fn effective_dynamic_playback_speed(
    definition: &light_dynamics::DynamicDefinition,
    active: &light_playback::ActiveDynamicPlayback,
) -> f32 {
    let local = active.local_speed_multiplier.factor();
    let learned = active.learned_duration_millis.and_then(|learned| {
        let light_dynamics::DynamicSpeed::Fixed { duration_millis } = &definition.speed else {
            return None;
        };
        Some(*duration_millis as f64 / learned.max(1) as f64)
    });
    (local * learned.unwrap_or(1.0)).clamp(f64::EPSILON, 1_024.0) as f32
}

fn dynamic_playback_controller_id(playback_number: u16) -> Uuid {
    Uuid::from_u128(0x4459_4e41_4d49_432d_504c_4159_4241_434b ^ u128::from(playback_number))
}

fn reconcile_programmer_dynamics(
    engine: &Engine,
    dynamics: &mut light_dynamics::DynamicRuntime,
    now_millis: u64,
    extra_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
) {
    struct DesiredController {
        programmer_id: Uuid,
        priority: i16,
        activated_at_millis: u64,
        reference: light_dynamics::DynamicReference,
        overrides: light_dynamics::DynamicInstanceOverrides,
        timing: light_dynamics::DynamicValueTiming,
        targets: Vec<FixtureId>,
    }

    let values = engine
        .dynamic_programmer_values()
        .into_iter()
        .chain(extra_values.iter().cloned());
    let mut desired = HashMap::<Uuid, DesiredController>::new();
    let mut off = HashMap::<Uuid, light_dynamics::DynamicValueTiming>::new();
    for (programmer_id, priority, stored) in values {
        match stored.value {
            light_dynamics::DynamicSemanticValue::DynamicOn {
                instance_link,
                dynamic,
                overrides,
                timing,
                ..
            } => {
                let controller =
                    desired
                        .entry(instance_link)
                        .or_insert_with(|| DesiredController {
                            programmer_id,
                            priority,
                            activated_at_millis: stored.changed_at_millis,
                            reference: dynamic,
                            overrides,
                            timing,
                            targets: Vec::new(),
                        });
                if !controller.targets.contains(&stored.fixture_id) {
                    controller.targets.push(stored.fixture_id);
                }
            }
            light_dynamics::DynamicSemanticValue::DynamicOff {
                instance_link,
                timing,
            } => {
                off.entry(instance_link).or_insert(timing);
            }
            light_dynamics::DynamicSemanticValue::Static { .. }
            | light_dynamics::DynamicSemanticValue::FixAt { .. }
            | light_dynamics::DynamicSemanticValue::Release => {}
        }
    }
    for controller_id in off.keys() {
        desired.remove(controller_id);
    }

    for (controller_id, timing) in &off {
        if let Some((instance_id, _)) = dynamics.controller(*controller_id) {
            let _ = dynamics.off_controller(
                instance_id,
                *controller_id,
                now_millis,
                timing.delay_millis.unwrap_or_default(),
                timing.fade_millis.unwrap_or_default(),
            );
        }
    }
    let desired_ids = desired.keys().copied().collect::<HashSet<_>>();
    for (instance_id, controller) in dynamics.controllers() {
        if matches!(
            controller.source,
            light_dynamics::DynamicControllerSource::Programmer { .. }
        ) && !desired_ids.contains(&controller.id)
            && !off.contains_key(&controller.id)
        {
            let _ = dynamics.off_controller(instance_id, controller.id, now_millis, 0, 0);
        }
    }

    let snapshot = engine.snapshot();
    let definitions = snapshot
        .dynamics
        .iter()
        .map(|definition| (definition.id, definition))
        .collect::<HashMap<_, _>>();
    for (controller_id, desired) in desired {
        if dynamics.controller(controller_id).is_some() {
            let _ = dynamics.update_controller(
                controller_id,
                Some(desired.overrides.size),
                Some(desired.overrides.speed_multiplier.factor() as f32),
                Some(desired.overrides.phase_offset_degrees),
            );
            continue;
        }
        let definition = desired
            .reference
            .dynamic_id
            .and_then(|id| definitions.get(&id).copied())
            .cloned()
            .unwrap_or_else(|| (*desired.reference.embedded_fallback.definition).clone());
        if !definitions.contains_key(&definition.id)
            && dynamics
                .install_fallback_definition(definition.clone())
                .is_err()
        {
            continue;
        }
        let _ = dynamics.start(light_dynamics::DynamicStartRequest {
            definition_id: definition.id,
            controller: light_dynamics::DynamicController {
                id: controller_id,
                source: light_dynamics::DynamicControllerSource::Programmer {
                    programmer_id: desired.programmer_id,
                },
                priority: desired.priority,
                activated_at_millis: desired.activated_at_millis,
                size: desired.overrides.size,
                speed_multiplier: desired.overrides.speed_multiplier.factor() as f32,
                phase_offset_degrees: desired.overrides.phase_offset_degrees,
                paused: false,
            },
            target_scope: light_dynamics::DynamicTargetScope {
                ordered_targets: desired.targets,
            },
            stage_positions: (*snapshot.dynamic_stage_positions).clone(),
            now_millis: desired.activated_at_millis.min(now_millis),
            activation_delay_millis: desired.timing.delay_millis.unwrap_or_default(),
            activation_duration_millis: desired.timing.fade_millis.unwrap_or_default(),
            activation_policy_override: None,
            reuse_matching_targetless: true,
        });
    }
}

fn reconcile_cue_dynamics(
    engine: &Engine,
    dynamics: &mut light_dynamics::DynamicRuntime,
    now_millis: u64,
) {
    struct DesiredController {
        controller_id: Uuid,
        instance_link: Uuid,
        cue_list_id: light_core::CueListId,
        priority: i16,
        activated_at_millis: u64,
        reference: light_dynamics::DynamicReference,
        overrides: light_dynamics::DynamicInstanceOverrides,
        timing: light_dynamics::DynamicValueTiming,
        targets: Vec<FixtureId>,
    }

    let mut desired = Vec::<DesiredController>::new();
    let mut release_timings = HashMap::<Uuid, light_dynamics::DynamicValueTiming>::new();
    for stored in engine.active_cue_dynamic_values() {
        if let light_dynamics::DynamicSemanticValue::DynamicOff {
            instance_link,
            timing,
        } = &stored.value
        {
            release_timings.insert(
                cue_dynamic_controller_id(stored.cue_list_id, *instance_link),
                *timing,
            );
            continue;
        }
        let light_dynamics::DynamicSemanticValue::DynamicOn {
            instance_link,
            dynamic,
            overrides,
            timing,
            ..
        } = stored.value
        else {
            continue;
        };
        let controller_id = cue_dynamic_controller_id(stored.cue_list_id, instance_link);
        if let Some(controller) = desired
            .iter_mut()
            .find(|candidate| candidate.controller_id == controller_id)
        {
            if !controller.targets.contains(&stored.fixture_id) {
                controller.targets.push(stored.fixture_id);
            }
            continue;
        }
        desired.push(DesiredController {
            controller_id,
            instance_link,
            cue_list_id: stored.cue_list_id,
            priority: stored.priority,
            activated_at_millis: stored.changed_at_millis,
            reference: dynamic,
            overrides,
            timing,
            targets: vec![stored.fixture_id],
        });
    }

    let desired_ids = desired
        .iter()
        .map(|controller| controller.controller_id)
        .collect::<HashSet<_>>();
    for (instance_id, controller) in dynamics.controllers() {
        if matches!(
            controller.source,
            light_dynamics::DynamicControllerSource::Cue { .. }
        ) && !desired_ids.contains(&controller.id)
        {
            let timing = release_timings
                .get(&controller.id)
                .copied()
                .unwrap_or_default();
            let _ = dynamics.off_controller(
                instance_id,
                controller.id,
                now_millis,
                timing.delay_millis.unwrap_or_default(),
                timing.fade_millis.unwrap_or_default(),
            );
        }
    }

    let snapshot = engine.snapshot();
    let definitions = snapshot
        .dynamics
        .iter()
        .map(|definition| (definition.id, definition))
        .collect::<HashMap<_, _>>();
    let groups = snapshot
        .groups
        .iter()
        .map(|group| (group.id.clone(), group.clone()))
        .collect::<HashMap<_, _>>();
    for desired in desired {
        if dynamics.controller(desired.controller_id).is_some() {
            let _ = dynamics.update_controller(
                desired.controller_id,
                Some(desired.overrides.size),
                Some(desired.overrides.speed_multiplier.factor() as f32),
                Some(desired.overrides.phase_offset_degrees),
            );
            continue;
        }
        let definition = desired
            .reference
            .dynamic_id
            .and_then(|id| definitions.get(&id).copied())
            .cloned()
            .unwrap_or_else(|| (*desired.reference.embedded_fallback.definition).clone());
        if !definitions.contains_key(&definition.id)
            && dynamics
                .install_fallback_definition(definition.clone())
                .is_err()
        {
            continue;
        }
        let targets = match &definition.target_binding {
            light_dynamics::DynamicTargetBinding::LiveGroup { group_id } => {
                light_programmer::resolve_group(group_id, &groups).unwrap_or_default()
            }
            light_dynamics::DynamicTargetBinding::FrozenTargets { targets } => targets.clone(),
            light_dynamics::DynamicTargetBinding::Targetless => desired.targets,
        };
        let _ = dynamics.start(light_dynamics::DynamicStartRequest {
            definition_id: definition.id,
            controller: light_dynamics::DynamicController {
                id: desired.controller_id,
                source: light_dynamics::DynamicControllerSource::Cue {
                    cue_list_id: desired.cue_list_id.0,
                    instance_link: desired.instance_link,
                },
                priority: desired.priority,
                activated_at_millis: desired.activated_at_millis,
                size: desired.overrides.size,
                speed_multiplier: desired.overrides.speed_multiplier.factor() as f32,
                phase_offset_degrees: desired.overrides.phase_offset_degrees,
                paused: false,
            },
            target_scope: light_dynamics::DynamicTargetScope {
                ordered_targets: targets,
            },
            stage_positions: (*snapshot.dynamic_stage_positions).clone(),
            now_millis: desired.activated_at_millis.min(now_millis),
            activation_delay_millis: desired.timing.delay_millis.unwrap_or_default(),
            activation_duration_millis: desired.timing.fade_millis.unwrap_or_default(),
            activation_policy_override: None,
            reuse_matching_targetless: false,
        });
    }
}

fn cue_dynamic_controller_id(cue_list_id: light_core::CueListId, instance_link: Uuid) -> Uuid {
    Uuid::from_u128(
        instance_link.as_u128()
            ^ cue_list_id.0.as_u128().rotate_left(1)
            ^ 0x4355_452d_4459_4e41_4d49_432d_4354_524c,
    )
}

fn update_timecode(runtime: &Runtime) {
    let current = runtime.timecode.lock().poll_loss().cloned();
    runtime
        .engine
        .set_timecode_frame(current.as_ref().map(timecode_frame));
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

    pub(super) fn into_task(mut self) -> OutputTask {
        self.start.take();
        self.task
    }
}

impl SharedResources {
    async fn create(config: &Config) -> anyhow::Result<Self> {
        Ok(Self {
            output: bind_output(config.bind_ip).await?,
            sequences: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            control: create_control(&config.persisted_runtime),
        })
    }

    fn runtime(&self, config: &Config) -> Runtime {
        Runtime {
            engine: Arc::clone(&config.engine),
            output: Arc::clone(&self.output),
            sequences: Arc::clone(&self.sequences),
            control: Arc::clone(&self.control),
            timecode: Arc::clone(&config.timecode),
            playback: config.playback.clone(),
            active_show: config.active_show.clone(),
            activation: config.activation.clone(),
            cancellation: config.cancellation.clone(),
            dynamics: Arc::clone(&config.dynamics),
            speed_groups: Arc::clone(&config.speed_groups),
            rate: Arc::clone(&config.rate),
            dynamic_auto_offs: Arc::clone(&config.dynamic_auto_offs),
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
            start: Some(start),
            task,
        }
    }
}

#[cfg(test)]
#[path = "output_scheduler_tests.rs"]
mod tests;
