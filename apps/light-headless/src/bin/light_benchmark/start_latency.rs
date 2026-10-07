//! TL-641: `--start-latency` times one Start Now Dynamic gesture on every fixture of a running
//! headless-stress Live lane, from the gesture to the first DMX frame that carries it.
//!
//! The gesture is the desk's: the runtime controller is started under the Dynamics lock
//! (`OutputCapability::start_dynamic`), then the expanded `DynamicOn` rows are applied under the
//! Programmer lock (`DynamicsService::start`). Two probes run on fresh scenarios:
//!
//! - `serialized`: the gesture between two frames, then the first frame split by phase (capture,
//!   reconciliation and merge, controller construction, family evaluation and render,
//!   publication).
//! - `concurrent`: an output thread paced at the configured rate and the gesture issued while a
//!   frame is in progress, the way an operator presses a key. It reports how many frames after
//!   the in-progress one first carried the Dynamic and whether the in-progress frame was delayed.
//!
//! The contract (criterion 2) holds when the first frame after the in-progress one carries the
//! Dynamic and the gesture-to-output time is within two frame periods.
use crate::light_benchmark::{
    arguments::{Arguments, ProfileConfig},
    runner::{prepare_scenario, profile_configs},
    scenario::BenchmarkScenario,
    semantic_runner::{LiveScenario, PendingStart},
    statistics::{Distribution, distribution},
};
use chrono::Duration as ChronoDuration;
use light_dynamics::{
    ActivationPolicy, DynamicController, DynamicControllerSource, DynamicDefinitionSnapshot,
    DynamicInstanceOverrides, DynamicReference, DynamicSemanticValue, DynamicStartRequest,
    DynamicTargetScope, Rational,
};
use light_headless_runtime::output_benchmark::{LiveOutputBench, LiveOutputFrame};
use light_programmer::DynamicProgrammerValueMutation;
use serde::Serialize;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use uuid::Uuid;

/// Frames rendered before the gesture and after its first frame.
const SETTLE_FRAMES: u64 = 8;
/// The concurrent probe gives up when the Dynamic has not reached output by then.
const CONCURRENT_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Serialize)]
pub struct StartLatencyReport {
    pub fixture_count: usize,
    pub started_targets: usize,
    pub lanes: usize,
    pub rate_hz: u16,
    pub frame_period_microseconds: f64,
    /// Two frame periods: the next frame after the one in progress, completed.
    pub budget_microseconds: f64,
    pub serialized: SerializedProbe,
    pub concurrent: ConcurrentProbe,
    pub definition: &'static str,
}

#[derive(Debug, Serialize)]
pub struct GesturePhases {
    /// Expanding the gesture into one `DynamicOn` row per target and lane (no lock held).
    pub expand_microseconds: f64,
    /// Runtime controller construction under the authoritative Dynamics lock.
    pub runtime_start_microseconds: f64,
    /// Applying the rows under the Programmer lock.
    pub programmer_apply_microseconds: f64,
    pub total_microseconds: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct FramePhases {
    pub capture_microseconds: f64,
    /// Reconciliation and merge of the captured Dynamic sources, without controller work.
    pub reconcile_merge_microseconds: f64,
    /// Controller start, lane selection and target scoping inside reconciliation.
    pub controller_construction_microseconds: f64,
    /// The rest of the transaction: sampling, family evaluation, final render and acceptance.
    pub family_evaluation_and_render_microseconds: f64,
    pub publication_microseconds: f64,
    pub total_microseconds: f64,
    pub dynamic_samples: usize,
    /// TL-659: this frame claimed the start from the desk's change lead ledger, the same claim
    /// the DMX statistics measure from.
    pub change_lead_claimed: bool,
    /// Application time from the claimed start to this frame's sample instant.
    pub logical_change_lead_microseconds: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct SerializedProbe {
    pub steady_before: Option<Distribution>,
    pub gesture: GesturePhases,
    pub first_frame: FramePhases,
    pub first_frame_changed_dmx: bool,
    /// Frames after the gesture until DMX first differs from the frame before it (1 = the first
    /// frame); `None` when it did not change within the settle frames. A wave that starts at the
    /// current level changes DMX only once it has moved away from it.
    pub frames_until_dmx_change: Option<u64>,
    pub steady_after: Option<Distribution>,
    /// The same gesture again after releasing the first start: what remains once caches built
    /// by the first start are warm.
    pub repeated_start: RepeatedStart,
    /// Gesture plus the first frame, without waiting for a frame boundary.
    pub start_to_first_output_microseconds: f64,
    /// TL-646 (`--start-latency-cycles`): repeated release and restart, first without and then
    /// with the desk's persistence checkpoint after each release.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cycles: Vec<StartCycle>,
}

#[derive(Debug, Serialize)]
pub struct StartCycle {
    pub checkpointed: bool,
    pub cycle: u16,
    pub gesture_microseconds: f64,
    pub first_frame_microseconds: f64,
    /// Source records in the published catalogue after the restart's first frame.
    pub source_records: usize,
}

#[derive(Debug, Serialize)]
pub struct ConcurrentProbe {
    pub steady_before: Option<Distribution>,
    pub gesture: GesturePhases,
    /// Pipeline time of the frame in progress when the gesture began, and of the median
    /// steady frame before it. A gesture that blocks that frame shows up as their difference.
    pub in_progress_frame_microseconds: f64,
    pub in_progress_frame_capture_microseconds: f64,
    /// 0 when the in-progress frame itself carried the Dynamic (the gesture reached the
    /// Dynamics runtime before that frame's Dynamics transaction), 1 for the next frame.
    pub frames_until_output: Option<u64>,
    /// The same count read from the desk's change lead ledger (TL-659): the frame that claimed
    /// the start. It equals `frames_until_output` when the measurement and the output agree.
    pub frames_until_claimed: Option<u64>,
    pub first_frame: Option<FramePhases>,
    /// From the gesture to the end of the first frame that carried the Dynamic.
    pub start_to_first_output_microseconds: Option<f64>,
    /// The contract: the in-progress frame or the one after it carried the Dynamic.
    pub next_frame_after_in_progress: bool,
    pub within_budget: bool,
}

pub fn run(arguments: &Arguments) -> Result<serde_json::Value, String> {
    let mut reports = Vec::new();
    for config in profile_configs(arguments) {
        let serialized = serialized(arguments, config)?;
        let (concurrent, shape) = concurrent(arguments, config, false)?;
        let period = 1_000_000.0 / f64::from(config.rate_hz);
        reports.push(StartLatencyReport {
            fixture_count: shape.0,
            started_targets: shape.1,
            lanes: shape.2,
            rate_hz: config.rate_hz,
            frame_period_microseconds: period,
            budget_microseconds: 2.0 * period,
            serialized,
            concurrent,
            definition: "one Start Now Dynamic on every headless-stress fixture and logical head; the gesture starts the runtime controller under the Dynamics lock and applies its DynamicOn rows under the Programmer lock",
        });
    }
    serde_json::to_value(reports).map_err(|error| error.to_string())
}

fn prepared(
    arguments: &Arguments,
    config: ProfileConfig,
) -> Result<(BenchmarkScenario, PendingStart), String> {
    let (_loopback, mut scenario) = prepare_scenario(arguments, config)?;
    let pending = scenario
        .live
        .as_mut()
        .and_then(|live| live.pending_start.take())
        .ok_or("--start-latency needs the semantic headless-stress scenario")?;
    if pending.definition.default_activation != ActivationPolicy::StartNow {
        return Err("the start-latency Dynamic must activate Start Now".into());
    }
    Ok((scenario, pending))
}

/// The thread-safe part of a scenario the output lane and the gesture share.
struct Lane<'s> {
    scenario: &'s BenchmarkScenario,
    bench: &'s LiveOutputBench,
}

impl<'s> Lane<'s> {
    fn new(scenario: &'s BenchmarkScenario) -> Self {
        let live: &LiveScenario = scenario.live.as_ref().expect("checked by prepared");
        Self {
            scenario,
            bench: &live.bench,
        }
    }
}

// The output thread and the gesture share only the `Sync` parts of the scenario (engine, clock, Programmers,
// bench); the scenario's tracking feed, which is not `Sync`, is never touched here.
struct SharedLane<'s> {
    engine: &'s light_engine::Engine,
    clock: &'s light_core::ManualClock,
    logical_start: chrono::DateTime<chrono::Utc>,
    programmers: &'s light_programmer::ProgrammerRegistry,
    bench: &'s LiveOutputBench,
}

impl<'s> From<&Lane<'s>> for SharedLane<'s> {
    fn from(lane: &Lane<'s>) -> Self {
        Self {
            engine: &lane.scenario.engine,
            clock: &lane.scenario.clock,
            logical_start: lane.scenario.logical_start,
            programmers: &lane.scenario.programmers,
            bench: lane.bench,
        }
    }
}

fn set_tick(lane: &SharedLane<'_>, tick: u64, rate_hz: u16) {
    let nanos = tick.saturating_mul(1_000_000_000) / u64::from(rate_hz);
    lane.clock
        .set(lane.logical_start + ChronoDuration::nanoseconds(nanos as i64));
}

fn render(lane: &SharedLane<'_>) -> Result<(LiveOutputFrame, Duration), String> {
    let started = Instant::now();
    let frame = lane
        .bench
        .render(Default::default(), &[])
        .map_err(|error| format!("render start-latency frame: {error}"))?;
    Ok((frame, started.elapsed()))
}

fn micros(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000_000.0
}

fn frame_phases(frame: &LiveOutputFrame, total: Duration) -> FramePhases {
    let construction = frame.start_path.controller_construction;
    let reconcile = frame.start_path.reconcile.saturating_sub(construction);
    FramePhases {
        capture_microseconds: micros(frame.capture),
        reconcile_merge_microseconds: micros(reconcile),
        controller_construction_microseconds: micros(construction),
        family_evaluation_and_render_microseconds: micros(
            frame.transaction.saturating_sub(frame.start_path.reconcile),
        ),
        publication_microseconds: micros(frame.publication),
        total_microseconds: micros(total),
        dynamic_samples: frame.dynamic_samples,
        change_lead_claimed: frame.change_lead_start.is_some(),
        logical_change_lead_microseconds: frame
            .change_lead_start
            .map(|start| (frame.rendered.sampled_at.timestamp_micros() - start) as f64),
    }
}

/// The desk's Start Now gesture: runtime controller first, then the Programmer rows.
fn gesture(
    lane: &SharedLane<'_>,
    pending: &PendingStart,
    link: Uuid,
) -> Result<GesturePhases, String> {
    let started = Instant::now();
    let state = lane
        .programmers
        .get(pending.session)
        .ok_or("the probe Programmer is unavailable")?;
    let definition = &pending.definition;
    let controller_id = light_dynamics::programmer_dynamic_controller_id(state.id, link);
    let now_millis =
        u64::try_from(lane.engine.application_time().timestamp_millis()).unwrap_or_default();
    let reference = DynamicReference {
        dynamic_id: Some(definition.id),
        last_known_pool_number: definition.pool_number,
        embedded_fallback: DynamicDefinitionSnapshot {
            definition: Arc::new(definition.clone()),
        },
    };
    let mutations = pending
        .targets
        .iter()
        .flat_map(|fixture_id| {
            definition
                .lanes
                .iter()
                .map(|lane| DynamicProgrammerValueMutation::Set {
                    fixture_id: *fixture_id,
                    attribute: lane.output_owner(),
                    value: DynamicSemanticValue::DynamicOn {
                        instance_link: link,
                        dynamic: reference.clone(),
                        lane_id: lane.id,
                        overrides: DynamicInstanceOverrides {
                            size: 1.0,
                            speed_multiplier: Rational::ONE,
                            phase_offset_degrees: 0.0,
                        },
                        timing: Default::default(),
                    },
                })
        })
        .collect::<Vec<_>>();
    let stage_positions = lane
        .engine
        .snapshot()
        .dynamic_stage_positions
        .as_ref()
        .clone();
    let expand = started.elapsed();
    let runtime_started = Instant::now();
    lane.bench
        .start_dynamic(DynamicStartRequest {
            definition_id: definition.id,
            controller: DynamicController {
                id: controller_id,
                source: DynamicControllerSource::Programmer {
                    programmer_id: state.id.0,
                    instance_link: Some(link),
                },
                priority: state.priority,
                activated_at_millis: now_millis,
                size: 1.0,
                speed_multiplier: 1.0,
                phase_offset_degrees: 0.0,
                paused: false,
            },
            target_scope: DynamicTargetScope {
                ordered_targets: pending.targets.clone(),
            },
            stage_positions,
            inherited_spatial_mapping: None,
            now_millis,
            activation_delay_millis: 0,
            activation_duration_millis: 0,
            activation_policy_override: None,
            reuse_matching_targetless: true,
        })
        .map_err(|error| format!("start the probe Dynamic: {error}"))?;
    let runtime_start = runtime_started.elapsed();
    let apply_started = Instant::now();
    if !lane
        .programmers
        .apply_dynamic_values(pending.session, &mutations, None)
    {
        return Err("the probe Dynamic start produced no Programmer change".into());
    }
    let programmer_apply = apply_started.elapsed();
    Ok(GesturePhases {
        expand_microseconds: micros(expand),
        runtime_start_microseconds: micros(runtime_start),
        programmer_apply_microseconds: micros(programmer_apply),
        total_microseconds: micros(started.elapsed()),
    })
}

fn dmx(frame: &LiveOutputFrame) -> Vec<(u16, Vec<u8>)> {
    let mut universes = frame
        .rendered
        .universes
        .iter()
        .map(|(universe, slots)| (*universe, slots.to_vec()))
        .collect::<Vec<_>>();
    universes.sort_unstable_by_key(|(universe, _)| *universe);
    universes
}

fn serialized(arguments: &Arguments, config: ProfileConfig) -> Result<SerializedProbe, String> {
    let (scenario, pending) = prepared(arguments, config)?;
    let lane = Lane::new(&scenario);
    let scenario = SharedLane::from(&lane);
    let mut before = Vec::new();
    let mut previous = None;
    for tick in 0..SETTLE_FRAMES {
        set_tick(&scenario, tick, config.rate_hz);
        let (frame, total) = render(&scenario)?;
        if frame.dynamic_samples != 0 {
            return Err("a Dynamic was running before the probe started one".into());
        }
        before.push(total);
        previous = Some(dmx(&frame));
    }
    set_tick(&scenario, SETTLE_FRAMES, config.rate_hz);
    let gesture = gesture(&scenario, &pending, start_link(&pending, 0))?;
    set_tick(&scenario, SETTLE_FRAMES + 1, config.rate_hz);
    let (frame, total) = render(&scenario)?;
    let first_frame = frame_phases(&frame, total);
    let first_frame_changed_dmx = previous.as_ref() != Some(&dmx(&frame));
    let mut after = Vec::new();
    let mut frames_until_dmx_change = first_frame_changed_dmx.then_some(1);
    for tick in SETTLE_FRAMES + 2..2 * SETTLE_FRAMES + 2 {
        set_tick(&scenario, tick, config.rate_hz);
        let (frame, total) = render(&scenario)?;
        if frames_until_dmx_change.is_none() && previous.as_ref() != Some(&dmx(&frame)) {
            frames_until_dmx_change = Some(tick - SETTLE_FRAMES);
        }
        after.push(total);
    }
    let repeated_start = restart(
        &scenario,
        &pending,
        config.rate_hz,
        2 * SETTLE_FRAMES + 2,
        1,
    )?;
    let cycles = start_cycles(
        &scenario,
        &pending,
        config.rate_hz,
        arguments.semantic.start_latency_cycles,
    )?;
    Ok(SerializedProbe {
        cycles,
        repeated_start,
        steady_before: distribution(&before),
        start_to_first_output_microseconds: gesture.total_microseconds
            + first_frame.total_microseconds,
        gesture,
        first_frame,
        first_frame_changed_dmx,
        frames_until_dmx_change,
        steady_after: distribution(&after),
    })
}

#[derive(Debug, Serialize)]
pub struct RepeatedStart {
    pub gesture: GesturePhases,
    pub first_frame: FramePhases,
}

fn start_link(pending: &PendingStart, attempt: u8) -> Uuid {
    Uuid::new_v5(
        &pending.definition.id,
        &[b"tosklight:tl641:start-now:", &[attempt][..]].concat(),
    )
}

/// TL-646: release and restart `count` times without the desk's persistence checkpoint, then
/// `count` times with it after each release, recording the first frame and catalogue size.
fn start_cycles(
    lane: &SharedLane<'_>,
    pending: &PendingStart,
    rate_hz: u16,
    count: u16,
) -> Result<Vec<StartCycle>, String> {
    let mut cycles = Vec::new();
    let mut attempt: u8 = 1;
    for checkpointed in [false, true] {
        for cycle in 1..=count {
            attempt += 1;
            // Every restart starts far past the previous one, so ticks stay monotonic.
            let tick = u64::from(attempt) * 100_000;
            let restarted = restart_with(lane, pending, rate_hz, tick, attempt, checkpointed)?;
            cycles.push(StartCycle {
                checkpointed,
                cycle,
                gesture_microseconds: restarted.gesture.total_microseconds,
                first_frame_microseconds: restarted.first_frame.total_microseconds,
                source_records: lane.bench.dynamic_source_records(),
            });
        }
    }
    Ok(cycles)
}

/// Release the first start, let output settle without it, then start it again.
fn restart(
    lane: &SharedLane<'_>,
    pending: &PendingStart,
    rate_hz: u16,
    tick: u64,
    attempt: u8,
) -> Result<RepeatedStart, String> {
    restart_with(lane, pending, rate_hz, tick, attempt, false)
}

/// [`restart`], optionally running the desk's persistence checkpoint once output has settled
/// without the released Dynamic, as a desk gesture persists the Output runtime.
fn restart_with(
    lane: &SharedLane<'_>,
    pending: &PendingStart,
    rate_hz: u16,
    mut tick: u64,
    attempt: u8,
    checkpointed: bool,
) -> Result<RepeatedStart, String> {
    let release = pending
        .targets
        .iter()
        .flat_map(|fixture_id| {
            pending.definition.lanes.iter().map(|definition_lane| {
                DynamicProgrammerValueMutation::Release {
                    fixture_id: *fixture_id,
                    attribute: definition_lane.output_owner(),
                    instance_link: Some(start_link(pending, attempt - 1)),
                }
            })
        })
        .collect::<Vec<_>>();
    if !lane
        .programmers
        .apply_dynamic_values(pending.session, &release, None)
    {
        return Err("releasing the probe Dynamic produced no Programmer change".into());
    }
    let mut idle = 0;
    while idle < SETTLE_FRAMES {
        set_tick(lane, tick, rate_hz);
        tick += 1;
        if render(lane)?.0.dynamic_samples == 0 {
            idle += 1;
        } else if tick > 10 * SETTLE_FRAMES + 100 {
            return Err("the released probe Dynamic stayed in output".into());
        }
    }
    if checkpointed {
        lane.bench.checkpoint_dynamic_sources()?;
    }
    set_tick(lane, tick, rate_hz);
    let gesture = gesture(lane, pending, start_link(pending, attempt))?;
    set_tick(lane, tick + 1, rate_hz);
    let (frame, total) = render(lane)?;
    Ok(RepeatedStart {
        gesture,
        first_frame: frame_phases(&frame, total),
    })
}

struct RecordedFrame {
    tick: u64,
    started: Instant,
    ended: Instant,
    total: Duration,
    phases: FramePhases,
    capture: Duration,
}

/// Paced output lane: renders until the Dynamic has been in output for `SETTLE_FRAMES` frames.
///
/// With a `gate`, the lane is deterministic instead of paced: the settle frame the gesture sees
/// in progress stays reported as in progress, and no later frame starts until the gesture has
/// finished. That models a gesture that completes within one frame period on any machine, so
/// the contract (carried by the in-progress frame or the next one) can be asserted without
/// depending on the host's speed.
fn output_lane(
    scenario: &SharedLane<'_>,
    rate_hz: u16,
    in_progress: &AtomicU64,
    stop: &AtomicBool,
    gate: Option<&AtomicBool>,
) -> Result<Vec<RecordedFrame>, String> {
    let period = Duration::from_nanos(1_000_000_000 / u64::from(rate_hz));
    let origin = Instant::now();
    let mut frames = Vec::new();
    let mut carried = 0;
    for tick in 0.. {
        if stop.load(Ordering::Relaxed) || origin.elapsed() > CONCURRENT_TIMEOUT {
            break;
        }
        if let Some(gesture_done) = gate {
            while tick > SETTLE_FRAMES
                && !gesture_done.load(Ordering::Acquire)
                && !stop.load(Ordering::Relaxed)
            {
                std::thread::yield_now();
            }
        } else {
            let deadline = origin + period * u32::try_from(tick).unwrap_or(u32::MAX);
            if let Some(wait) = deadline.checked_duration_since(Instant::now()) {
                std::thread::sleep(wait);
            }
        }
        set_tick(scenario, tick, rate_hz);
        let started = Instant::now();
        in_progress.store(tick + 1, Ordering::Release);
        let (frame, total) = render(scenario)?;
        let ended = Instant::now();
        if gate.is_none() {
            in_progress.store(0, Ordering::Release);
        }
        carried += u64::from(frame.dynamic_samples != 0);
        frames.push(RecordedFrame {
            tick,
            started,
            ended,
            total,
            capture: frame.capture,
            phases: frame_phases(&frame, total),
        });
        if carried > SETTLE_FRAMES {
            break;
        }
    }
    Ok(frames)
}

type Shape = (usize, usize, usize);

/// `gated` replaces wall-clock pacing with the deterministic gate of [`output_lane`].
fn concurrent(
    arguments: &Arguments,
    config: ProfileConfig,
    gated: bool,
) -> Result<(ConcurrentProbe, Shape), String> {
    let (scenario, pending) = prepared(arguments, config)?;
    let shape = (
        scenario.fixture_count,
        pending.targets.len(),
        pending.definition.lanes.len(),
    );
    let lane = Lane::new(&scenario);
    let shared = SharedLane::from(&lane);
    let in_progress = AtomicU64::new(0);
    let stop = AtomicBool::new(false);
    let gesture_done = AtomicBool::new(false);
    let gate = gated.then_some(&gesture_done);
    let (frames, gesture) = std::thread::scope(|scope| {
        let output =
            scope.spawn(|| output_lane(&shared, config.rate_hz, &in_progress, &stop, gate));
        // Wait for the settle frames, then press while the next frame is in progress.
        let pressed = loop {
            let tick = in_progress.load(Ordering::Acquire);
            if tick > SETTLE_FRAMES || output.is_finished() {
                break tick;
            }
            std::hint::spin_loop();
        };
        let started = Instant::now();
        let gesture = if output.is_finished() {
            Err("the output lane stopped before the gesture".to_owned())
        } else {
            gesture(&shared, &pending, start_link(&pending, 0))
        };
        gesture_done.store(true, Ordering::Release);
        if gesture.is_err() {
            stop.store(true, Ordering::Relaxed);
        }
        let frames = output
            .join()
            .map_err(|_| "the output lane panicked".to_owned())
            .and_then(|frames| frames);
        (frames, gesture.map(|phases| (phases, started, pressed - 1)))
    });
    let frames = frames?;
    let (gesture, pressed_at, in_progress_tick) = gesture?;
    let before = frames
        .iter()
        .filter(|frame| frame.tick < in_progress_tick)
        .map(|frame| frame.total)
        .collect::<Vec<_>>();
    let in_progress = frames
        .iter()
        .find(|frame| frame.tick == in_progress_tick)
        .ok_or("the in-progress frame was not recorded")?;
    debug_assert!(in_progress.started <= pressed_at);
    let first = frames
        .iter()
        .find(|frame| frame.phases.dynamic_samples != 0);
    let frames_until_output = first.map(|frame| frame.tick - in_progress_tick);
    // Starts the scenario build itself made are claimed by the settle frames before the gesture.
    let frames_until_claimed = frames
        .iter()
        .find(|frame| frame.tick >= in_progress_tick && frame.phases.change_lead_claimed)
        .map(|frame| frame.tick - in_progress_tick);
    let start_to_first_output =
        first.map(|frame| micros(frame.ended.saturating_duration_since(pressed_at)));
    let budget = 2.0 * 1_000_000.0 / f64::from(config.rate_hz);
    Ok((
        ConcurrentProbe {
            steady_before: distribution(&before),
            in_progress_frame_microseconds: micros(in_progress.total),
            in_progress_frame_capture_microseconds: micros(in_progress.capture),
            next_frame_after_in_progress: frames_until_output.is_some_and(|frames| frames <= 1),
            within_budget: frames_until_output.is_some_and(|frames| frames <= 1)
                && start_to_first_output.is_some_and(|latency| latency <= budget),
            frames_until_output,
            frames_until_claimed,
            first_frame: first.map(|frame| frame.phases.clone()),
            start_to_first_output_microseconds: start_to_first_output,
            gesture,
        },
        shape,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::light_benchmark::ParseOutcome;

    #[test]
    fn a_start_now_dynamic_on_every_fixture_reaches_the_first_frame_after_the_gesture() {
        let package_dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fixture-library");
        let ParseOutcome::Run(arguments) = Arguments::parse(
            [
                "--headless-stress-fixtures",
                "1000",
                "--semantic",
                "--start-latency",
                "--fixture-package-dir",
                package_dir.to_str().unwrap(),
            ]
            .map(str::to_owned),
        )
        .unwrap() else {
            panic!("expected a run");
        };
        let config = profile_configs(&arguments)[0];
        let probe = serialized(&arguments, config).unwrap();
        assert!(probe.first_frame.dynamic_samples > 0);
        assert!(probe.first_frame_changed_dmx);
        assert!(probe.gesture.programmer_apply_microseconds > 0.0);
        // TL-659: the desk's change lead ledger names the same frame, one frame period after the
        // gesture's application time (Dynamic start instants are whole milliseconds).
        assert!(probe.first_frame.change_lead_claimed);
        let period = 1_000_000.0 / f64::from(config.rate_hz);
        let lead = probe.first_frame.logical_change_lead_microseconds.unwrap();
        assert!(
            (period..period + 1_000.0).contains(&lead),
            "{lead} against {period}"
        );
        // Gated, not paced: the result cannot depend on how fast the host renders or applies
        // the gesture (a debug build on a loaded CI runner included).
        let (concurrent, (fixtures, targets, lanes)) =
            concurrent(&arguments, config, true).unwrap();
        // 540 animated fixtures with their logical heads (1,480 targets) and 460 Dimmers.
        assert_eq!((fixtures, targets, lanes), (1_000, 1_940, 6));
        // The contract: the frame in progress at the gesture, or the next one, carries it.
        assert!(
            concurrent
                .frames_until_output
                .is_some_and(|frames| frames <= 1),
            "carried {:?} frames after the in-progress one",
            concurrent.frames_until_output
        );
        assert!(concurrent.next_frame_after_in_progress);
        assert_eq!(
            concurrent.frames_until_claimed, concurrent.frames_until_output,
            "the ledger claims the start on the frame that carries it"
        );
        assert!(
            concurrent
                .first_frame
                .is_some_and(|frame| frame.dynamic_samples == probe.first_frame.dynamic_samples)
        );
    }

    #[test]
    fn the_sustained_show_probe_changes_every_fixture_on_the_first_frame() {
        let package_dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fixture-library");
        let ParseOutcome::Run(arguments) = Arguments::parse(
            [
                "--profile",
                "hard-floor",
                "--sustained-show",
                "--universes",
                "8",
                "--semantic",
                "--start-latency",
                "--fixture-package-dir",
                package_dir.to_str().unwrap(),
            ]
            .map(str::to_owned),
        )
        .unwrap() else {
            panic!("expected a run");
        };
        let config = profile_configs(&arguments)[0];
        let probe = serialized(&arguments, config).unwrap();
        // 1,037 fixtures on 8 universes: one sample per fixture, and the DMX moves at once.
        assert_eq!(probe.first_frame.dynamic_samples, 1_037);
        assert!(probe.first_frame_changed_dmx);
        assert_eq!(probe.frames_until_dmx_change, Some(1));
        assert!(probe.first_frame.change_lead_claimed);
    }
}
