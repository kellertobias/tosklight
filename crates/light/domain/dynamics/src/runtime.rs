use crate::{
    DynamicDefinition, DynamicEvaluationContext, DynamicEvaluator, DynamicTargetBinding,
    ScalarSourceResolver, SpatialPosition, project_phase, validate_definition,
};
use light_core::{AttributeKey, FixtureId};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use thiserror::Error;
use uuid::Uuid;

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
    pub phase_by_target: Vec<(FixtureId, f32)>,
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
    phase_by_target: HashMap<FixtureId, f32>,
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
                let mut phase_by_target = instance
                    .phase_by_target
                    .iter()
                    .map(|(target, phase)| (*target, *phase))
                    .collect::<Vec<_>>();
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
                DynamicInstanceSnapshot {
                    id: instance.id,
                    definition: instance.definition.as_ref().clone(),
                    targets: instance.targets.clone(),
                    phase_by_target,
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
            {
                return Err(DynamicRuntimeError::InvalidSnapshot(
                    "instances require targets, controllers, and finite phase values".into(),
                ));
            }
            for controller in &stored.controllers {
                validate_controller(controller)?;
            }
            let definition = self
                .definitions
                .get(&stored.definition.id)
                .cloned()
                .unwrap_or_else(|| Arc::new(stored.definition));
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
            instances.insert(
                stored.id,
                DynamicInstance {
                    id: stored.id,
                    definition,
                    targets: stored.targets,
                    phase_by_target: stored.phase_by_target.into_iter().collect(),
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
        let phase_by_target = project_phase(
            &definition.phase,
            &request.target_scope.ordered_targets,
            &request.stage_positions,
            0,
        )
        .into_iter()
        .map(|phase| (phase.target, phase.degrees))
        .collect();
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
            phase_by_target,
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
        };
        if bound {
            self.bound_instances
                .insert(instance.definition.id, instance_id);
        }
        self.instances.insert(instance_id, instance);
        Ok(instance_id)
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
            if let Some(policy) = resume_policy {
                let instance = self
                    .instances
                    .get_mut(&instance_id)
                    .ok_or(DynamicRuntimeError::MissingInstance)?;
                instance.activation_policy = policy;
                instance.pending_until_millis = None;
            }
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
            reconcile_pause(instance, paused, now_millis);
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

    pub fn sample(
        &mut self,
        instance_id: Uuid,
        now_millis: u64,
        cycle_duration_millis: u64,
        output_interval_millis: u64,
        sources: &dyn ScalarSourceResolver,
    ) -> Result<Vec<DynamicRuntimeSample>, DynamicRuntimeError> {
        let (samples, completed) = self.sample_with_transport(
            instance_id,
            now_millis,
            cycle_duration_millis,
            output_interval_millis,
            None,
            sources,
        )?;
        if completed {
            self.complete_one_shot(instance_id);
        }
        Ok(samples)
    }

    fn sample_with_transport(
        &mut self,
        instance_id: Uuid,
        now_millis: u64,
        cycle_duration_millis: u64,
        output_interval_millis: u64,
        transport: Option<DynamicSpeedTransport>,
        sources: &dyn ScalarSourceResolver,
    ) -> Result<(Vec<DynamicRuntimeSample>, bool), DynamicRuntimeError> {
        let instance = self
            .instances
            .get_mut(&instance_id)
            .ok_or(DynamicRuntimeError::MissingInstance)?;
        if instance.completed {
            return Ok((Vec::new(), false));
        }
        let winning = winning_controller(instance)
            .cloned()
            .ok_or(DynamicRuntimeError::MissingController)?;
        let mut controllers = instance.controllers.values().cloned().collect::<Vec<_>>();
        controllers.sort_by_key(|controller| {
            std::cmp::Reverse((
                controller.priority,
                controller.activated_at_millis,
                controller.id,
            ))
        });
        if let Some(transport) = transport {
            if transport.phase_advancing {
                if let Some(paused_at) = instance.speed_paused_at_millis.take() {
                    instance.speed_paused_elapsed_millis = instance
                        .speed_paused_elapsed_millis
                        .saturating_add(now_millis.saturating_sub(paused_at));
                }
            } else if instance.speed_paused_at_millis.is_none() {
                instance.speed_paused_at_millis = Some(now_millis);
            }
        }
        let effective_now = instance
            .paused_at_millis
            .or(instance.speed_paused_at_millis)
            .unwrap_or(now_millis);
        let elapsed = match (instance.activation_policy, transport) {
            (crate::ActivationPolicy::JoinSyncNow, Some(transport)) => transport
                .phase_reference_millis
                .saturating_sub(transport.phase_origin_millis),
            (crate::ActivationPolicy::NextBoundary, Some(transport)) => {
                if !transport.phase_advancing {
                    return Ok((Vec::new(), false));
                }
                let boundary = *instance.pending_until_millis.get_or_insert_with(|| {
                    let beat_millis = (60_000.0 / transport.effective_bpm.max(f64::EPSILON))
                        .round()
                        .max(1.0) as u64;
                    match instance.definition.activation_boundary {
                        crate::ActivationBoundary::Beat => now_millis.saturating_add(
                            ((1.0 - transport.beat_phase.rem_euclid(1.0)) * beat_millis as f64)
                                .round() as u64,
                        ),
                        crate::ActivationBoundary::Bar => {
                            let elapsed = transport
                                .phase_reference_millis
                                .saturating_sub(transport.phase_origin_millis);
                            let completed_beats = elapsed / beat_millis;
                            let next_bar_beat = completed_beats
                                .checked_div(4)
                                .unwrap_or_default()
                                .saturating_add(1)
                                .saturating_mul(4);
                            transport
                                .phase_origin_millis
                                .saturating_add(next_bar_beat.saturating_mul(beat_millis))
                                .max(now_millis.saturating_add(1))
                        }
                    }
                });
                if now_millis < boundary {
                    return Ok((Vec::new(), false));
                }
                effective_now.saturating_sub(boundary)
            }
            _ => effective_now
                .saturating_sub(instance.started_at_millis)
                .saturating_sub(instance.paused_elapsed_millis)
                .saturating_sub(instance.speed_paused_elapsed_millis),
        };
        let definition = Arc::clone(&instance.definition);
        let speed = (f64::from(winning.speed_multiplier)
            * definition.overall_speed_multiplier.factor())
        .max(f64::EPSILON);
        let elapsed = (elapsed as f64 * speed).round() as u64;
        let lifecycle_elapsed = match instance.activation_policy {
            crate::ActivationPolicy::NextBoundary => instance
                .pending_until_millis
                .map_or(0, |boundary| effective_now.saturating_sub(boundary)),
            crate::ActivationPolicy::StartNow | crate::ActivationPolicy::JoinSyncNow => {
                effective_now
                    .saturating_sub(instance.started_at_millis)
                    .saturating_sub(instance.paused_elapsed_millis)
                    .saturating_sub(instance.speed_paused_elapsed_millis)
            }
        };
        let lifecycle_elapsed = (lifecycle_elapsed as f64 * speed).round() as u64;
        if definition.run_mode == crate::DynamicRunMode::OneShot
            && lifecycle_elapsed >= cycle_duration_millis
        {
            return Ok((Vec::new(), true));
        }
        let evaluator = DynamicEvaluator::new(&definition);
        let mut samples = Vec::new();
        let targets = instance.targets.clone();
        let random_phase_by_target = matches!(
            definition.phase.ordering,
            crate::PhaseOrdering::RandomEachLoop { .. }
        )
        .then(|| {
            project_phase(
                &definition.phase,
                &targets,
                &HashMap::new(),
                elapsed / cycle_duration_millis.max(1),
            )
            .into_iter()
            .map(|phase| (phase.target, phase.degrees))
            .collect::<HashMap<_, _>>()
        });
        for controller in controllers {
            if controller.size == 0.0 {
                continue;
            }
            let transition = instance
                .controller_transitions
                .get(&controller.id)
                .copied()
                .unwrap_or(DynamicControllerTransitionSnapshot {
                    controller_id: controller.id,
                    activation_started_at_millis: controller.activated_at_millis,
                    ..Default::default()
                });
            let activation_mix = transition_mix(transition, now_millis);
            for target in &targets {
                let phase = random_phase_by_target
                    .as_ref()
                    .and_then(|phases| phases.get(target))
                    .or_else(|| instance.phase_by_target.get(target))
                    .copied()
                    .unwrap_or(0.0)
                    + controller.phase_offset_degrees;
                for lane in &definition.lanes {
                    let random_envelope = lane.random_group_id.and_then(|group_id| {
                        definition
                            .random_groups
                            .iter()
                            .find(|group| group.id == group_id)
                            .map(|group| {
                                let stream = instance
                                    .random_streams
                                    .entry((group_id, *target))
                                    .or_default();
                                random_envelope(
                                    stream,
                                    group,
                                    instance_id,
                                    *target,
                                    elapsed,
                                    random_group_speed_factor(&definition, group_id),
                                    output_interval_millis,
                                )
                            })
                    });
                    let Some(value) = evaluator.sample_lane(
                        lane,
                        DynamicEvaluationContext {
                            instance_id,
                            target: *target,
                            elapsed_millis: elapsed,
                            cycle_duration_millis,
                            phase_degrees: phase,
                            output_interval_millis,
                            random_envelope,
                            sources,
                        },
                    ) else {
                        continue;
                    };
                    let value = sources
                        .current(*target, &lane.attribute)
                        .map_or(value, |base| base + (value - base) * controller.size);
                    samples.push(DynamicRuntimeSample {
                        instance_id,
                        controller_id: controller.id,
                        target: *target,
                        lane_id: lane.id,
                        attribute: lane.attribute.clone(),
                        value,
                        priority: controller.priority,
                        activated_at_millis: controller.activated_at_millis,
                        activation_mix,
                    });
                }
            }
        }
        Ok((samples, false))
    }

    /// Samples every active instance at one authoritative output timestamp.
    pub fn sample_all(
        &mut self,
        now_millis: u64,
        output_interval_millis: u64,
        speed_groups: &[DynamicSpeedTransport; 5],
        sources: &dyn ScalarSourceResolver,
    ) -> Vec<DynamicRuntimeSample> {
        self.remove_completed_releases(now_millis);
        let mut samples = Vec::new();
        let instances = self
            .instances
            .iter()
            .map(|(id, instance)| (*id, instance.definition.speed.clone()))
            .collect::<Vec<_>>();
        for (instance_id, speed) in instances {
            let transport = speed_group_transport(&speed, speed_groups);
            let cycle_duration_millis = cycle_duration(&speed, speed_groups);
            if let Ok((mut instance_samples, completed)) = self.sample_with_transport(
                instance_id,
                now_millis,
                cycle_duration_millis,
                output_interval_millis,
                transport,
                sources,
            ) {
                samples.append(&mut instance_samples);
                if completed {
                    self.complete_one_shot(instance_id);
                }
            }
        }
        samples
    }

    fn complete_one_shot(&mut self, instance_id: Uuid) {
        if let Some(instance) = self.instances.get_mut(&instance_id) {
            instance.completed = true;
        }
    }

    fn remove_completed_releases(&mut self, now_millis: u64) {
        let completed = self
            .instances
            .iter()
            .flat_map(|(instance_id, instance)| {
                instance
                    .controller_transitions
                    .values()
                    .filter(move |transition| {
                        transition.release_started_at_millis.is_some_and(|started| {
                            now_millis
                                >= started
                                    .saturating_add(transition.release_delay_millis)
                                    .saturating_add(transition.release_duration_millis)
                        })
                    })
                    .map(move |transition| (*instance_id, transition.controller_id))
            })
            .collect::<Vec<_>>();
        for (instance_id, controller_id) in completed {
            let _ = self.off_controller(instance_id, controller_id, now_millis, 0, 0);
        }
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

fn random_group_speed_factor(definition: &DynamicDefinition, group_id: Uuid) -> f64 {
    definition
        .lanes
        .iter()
        .find(|lane| lane.random_group_id == Some(group_id))
        .map_or(1.0, |lane| lane.speed_multiplier.factor())
}

fn random_envelope(
    state: &mut RandomStreamState,
    group: &crate::DynamicRandomGroup,
    instance_id: Uuid,
    target: FixtureId,
    elapsed_millis: u64,
    speed_factor: f64,
    output_interval_millis: u64,
) -> f32 {
    if elapsed_millis < state.last_elapsed_millis {
        *state = RandomStreamState::default();
    }
    let speed_factor = speed_factor.max(f64::EPSILON);
    let interval_millis = (group.decision_interval_millis as f64 / speed_factor)
        .round()
        .max(1.0) as u64;
    while state.next_decision_index.saturating_mul(interval_millis) <= elapsed_millis {
        let boundary = state.next_decision_index.saturating_mul(interval_millis);
        if state.active.is_some_and(|pulse| {
            pulse
                .started_at_millis
                .saturating_add(pulse.duration_millis)
                <= boundary
        }) {
            state.active = None;
        }
        if state.active.is_none()
            && crate::evaluate::uniform(group.seed, instance_id, target, state.next_decision_index)
                <= f64::from(group.start_probability)
        {
            let gaussian = crate::evaluate::gaussian(
                group.seed,
                instance_id,
                target,
                state.next_decision_index,
            );
            let duration_millis = ((group.mean_duration_millis as f64
                + gaussian * group.duration_spread_millis as f64)
                / speed_factor)
                .round()
                .max(output_interval_millis.max(1) as f64) as u64;
            state.active = Some(RandomPulse {
                started_at_millis: boundary,
                duration_millis,
            });
        }
        state.next_decision_index = state.next_decision_index.saturating_add(1);
    }
    state.last_elapsed_millis = elapsed_millis;
    let Some(pulse) = state.active else {
        return 0.0;
    };
    let end = pulse
        .started_at_millis
        .saturating_add(pulse.duration_millis);
    if elapsed_millis >= end {
        state.active = None;
        return 0.0;
    }
    let progress = elapsed_millis.saturating_sub(pulse.started_at_millis) as f32
        / pulse.duration_millis.max(1) as f32;
    if group.attack_ratio > 0.0 && progress < group.attack_ratio {
        progress / group.attack_ratio
    } else if group.decay_ratio > 0.0 && progress > 1.0 - group.decay_ratio {
        ((1.0 - progress) / group.decay_ratio).clamp(0.0, 1.0)
    } else {
        1.0
    }
}

fn speed_group_transport(
    speed: &crate::DynamicSpeed,
    speed_groups: &[DynamicSpeedTransport; 5],
) -> Option<DynamicSpeedTransport> {
    let crate::DynamicSpeed::SpeedGroup { group, .. } = speed else {
        return None;
    };
    Some(speed_groups[speed_group_index(*group)])
}

fn speed_group_index(group: crate::SpeedGroup) -> usize {
    match group {
        crate::SpeedGroup::A => 0,
        crate::SpeedGroup::B => 1,
        crate::SpeedGroup::C => 2,
        crate::SpeedGroup::D => 3,
        crate::SpeedGroup::E => 4,
    }
}

fn cycle_duration(speed: &crate::DynamicSpeed, speed_groups: &[DynamicSpeedTransport; 5]) -> u64 {
    match speed {
        crate::DynamicSpeed::Fixed { duration_millis } => (*duration_millis).max(1),
        crate::DynamicSpeed::SpeedGroup {
            group,
            beats_per_cycle,
        } => {
            let bpm = speed_groups[speed_group_index(*group)]
                .effective_bpm
                .max(f64::EPSILON);
            (60_000.0 / bpm * beats_per_cycle.factor()).round().max(1.0) as u64
        }
    }
}

fn validate_controller(controller: &DynamicController) -> Result<(), DynamicRuntimeError> {
    if !controller.size.is_finite()
        || controller.size < 0.0
        || !controller.speed_multiplier.is_finite()
        || controller.speed_multiplier <= 0.0
        || !controller.phase_offset_degrees.is_finite()
    {
        return Err(DynamicRuntimeError::InvalidController);
    }
    Ok(())
}

fn transition_mix(transition: DynamicControllerTransitionSnapshot, now_millis: u64) -> f32 {
    if let Some(release_started) = transition.release_started_at_millis {
        let release_elapsed = now_millis
            .saturating_sub(release_started)
            .saturating_sub(transition.release_delay_millis);
        if now_millis < release_started.saturating_add(transition.release_delay_millis) {
            return 1.0;
        }
        if transition.release_duration_millis == 0 {
            return 0.0;
        }
        return (1.0 - release_elapsed as f32 / transition.release_duration_millis as f32)
            .clamp(0.0, 1.0);
    }
    if now_millis
        < transition
            .activation_started_at_millis
            .saturating_add(transition.activation_delay_millis)
    {
        return 0.0;
    }
    if transition.activation_duration_millis == 0 {
        return 1.0;
    }
    let elapsed = now_millis
        .saturating_sub(transition.activation_started_at_millis)
        .saturating_sub(transition.activation_delay_millis);
    (elapsed as f32 / transition.activation_duration_millis as f32).clamp(0.0, 1.0)
}

fn winning_controller(instance: &DynamicInstance) -> Option<&DynamicController> {
    instance.controllers.values().max_by_key(|controller| {
        (
            controller.priority,
            controller.activated_at_millis,
            controller.id,
        )
    })
}

fn reconcile_pause(instance: &mut DynamicInstance, global_paused: bool, now_millis: u64) {
    let paused =
        global_paused || winning_controller(instance).is_some_and(|controller| controller.paused);
    match (paused, instance.paused_at_millis) {
        (true, None) => instance.paused_at_millis = Some(now_millis),
        (false, Some(paused_at)) => {
            instance.paused_elapsed_millis = instance
                .paused_elapsed_millis
                .saturating_add(now_millis.saturating_sub(paused_at));
            instance.paused_at_millis = None;
        }
        _ => {}
    }
}
