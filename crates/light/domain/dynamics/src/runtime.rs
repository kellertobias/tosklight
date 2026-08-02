use crate::{
    DynamicDefinition, DynamicTargetBinding, PhaseOrdering, Position3d, ScalarSourceResolver,
    SpatialPosition, SpatialSelectionMapping, SpatialTarget, evaluate_dynamic_spatial_mapping,
    project_phase, project_ranked_phase, validate_definition,
};
use light_core::{AttributeKey, FixtureId};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use thiserror::Error;
use uuid::Uuid;

mod helpers;
mod sampling;

use helpers::*;

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DynamicControllerSource {
    Programmer {
        programmer_id: Uuid,
    },
    Cue {
        cue_list_id: Uuid,
        instance_link: Uuid,
    },
    Playback {
        playback_number: u16,
    },
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
    pub attribute: AttributeKey,
    pub value: f32,
    pub priority: i16,
    pub activated_at_millis: u64,
    /// Ownership influence after activation/release timing. Size remains part of `value`.
    pub activation_mix: f32,
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
    #[serde(default)]
    pub last_synchronized_elapsed_millis: Option<u64>,
    #[serde(default)]
    pub synchronized_resume_transition: Option<DynamicSynchronizedResumeTransitionSnapshot>,
    #[serde(default)]
    pub last_sample_values: Vec<DynamicHeldSampleSnapshot>,
    #[serde(default)]
    pub synchronized_hold_values: Vec<DynamicHeldSampleSnapshot>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct DynamicSynchronizedResumeTransitionSnapshot {
    pub started_at_millis: u64,
    pub duration_millis: u64,
    pub held_elapsed_millis: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DynamicHeldSampleSnapshot {
    pub controller_id: Uuid,
    pub target: FixtureId,
    pub lane_id: Uuid,
    pub value: f32,
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

#[derive(Default)]
pub struct DynamicRuntime {
    definitions: HashMap<Uuid, Arc<DynamicDefinition>>,
    instances: HashMap<Uuid, DynamicInstance>,
    bound_instances: HashMap<Uuid, Uuid>,
    global_paused: bool,
    definitions_pinned: bool,
}

struct DynamicInstance {
    id: Uuid,
    definition: Arc<DynamicDefinition>,
    targets: Vec<FixtureId>,
    phase_by_lane_target: HashMap<(Uuid, FixtureId), f32>,
    controllers: HashMap<Uuid, DynamicController>,
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
    last_synchronized_elapsed_millis: Option<u64>,
    synchronized_resume_transition: Option<DynamicSynchronizedResumeTransitionSnapshot>,
    last_sample_values: HashMap<(Uuid, FixtureId, Uuid), f32>,
    synchronized_hold_values: HashMap<(Uuid, FixtureId, Uuid), f32>,
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
    pub fn snapshot(&self) -> DynamicRuntimeSnapshot {
        let mut instances = self
            .instances
            .values()
            .map(|instance| {
                let mut controllers = instance.controllers.values().cloned().collect::<Vec<_>>();
                controllers.sort_by_key(|controller| controller.id);
                let mut controller_transitions = instance
                    .controller_transitions
                    .values()
                    .copied()
                    .collect::<Vec<_>>();
                controller_transitions.sort_by_key(|transition| transition.controller_id);
                let mut phase_by_lane_target = instance
                    .phase_by_lane_target
                    .iter()
                    .map(|((lane_id, target), phase)| (*lane_id, *target, *phase))
                    .collect::<Vec<_>>();
                phase_by_lane_target.sort_by_key(|(lane_id, target, _)| (*lane_id, target.0));
                let mut phase_by_target = if instance.definition.phase_spread_mode
                    == crate::DynamicPhaseSpreadMode::Uniform
                {
                    instance
                        .definition
                        .lanes
                        .first()
                        .map(|lane| {
                            instance
                                .targets
                                .iter()
                                .filter_map(|target| {
                                    instance
                                        .phase_by_lane_target
                                        .get(&(lane.id, *target))
                                        .map(|phase| (*target, *phase))
                                })
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };
                phase_by_target.sort_by_key(|(target, _)| target.0);
                let mut random_streams = instance
                    .random_streams
                    .iter()
                    .map(|((group_id, target), stream)| DynamicRandomStreamSnapshot {
                        group_id: *group_id,
                        target: *target,
                        last_elapsed_millis: stream.last_elapsed_millis,
                        next_decision_index: stream.next_decision_index,
                        active: stream.active.map(|pulse| DynamicRandomPulseSnapshot {
                            started_at_millis: pulse.started_at_millis,
                            duration_millis: pulse.duration_millis,
                        }),
                    })
                    .collect::<Vec<_>>();
                random_streams.sort_by_key(|stream| (stream.group_id, stream.target.0));
                let last_sample_values = sample_values_snapshot(&instance.last_sample_values);
                let synchronized_hold_values =
                    sample_values_snapshot(&instance.synchronized_hold_values);
                DynamicInstanceSnapshot {
                    id: instance.id,
                    definition: instance.definition.as_ref().clone(),
                    targets: instance.targets.clone(),
                    phase_by_target,
                    phase_by_lane_target,
                    controllers,
                    controller_transitions,
                    started_at_millis: instance.started_at_millis,
                    paused_at_millis: instance.paused_at_millis,
                    paused_elapsed_millis: instance.paused_elapsed_millis,
                    activation_policy: instance.activation_policy,
                    pending_until_millis: instance.pending_until_millis,
                    speed_paused_at_millis: instance.speed_paused_at_millis,
                    speed_paused_elapsed_millis: instance.speed_paused_elapsed_millis,
                    random_streams,
                    completed: instance.completed,
                    synchronized_hold_elapsed_millis: instance.synchronized_hold_elapsed_millis,
                    last_synchronized_elapsed_millis: instance.last_synchronized_elapsed_millis,
                    synchronized_resume_transition: instance.synchronized_resume_transition,
                    last_sample_values,
                    synchronized_hold_values,
                }
            })
            .collect::<Vec<_>>();
        instances.sort_by_key(|instance| instance.id);
        DynamicRuntimeSnapshot {
            global_paused: self.global_paused,
            instances,
        }
    }

    /// Snapshot only the runtime identity and controller state consumed on the output path.
    ///
    /// Output arbitration, transition events, auto-off, and the visualization Dynamic stack do
    /// not read phase maps, random streams, synchronized hold samples, or retained sample values.
    /// Omitting those persistence-only fields avoids cloning and sorting the largest runtime maps
    /// twice per output frame while leaving `snapshot()` and show persistence unchanged.
    pub fn output_projection_snapshot(&self) -> DynamicRuntimeSnapshot {
        let mut instances = self
            .instances
            .values()
            .map(|instance| {
                let mut controllers = instance.controllers.values().cloned().collect::<Vec<_>>();
                controllers.sort_by_key(|controller| controller.id);
                let mut controller_transitions = instance
                    .controller_transitions
                    .values()
                    .copied()
                    .collect::<Vec<_>>();
                controller_transitions.sort_by_key(|transition| transition.controller_id);
                DynamicInstanceSnapshot {
                    id: instance.id,
                    definition: instance.definition.as_ref().clone(),
                    targets: instance.targets.clone(),
                    phase_by_target: Vec::new(),
                    phase_by_lane_target: Vec::new(),
                    controllers,
                    controller_transitions,
                    started_at_millis: instance.started_at_millis,
                    paused_at_millis: instance.paused_at_millis,
                    paused_elapsed_millis: instance.paused_elapsed_millis,
                    activation_policy: instance.activation_policy,
                    pending_until_millis: instance.pending_until_millis,
                    speed_paused_at_millis: instance.speed_paused_at_millis,
                    speed_paused_elapsed_millis: instance.speed_paused_elapsed_millis,
                    random_streams: Vec::new(),
                    completed: instance.completed,
                    synchronized_hold_elapsed_millis: None,
                    last_synchronized_elapsed_millis: None,
                    synchronized_resume_transition: instance.synchronized_resume_transition,
                    last_sample_values: Vec::new(),
                    synchronized_hold_values: Vec::new(),
                }
            })
            .collect::<Vec<_>>();
        instances.sort_by_key(|instance| instance.id);
        DynamicRuntimeSnapshot {
            global_paused: self.global_paused,
            instances,
        }
    }

    pub fn restore_snapshot(
        &mut self,
        snapshot: DynamicRuntimeSnapshot,
    ) -> Result<(), DynamicRuntimeError> {
        let mut instances = HashMap::new();
        let mut bound_instances = HashMap::new();
        for stored in snapshot.instances {
            validate_definition(&stored.definition)
                .map_err(|error| DynamicRuntimeError::InvalidDefinition(error.to_string()))?;
            if stored.targets.is_empty()
                || stored.controllers.is_empty()
                || stored
                    .phase_by_target
                    .iter()
                    .any(|(_, phase)| !phase.is_finite())
                || stored
                    .phase_by_lane_target
                    .iter()
                    .any(|(_, _, phase)| !phase.is_finite())
                || stored
                    .last_sample_values
                    .iter()
                    .chain(&stored.synchronized_hold_values)
                    .any(|sample| !sample.value.is_finite())
            {
                return Err(DynamicRuntimeError::InvalidSnapshot(
                    "instances require targets, controllers, and finite phase and sample values"
                        .into(),
                ));
            }
            for controller in &stored.controllers {
                validate_controller(controller)?;
            }
            let definition = self
                .definitions
                .get(&stored.definition.id)
                .cloned()
                .unwrap_or_else(|| Arc::new(stored.definition.clone()));
            let bound = !matches!(definition.target_binding, DynamicTargetBinding::Targetless);
            if bound && bound_instances.insert(definition.id, stored.id).is_some() {
                return Err(DynamicRuntimeError::InvalidSnapshot(
                    "a target-bound Dynamic has multiple singleton instances".into(),
                ));
            }
            let stored_controllers = stored.controllers.clone();
            let controllers = stored
                .controllers
                .into_iter()
                .map(|controller| (controller.id, controller))
                .collect();
            let controller_transitions = if stored.controller_transitions.is_empty() {
                stored_controllers
                    .iter()
                    .map(|controller| {
                        (
                            controller.id,
                            DynamicControllerTransitionSnapshot {
                                controller_id: controller.id,
                                activation_started_at_millis: controller.activated_at_millis,
                                ..Default::default()
                            },
                        )
                    })
                    .collect()
            } else {
                let transitions = stored
                    .controller_transitions
                    .into_iter()
                    .map(|transition| (transition.controller_id, transition))
                    .collect::<HashMap<_, _>>();
                if stored_controllers
                    .iter()
                    .any(|controller| !transitions.contains_key(&controller.id))
                {
                    return Err(DynamicRuntimeError::InvalidSnapshot(
                        "every Dynamic controller requires transition state".into(),
                    ));
                }
                transitions
            };
            let random_streams = stored
                .random_streams
                .into_iter()
                .map(|stream| {
                    (
                        (stream.group_id, stream.target),
                        RandomStreamState {
                            last_elapsed_millis: stream.last_elapsed_millis,
                            next_decision_index: stream.next_decision_index,
                            active: stream.active.map(|pulse| RandomPulse {
                                started_at_millis: pulse.started_at_millis,
                                duration_millis: pulse.duration_millis,
                            }),
                        },
                    )
                })
                .collect();
            let mut phase_by_lane_target = stored
                .definition
                .lanes
                .iter()
                .flat_map(|lane| {
                    stored
                        .phase_by_target
                        .iter()
                        .map(move |(target, phase)| ((lane.id, *target), *phase))
                })
                .collect::<HashMap<_, _>>();
            phase_by_lane_target.extend(
                stored
                    .phase_by_lane_target
                    .iter()
                    .map(|(lane_id, target, phase)| ((*lane_id, *target), *phase)),
            );
            instances.insert(
                stored.id,
                DynamicInstance {
                    id: stored.id,
                    definition,
                    targets: stored.targets,
                    phase_by_lane_target,
                    controllers,
                    controller_transitions,
                    started_at_millis: stored.started_at_millis,
                    paused_at_millis: stored.paused_at_millis,
                    paused_elapsed_millis: stored.paused_elapsed_millis,
                    activation_policy: stored.activation_policy,
                    pending_until_millis: stored.pending_until_millis,
                    speed_paused_at_millis: stored.speed_paused_at_millis,
                    speed_paused_elapsed_millis: stored.speed_paused_elapsed_millis,
                    random_streams,
                    completed: stored.completed,
                    synchronized_hold_elapsed_millis: stored.synchronized_hold_elapsed_millis,
                    last_synchronized_elapsed_millis: stored.last_synchronized_elapsed_millis,
                    synchronized_resume_transition: stored.synchronized_resume_transition,
                    last_sample_values: sample_values_from_snapshot(stored.last_sample_values),
                    synchronized_hold_values: sample_values_from_snapshot(
                        stored.synchronized_hold_values,
                    ),
                },
            );
        }
        self.instances = instances;
        self.bound_instances = bound_instances;
        self.global_paused = snapshot.global_paused;
        Ok(())
    }

    pub fn install_definitions(
        &mut self,
        definitions: impl IntoIterator<Item = DynamicDefinition>,
    ) -> Result<(), DynamicRuntimeError> {
        let mut installed = HashMap::new();
        for definition in definitions {
            validate_definition(&definition)
                .map_err(|error| DynamicRuntimeError::InvalidDefinition(error.to_string()))?;
            installed.insert(definition.id, Arc::new(definition));
        }
        self.definitions = installed;
        if !self.definitions_pinned {
            for instance in self.instances.values_mut() {
                if let Some(definition) = self.definitions.get(&instance.definition.id) {
                    instance.definition = Arc::clone(definition);
                }
            }
        }
        Ok(())
    }

    /// Pins effective definitions for already-running instances during blind Preload editing.
    ///
    /// New definitions still compile into the registry for projected Preload use. Unpinning
    /// atomically hot-swaps every live reference to the latest valid revision without changing
    /// clocks, controller stacks, targets, or Random streams.
    pub fn set_definitions_pinned(&mut self, pinned: bool) {
        if self.definitions_pinned == pinned {
            return;
        }
        self.definitions_pinned = pinned;
        if !pinned {
            for instance in self.instances.values_mut() {
                if let Some(definition) = self.definitions.get(&instance.definition.id) {
                    instance.definition = Arc::clone(definition);
                }
            }
        }
    }

    /// Retains an embedded deletion fallback without replacing the current show definition set.
    pub fn install_fallback_definition(
        &mut self,
        definition: DynamicDefinition,
    ) -> Result<(), DynamicRuntimeError> {
        validate_definition(&definition)
            .map_err(|error| DynamicRuntimeError::InvalidDefinition(error.to_string()))?;
        self.definitions
            .entry(definition.id)
            .or_insert_with(|| Arc::new(definition));
        Ok(())
    }

    pub fn start(&mut self, request: DynamicStartRequest) -> Result<Uuid, DynamicRuntimeError> {
        validate_controller(&request.controller)?;
        if request.target_scope.ordered_targets.is_empty() {
            return Err(DynamicRuntimeError::EmptyTargets);
        }
        let definition = Arc::clone(
            self.definitions
                .get(&request.definition_id)
                .ok_or(DynamicRuntimeError::MissingDefinition)?,
        );
        let bound = !matches!(definition.target_binding, DynamicTargetBinding::Targetless);
        let existing = if bound {
            self.bound_instances.get(&definition.id).copied()
        } else if request.reuse_matching_targetless {
            self.instances
                .values()
                .find(|instance| {
                    instance.definition.id == definition.id
                        && instance.targets == request.target_scope.ordered_targets
                        && instance
                            .controllers
                            .values()
                            .any(|controller| controller.source == request.controller.source)
                })
                .map(|instance| instance.id)
        } else {
            None
        };
        if let Some(instance_id) = existing {
            let instance = self
                .instances
                .get_mut(&instance_id)
                .expect("instance indices stay synchronized");
            if instance.completed {
                instance.completed = false;
                instance.started_at_millis = request.now_millis;
                instance.paused_at_millis = self.global_paused.then_some(request.now_millis);
                instance.paused_elapsed_millis = 0;
                instance.pending_until_millis = None;
                instance.speed_paused_at_millis = None;
                instance.speed_paused_elapsed_millis = 0;
                instance.random_streams.clear();
                instance.synchronized_hold_elapsed_millis = None;
                instance.last_synchronized_elapsed_millis = None;
                instance.synchronized_resume_transition = None;
                instance.last_sample_values.clear();
                instance.synchronized_hold_values.clear();
                instance
                    .controllers
                    .retain(|_, controller| controller.source != request.controller.source);
                instance
                    .controller_transitions
                    .retain(|controller_id, _| instance.controllers.contains_key(controller_id));
            }
            instance
                .controllers
                .insert(request.controller.id, request.controller.clone());
            instance.controller_transitions.insert(
                request.controller.id,
                DynamicControllerTransitionSnapshot {
                    controller_id: request.controller.id,
                    activation_started_at_millis: request.now_millis,
                    activation_delay_millis: request.activation_delay_millis,
                    activation_duration_millis: request.activation_duration_millis,
                    ..Default::default()
                },
            );
            reconcile_pause(instance, self.global_paused, request.now_millis);
            return Ok(instance_id);
        }

        let instance_id = Uuid::new_v4();
        let phase_by_lane_target = project_instance_phases(
            &definition,
            &request.target_scope.ordered_targets,
            &request.stage_positions,
            request.inherited_spatial_mapping.as_ref(),
        )?;
        let mut controllers = HashMap::new();
        controllers.insert(request.controller.id, request.controller.clone());
        let controller_transitions = HashMap::from([(
            request.controller.id,
            DynamicControllerTransitionSnapshot {
                controller_id: request.controller.id,
                activation_started_at_millis: request.now_millis,
                activation_delay_millis: request.activation_delay_millis,
                activation_duration_millis: request.activation_duration_millis,
                ..Default::default()
            },
        )]);
        let activation_policy = request
            .activation_policy_override
            .unwrap_or(definition.default_activation);
        let instance = DynamicInstance {
            id: instance_id,
            definition,
            targets: request.target_scope.ordered_targets,
            phase_by_lane_target,
            controllers,
            controller_transitions,
            started_at_millis: request.now_millis,
            paused_at_millis: self.global_paused.then_some(request.now_millis),
            paused_elapsed_millis: 0,
            activation_policy,
            pending_until_millis: None,
            speed_paused_at_millis: None,
            speed_paused_elapsed_millis: 0,
            random_streams: HashMap::new(),
            completed: false,
            synchronized_hold_elapsed_millis: None,
            last_synchronized_elapsed_millis: None,
            synchronized_resume_transition: None,
            last_sample_values: HashMap::new(),
            synchronized_hold_values: HashMap::new(),
        };
        if bound {
            self.bound_instances
                .insert(instance.definition.id, instance_id);
        }
        self.instances.insert(instance_id, instance);
        Ok(instance_id)
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
        instance
            .random_streams
            .retain(|(_, target), _| retained_targets.contains(target));
        instance
            .last_sample_values
            .retain(|(_, target, _), _| retained_targets.contains(target));
        instance
            .synchronized_hold_values
            .retain(|(_, target, _), _| retained_targets.contains(target));
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
        instance.controller_transitions.remove(&controller_id);
        if instance.controllers.is_empty() {
            let definition_id = instance.definition.id;
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
        let controller = self
            .instances
            .values_mut()
            .find_map(|instance| instance.controllers.get_mut(&controller_id))
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
        *controller = candidate;
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
