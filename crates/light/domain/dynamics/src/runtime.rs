use crate::{
    DynamicDefinition, DynamicSampleExpression, DynamicTargetBinding, PhaseOrdering, Position3d,
    ScalarSourceResolver, SpatialPosition, SpatialSelectionMapping, SpatialTarget,
    evaluate_dynamic_spatial_mapping, project_phase, project_ranked_phase, validate_definition,
};
use light_core::{AttributeKey, FixtureId, FrameAddress, FrameAddressResolver};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use thiserror::Error;
use uuid::Uuid;

mod control_batch;
mod control_log;
mod helpers;
mod recording;
pub use control_batch::{
    DynamicControl, DynamicControlBatch, DynamicControlJournal, DynamicControlOutcome,
    TimedDynamicControl, replay_dynamic_controls,
};
pub use control_log::{
    ControlCursor as DynamicControlCursor, ControlLogError as DynamicControlLogError,
};
mod lanes;
mod native_capability;
mod output_gate;
mod owner;
mod preset_values;
mod programmer_identity;
mod programming;
mod sample_boundary;
mod sampling;
mod snapshot;
mod snapshot_restore;
mod start;
mod transaction;
pub use output_gate::DynamicControllerOutputGateSnapshot;
pub use programmer_identity::normalize_legacy_programmer_controller_ids;
pub use sample_boundary::{DynamicSampleBoundary, DynamicSampleScope};
pub use sampling::{CompletedDynamicSamples, DeferredTypedSampling, DynamicSamplingScratch};
pub use transaction::DynamicOutputFrameScratch;

use helpers::*;
use lanes::{CompiledLaneSelection, restore_lane_selections};
pub use lanes::{DynamicControllerLaneSelection, DynamicLaneSelection, DynamicTargetLanes};
pub use native_capability::DynamicNativeSourceStatus;
pub use preset_values::PreparedDynamicPresetSources;
use programming::ProgrammingLanes;
pub use programming::{
    DynamicInstancePresetSources, DynamicNativeModelResolver, NativeColorModelCapability,
    NativeColorModelUnavailable, NativeColorUnavailableReason,
};

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DynamicControllerSource {
    Programmer {
        programmer_id: Uuid,
        /// Authored link for publication and exact source tracing. Old checkpoints have no
        /// retained link; runtime owner matching still uses only the Programmer identity.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        instance_link: Option<Uuid>,
    },
    Cue {
        cue_list_id: Uuid,
        instance_link: Uuid,
    },
    /// Current operational Playback owner. `virtual_page` qualifies a Virtual Playback
    /// assignment; physical owners and old checkpoints carry none. Surviving controllers refresh
    /// this through [`DynamicControl::Owner`]; it is not an immutable source-origin record.
    Playback {
        playback_number: u16,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        virtual_page: Option<u8>,
    },
}

impl DynamicControllerSource {
    fn same_runtime_owner(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Programmer {
                    programmer_id: left,
                    ..
                },
                Self::Programmer {
                    programmer_id: right,
                    ..
                },
            ) => left == right,
            _ => self == other,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DynamicController {
    pub id: Uuid,
    pub source: DynamicControllerSource,
    pub priority: i16,
    pub activated_at_millis: u64,
    pub size: f32,
    pub speed_multiplier: f32,
    pub phase_offset_degrees: f32,
    pub paused: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DynamicTargetScope {
    pub ordered_targets: Vec<FixtureId>,
}

#[derive(Clone, Debug)]
pub struct DynamicStartRequest {
    pub definition_id: Uuid,
    pub controller: DynamicController,
    pub target_scope: DynamicTargetScope,
    pub stage_positions: HashMap<FixtureId, SpatialPosition>,
    pub inherited_spatial_mapping: Option<SpatialSelectionMapping>,
    pub now_millis: u64,
    pub activation_delay_millis: u64,
    pub activation_duration_millis: u64,
    pub activation_policy_override: Option<crate::ActivationPolicy>,
    /// Programmer pool toggles may reuse one targetless instance with the exact same source/scope.
    /// Cue and Playback starts leave this false and therefore remain independent.
    pub reuse_matching_targetless: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DynamicRuntimeSample {
    pub instance_id: Uuid,
    pub controller_id: Uuid,
    pub target: FixtureId,
    pub lane_id: Uuid,
    pub expression: DynamicSampleExpression,
    pub priority: i16,
    pub activated_at_millis: u64,
    /// Ownership influence after activation/release timing. Size remains part of `value`.
    pub activation_mix: f32,
    /// Where the engine keeps this pair, when the sampler was told how to find out.
    pub address: Option<FrameAddress>,
}

#[derive(Clone, Copy)]
pub struct LegacyDynamicSample<'a> {
    pub sample: &'a DynamicRuntimeSample,
    pub attribute: &'a AttributeKey,
    pub value: f32,
}

impl std::ops::Deref for LegacyDynamicSample<'_> {
    type Target = DynamicRuntimeSample;
    fn deref(&self) -> &Self::Target {
        self.sample
    }
}

impl DynamicRuntimeSample {
    pub fn legacy(&self) -> Option<LegacyDynamicSample<'_>> {
        let (attribute, value) = self.expression.legacy_leaf()?;
        Some(LegacyDynamicSample {
            sample: self,
            attribute,
            value,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DynamicSpeedTransport {
    pub effective_bpm: f64,
    pub phase_origin_millis: u64,
    pub phase_reference_millis: u64,
    pub beat_phase: f64,
    pub phase_advancing: bool,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DynamicRuntimeError {
    #[error("Dynamic definition is missing")]
    MissingDefinition,
    #[error("Dynamic target scope is empty")]
    EmptyTargets,
    #[error("Dynamic controller values are invalid")]
    InvalidController,
    #[error("Dynamic spatial mapping is invalid: {0}")]
    InvalidSpatialMapping(String),
    #[error("Dynamic instance is missing")]
    MissingInstance,
    #[error("Dynamic controller is missing")]
    MissingController,
    #[error("Dynamic definition is invalid: {0}")]
    InvalidDefinition(String),
    #[error("Dynamic runtime snapshot is invalid: {0}")]
    InvalidSnapshot(String),
    #[error("Dynamic sampling failed: {0}")]
    InvalidSample(String),
    #[error("Dynamic control replay is invalid: {0}")]
    InvalidReplay(String),
}

/// Cold-compiled definition data, independent of the live transport and controller state.
/// Prepared tokens keep their exact original-model proofs but never replace the live provider.
/// Install only into the originating runtime, whose supported programming contract is immutable.
/// The owning layer serializes definition-registry changes; this token is not a registry CAS.
#[must_use = "prepared Dynamic definitions must be installed to become live"]
pub struct PreparedDynamicDefinitions {
    definitions: HashMap<Uuid, Arc<DynamicDefinition>>,
    compiled_lanes: HashMap<Uuid, ProgrammingLanes>,
    native_pins: native_capability::PreparedNativePins,
}

impl std::fmt::Debug for PreparedDynamicDefinitions {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreparedDynamicDefinitions")
            .field("definitions", &self.definitions)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct DynamicRuntimeSnapshot {
    pub global_paused: bool,
    pub instances: Vec<DynamicInstanceSnapshot>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DynamicInstanceSnapshot {
    pub id: Uuid,
    pub definition: DynamicDefinition,
    pub targets: Vec<FixtureId>,
    #[serde(default)]
    pub phase_by_target: Vec<(FixtureId, f32)>,
    #[serde(default)]
    pub phase_by_lane_target: Vec<(Uuid, FixtureId, f32)>,
    pub controllers: Vec<DynamicController>,
    #[serde(default)]
    pub lane_selections: Vec<DynamicControllerLaneSelection>,
    #[serde(default)]
    pub controller_transitions: Vec<DynamicControllerTransitionSnapshot>,
    pub started_at_millis: u64,
    pub paused_at_millis: Option<u64>,
    pub paused_elapsed_millis: u64,
    pub activation_policy: crate::ActivationPolicy,
    pub pending_until_millis: Option<u64>,
    pub speed_paused_at_millis: Option<u64>,
    pub speed_paused_elapsed_millis: u64,
    pub random_streams: Vec<DynamicRandomStreamSnapshot>,
    #[serde(default)]
    pub completed: bool,
    #[serde(default)]
    pub synchronized_hold_elapsed_millis: Option<u64>,
    /// Distinguish a captured empty source set from the first paused sample.
    #[serde(default)]
    pub synchronized_hold_captured: bool,
    #[serde(default)]
    pub last_synchronized_elapsed_millis: Option<u64>,
    #[serde(default)]
    pub synchronized_resume_transition: Option<DynamicSynchronizedResumeTransitionSnapshot>,
    #[serde(default)]
    pub last_sample_values: Vec<DynamicHeldSampleSnapshot>,
    #[serde(default)]
    pub synchronized_hold_values: Vec<DynamicHeldSampleSnapshot>,
    /// One shared graph for both keyed checkpoint maps; rows retain only root IDs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expression_tape: Option<Arc<crate::RetainedExpressionTape>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preset_source_values: Vec<crate::DynamicPresetSourceValues>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct DynamicSynchronizedResumeTransitionSnapshot {
    #[serde(default = "Uuid::new_v4")]
    pub occurrence_id: Uuid,
    pub started_at_millis: u64,
    pub duration_millis: u64,
    pub held_elapsed_millis: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DynamicHeldSampleSnapshot {
    pub controller_id: Uuid,
    pub target: FixtureId,
    pub lane_id: Uuid,
    #[serde(flatten)]
    pub payload: DynamicHeldPayload,
}

/// Old checkpoints recorded only a number. Resolve that cold against their retained
/// definition once; all current checkpoints retain the complete original address.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum DynamicHeldPayload {
    TapeRoot { tape_root: crate::RetainedNodeId },
    Expression { expression: DynamicSampleExpression },
    Legacy { value: f32 },
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct DynamicControllerTransitionSnapshot {
    pub controller_id: Uuid,
    pub activation_started_at_millis: u64,
    pub activation_delay_millis: u64,
    pub activation_duration_millis: u64,
    pub release_started_at_millis: Option<u64>,
    pub release_delay_millis: u64,
    pub release_duration_millis: u64,
    /// Output-only retention mask. Completion never removes the running controller.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_gate: Option<DynamicControllerOutputGateSnapshot>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DynamicRandomStreamSnapshot {
    pub group_id: Uuid,
    pub target: FixtureId,
    pub last_elapsed_millis: u64,
    pub next_decision_index: u64,
    pub active: Option<DynamicRandomPulseSnapshot>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct DynamicRandomPulseSnapshot {
    pub started_at_millis: u64,
    pub duration_millis: u64,
}

pub struct DynamicRuntime {
    supported_programming_contract: u16,
    definitions: HashMap<Uuid, Arc<DynamicDefinition>>,
    compiled_lanes: HashMap<Uuid, ProgrammingLanes>,
    native_models: Option<Arc<dyn DynamicNativeModelResolver>>,
    native_model_pins: native_capability::NativeModelPins,
    instances: HashMap<Uuid, DynamicInstance>,
    bound_instances: HashMap<Uuid, Uuid>,
    global_paused: bool,
    definitions_pinned: bool,
    sample_boundary: Option<DynamicSampleBoundary>,
    control_recording: Option<DynamicControlJournal>,
    output_frame_undo: Option<transaction::OutputFrameUndo>,
    sampling_buffers: sampling::SamplingWorkBuffers,
    /// TL-639: `Some` only after `derive_instance_ids_from`; new instances then take name-based
    /// identities from their definition instead of random ones.
    derived_instance_ids: Option<(Uuid, HashMap<Uuid, u64>)>,
}

impl Default for DynamicRuntime {
    fn default() -> Self {
        Self::with_programming_contract_support(
            light_core::programming::PROGRAMMING_CONTRACT_VERSION,
        )
    }
}

#[derive(Clone)]
struct DynamicInstance {
    id: Uuid,
    definition: Arc<DynamicDefinition>,
    programming_lanes: ProgrammingLanes,
    preset_values: preset_values::PresetValues,
    preset_dependency_generation: Uuid,
    targets: Vec<FixtureId>,
    phase_by_lane_target: HashMap<(Uuid, FixtureId), f32>,
    controllers: HashMap<Uuid, DynamicController>,
    lane_selections: HashMap<Uuid, CompiledLaneSelection>,
    controller_transitions: HashMap<Uuid, DynamicControllerTransitionSnapshot>,
    started_at_millis: u64,
    paused_at_millis: Option<u64>,
    paused_elapsed_millis: u64,
    activation_policy: crate::ActivationPolicy,
    pending_until_millis: Option<u64>,
    speed_paused_at_millis: Option<u64>,
    speed_paused_elapsed_millis: u64,
    random_streams: HashMap<(Uuid, FixtureId), RandomStreamState>,
    completed: bool,
    synchronized_hold_elapsed_millis: Option<u64>,
    synchronized_hold_captured: bool,
    last_synchronized_elapsed_millis: Option<u64>,
    synchronized_resume_transition: Option<DynamicSynchronizedResumeTransitionSnapshot>,
    last_sample_values: HashMap<(Uuid, FixtureId, Uuid), DynamicSampleExpression>,
    synchronized_hold_values: HashMap<(Uuid, FixtureId, Uuid), DynamicSampleExpression>,
    synchronized_hold_angle_sources: std::collections::HashSet<(Uuid, FixtureId, Uuid)>,
    /// Cold-derived capability gaps; payloads remain intact until the original is available.
    unavailable_samples: native_capability::UnavailableSamples,
    /// Where each target-and-lane pair lives in the engine's frame, remembered across ticks.
    frame_addresses: Option<FrameAddressTable>,
}

/// One instance's addresses, valid for one patch generation, one definition and one target list.
#[derive(Clone, Debug)]
struct FrameAddressTable {
    generation: u64,
    /// The definition the lanes were read from, compared by identity.
    definition: Arc<DynamicDefinition>,
    targets: Vec<FixtureId>,
    /// Indexed by `target_index * lanes + lane_index`.
    addresses: Arc<[Option<FrameAddress>]>,
}

impl DynamicInstance {
    /// This instance's addresses for `resolver`'s generation, resolved once and kept until the
    /// patch, the definition or the targets change.
    fn frame_addresses(
        &mut self,
        resolver: &dyn FrameAddressResolver,
    ) -> Arc<[Option<FrameAddress>]> {
        let generation = resolver.generation();
        if let Some(table) = &self.frame_addresses
            && table.generation == generation
            && Arc::ptr_eq(&table.definition, &self.definition)
            && table.targets == self.targets
        {
            return Arc::clone(&table.addresses);
        }
        let addresses = self
            .targets
            .iter()
            .flat_map(|target| {
                self.definition
                    .lanes
                    .iter()
                    .map(|lane| resolver.frame_address(*target, &lane.output_owner()))
            })
            .collect::<Arc<[_]>>();
        self.frame_addresses = Some(FrameAddressTable {
            generation,
            definition: Arc::clone(&self.definition),
            targets: self.targets.clone(),
            addresses: Arc::clone(&addresses),
        });
        addresses
    }
}

#[derive(Clone, Debug, Default)]
struct RandomStreamState {
    last_elapsed_millis: u64,
    next_decision_index: u64,
    active: Option<RandomPulse>,
}

#[derive(Clone, Copy, Debug)]
struct RandomPulse {
    started_at_millis: u64,
    duration_millis: u64,
}

impl DynamicRuntime {
    /// Fork the current output state for isolated preview sampling and edits.
    ///
    /// Clocks, controllers, phase and Random state, held history, lane selections and warm
    /// address caches are copied. Immutable definitions, compiled source/model references and
    /// retained expression tapes remain shared through their Arcs. Mutable original-model pins
    /// are detached so preview-only discovery cannot change Live capability state. This neither
    /// serializes a snapshot nor recompiles definitions; sampling, pausing, Off and edits in the returned
    /// runtime cannot advance or replace this runtime's state.
    ///
    /// Mutable maps and compiled lane caches are copied, so callers should retain one fork per
    /// preview lane/frame rather than fork separately for every observer.
    pub fn fork_for_preview(&self) -> Self {
        Self {
            supported_programming_contract: self.supported_programming_contract,
            definitions: self.definitions.clone(),
            compiled_lanes: self.compiled_lanes.clone(),
            native_models: self.native_models.clone(),
            native_model_pins: self.native_model_pins.detached(),
            instances: self.instances.clone(),
            bound_instances: self.bound_instances.clone(),
            global_paused: self.global_paused,
            definitions_pinned: self.definitions_pinned,
            // A fork of provisional state has no accepted history/capture anchor.
            sample_boundary: self
                .output_frame_undo
                .is_none()
                .then_some(self.sample_boundary)
                .flatten(),
            output_frame_undo: None,
            control_recording: None,
            sampling_buffers: Default::default(),
            derived_instance_ids: None,
        }
    }

    /// Prepare a cold dependency installation without changing the live runtime's model pins.
    ///
    /// Clocks, controllers, Random state, held history and definition pinning follow the same
    /// exact copy as a preview fork. Immutable definitions, compiled addresses, models and
    /// expression tapes remain shared, but the mutable original-model pin collection is always
    /// detached, including when the candidate keeps the same provider. Failed compilation can
    /// therefore discover new original models without replacing Live's captured model view.
    ///
    /// This does not authorize publishing a stale candidate: the caller must exclude live
    /// mutations from capture through installation, or validate its own installation generation.
    pub fn fork_for_cold_install(&self) -> Self {
        let mut candidate = self.fork_for_preview();
        candidate.control_recording = self
            .output_frame_undo
            .is_none()
            .then(|| self.control_recording.clone())
            .flatten();
        candidate
    }

    /// Begin a pending preview that follows edited definitions while Blind Live stays pinned.
    /// Clocks, stable controller/instance identities, Random and held history are preserved by
    /// the ordinary unpin/rebind operation. Call once at episode creation, not every frame;
    /// subsequent controls and cold inputs still require ordered synchronization.
    pub fn fork_for_pending_preview(&self) -> Self {
        let mut pending = self.fork_for_preview();
        pending.set_definitions_pinned(false);
        pending
    }

    pub fn with_programming_contract_support(supported_programming_contract: u16) -> Self {
        Self {
            supported_programming_contract,
            definitions: HashMap::new(),
            compiled_lanes: HashMap::new(),
            native_models: None,
            native_model_pins: Default::default(),
            instances: HashMap::new(),
            bound_instances: HashMap::new(),
            global_paused: false,
            definitions_pinned: false,
            sample_boundary: None,
            control_recording: None,
            output_frame_undo: None,
            sampling_buffers: Default::default(),
            derived_instance_ids: None,
        }
    }

    /// TL-639: reproducible runs. Instances started without an authoritative identity take a
    /// name-based identity from their definition and how often it started, instead of a random
    /// v4 identity, so Random lanes repeat exactly between processes. (Controller identities can
    /// still be random, for example a Programmer's.) Benchmarks comparing two builds use this;
    /// the desk never does.
    pub fn derive_instance_ids_from(&mut self, namespace: Uuid) {
        self.derived_instance_ids = Some((namespace, HashMap::new()));
    }

    pub(crate) fn derives_instance_ids(&self) -> bool {
        self.derived_instance_ids.is_some()
    }

    fn new_instance_id(&mut self, definition: Uuid) -> Uuid {
        match &mut self.derived_instance_ids {
            Some((namespace, starts)) => {
                let ordinal = starts.entry(definition).or_default();
                *ordinal += 1;
                let mut name = [0; 24];
                name[..16].copy_from_slice(definition.as_bytes());
                name[16..].copy_from_slice(&ordinal.to_le_bytes());
                Uuid::new_v5(namespace, &name)
            }
            None => Uuid::new_v4(),
        }
    }

    fn validate_supported_definition(
        &self,
        definition: &DynamicDefinition,
    ) -> Result<(), DynamicRuntimeError> {
        validate_definition(definition)
            .map_err(|error| DynamicRuntimeError::InvalidDefinition(error.to_string()))?;
        let required = definition.required_programming_contract();
        if required > self.supported_programming_contract {
            return Err(DynamicRuntimeError::InvalidDefinition(format!(
                "Dynamic requires programming contract {required}; this runtime supports {}",
                self.supported_programming_contract
            )));
        }
        Ok(())
    }

    pub fn install_definitions(
        &mut self,
        definitions: impl IntoIterator<Item = DynamicDefinition>,
    ) -> Result<(), DynamicRuntimeError> {
        assert!(
            self.output_frame_undo.is_none(),
            "definition installation is outside an output transaction"
        );
        let prepared = self.prepare_definitions(definitions)?;
        self.install_prepared_definitions(prepared);
        Ok(())
    }

    /// Normalize and compile against this runtime's immutable provider without publishing
    /// definitions or newly verified pins. The token contains no instances, clocks, controller
    /// state, held history or definition-pin policy, so these may evolve before installation.
    pub fn prepare_definitions(
        &self,
        definitions: impl IntoIterator<Item = DynamicDefinition>,
    ) -> Result<PreparedDynamicDefinitions, DynamicRuntimeError> {
        // Use the cold fork's detached-pin boundary without copying instances, Random streams
        // or held forests that definition compilation neither reads nor installs.
        let mut candidate =
            Self::with_programming_contract_support(self.supported_programming_contract);
        candidate.native_models = self.native_models.clone();
        candidate.native_model_pins = self.native_model_pins.detached();
        let mut installed = HashMap::new();
        let mut compiled_lanes = HashMap::new();
        for mut definition in definitions {
            definition.normalize_angle_pair();
            let id = definition.id;
            if self
                .definitions
                .get(&id)
                .is_some_and(|current| current.as_ref() == &definition)
            {
                installed.insert(id, Arc::clone(&self.definitions[&id]));
                compiled_lanes.insert(id, self.compiled_lanes[&id].clone());
            } else {
                compiled_lanes.insert(id, candidate.compile_programming_lanes(&definition)?);
                installed.insert(id, Arc::new(definition));
            }
        }
        Ok(PreparedDynamicDefinitions {
            definitions: installed,
            compiled_lanes,
            native_pins: candidate.native_model_pins.prepare_verified(),
        })
    }

    /// Install already validated definitions without recompiling or replacing current runtime
    /// state. Current definition pinning controls instance rebinding. Verified original pins
    /// accumulate; models acquired by Live after preparation remain authoritative for their
    /// identities. A token's unavailable lanes remain suspended until an explicit refresh.
    /// The token must come from this runtime; installation does not change or renegotiate its
    /// immutable supported programming contract.
    pub fn install_prepared_definitions(&mut self, prepared: PreparedDynamicDefinitions) {
        assert!(
            self.output_frame_undo.is_none(),
            "definition installation is outside an output transaction"
        );
        self.native_model_pins.merge_verified(prepared.native_pins);
        self.definitions = prepared.definitions;
        self.compiled_lanes = prepared.compiled_lanes;
        if !self.definitions_pinned {
            for instance in self.instances.values_mut() {
                if let Some(definition) = self.definitions.get(&instance.definition.id) {
                    if !Arc::ptr_eq(&instance.definition, definition) {
                        instance.programming_lanes = self.compiled_lanes[&definition.id].clone();
                        instance.definition = Arc::clone(definition);
                        instance.rebind_angle_lane_selections();
                        instance.rebind_preset_values();
                    }
                }
            }
        }
    }

    /// Pins effective definitions for already-running instances during blind Preload editing.
    ///
    /// New definitions still compile into the registry for projected Preload use. Unpinning
    /// atomically hot-swaps every live reference to the latest valid revision without changing
    /// clocks, controller stacks, targets, or Random streams.
    pub fn set_definitions_pinned(&mut self, pinned: bool) {
        assert!(
            self.output_frame_undo.is_none(),
            "definition pinning is outside an output transaction"
        );
        if self.definitions_pinned == pinned {
            return;
        }
        self.definitions_pinned = pinned;
        if !pinned {
            for instance in self.instances.values_mut() {
                if let Some(definition) = self.definitions.get(&instance.definition.id) {
                    if !Arc::ptr_eq(&instance.definition, definition) {
                        instance.programming_lanes = self.compiled_lanes[&definition.id].clone();
                        instance.definition = Arc::clone(definition);
                        instance.rebind_angle_lane_selections();
                        instance.rebind_preset_values();
                    }
                }
            }
        }
    }

    /// Retains an embedded deletion fallback without replacing the current show definition set.
    pub fn install_fallback_definition(
        &mut self,
        mut definition: DynamicDefinition,
    ) -> Result<(), DynamicRuntimeError> {
        definition.normalize_angle_pair();
        self.validate_supported_definition(&definition)?;
        if !self.definitions.contains_key(&definition.id) {
            let lanes = self.compile_programming_lanes(&definition)?;
            if let Some(undo) = &mut self.output_frame_undo {
                undo.record_fallback(definition.id);
            }
            self.compiled_lanes.insert(definition.id, lanes);
            self.definitions.insert(definition.id, Arc::new(definition));
        }
        Ok(())
    }

    /// Replaces one running instance's authoritative target/mapping evaluation without restarting
    /// its clock or controller stack.
    ///
    /// The candidate phase map is resolved before any live state changes. This makes invalid
    /// Group/Dynamic mapping edits fail atomically and leaves the prior output snapshot intact.
    pub fn reconcile_instance_targets(
        &mut self,
        instance_id: Uuid,
        target_scope: DynamicTargetScope,
        stage_positions: &HashMap<FixtureId, SpatialPosition>,
        inherited_spatial_mapping: Option<&SpatialSelectionMapping>,
    ) -> Result<bool, DynamicRuntimeError> {
        let instance = self
            .instances
            .get(&instance_id)
            .ok_or(DynamicRuntimeError::MissingInstance)?;
        let phase_by_lane_target = project_instance_phases(
            &instance.definition,
            &target_scope.ordered_targets,
            stage_positions,
            inherited_spatial_mapping,
        )?;
        let changed = instance.targets != target_scope.ordered_targets
            || instance.phase_by_lane_target != phase_by_lane_target;
        if !changed {
            return Ok(false);
        }

        self.journal_instance(instance_id);

        let retained_targets = target_scope
            .ordered_targets
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        let instance = self
            .instances
            .get_mut(&instance_id)
            .expect("instance remains present during atomic reconciliation");
        instance.targets = target_scope.ordered_targets;
        instance.phase_by_lane_target = phase_by_lane_target;
        // A cold Preset result also depends on rank/mapping, even when its target list and
        // source bindings are unchanged. The instance journal restores this token on rollback.
        instance.preset_dependency_generation = Uuid::new_v4();
        instance
            .random_streams
            .retain(|(_, target), _| retained_targets.contains(target));
        instance
            .last_sample_values
            .retain(|(_, target, _), _| retained_targets.contains(target));
        instance
            .synchronized_hold_values
            .retain(|(_, target, _), _| retained_targets.contains(target));
        instance
            .unavailable_samples
            .retain(|(_, target, _), _| retained_targets.contains(target));
        for lane in instance.programming_lanes.values_mut() {
            lane.retain_targets(&retained_targets);
        }
        instance.rebind_preset_values();
        Ok(true)
    }

    pub fn off_controller(
        &mut self,
        instance_id: Uuid,
        controller_id: Uuid,
        now_millis: u64,
        release_delay_millis: u64,
        release_duration_millis: u64,
    ) -> Result<bool, DynamicRuntimeError> {
        let current = self
            .instances
            .get(&instance_id)
            .ok_or(DynamicRuntimeError::MissingInstance)?;
        if !current.controllers.contains_key(&controller_id) {
            return Err(DynamicRuntimeError::MissingController);
        }
        if !current.completed
            && (release_delay_millis > 0 || release_duration_millis > 0)
            && current
                .controller_transitions
                .get(&controller_id)
                .is_some_and(|transition| {
                    transition.release_started_at_millis.is_some()
                        && transition.release_delay_millis == release_delay_millis
                        && transition.release_duration_millis == release_duration_millis
                })
        {
            return Ok(false);
        }
        self.journal_instance(instance_id);
        let instance = self
            .instances
            .get_mut(&instance_id)
            .ok_or(DynamicRuntimeError::MissingInstance)?;
        if !instance.controllers.contains_key(&controller_id) {
            return Err(DynamicRuntimeError::MissingController);
        }
        if !instance.completed && (release_delay_millis > 0 || release_duration_millis > 0) {
            let transition = instance
                .controller_transitions
                .get_mut(&controller_id)
                .ok_or(DynamicRuntimeError::MissingController)?;
            transition
                .release_started_at_millis
                .get_or_insert(now_millis);
            transition.release_delay_millis = release_delay_millis;
            transition.release_duration_millis = release_duration_millis;
            return Ok(false);
        }
        instance.controllers.remove(&controller_id);
        instance.lane_selections.remove(&controller_id);
        instance.controller_transitions.remove(&controller_id);
        instance
            .last_sample_values
            .retain(|(id, _, _), _| *id != controller_id);
        instance
            .synchronized_hold_values
            .retain(|(id, _, _), _| *id != controller_id);
        instance
            .unavailable_samples
            .retain(|(id, _, _), _| *id != controller_id);
        if instance.controllers.is_empty() {
            let definition_id = instance.definition.id;
            if let Some(undo) = &mut self.output_frame_undo {
                undo.record_bound(
                    definition_id,
                    self.bound_instances.get(&definition_id).copied(),
                );
            }
            self.instances.remove(&instance_id);
            self.bound_instances.remove(&definition_id);
            return Ok(true);
        }
        reconcile_pause(instance, self.global_paused, now_millis);
        Ok(false)
    }

    pub fn set_controller_paused(
        &mut self,
        instance_id: Uuid,
        controller_id: Uuid,
        paused: bool,
        now_millis: u64,
    ) -> Result<(), DynamicRuntimeError> {
        let current = self
            .instances
            .get(&instance_id)
            .ok_or(DynamicRuntimeError::MissingInstance)?;
        let controller = current
            .controllers
            .get(&controller_id)
            .ok_or(DynamicRuntimeError::MissingController)?;
        let effective_paused = self.global_paused
            || winning_controller(current).is_some_and(|controller| controller.paused);
        if controller.paused == paused && current.paused_at_millis.is_some() == effective_paused {
            return Ok(());
        }
        self.journal_instance(instance_id);
        let instance = self
            .instances
            .get_mut(&instance_id)
            .ok_or(DynamicRuntimeError::MissingInstance)?;
        instance
            .controllers
            .get_mut(&controller_id)
            .ok_or(DynamicRuntimeError::MissingController)?
            .paused = paused;
        reconcile_pause(instance, self.global_paused, now_millis);
        Ok(())
    }

    pub fn set_controller_paused_with_resume(
        &mut self,
        instance_id: Uuid,
        controller_id: Uuid,
        paused: bool,
        now_millis: u64,
        resume_policy: Option<crate::ActivationPolicy>,
    ) -> Result<(), DynamicRuntimeError> {
        let was_paused = self
            .instances
            .get(&instance_id)
            .and_then(|instance| instance.controllers.get(&controller_id))
            .ok_or(DynamicRuntimeError::MissingController)?
            .paused;
        self.set_controller_paused(instance_id, controller_id, paused, now_millis)?;
        if was_paused && !paused {
            let instance = self
                .instances
                .get_mut(&instance_id)
                .ok_or(DynamicRuntimeError::MissingInstance)?;
            if let Some(policy) = resume_policy {
                instance.activation_policy = policy;
                instance.pending_until_millis = None;
            }
            schedule_synchronized_resume(instance, now_millis);
        }
        Ok(())
    }

    /// Applies the desk-wide Pause Dynamics transport without changing source ownership.
    pub fn set_global_paused(&mut self, paused: bool, now_millis: u64) {
        if self.global_paused == paused {
            return;
        }
        if let Some(undo) = &mut self.output_frame_undo {
            undo.record_global_pause(self.global_paused);
            for (id, instance) in &self.instances {
                undo.record_instance(*id, Some(instance));
            }
        }
        self.global_paused = paused;
        for instance in self.instances.values_mut() {
            let was_paused = instance.paused_at_millis.is_some();
            reconcile_pause(instance, paused, now_millis);
            if was_paused && !paused && instance.paused_at_millis.is_none() {
                schedule_synchronized_resume(instance, now_millis);
            }
        }
    }

    pub fn off_controller_by_id(
        &mut self,
        controller_id: Uuid,
        now_millis: u64,
        release_delay_millis: u64,
        release_duration_millis: u64,
    ) -> Result<(Uuid, bool), DynamicRuntimeError> {
        let instance_id = self
            .instances
            .iter()
            .find_map(|(instance_id, instance)| {
                instance
                    .controllers
                    .contains_key(&controller_id)
                    .then_some(*instance_id)
            })
            .ok_or(DynamicRuntimeError::MissingController)?;
        let ended = self.off_controller(
            instance_id,
            controller_id,
            now_millis,
            release_delay_millis,
            release_duration_millis,
        )?;
        Ok((instance_id, ended))
    }

    pub fn update_controller(
        &mut self,
        controller_id: Uuid,
        size: Option<f32>,
        speed_multiplier: Option<f32>,
        phase_offset_degrees: Option<f32>,
    ) -> Result<(), DynamicRuntimeError> {
        let (instance_id, controller) = self
            .controller(controller_id)
            .ok_or(DynamicRuntimeError::MissingController)?;
        let mut candidate = controller.clone();
        if let Some(size) = size {
            candidate.size = size;
        }
        if let Some(speed_multiplier) = speed_multiplier {
            candidate.speed_multiplier = speed_multiplier;
        }
        if let Some(phase_offset_degrees) = phase_offset_degrees {
            candidate.phase_offset_degrees = phase_offset_degrees;
        }
        validate_controller(&candidate)?;
        if candidate != controller {
            self.journal_instance(instance_id);
            *self
                .instances
                .get_mut(&instance_id)
                .expect("existing instance")
                .controllers
                .get_mut(&controller_id)
                .expect("existing controller") = candidate;
        }
        Ok(())
    }

    /// Restamp source arbitration without resetting the instance clock or activation transition.
    pub fn update_controller_rank(
        &mut self,
        controller_id: Uuid,
        priority: i16,
        authored_at_millis: u64,
        now_millis: u64,
    ) -> Result<(), DynamicRuntimeError> {
        let (instance_id, controller) = self
            .controller(controller_id)
            .ok_or(DynamicRuntimeError::MissingController)?;
        if controller.priority == priority && controller.activated_at_millis == authored_at_millis {
            return Ok(());
        }
        self.journal_instance(instance_id);
        let controller = self
            .instances
            .get_mut(&instance_id)
            .expect("existing instance")
            .controllers
            .get_mut(&controller_id)
            .expect("existing controller");
        controller.priority = priority;
        controller.activated_at_millis = authored_at_millis;
        reconcile_pause(
            self.instances
                .get_mut(&instance_id)
                .expect("existing instance"),
            self.global_paused,
            now_millis,
        );
        Ok(())
    }

    /// Restore an already-running logical controller whose authored On becomes effective again.
    /// Release timing alone is cleared; phase, activation, held samples, and Random state remain.
    pub fn cancel_controller_release(
        &mut self,
        controller_id: Uuid,
    ) -> Result<(), DynamicRuntimeError> {
        let (instance_id, _) = self
            .controller(controller_id)
            .ok_or(DynamicRuntimeError::MissingController)?;
        let releasing = self
            .instances
            .get(&instance_id)
            .and_then(|instance| instance.controller_transitions.get(&controller_id))
            .is_some_and(|transition| transition.release_started_at_millis.is_some());
        if !releasing {
            return Ok(());
        }
        self.journal_instance(instance_id);
        let transition = self
            .instances
            .get_mut(&instance_id)
            .expect("existing instance")
            .controller_transitions
            .get_mut(&controller_id)
            .ok_or(DynamicRuntimeError::MissingController)?;
        transition.release_started_at_millis = None;
        transition.release_delay_millis = 0;
        transition.release_duration_millis = 0;
        Ok(())
    }

    pub fn controller(&self, controller_id: Uuid) -> Option<(Uuid, DynamicController)> {
        self.instances.iter().find_map(|(instance_id, instance)| {
            instance
                .controllers
                .get(&controller_id)
                .cloned()
                .map(|controller| (*instance_id, controller))
        })
    }

    pub fn controllers(&self) -> Vec<(Uuid, DynamicController)> {
        self.instances
            .iter()
            .flat_map(|(instance_id, instance)| {
                instance
                    .controllers
                    .values()
                    .cloned()
                    .map(|controller| (*instance_id, controller))
            })
            .collect()
    }

    /// A fading-out controller still samples its original authored lanes. Source owners keep
    /// those active bindings until the release completes, without searching retained history.
    pub fn source_scope_is_releasing(&self, instance_id: Uuid, controller_id: Uuid) -> bool {
        self.instances
            .get(&instance_id)
            .and_then(|instance| instance.controller_transitions.get(&controller_id))
            .is_some_and(|transition| transition.release_started_at_millis.is_some())
    }

    /// Effective immutable definition of a running instance. Blind Preload can install a newer
    /// registry definition while this instance remains pinned to its existing lane set.
    pub fn instance_definition(&self, instance_id: Uuid) -> Option<&Arc<DynamicDefinition>> {
        self.instances
            .get(&instance_id)
            .map(|instance| &instance.definition)
    }

    pub fn instance_ids(&self) -> Vec<Uuid> {
        self.instances.keys().copied().collect()
    }

    pub fn instance_count(&self) -> usize {
        self.instances.len()
    }

    pub fn is_definition_running(&self, definition_id: Uuid) -> bool {
        self.instances
            .values()
            .any(|instance| instance.definition.id == definition_id)
    }
}
