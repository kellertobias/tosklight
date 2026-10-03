use super::*;
use rustc_hash::FxHashMap;
use std::borrow::Cow;
use std::sync::OnceLock;

mod candidates;
mod endpoint_controls;
mod family_inputs;
mod fixed_masks;
mod full_control;
mod output_transaction;
#[path = "../pending_publication.rs"]
pub(in crate::runtime) mod pending_publication;
pub(in crate::runtime) mod physical_adapter;
mod programming_projection;
pub(in crate::runtime) mod retained_preload_hybrid;
mod scalar_projection;
mod source_bindings;
mod transition_events;
use candidates::{
    CandidateKey, CandidateStack, authored_activation_mix, collect_dynamic_candidates,
    collect_dynamic_candidates_with_fixed_rows, resolve_dynamic_stack,
};
#[cfg(test)]
use candidates::{DynamicCandidate, blend_attribute_value};
use full_control::fully_controlled_dynamic_playbacks_from;
#[cfg(test)]
use full_control::persistent_fat_values;
pub(in crate::runtime) use full_control::{
    DynamicPlaybackControl, fully_controlled_dynamic_playbacks,
};
pub(in crate::runtime) use output_transaction::{LiveFamilyAdapters, dynamic_output_frame};
pub(in crate::runtime) use transition_events::dynamic_transition_events;

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

#[cfg(test)]
mod tick_source_tests;
