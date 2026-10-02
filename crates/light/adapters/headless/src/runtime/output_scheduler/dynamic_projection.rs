use super::*;
use rustc_hash::FxHashMap;
use std::borrow::Cow;
use std::sync::OnceLock;

mod endpoint_controls;
mod family_inputs;
mod fixed_masks;
mod output_transaction;
#[path = "../pending_publication.rs"]
pub(in crate::runtime) mod pending_publication;
pub(in crate::runtime) mod physical_adapter;
mod programming_projection;
pub(in crate::runtime) mod retained_preload_hybrid;
mod scalar_projection;
mod source_bindings;
pub(in crate::runtime) use output_transaction::{LiveFamilyAdapters, dynamic_output_frame};

type DynamicProgrammerValues = Vec<(Uuid, i16, light_dynamics::DynamicAddressValue)>;
type DynamicDefinitions = Vec<light_dynamics::DynamicDefinition>;
type DynamicStagePositions = HashMap<FixtureId, light_dynamics::SpatialPosition>;

#[derive(Default)]
pub(in crate::runtime) struct ProgrammerReconciliationCache {
    // Owned by the output lane. Kept separately from the signature so merely looking up
    // unchanged sources never acknowledges a frame which later fails to render.
    transaction: Mutex<light_dynamics::DynamicOutputFrameScratch>,
    signature: Mutex<
        Option<(
            Arc<DynamicProgrammerValues>,
            Arc<DynamicDefinitions>,
            Arc<DynamicStagePositions>,
            Arc<Vec<light_programmer::GroupDefinition>>,
        )>,
    >,
    origins: Mutex<Option<Arc<crate::runtime::dynamic_source_origins::DynamicSourceOrigins>>>,
}

impl ProgrammerReconciliationCache {
    fn changed(
        &self,
        values: &Arc<DynamicProgrammerValues>,
        snapshot: &Arc<light_engine::EngineSnapshot>,
    ) -> bool {
        let signature = self.signature.lock();
        signature.as_ref().is_none_or(
            |(previous_values, previous_definitions, previous_positions, previous_groups)| {
                !Arc::ptr_eq(previous_values, values)
                    || !Arc::ptr_eq(previous_definitions, &snapshot.dynamics)
                    || !Arc::ptr_eq(previous_positions, &snapshot.dynamic_stage_positions)
                    || !Arc::ptr_eq(previous_groups, &snapshot.groups)
            },
        )
    }

    fn acknowledge(
        &self,
        values: &Arc<DynamicProgrammerValues>,
        snapshot: &Arc<light_engine::EngineSnapshot>,
    ) {
        *self.signature.lock() = Some((
            Arc::clone(values),
            Arc::clone(&snapshot.dynamics),
            Arc::clone(&snapshot.dynamic_stage_positions),
            Arc::clone(&snapshot.groups),
        ));
    }

    fn sources_changed(
        &self,
        origins: &Arc<crate::runtime::dynamic_source_origins::DynamicSourceOrigins>,
    ) -> bool {
        self.origins
            .lock()
            .as_ref()
            .is_none_or(|previous| !Arc::ptr_eq(previous, origins))
    }

    fn acknowledge_sources(
        &self,
        origins: &Arc<crate::runtime::dynamic_source_origins::DynamicSourceOrigins>,
    ) {
        *self.origins.lock() = Some(Arc::clone(origins));
    }
}

struct TickSources<'a> {
    engine: &'a Engine,
    prepared: Option<&'a light_engine::PreparedOutputFrame>,
    baseline_samples: &'a [ContributionBatch],
    values: OnceLock<TickValues>,
}

enum TickValues {
    Legacy(light_engine::ResolvedValues),
    Prepared(light_engine::FrameValues),
}

impl<'a> TickSources<'a> {
    fn new(engine: &'a Engine) -> Self {
        Self {
            engine,
            prepared: None,
            baseline_samples: &[],
            values: OnceLock::new(),
        }
    }

    fn prepared(
        engine: &'a Engine,
        frame: &'a light_engine::PreparedOutputFrame,
        baseline_samples: &'a [ContributionBatch],
    ) -> Self {
        Self {
            engine,
            prepared: Some(frame),
            baseline_samples,
            values: OnceLock::new(),
        }
    }

    fn captured_values(&self) -> &TickValues {
        self.values.get_or_init(|| match self.prepared {
            Some(frame) => TickValues::Prepared(
                self.engine
                    .observe_prepared_values(frame, self.baseline_samples),
            ),
            None => TickValues::Legacy(self.engine.resolved_values()),
        })
    }

    fn value(&self, target: FixtureId, attribute: &AttributeKey) -> Option<&AttributeValue> {
        match self.captured_values() {
            TickValues::Legacy(values) => values.get(&(target, attribute.clone())),
            TickValues::Prepared(frame) => frame.value(target, attribute),
        }
    }
}

impl light_dynamics::ScalarSourceResolver for TickSources<'_> {
    fn current(&self, target: FixtureId, attribute: &AttributeKey) -> Option<f32> {
        self.value(target, attribute)
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

/// Complete values and scalar Current must come from the same lazily evaluated source lane.
trait DynamicTickSource: light_dynamics::ScalarSourceResolver {
    fn value(&self, target: FixtureId, attribute: &AttributeKey) -> Option<&AttributeValue>;

    /// Exact evidence from the same captured static frame as `value`. Legacy map-only sources
    /// are explicitly unknown; a bare winning origin does not establish family footprints.
    fn family_evidence(
        &self,
        _: FixtureId,
        _: &AttributeKey,
    ) -> Option<&Arc<light_engine::ContributionFamilyEvidence>> {
        None
    }
}

impl DynamicTickSource for TickSources<'_> {
    fn value(&self, target: FixtureId, attribute: &AttributeKey) -> Option<&AttributeValue> {
        TickSources::value(self, target, attribute)
    }

    fn family_evidence(
        &self,
        target: FixtureId,
        attribute: &AttributeKey,
    ) -> Option<&Arc<light_engine::ContributionFamilyEvidence>> {
        match self.captured_values() {
            TickValues::Prepared(frame) => frame.contribution_family_evidence(target, attribute),
            TickValues::Legacy(_) => None,
        }
    }
}

struct PreloadTickSources<'a, 'frame> {
    engine: &'a Engine,
    input: &'a light_engine::PreparedPreloadFrame<'frame>,
    state: &'a light_engine::PreloadFrameState,
    before_release: bool,
    baseline_samples: &'a [ContributionBatch],
    values: OnceLock<light_engine::FrameValues>,
}

impl PreloadTickSources<'_, '_> {
    fn captured_values(&self) -> &light_engine::FrameValues {
        self.values.get_or_init(|| {
            self.engine.observe_prepared_preload_values(
                self.input,
                self.baseline_samples,
                self.state,
                if self.before_release {
                    light_engine::PreloadBranch::BeforeRelease
                } else {
                    light_engine::PreloadBranch::AfterRelease
                },
            )
        })
    }
}

impl DynamicTickSource for PreloadTickSources<'_, '_> {
    fn value(&self, target: FixtureId, attribute: &AttributeKey) -> Option<&AttributeValue> {
        self.captured_values().value(target, attribute)
    }

    fn family_evidence(
        &self,
        target: FixtureId,
        attribute: &AttributeKey,
    ) -> Option<&Arc<light_engine::ContributionFamilyEvidence>> {
        self.captured_values()
            .contribution_family_evidence(target, attribute)
    }
}

impl light_dynamics::ScalarSourceResolver for PreloadTickSources<'_, '_> {
    fn current(&self, target: FixtureId, attribute: &AttributeKey) -> Option<f32> {
        self.value(target, attribute)
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

/// Immutable scheduler inputs. Sampling below this boundary never rereads Live controls.
struct CapturedDynamicInputs<'a> {
    now: chrono::DateTime<chrono::Utc>,
    speed_transports: &'a [light_dynamics::DynamicSpeedTransport; 5],
    rate: u16,
    snapshot: &'a Arc<light_engine::EngineSnapshot>,
    programmer_values: &'a Arc<DynamicProgrammerValues>,
    programmer_rows: Option<&'a [light_engine::CapturedDynamicProgrammerRow]>,
    cue_values: &'a [light_playback::ActiveCueDynamicValue],
    dynamic_playbacks: &'a [light_playback::ActiveDynamicPlayback],
    playback_paused: bool,
    addresser: &'a dyn light_core::FrameAddressResolver,
    extra_programmer_values: &'a [(Uuid, i16, light_dynamics::DynamicAddressValue)],
    programmer_reconciliation_cache: Option<&'a ProgrammerReconciliationCache>,
    force_source_reconciliation: bool,
}

pub(in crate::runtime) struct CapturedDynamicSample {
    // Exact immutable original-source models used by these running/held expressions.
    // Preparation must not resolve them against a newer patch or an unrelated catalogue.
    native_models: Arc<dyn light_dynamics::DynamicNativeModelResolver>,
    idle: bool,
    before_runtime: light_dynamics::DynamicRuntimeSnapshot,
    after_runtime: light_dynamics::DynamicRuntimeSnapshot,
    samples: Vec<light_dynamics::DynamicRuntimeSample>,
    playback_controls: HashMap<Uuid, DynamicPlaybackControl>,
    cue_controls: HashMap<Uuid, CueDynamicOutputControl>,
}

#[derive(Clone, Copy)]
struct CapturedSourceAssignments<'a> {
    programmer: Option<&'a [super::dynamic_reconciliation::ReconciledSourceAssignment]>,
    cues: &'a [super::dynamic_reconciliation::ReconciledSourceAssignment],
    playbacks: &'a [super::dynamic_reconciliation::ReconciledSourceAssignment],
}

/// Captured output masks/masters are needed when the scalar stage resolves Point geometry,
/// before typed sampling has completed. They never come from a second Playback observation.
#[derive(Clone, Copy)]
struct CapturedDynamicOutputControls<'a> {
    playbacks: &'a HashMap<Uuid, DynamicPlaybackControl>,
    cues: &'a HashMap<Uuid, CueDynamicOutputControl>,
}

/// Current-frame output controls are separate from authored expression provenance. In
/// particular, a newer Cue must not relabel the older leaves retained by a paused Dynamic.
#[derive(Clone, Copy)]
struct CueDynamicOutputControl {
    enabled: bool,
    sequence_master: f32,
}

fn cue_dynamic_output_controls(
    rows: &[light_playback::ActiveCueDynamicValue],
) -> HashMap<Uuid, CueDynamicOutputControl> {
    rows.iter()
        .filter_map(|row| {
            let link = match &row.value {
                light_dynamics::DynamicSemanticValue::DynamicOn { instance_link, .. }
                | light_dynamics::DynamicSemanticValue::DynamicOff { instance_link, .. } => {
                    *instance_link
                }
                _ => return None,
            };
            Some((
                row.source_key.controller_id(link),
                CueDynamicOutputControl {
                    enabled: row.output_enabled,
                    sequence_master: row.sequence_master,
                },
            ))
        })
        .collect()
}

pub(in crate::runtime) fn capture_dynamic_speed_transports(
    speed_groups: &Mutex<[light_control::speed::SpeedGroupController; 5]>,
    now_millis: u64,
) -> [light_dynamics::DynamicSpeedTransport; 5] {
    let speed_groups = speed_groups.lock();
    std::array::from_fn(|index| {
        let snapshot = speed_groups[index].snapshot(now_millis);
        light_dynamics::DynamicSpeedTransport {
            effective_bpm: snapshot.effective_bpm,
            phase_origin_millis: snapshot.phase_origin_millis,
            phase_reference_millis: speed_groups[index].phase_reference_millis(now_millis),
            beat_phase: snapshot.beat_phase,
            phase_advancing: snapshot.phase_advancing,
        }
    })
}

fn sample_captured_dynamic_inputs(
    dynamics: &mut light_dynamics::DynamicRuntime,
    inputs: &CapturedDynamicInputs<'_>,
    sources: &impl DynamicTickSource,
) -> CapturedDynamicSample {
    sample_captured_dynamic_inputs_with(dynamics, inputs, |dynamics, now, interval, _, _| {
        Ok(dynamics.sample_all_addressed(
            now,
            interval,
            inputs.speed_transports,
            sources,
            Some(inputs.addresser),
        ))
    })
    .expect("legacy sampling has no fallible adapter")
}

/// Prepare semantic Preset sources only at a fallible typed sampling boundary. Legacy scalar
/// sampling does not consume these tables and must retain its infallible adapter contract.
fn prepare_captured_preset_dependencies(
    dynamics: &mut light_dynamics::DynamicRuntime,
    inputs: &CapturedDynamicInputs<'_>,
) -> Result<(), light_dynamics::DynamicRuntimeError> {
    // Reconciliation may have just created or rebound a controller. An enclosing frame
    // transaction restores both tables and freshness markers if preparation or rendering fails.
    super::cold_preset_materialization::materialize_pending_preset_dependencies(
        inputs.snapshot,
        dynamics,
    )
    .map(|_| ())
    .map_err(|error| light_dynamics::DynamicRuntimeError::InvalidSample(error.to_string()))
}

/// Both output contracts share reconciliation and controller timing. A typed adapter must
/// return its sampling failure to the frame owner; it must not turn it into an empty frame.
fn sample_captured_dynamic_inputs_with(
    dynamics: &mut light_dynamics::DynamicRuntime,
    inputs: &CapturedDynamicInputs<'_>,
    sample: impl FnOnce(
        &mut light_dynamics::DynamicRuntime,
        u64,
        u64,
        CapturedSourceAssignments<'_>,
        CapturedDynamicOutputControls<'_>,
    ) -> Result<
        Vec<light_dynamics::DynamicRuntimeSample>,
        light_dynamics::DynamicRuntimeError,
    >,
) -> Result<CapturedDynamicSample, light_dynamics::DynamicRuntimeError> {
    sample_captured_dynamic_inputs_with_context(
        dynamics,
        inputs,
        |runtime, now, interval, assignments, controls| {
            sample(runtime, now, interval, assignments, controls).map(|samples| (samples, ()))
        },
    )
    .map(|(sampled, ())| sampled)
}

/// A staged producer returns its prepared engine token together with completed samples.
/// Keeping that context in the result avoids publishing or committing geometry from inside
/// the sampling callback, before the sampler has validated its completion proof.
fn sample_captured_dynamic_inputs_with_context<T>(
    dynamics: &mut light_dynamics::DynamicRuntime,
    inputs: &CapturedDynamicInputs<'_>,
    sample: impl FnOnce(
        &mut light_dynamics::DynamicRuntime,
        u64,
        u64,
        CapturedSourceAssignments<'_>,
        CapturedDynamicOutputControls<'_>,
    ) -> Result<
        (Vec<light_dynamics::DynamicRuntimeSample>, T),
        light_dynamics::DynamicRuntimeError,
    >,
) -> Result<(CapturedDynamicSample, T), light_dynamics::DynamicRuntimeError> {
    let now_millis = u64::try_from(inputs.now.timestamp_millis()).unwrap_or_default();
    let before_runtime = dynamics.output_projection_snapshot();
    let active_dynamic_playbacks = inputs.dynamic_playbacks;
    let interval = (1_000 / u64::from(inputs.rate.max(1))).max(1);
    if dynamic_tick_is_idle(
        &before_runtime,
        inputs.programmer_values,
        inputs.cue_values,
        &active_dynamic_playbacks,
        inputs.extra_programmer_values,
        inputs.playback_paused,
    ) {
        // Let the source owner retire obsolete active bindings even when no Dynamic remains.
        let playback_controls = HashMap::new();
        let cue_controls = HashMap::new();
        let (samples, context) = sample(
            dynamics,
            now_millis,
            interval,
            CapturedSourceAssignments {
                programmer: Some(&[]),
                cues: &[],
                playbacks: &[],
            },
            CapturedDynamicOutputControls {
                playbacks: &playback_controls,
                cues: &cue_controls,
            },
        )?;
        return Ok((
            CapturedDynamicSample {
                native_models: dynamics.captured_native_color_models(),
                idle: true,
                after_runtime: before_runtime.clone(),
                before_runtime,
                samples,
                playback_controls,
                cue_controls,
            },
            context,
        ));
    }

    // Reconciliation runs before sampling, so a bool transport request has no input-dependent
    // failure. A misplaced call after a provisional sample is still rejected transactionally.
    dynamics.apply_recorded_control(light_dynamics::TimedDynamicControl {
        at_millis: now_millis,
        control: light_dynamics::DynamicControl::GlobalPause(inputs.playback_paused),
    })?;
    let reconcile_programmer = inputs.force_source_reconciliation
        || !inputs.extra_programmer_values.is_empty()
        || inputs
            .programmer_reconciliation_cache
            .is_none_or(|cache| cache.changed(inputs.programmer_values, inputs.snapshot));
    let mut assignments = Vec::new();
    if reconcile_programmer {
        super::dynamic_reconciliation::reconcile_programmer_dynamics_with_sources(
            dynamics,
            now_millis,
            inputs.snapshot,
            inputs.programmer_values,
            inputs.extra_programmer_values,
            |assignment| assignments.push(assignment),
        );
    }
    let mut cue_assignments = Vec::new();
    super::dynamic_reconciliation::reconcile_cue_dynamics_with_sources(
        dynamics,
        now_millis,
        inputs.snapshot,
        inputs.cue_values,
        |assignment| cue_assignments.push(assignment),
    );
    let mut playback_assignments = Vec::new();
    let playback_controls = super::dynamic_reconciliation::reconcile_dynamic_playbacks_with_sources(
        dynamics,
        now_millis,
        inputs.snapshot,
        &active_dynamic_playbacks,
        |assignment| playback_assignments.push(assignment),
    );
    let cue_controls = cue_dynamic_output_controls(inputs.cue_values);
    let (mut samples, context) = sample(
        dynamics,
        now_millis,
        interval,
        CapturedSourceAssignments {
            programmer: reconcile_programmer.then_some(assignments.as_slice()),
            cues: &cue_assignments,
            playbacks: &playback_assignments,
        },
        CapturedDynamicOutputControls {
            playbacks: &playback_controls,
            cues: &cue_controls,
        },
    )?;
    // Suppression is an output mask. Reconciliation and sampling above still advance the
    // retained runtime so releasing Swap reveals the same continuing clock.
    samples.retain(|sample| {
        cue_controls
            .get(&sample.controller_id)
            .is_none_or(|control| control.enabled)
    });
    Ok((
        CapturedDynamicSample {
            native_models: dynamics.captured_native_color_models(),
            idle: false,
            before_runtime,
            after_runtime: dynamics.output_projection_snapshot(),
            samples,
            playback_controls,
            cue_controls,
        },
        context,
    ))
}

fn project_captured_dynamic_sample(
    inputs: &CapturedDynamicInputs<'_>,
    sampled: &CapturedDynamicSample,
    sources: &impl DynamicTickSource,
) -> Vec<ContributionBatch> {
    project_captured_dynamic_samples(
        inputs,
        &sampled.samples,
        CapturedDynamicOutputControls {
            playbacks: &sampled.playback_controls,
            cues: &sampled.cue_controls,
        },
        sources,
    )
}

fn project_captured_dynamic_samples(
    inputs: &CapturedDynamicInputs<'_>,
    samples: &[light_dynamics::DynamicRuntimeSample],
    controls: CapturedDynamicOutputControls<'_>,
    sources: &impl DynamicTickSource,
) -> Vec<ContributionBatch> {
    let candidates = collect_dynamic_candidates(
        inputs.addresser,
        inputs.programmer_values,
        inputs.cue_values,
        inputs.extra_programmer_values,
        samples,
        controls.playbacks,
        controls.cues,
        sources,
        u64::try_from(inputs.now.timestamp_millis()).unwrap_or_default(),
    );
    if candidates.is_empty() {
        Vec::new()
    } else {
        vec![dynamic_contribution_batch(candidates, sources, inputs.now)]
    }
}

/// Sample an explicitly isolated Dynamic runtime against one complete pending branch. The
/// caller retains/forks the runtime; this never clones or mutates the Live runtime, emits events,
/// applies Playback auto-off, or reads a new clock/speed/configuration state.
#[allow(clippy::too_many_arguments)]
pub(in crate::runtime) fn dynamic_projection_preload(
    engine: &Engine,
    input: &light_engine::PreparedPreloadFrame<'_>,
    state: &light_engine::PreloadFrameState,
    before_release: bool,
    baseline_samples: &[ContributionBatch],
    dynamics: &mut light_dynamics::DynamicRuntime,
    speed_transports: &[light_dynamics::DynamicSpeedTransport; 5],
    rate: u16,
) -> (
    Vec<ContributionBatch>,
    Vec<light_dynamics::DynamicRuntimeSample>,
) {
    let frame = input.frame();
    let snapshot = frame.snapshot();
    let addresser = frame.frame_addresser();
    let sources = PreloadTickSources {
        engine,
        input,
        state,
        before_release,
        baseline_samples,
        values: OnceLock::new(),
    };
    let inputs = CapturedDynamicInputs {
        now: frame.sampled_at(),
        speed_transports,
        rate,
        snapshot: &snapshot,
        programmer_values: if before_release {
            &input.sources().dynamic_values_before
        } else {
            &input.sources().dynamic_values_after
        },
        programmer_rows: Some(if before_release {
            &input.sources().dynamic_rows_before
        } else {
            &input.sources().dynamic_rows_after
        }),
        cue_values: input.cue_dynamic_values(),
        dynamic_playbacks: input.dynamic_playbacks(),
        playback_paused: input.playback_dynamics_paused(),
        addresser: &addresser,
        extra_programmer_values: &[],
        programmer_reconciliation_cache: None,
        force_source_reconciliation: false,
    };
    let sampled = sample_captured_dynamic_inputs(dynamics, &inputs, &sources);
    (
        project_captured_dynamic_sample(&inputs, &sampled, &sources),
        sampled.samples,
    )
}

pub(in crate::runtime) fn dynamic_contributions(
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
        None,
        apply_auto_off,
    )
    .0
}

/// Cold action reconciliation changes controller membership and timing only. Sampling here
/// would overwrite retained source-aware leaves before the authoritative frame has bound the
/// new authored assignment. GO can persist the updated controller with its existing history.
pub(in crate::runtime) fn reconcile_dynamic_controllers(
    engine: &Engine,
    dynamics: &Mutex<light_dynamics::DynamicRuntime>,
    dynamic_snapshot: &crate::runtime::DynamicSnapshotPublication,
) {
    let now = engine.application_time();
    let now_millis = u64::try_from(now.timestamp_millis()).unwrap_or_default();
    let snapshot = engine.snapshot();
    let programmer_values = engine.dynamic_programmer_values();
    let cue_values = engine.active_cue_dynamic_values();
    let playbacks = engine
        .active_dynamic_playbacks()
        .into_iter()
        .filter(|row| row.enabled)
        .collect::<Vec<_>>();
    let playback_paused = engine.playback_dynamics().paused;
    // A shared Programmer action may overlap a registry publication. Leave membership and
    // clocks unchanged; the next coherent output capture reconciles the retained authored rows.
    if !Arc::ptr_eq(&snapshot, &engine.snapshot()) {
        return;
    }
    let mut runtime = dynamics.lock();
    if !dynamic_snapshot.matches(&snapshot) {
        return;
    }
    runtime
        .apply_recorded_control(light_dynamics::TimedDynamicControl {
            at_millis: now_millis,
            control: light_dynamics::DynamicControl::GlobalPause(playback_paused),
        })
        .expect("action-time reconciliation is outside sampling");
    reconcile_programmer_dynamics(&mut runtime, now_millis, &snapshot, &programmer_values, &[]);
    reconcile_cue_dynamics(&mut runtime, now_millis, &snapshot, &cue_values);
    reconcile_dynamic_playbacks(&mut runtime, now_millis, &snapshot, &playbacks);
}

pub(in crate::runtime) fn dynamic_projection(
    engine: &Engine,
    dynamics: &Mutex<light_dynamics::DynamicRuntime>,
    speed_groups: &Mutex<[light_control::speed::SpeedGroupController; 5]>,
    rate: &AtomicU16,
    extra_programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
) -> (
    Vec<ContributionBatch>,
    Vec<light_dynamics::DynamicRuntimeSample>,
) {
    let (batches, _, _, _, samples) = dynamic_contributions_with_auto_off(
        engine,
        dynamics,
        speed_groups,
        rate,
        extra_programmer_values,
        None,
        false,
    );
    (batches, samples)
}

pub(super) fn dynamic_contributions_with_auto_off(
    engine: &Engine,
    dynamics: &Mutex<light_dynamics::DynamicRuntime>,
    speed_groups: &Mutex<[light_control::speed::SpeedGroupController; 5]>,
    rate: &AtomicU16,
    extra_programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    programmer_reconciliation_cache: Option<&ProgrammerReconciliationCache>,
    apply_auto_off: bool,
) -> (
    Vec<ContributionBatch>,
    Vec<PlaybackIdentity>,
    Vec<light_application::EventDraft>,
    light_dynamics::DynamicRuntimeSnapshot,
    Vec<light_dynamics::DynamicRuntimeSample>,
) {
    dynamic_contributions_inner(
        engine,
        None,
        &[],
        dynamics,
        speed_groups,
        rate,
        extra_programmer_values,
        programmer_reconciliation_cache,
        apply_auto_off,
    )
}

/// Test-only projection for assertions which inspect the intermediate contribution batches.
/// Authoritative output and the bench commit through `dynamic_output_frame` instead.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(in crate::runtime) fn dynamic_contributions_prepared(
    engine: &Engine,
    frame: &light_engine::PreparedOutputFrame,
    baseline_samples: &[ContributionBatch],
    dynamics: &Mutex<light_dynamics::DynamicRuntime>,
    speed_groups: &Mutex<[light_control::speed::SpeedGroupController; 5]>,
    rate: &AtomicU16,
    extra_programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    programmer_reconciliation_cache: Option<&ProgrammerReconciliationCache>,
    apply_auto_off: bool,
) -> (
    Vec<ContributionBatch>,
    Vec<PlaybackIdentity>,
    Vec<light_application::EventDraft>,
    light_dynamics::DynamicRuntimeSnapshot,
    Vec<light_dynamics::DynamicRuntimeSample>,
) {
    dynamic_contributions_inner(
        engine,
        Some(frame),
        baseline_samples,
        dynamics,
        speed_groups,
        rate,
        extra_programmer_values,
        programmer_reconciliation_cache,
        apply_auto_off,
    )
}

#[allow(clippy::too_many_arguments)]
fn dynamic_contributions_inner(
    engine: &Engine,
    prepared: Option<&light_engine::PreparedOutputFrame>,
    baseline_samples: &[ContributionBatch],
    dynamics: &Mutex<light_dynamics::DynamicRuntime>,
    speed_groups: &Mutex<[light_control::speed::SpeedGroupController; 5]>,
    rate: &AtomicU16,
    extra_programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    programmer_reconciliation_cache: Option<&ProgrammerReconciliationCache>,
    apply_auto_off: bool,
) -> (
    Vec<ContributionBatch>,
    Vec<PlaybackIdentity>,
    Vec<light_application::EventDraft>,
    light_dynamics::DynamicRuntimeSnapshot,
    Vec<light_dynamics::DynamicRuntimeSample>,
) {
    let now = prepared.map_or_else(|| engine.application_time(), |frame| frame.sampled_at());
    let now_millis = u64::try_from(now.timestamp_millis()).unwrap_or_default();
    // Most ticks have no Dynamic source that needs an underlay value. Keep the whole-show
    // semantic projection lazy so the ordinary output path does not resolve every Playback,
    // Group, and Programmer contribution once here and then again during DMX rendering.
    // Dynamic sampling and candidate blending share this cache when either actually needs it.
    let sources = prepared.map_or_else(
        || TickSources::new(engine),
        |frame| TickSources::prepared(engine, frame, baseline_samples),
    );
    let speed_transports = capture_dynamic_speed_transports(speed_groups, now_millis);
    let programmer_values = prepared.map_or_else(
        || engine.dynamic_programmer_values(),
        |frame| Arc::clone(frame.dynamic_programmer_values()),
    );
    let cue_values = prepared.map_or_else(
        || Cow::Owned(engine.active_cue_dynamic_values()),
        |frame| Cow::Borrowed(frame.cue_dynamic_values()),
    );
    let captured_dynamic_playbacks = prepared.map_or_else(
        || Cow::Owned(engine.active_dynamic_playbacks()),
        |frame| Cow::Borrowed(frame.dynamic_playbacks()),
    );
    let playback_paused = prepared.map_or_else(
        || engine.playback_dynamics().paused,
        |frame| frame.playback_dynamics_paused(),
    );
    let engine_snapshot = prepared.map_or_else(|| engine.snapshot(), |frame| frame.snapshot());
    let addresser =
        prepared.map_or_else(|| engine.frame_addresser(), |frame| frame.frame_addresser());
    let inputs = CapturedDynamicInputs {
        now,
        speed_transports: &speed_transports,
        rate: rate.load(std::sync::atomic::Ordering::Relaxed),
        snapshot: &engine_snapshot,
        programmer_values: &programmer_values,
        programmer_rows: prepared.map(|frame| frame.dynamic_programmer_rows().as_slice()),
        cue_values: &cue_values,
        dynamic_playbacks: &captured_dynamic_playbacks,
        playback_paused,
        addresser: &addresser,
        extra_programmer_values,
        programmer_reconciliation_cache,
        force_source_reconciliation: false,
    };
    let sampled = {
        let mut dynamics = dynamics.lock();
        let sampled = sample_captured_dynamic_inputs(&mut dynamics, &inputs, &sources);
        if extra_programmer_values.is_empty()
            && let Some(cache) = programmer_reconciliation_cache
        {
            cache.acknowledge(&programmer_values, &engine_snapshot);
        }
        sampled
    };
    if sampled.idle {
        return (
            Vec::new(),
            Vec::new(),
            Vec::new(),
            sampled.after_runtime,
            Vec::new(),
        );
    }
    let dynamic_events =
        dynamic_transition_events(&sampled.before_runtime, &sampled.after_runtime, now_millis);
    let auto_offs = if apply_auto_off {
        let fully_controlled = match prepared {
            Some(frame) => fully_controlled_dynamic_playbacks_from(
                frame.playback_contributions(),
                &sampled.samples,
                &sampled.playback_controls,
                &sampled.after_runtime,
                &programmer_values,
                &cue_values,
            ),
            None => fully_controlled_dynamic_playbacks(
                engine,
                &sampled.samples,
                &sampled.playback_controls,
                &sampled.after_runtime,
                &programmer_values,
                &cue_values,
                now,
            ),
        };
        engine.auto_off_fully_controlled_dynamic_playbacks_at(fully_controlled)
    } else {
        Vec::new()
    };

    (
        project_captured_dynamic_sample(&inputs, &sampled, &sources),
        auto_offs,
        dynamic_events,
        sampled.after_runtime,
        sampled.samples,
    )
}

fn dynamic_contribution_batch(
    candidates: FxHashMap<CandidateKey, CandidateStack>,
    sources: &impl DynamicTickSource,
    now: chrono::DateTime<chrono::Utc>,
) -> ContributionBatch {
    ContributionBatch::new(candidates.into_values().map(|entry| {
        let CandidateStack {
            fixture_id,
            attribute,
            address,
            candidates: mut stack,
        } = entry;
        stack.sort_by_key(|candidate| {
            (
                candidate.priority,
                candidate.changed_at_millis,
                candidate
                    .exact_changed_at
                    .map_or(0, |at| at.timestamp_subsec_nanos() % 1_000_000),
                candidate.stable_order,
            )
        });
        let has_dynamic = stack.iter().any(|candidate| candidate.dynamic);
        let resolved =
            resolve_dynamic_stack(&stack, || sources.value(fixture_id, &attribute).cloned());
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
            changed_at: candidate
                .exact_changed_at
                .or_else(|| {
                    chrono::DateTime::from_timestamp_millis(
                        i64::try_from(candidate.changed_at_millis).unwrap_or(i64::MAX),
                    )
                })
                .unwrap_or(now),
            programmer_order: candidate.stable_order.min(u128::from(u64::MAX)) as u64,
            merge_mode,
            fade: false,
            fade_millis: None,
            delay_millis: None,
        })
        .at(address)
    }))
}

fn dynamic_tick_is_idle(
    runtime: &light_dynamics::DynamicRuntimeSnapshot,
    programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    cue_values: &[light_playback::ActiveCueDynamicValue],
    active_playbacks: &[light_playback::ActiveDynamicPlayback],
    extra_programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    playback_paused: bool,
) -> bool {
    dynamic_tick_is_idle_from_presence(
        runtime.instances.is_empty(),
        runtime.global_paused,
        playback_paused,
        !programmer_values.is_empty(),
        !cue_values.is_empty(),
        active_playbacks.iter().any(|playback| playback.enabled),
        !extra_programmer_values.is_empty(),
    )
}

fn dynamic_tick_is_idle_from_presence(
    runtime_empty: bool,
    runtime_paused: bool,
    playback_paused: bool,
    has_programmer_values: bool,
    has_cue_values: bool,
    has_active_playbacks: bool,
    has_extra_programmer_values: bool,
) -> bool {
    runtime_empty
        && runtime_paused == playback_paused
        && !has_programmer_values
        && !has_cue_values
        && !has_active_playbacks
        && !has_extra_programmer_values
}

struct DynamicCandidate {
    value: AttributeValue,
    priority: i16,
    changed_at_millis: u64,
    /// Cue capture retains submillisecond action order. Legacy Dynamic/programmer rows only
    /// carry milliseconds; absence here must not be presented as exact producer provenance.
    exact_changed_at: Option<chrono::DateTime<chrono::Utc>>,
    stable_order: u128,
    activation_mix: f32,
    dynamic: bool,
}

fn resolve_dynamic_stack(
    stack: &[DynamicCandidate],
    resolve_underlay: impl FnOnce() -> Option<AttributeValue>,
) -> AttributeValue {
    let (first, remaining) = stack
        .split_first()
        .expect("one Dynamic/FAT candidate exists for every stack");
    let mut resolved = if first.activation_mix >= 1.0 {
        first.value.clone()
    } else {
        blend_attribute_value(
            resolve_underlay().unwrap_or_else(|| first.value.clone()),
            first.value.clone(),
            first.activation_mix,
        )
    };
    for candidate in remaining {
        resolved =
            blend_attribute_value(resolved, candidate.value.clone(), candidate.activation_mix);
    }
    resolved
}

/// How candidates for one attribute are gathered into one stack.
///
/// A pair the patch numbered is keyed by its number, so a Dynamic's samples cost no hash of the
/// attribute name; a pair it did not is keyed by name. Both sides ask the same resolver, so a
/// Programmer value and a Dynamic sample for one pair always land in the same stack.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum CandidateKey {
    Address(light_core::FrameAddress),
    Name(FixtureId, AttributeKey),
}

/// One attribute's candidates and the pair they are for.
struct CandidateStack {
    fixture_id: FixtureId,
    attribute: AttributeKey,
    address: Option<light_core::FrameAddress>,
    candidates: Vec<DynamicCandidate>,
}

#[allow(clippy::too_many_arguments)]
fn collect_dynamic_candidates(
    addresser: &dyn light_core::FrameAddressResolver,
    programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    cue_values: &[light_playback::ActiveCueDynamicValue],
    extra_programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    samples: &[light_dynamics::DynamicRuntimeSample],
    playback_controls: &HashMap<Uuid, DynamicPlaybackControl>,
    cue_controls: &HashMap<Uuid, CueDynamicOutputControl>,
    sources: &impl DynamicTickSource,
    now_millis: u64,
) -> FxHashMap<CandidateKey, CandidateStack> {
    collect_dynamic_candidates_with_fixed_rows(
        addresser,
        programmer_values,
        cue_values,
        extra_programmer_values,
        samples,
        playback_controls,
        cue_controls,
        sources,
        now_millis,
        |_, _| true,
    )
}

#[allow(clippy::too_many_arguments)]
fn collect_dynamic_candidates_with_fixed_rows(
    addresser: &dyn light_core::FrameAddressResolver,
    programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    cue_values: &[light_playback::ActiveCueDynamicValue],
    extra_programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    samples: &[light_dynamics::DynamicRuntimeSample],
    playback_controls: &HashMap<Uuid, DynamicPlaybackControl>,
    cue_controls: &HashMap<Uuid, CueDynamicOutputControl>,
    sources: &impl DynamicTickSource,
    now_millis: u64,
    include_fixed_row: impl Fn(light_dynamics::FamilyFixedSampleSource, usize) -> bool,
) -> FxHashMap<CandidateKey, CandidateStack> {
    // One entry per addressed attribute, rebuilt every tick: the hash is the cost, so it is the
    // cheap one rather than the DoS-resistant one nothing here needs.
    let mut candidates = FxHashMap::<CandidateKey, CandidateStack>::default();
    let mut consider = |fixture_id: FixtureId,
                        attribute: &AttributeKey,
                        address: Option<light_core::FrameAddress>,
                        candidate: DynamicCandidate| {
        let key = match address {
            Some(address) => CandidateKey::Address(address),
            None => CandidateKey::Name(fixture_id, attribute.clone()),
        };
        candidates
            .entry(key)
            .or_insert_with(|| CandidateStack {
                fixture_id,
                attribute: attribute.clone(),
                address,
                candidates: Vec::new(),
            })
            .candidates
            .push(candidate);
    };
    for sample in samples {
        // A retained, fully covered source still samples to maintain history, but it must not
        // cast a zero-gain LTP vote or manufacture an underlying Current contribution.
        if sample.activation_mix <= 0.0
            || cue_controls
                .get(&sample.controller_id)
                .is_some_and(|control| !control.enabled)
        {
            continue;
        }
        sample
            .expression
            .visit_legacy_contributions(|attribute, value, influence| {
                let value = cue_controls
                    .get(&sample.controller_id)
                    .map_or(value, |control| {
                        if attribute.is_intensity() {
                            value * control.sequence_master
                        } else {
                            value
                        }
                    });
                let dynamic_value =
                    playback_controls
                        .get(&sample.controller_id)
                        .map_or(value, |control| {
                            if attribute.is_intensity() {
                                value * control.master
                            } else if control.crossfade_non_intensity {
                                sources
                                    .current(sample.target, &attribute)
                                    .map_or(value, |base| base + (value - base) * control.master)
                            } else {
                                value
                            }
                        });
                if playback_controls
                    .get(&sample.controller_id)
                    .is_some_and(|control| {
                        control.master == 0.0
                            && !attribute.is_intensity()
                            && !control.crossfade_non_intensity
                    })
                {
                    return;
                }
                consider(
                    sample.target,
                    &attribute,
                    if influence == 1.0 {
                        sample.address
                    } else {
                        None
                    },
                    DynamicCandidate {
                        value: AttributeValue::Normalized(dynamic_value),
                        priority: sample.priority,
                        changed_at_millis: sample.activated_at_millis,
                        exact_changed_at: None,
                        stable_order: sample.controller_id.as_u128(),
                        activation_mix: sample.activation_mix * influence,
                        dynamic: true,
                    },
                );
            });
    }
    let programmer_rows = programmer_values.iter().enumerate().map(|(index, row)| {
        (
            light_dynamics::FamilyFixedSampleSource::Programmer,
            index,
            row,
        )
    });
    let extra_rows = extra_programmer_values
        .iter()
        .enumerate()
        .map(|(index, row)| {
            (
                light_dynamics::FamilyFixedSampleSource::ExtraProgrammer,
                index,
                row,
            )
        });
    for (source, index, (_, priority, stored)) in programmer_rows.chain(extra_rows) {
        if !include_fixed_row(source, index) {
            continue;
        }
        let (value, timing) = match &stored.value {
            light_dynamics::DynamicSemanticValue::Static { value, timing } => {
                (value.clone(), *timing)
            }
            light_dynamics::DynamicSemanticValue::FixAt { value, timing } => {
                (AttributeValue::Normalized(*value), *timing)
            }
            light_dynamics::DynamicSemanticValue::DynamicOn { .. }
            | light_dynamics::DynamicSemanticValue::DynamicOff { .. }
            // Contract support remains disabled until the typed family compositor is wired.
            | light_dynamics::DynamicSemanticValue::ProgrammingFixAt { .. }
            | light_dynamics::DynamicSemanticValue::ProgrammingRelease { .. }
            | light_dynamics::DynamicSemanticValue::Release => continue,
        };
        consider(
            stored.fixture_id,
            &stored.attribute,
            addresser.frame_address(stored.fixture_id, &stored.attribute),
            DynamicCandidate {
                value,
                priority: *priority,
                changed_at_millis: stored.changed_at_millis,
                exact_changed_at: None,
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
    for (index, stored) in cue_values
        .iter()
        .enumerate()
        .filter(|(_, row)| row.output_enabled)
    {
        if !include_fixed_row(light_dynamics::FamilyFixedSampleSource::Cue, index) {
            continue;
        }
        let (value, timing) = match &stored.value {
            light_dynamics::DynamicSemanticValue::Static { value, timing } => {
                (value.clone(), *timing)
            }
            light_dynamics::DynamicSemanticValue::FixAt { value, timing } => {
                (AttributeValue::Normalized(*value), *timing)
            }
            light_dynamics::DynamicSemanticValue::DynamicOn { .. }
            | light_dynamics::DynamicSemanticValue::DynamicOff { .. }
            | light_dynamics::DynamicSemanticValue::ProgrammingFixAt { .. }
            | light_dynamics::DynamicSemanticValue::ProgrammingRelease { .. }
            | light_dynamics::DynamicSemanticValue::Release => continue,
        };
        consider(
            stored.fixture_id,
            &stored.attribute,
            addresser.frame_address(stored.fixture_id, &stored.attribute),
            DynamicCandidate {
                value: if stored.attribute.is_intensity() {
                    value.normalized().map_or(value.clone(), |value| {
                        AttributeValue::Normalized(value * stored.sequence_master)
                    })
                } else {
                    value
                },
                priority: stored.priority,
                changed_at_millis: stored.changed_at_millis,
                exact_changed_at: Some(stored.changed_at),
                stable_order: u128::from(stored.transition_ordinal),
                activation_mix: authored_activation_mix(
                    stored.changed_at_millis,
                    timing,
                    now_millis,
                ),
                dynamic: false,
            },
        );
    }
    candidates
}

pub(in crate::runtime) fn dynamic_transition_events(
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
    append_current_instance_events(&mut events, after, &before_instances, now_millis);
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

fn append_current_instance_events(
    events: &mut Vec<light_application::EventDraft>,
    after: &light_dynamics::DynamicRuntimeSnapshot,
    before_instances: &HashMap<Uuid, &light_dynamics::DynamicInstanceSnapshot>,
    now_millis: u64,
) {
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
            if previous_controllers.contains(&controller.id) {
                let old_gate = previous.and_then(|previous| {
                    previous
                        .controller_transitions
                        .iter()
                        .find(|transition| transition.controller_id == controller.id)
                        .and_then(|transition| transition.output_gate)
                });
                let new_gate = instance
                    .controller_transitions
                    .iter()
                    .find(|transition| transition.controller_id == controller.id)
                    .and_then(|transition| transition.output_gate);
                if old_gate != new_gate {
                    events.push(light_application::EventDraft::dynamic_runtime_changed(
                        None,
                        light_application::DynamicRuntimeChange {
                            kind: light_application::DynamicRuntimeEventKind::ControllerUpdated,
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
    if underlying.programming_owner().is_some() || contribution.programming_owner().is_some() {
        return match light_core::programming::interpolate_programming_value(
            &underlying,
            &contribution,
            mix,
        ) {
            Ok(value) => value,
            // Typed frame activation retains this unresolved transition, rather than emitting
            // only its source. This compatibility path deliberately keeps the old owner.
            Err(light_core::programming::TransitionError::Requires(_)) => underlying,
            // Reject an invalid contribution and retain the eligible underlay.
            Err(light_core::programming::TransitionError::Invalid(_)) => underlying,
        };
    }
    match (underlying.normalized(), contribution.normalized()) {
        (Some(underlying), Some(contribution)) => AttributeValue::Normalized(
            underlying + (contribution - underlying) * mix.clamp(0.0, 1.0),
        ),
        _ if mix >= 0.5 => contribution,
        _ => underlying,
    }
}

pub(in crate::runtime) struct DynamicPlaybackControl {
    pub(in crate::runtime) identity: PlaybackIdentity,
    pub(in crate::runtime) master: f32,
    pub(in crate::runtime) crossfade_non_intensity: bool,
    pub(in crate::runtime) auto_off_full_control: bool,
    pub(in crate::runtime) temporary_only: bool,
}

pub(in crate::runtime) fn fully_controlled_dynamic_playbacks(
    engine: &Engine,
    samples: &[light_dynamics::DynamicRuntimeSample],
    controls: &HashMap<Uuid, DynamicPlaybackControl>,
    runtime: &light_dynamics::DynamicRuntimeSnapshot,
    programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    cue_values: &[light_playback::ActiveCueDynamicValue],
    now: chrono::DateTime<chrono::Utc>,
) -> Vec<PlaybackIdentity> {
    if !controls
        .values()
        .any(|control| control.auto_off_full_control)
    {
        return Vec::new();
    }
    let persistent = engine.playback_contributions_at(now);
    fully_controlled_dynamic_playbacks_from(
        persistent
            .iter()
            .map(|candidate| (candidate.source, &candidate.value)),
        samples,
        controls,
        runtime,
        programmer_values,
        cue_values,
    )
}

fn fully_controlled_dynamic_playbacks_from<'a>(
    playback: impl IntoIterator<Item = (light_playback::SequenceMasterSource, &'a TimedValue)>,
    samples: &[light_dynamics::DynamicRuntimeSample],
    controls: &HashMap<Uuid, DynamicPlaybackControl>,
    runtime: &light_dynamics::DynamicRuntimeSnapshot,
    programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    cue_values: &[light_playback::ActiveCueDynamicValue],
) -> Vec<PlaybackIdentity> {
    if !controls
        .values()
        .any(|control| control.auto_off_full_control)
    {
        return Vec::new();
    }
    let persistent = playback
        .into_iter()
        .filter(|(source, _)| !source.temporary)
        .collect::<Vec<_>>();
    let mut addresses = HashMap::<
        PlaybackIdentity,
        Vec<(
            light_dynamics::LegacyDynamicSample<'_>,
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
    let persistent_fat = persistent_fat_values(programmer_values, cue_values);
    let temporary_cue_controllers = cue_values
        .iter()
        .filter(|row| row.source.temporary)
        .filter_map(|row| {
            row.value
                .track_key()
                .instance_link
                .map(|link| row.source_key.controller_id(link))
        })
        .collect::<HashSet<_>>();
    for sample in samples
        .iter()
        .filter_map(light_dynamics::DynamicRuntimeSample::legacy)
    {
        if let Some(control) = controls.get(&sample.controller_id)
            && control.auto_off_full_control
        {
            addresses
                .entry(control.identity)
                .or_default()
                .push((sample, control));
        }
    }
    addresses
        .into_iter()
        .filter_map(|(identity, target_samples)| {
            (!target_samples.is_empty()
                && target_samples.iter().all(|(sample, control)| {
                    let dynamic_value = if sample.attribute.is_intensity() {
                        sample.value * control.master
                    } else {
                        sample.value
                    };
                    persistent.iter().any(|(source, candidate)| {
                        candidate_playback_identity(*source).is_some_and(|other| other != identity)
                            && candidate.fixture_id == sample.target
                            && candidate.attribute == *sample.attribute
                            && persistent_playback_wins_dynamic(candidate, sample, dynamic_value)
                    }) || samples
                        .iter()
                        .filter_map(light_dynamics::DynamicRuntimeSample::legacy)
                        .any(|candidate| {
                            candidate.controller_id != sample.controller_id
                                && candidate.target == sample.target
                                && candidate.attribute == sample.attribute
                                && candidate.activation_mix >= 1.0
                                && controller_sources
                                    .get(&candidate.controller_id)
                                    .is_some_and(|source| match source {
                                        light_dynamics::DynamicControllerSource::Playback {
                                            ..
                                        } => controls.get(&candidate.controller_id).is_some_and(
                                            |control| {
                                                control.identity != identity
                                                    && !control.temporary_only
                                            },
                                        ),
                                        light_dynamics::DynamicControllerSource::Programmer {
                                            ..
                                        } => true,
                                        light_dynamics::DynamicControllerSource::Cue { .. } => {
                                            !temporary_cue_controllers
                                                .contains(&candidate.controller_id)
                                        }
                                    })
                                && persistent_dynamic_wins_dynamic(&candidate, sample)
                        })
                        || persistent_fat.iter().any(|candidate| {
                            candidate.fixture_id == sample.target
                                && candidate.attribute == *sample.attribute
                                && persistent_semantic_wins_dynamic(
                                    candidate.priority,
                                    candidate.changed_at_millis,
                                    sample,
                                )
                        })
                }))
            .then_some(identity)
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

fn persistent_fat_values(
    programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    cue_values: &[light_playback::ActiveCueDynamicValue],
) -> Vec<PersistentFatValue> {
    let programmer = programmer_values
        .iter()
        .filter_map(|(_, priority, stored)| {
            matches!(
                &stored.value,
                light_dynamics::DynamicSemanticValue::FixAt { .. }
                    | light_dynamics::DynamicSemanticValue::Static { .. }
            )
            .then_some(PersistentFatValue {
                fixture_id: stored.fixture_id,
                attribute: stored.attribute.clone(),
                priority: *priority,
                changed_at_millis: stored.changed_at_millis,
            })
        });
    let cues = cue_values
        .iter()
        .filter(|row| row.output_enabled && !row.source.temporary)
        .filter_map(|stored| {
            matches!(
                &stored.value,
                light_dynamics::DynamicSemanticValue::FixAt { .. }
                    | light_dynamics::DynamicSemanticValue::Static { .. }
            )
            .then_some(PersistentFatValue {
                fixture_id: stored.fixture_id,
                attribute: stored.attribute.clone(),
                priority: stored.priority,
                changed_at_millis: stored.changed_at_millis,
            })
        });
    programmer.chain(cues).collect()
}

fn candidate_playback_identity(
    source: light_playback::SequenceMasterSource,
) -> Option<PlaybackIdentity> {
    source.playback_identity.or_else(|| {
        source
            .playback_number
            .and_then(|number| PlaybackIdentity::physical(number).ok())
    })
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
    candidate: &TimedValue,
    dynamic: &light_dynamics::DynamicRuntimeSample,
    dynamic_value: f32,
) -> bool {
    if candidate.priority != dynamic.priority {
        return candidate.priority > dynamic.priority;
    }
    if candidate.merge_mode == MergeMode::Htp {
        return candidate.value.normalized().unwrap_or(0.0) > dynamic_value;
    }
    u64::try_from(candidate.changed_at.timestamp_millis()).unwrap_or_default()
        > dynamic.activated_at_millis
}

#[cfg(test)]
mod tick_source_tests {
    use super::*;
    use light_programmer::ProgrammerRegistry;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn scalar_stage_applies_captured_cue_suppression_before_point_geometry() {
        let engine = Engine::new(ProgrammerRegistry::default());
        let frame = engine.prepare_output_frame(Default::default());
        let sources = TickSources::prepared(&engine, &frame, &[]);
        let controller = Uuid::new_v4();
        let target = FixtureId::new();
        let sample = |attribute| light_dynamics::DynamicRuntimeSample {
            instance_id: Uuid::new_v4(),
            controller_id: controller,
            target,
            lane_id: Uuid::new_v4(),
            expression: light_dynamics::DynamicSampleExpression::LegacyScalar {
                attribute,
                value: 0.8,
                occurrence: None,
                dependency_occurrence: None,
            },
            priority: 100,
            activated_at_millis: 100,
            activation_mix: 1.0,
            address: None,
        };
        let samples = [
            sample(AttributeKey("point.position.x".into())),
            sample(AttributeKey::intensity()),
        ];
        let collect = |enabled| {
            collect_dynamic_candidates(
                &frame.frame_addresser(),
                &[],
                &[],
                &[],
                &samples,
                &HashMap::new(),
                &HashMap::from([(
                    controller,
                    CueDynamicOutputControl {
                        enabled,
                        sequence_master: 0.5,
                    },
                )]),
                &sources,
                100,
            )
        };
        assert!(
            collect(false).is_empty(),
            "suppressed Cue motion must not move the geometry used by typed sampling"
        );
        let enabled = collect(true);
        assert_eq!(enabled.len(), 2);
        for stack in enabled.values() {
            let expected = if stack.attribute.is_intensity() {
                0.4
            } else {
                0.8
            };
            assert_eq!(
                stack.candidates[0].value,
                AttributeValue::Normalized(expected)
            );
        }
        assert!(
            sources.values.get().is_none(),
            "fully active constants need no extra Current solve"
        );
    }

    #[test]
    fn semantic_activation_blends_the_complete_owner_and_holds_unknown_appearance() {
        use light_core::programming::{ColorIntent, ColorProgram, PositionIntent, UvIntent};
        let position = |pan| AttributeValue::Position(Arc::new(PositionIntent::angles(pan, 0.0)));
        assert_eq!(
            blend_attribute_value(position(0.0), position(720.0), 0.5),
            position(360.0)
        );
        let color = |uv| {
            AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
                intent: ColorIntent {
                    uv: UvIntent { amount: uv },
                    ..Default::default()
                },
            }))
        };
        let AttributeValue::ColorProgram(blended) =
            blend_attribute_value(color(0.0), color(1.0), 0.25)
        else {
            panic!()
        };
        let ColorProgram::Semantic { intent } = blended.as_ref() else {
            panic!()
        };
        assert_eq!(intent.uv.amount, 0.25);
        assert_eq!(intent.recipe, ColorIntent::default().recipe);
        // A legacy scalar cannot be reinterpreted as a complete physical family at half mix.
        assert_eq!(
            blend_attribute_value(AttributeValue::Normalized(0.2), color(1.0), 0.75),
            AttributeValue::Normalized(0.2)
        );
        assert_eq!(
            blend_attribute_value(AttributeValue::Normalized(0.2), color(1.0), 1.0),
            color(1.0)
        );
    }

    #[test]
    fn tick_sources_defer_whole_show_resolution() {
        let engine = Engine::new(ProgrammerRegistry::default());
        let sources = TickSources::new(&engine);

        assert!(
            sources.values.get().is_none(),
            "constructing an output tick must not eagerly resolve the whole show"
        );
    }

    #[test]
    fn prepared_dynamic_tick_pins_programmer_sources_clock_and_lazy_current() {
        let started = chrono::DateTime::from_timestamp_millis(1_000_000).unwrap();
        let clock = Arc::new(light_core::ManualClock::new(started));
        let programmers = ProgrammerRegistry::with_clock(clock.clone());
        let session = light_core::SessionId::new();
        let fixture = FixtureId::new();
        let attribute = AttributeKey("pan".into());
        programmers.start(session);
        programmers.set(
            session,
            fixture,
            attribute.clone(),
            AttributeValue::Normalized(0.2),
        );
        let set_fat = |value| {
            assert!(programmers.apply_dynamic_values(
                session,
                &[light_programmer::DynamicProgrammerValueMutation::Set {
                    fixture_id: fixture,
                    attribute: attribute.clone(),
                    value: light_dynamics::DynamicSemanticValue::Static {
                        value: AttributeValue::Normalized(value),
                        timing: light_dynamics::DynamicValueTiming {
                            fade_millis: Some(1_000),
                            delay_millis: None,
                        },
                    },
                }],
                None,
            ));
        };
        // Legacy final contribution arbitration uses authored timestamps for distinct sources.
        // Keep this source-capture test independent of an exact-timestamp LTP tie.
        clock.advance_millis(1);
        set_fat(1.0);
        let engine = Engine::new(programmers.clone());
        clock.advance_millis(500);
        let frame = engine.prepare_output_frame(RenderOptions::default());
        let sources = TickSources::prepared(&engine, &frame, &[]);
        assert!(sources.values.get().is_none());

        // Neither first use of Current nor the Dynamic/FAT projection may reread these edits.
        clock.advance_millis(400);
        programmers.set(
            session,
            fixture,
            attribute.clone(),
            AttributeValue::Normalized(0.9),
        );
        set_fat(0.1);
        assert_eq!(sources.current(fixture, &attribute), Some(0.2));
        let dynamics = Mutex::new(light_dynamics::DynamicRuntime::default());
        let speed_groups = Mutex::new(std::array::from_fn(|_| {
            light_control::speed::SpeedGroupController::new(120.0, Default::default()).unwrap()
        }));
        let (sampled, _, _, _, _) = dynamic_contributions_prepared(
            &engine,
            &frame,
            &[],
            &dynamics,
            &speed_groups,
            &AtomicU16::new(40),
            &[],
            None,
            false,
        );
        assert_eq!(sampled.len(), 1);
        let sampled_value = sampled[0].samples()[0].value().value.normalized().unwrap();
        assert!(
            (sampled_value - 0.6).abs() < 0.0001,
            "captured half-fade from Current 0.2"
        );
        let rendered = engine.render_prepared(&frame, &sampled).unwrap();
        assert_eq!(
            rendered.sampled_at,
            started + chrono::Duration::milliseconds(501)
        );
        assert!(
            (rendered
                .resolved_values
                .value(fixture, &attribute)
                .and_then(AttributeValue::normalized)
                .unwrap()
                - 0.6)
                .abs()
                < 0.0001
        );
    }

    #[test]
    fn preload_dynamic_projection_uses_pending_current_and_only_mutates_its_fork() {
        let started = chrono::DateTime::from_timestamp_millis(1_000_000).unwrap();
        let clock = Arc::new(light_core::ManualClock::new(started));
        let programmers = ProgrammerRegistry::with_clock(clock.clone());
        let session = light_core::SessionId::new();
        let fixture = FixtureId::new();
        let attribute = AttributeKey("pan".into());
        programmers.start(session);
        programmers.set(
            session,
            fixture,
            attribute.clone(),
            AttributeValue::Normalized(0.2),
        );
        programmers.arm_preload(session, true);
        clock.advance_millis(1);
        programmers.set(
            session,
            fixture,
            attribute.clone(),
            AttributeValue::Normalized(0.6),
        );
        let set_pending_fat = |value| {
            assert!(programmers.apply_dynamic_values(
                session,
                &[light_programmer::DynamicProgrammerValueMutation::Set {
                    fixture_id: fixture,
                    attribute: attribute.clone(),
                    value: light_dynamics::DynamicSemanticValue::Static {
                        value: AttributeValue::Normalized(value),
                        timing: light_dynamics::DynamicValueTiming {
                            fade_millis: Some(1_000),
                            delay_millis: None,
                        },
                    },
                }],
                None,
            ));
        };
        clock.advance_millis(1);
        set_pending_fat(1.0);
        let engine = Engine::new(programmers.clone());
        engine
            .execute_playback(light_engine::EnginePlaybackCommand::SetDynamicsPaused(true))
            .unwrap();
        clock.advance_millis(500);
        let frame = engine.prepare_output_frame(RenderOptions::default());
        let input = engine.prepare_preload_frame(&frame, None);
        let state = light_engine::PreloadFrameState::default();
        let sources = PreloadTickSources {
            engine: &engine,
            input: &input,
            state: &state,
            before_release: false,
            baseline_samples: &[],
            values: OnceLock::new(),
        };
        assert!(sources.values.get().is_none());
        let live_dynamics = light_dynamics::DynamicRuntime::default();
        let live_before = live_dynamics.output_projection_snapshot();
        let mut preview_dynamics = live_dynamics.fork_for_preview();
        let speed_groups = Mutex::new(std::array::from_fn(|_| {
            light_control::speed::SpeedGroupController::new(120.0, Default::default()).unwrap()
        }));
        let speed_transports = capture_dynamic_speed_transports(
            &speed_groups,
            u64::try_from(frame.sampled_at().timestamp_millis()).unwrap(),
        );

        // Edits after capture must affect neither the first lazy Current lookup nor sampling.
        clock.advance_millis(400);
        programmers.set(
            session,
            fixture,
            attribute.clone(),
            AttributeValue::Normalized(0.1),
        );
        set_pending_fat(0.0);
        engine
            .execute_playback(light_engine::EnginePlaybackCommand::SetDynamicsPaused(
                false,
            ))
            .unwrap();
        assert_eq!(sources.current(fixture, &attribute), Some(0.6));
        assert_eq!(
            TickSources::prepared(&engine, &frame, &[]).current(fixture, &attribute),
            Some(0.2),
        );
        let (batches, samples) = dynamic_projection_preload(
            &engine,
            &input,
            &state,
            false,
            &[],
            &mut preview_dynamics,
            &speed_transports,
            40,
        );
        assert!(
            samples.is_empty(),
            "Static/FAT produces a candidate, not a running lane"
        );
        assert_eq!(batches.len(), 1);
        let actual = batches[0].samples()[0].value().value.normalized().unwrap();
        assert!(
            (actual - 0.8).abs() < 0.0001,
            "half fade must start from pending Current0.6"
        );
        assert!(preview_dynamics.output_projection_snapshot().global_paused);
        assert_eq!(live_dynamics.output_projection_snapshot(), live_before);
        assert!(!engine.playback_dynamics().paused);
    }

    #[test]
    fn prepared_current_and_render_share_extra_static_samples() {
        let engine = Engine::new(ProgrammerRegistry::default());
        let frame = engine.prepare_output_frame(RenderOptions::default());
        let fixture = FixtureId::new();
        let attribute = AttributeKey("audio.volume".into());
        let baseline = [ContributionBatch::new([ContributionSample::independent(
            TimedValue {
                fixture_id: fixture,
                attribute: attribute.clone(),
                value: AttributeValue::Normalized(0.35),
                priority: 75,
                changed_at: frame.sampled_at(),
                programmer_order: 1,
                merge_mode: MergeMode::Ltp,
                fade: false,
                fade_millis: None,
                delay_millis: None,
            },
        )])];
        let sources = TickSources::prepared(&engine, &frame, &baseline);
        assert!(sources.values.get().is_none());
        assert_eq!(sources.current(fixture, &attribute), Some(0.35));
        let rendered = engine.render_prepared(&frame, &baseline).unwrap();
        assert_eq!(
            rendered
                .resolved_values
                .value(fixture, &attribute)
                .and_then(AttributeValue::normalized),
            Some(0.35)
        );
    }

    #[test]
    fn fully_active_first_candidate_does_not_resolve_the_underlay() {
        let underlay_requested = AtomicBool::new(false);
        let stack = [DynamicCandidate {
            value: AttributeValue::Normalized(0.75),
            priority: 10,
            changed_at_millis: 1,
            exact_changed_at: None,
            stable_order: 1,
            activation_mix: 1.0,
            dynamic: true,
        }];

        let resolved = resolve_dynamic_stack(&stack, || {
            underlay_requested.store(true, Ordering::Relaxed);
            Some(AttributeValue::Normalized(0.25))
        });

        assert_eq!(resolved, AttributeValue::Normalized(0.75));
        assert!(
            !underlay_requested.load(Ordering::Relaxed),
            "a fully active winning candidate makes the underlay irrelevant"
        );
    }

    #[test]
    fn fading_first_candidate_blends_from_the_underlay() {
        let stack = [DynamicCandidate {
            value: AttributeValue::Normalized(1.0),
            priority: 10,
            changed_at_millis: 1,
            exact_changed_at: None,
            stable_order: 1,
            activation_mix: 0.5,
            dynamic: true,
        }];

        let resolved = resolve_dynamic_stack(&stack, || Some(AttributeValue::Normalized(0.0)));

        assert_eq!(resolved, AttributeValue::Normalized(0.5));
    }

    #[test]
    fn only_a_completely_static_tick_is_idle() {
        assert!(dynamic_tick_is_idle_from_presence(
            true, false, false, false, false, false, false,
        ));

        for non_idle in [
            dynamic_tick_is_idle_from_presence(false, false, false, false, false, false, false),
            dynamic_tick_is_idle_from_presence(true, false, true, false, false, false, false),
            dynamic_tick_is_idle_from_presence(true, false, false, true, false, false, false),
            dynamic_tick_is_idle_from_presence(true, false, false, false, true, false, false),
            dynamic_tick_is_idle_from_presence(true, false, false, false, false, true, false),
            dynamic_tick_is_idle_from_presence(true, false, false, false, false, false, true),
        ] {
            assert!(
                !non_idle,
                "runtime, pause, Dynamic/FAT, Playback, and extra inputs must use the full path"
            );
        }
    }

    #[test]
    fn matching_paused_state_can_still_be_idle() {
        assert!(dynamic_tick_is_idle_from_presence(
            true, true, true, false, false, false, false,
        ));
    }

    #[test]
    fn programmer_reconciliation_ignores_unrelated_engine_snapshot_replacement() {
        let cache = ProgrammerReconciliationCache::default();
        let values = Arc::new(Vec::new());
        let snapshot = Arc::new(light_engine::EngineSnapshot::default());

        assert!(cache.changed(&values, &snapshot));
        assert!(
            cache.changed(&values, &snapshot),
            "comparison cannot acknowledge an uncommitted frame"
        );
        cache.acknowledge(&values, &snapshot);
        assert!(!cache.changed(&values, &Arc::new((*snapshot).clone())));

        let mut changed_definitions = (*snapshot).clone();
        changed_definitions.dynamics = Arc::new(Vec::new());
        assert!(cache.changed(&values, &Arc::new(changed_definitions)));

        let mut changed_positions = (*snapshot).clone();
        changed_positions.dynamic_stage_positions = Arc::new(HashMap::new());
        assert!(cache.changed(&values, &Arc::new(changed_positions)));

        assert!(cache.changed(&Arc::new(Vec::new()), &snapshot));
    }
}
