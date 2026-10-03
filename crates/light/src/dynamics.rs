//! Server-authoritative Dynamic instance and FAT application operations.

mod conversion;
mod fix_at;
mod helpers;
mod legacy_addresses;
mod programmer_checkpoint;
pub use helpers::{
    ProgrammerDynamicController, effective_programmer_dynamic_controllers,
    resolve_programmer_dynamic_controller,
};
pub use programmer_checkpoint::normalize_programmer_dynamic_checkpoint;
#[cfg(test)]
mod controller_tests;
mod preset_sources;
pub use fix_at::{DynamicFixAtCaptureCommand, DynamicFixAtEnvironment};
pub use preset_sources::{
    CompiledDynamicPresetSources, DynamicPresetSourceIssue, DynamicPresetSourceIssueReason,
    compile_dynamic_preset_sources, compile_runtime_dynamic_preset_sources,
};

use crate::{ActionContext, ActionError, ActionErrorKind};
use conversion::{factor_rational, runtime_error};
use helpers::*;
use light_core::{AttributeKey, AttributeValue, FixtureId, SessionId};
use light_dynamics::{
    DynamicController, DynamicControllerSource, DynamicDefinition, DynamicDefinitionSnapshot,
    DynamicInstanceOverrides, DynamicReference, DynamicRuntimeError, DynamicSemanticValue,
    DynamicStartRequest, DynamicTargetBinding, DynamicTargetScope, DynamicValueTiming, Position3d,
    SpatialSelectionMapping,
};
use light_engine::EngineSnapshot;
use light_programmer::{
    DynamicProgrammerValueMutation, ProgrammerRegistry, ReleaseProgrammerFixtureValue,
    ReleaseProgrammerGroupValue, resolve_group_spatial,
};
use parking_lot::Mutex;
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
pub struct DynamicStartCommand {
    pub dynamic_id: Uuid,
    /// Explicit ordered target scope for a targetless Dynamic. A target-bound Dynamic always
    /// resolves its stored Group or frozen targets instead.
    pub targets: Vec<FixtureId>,
    pub overrides: DynamicInstanceOverrides,
    pub timing: DynamicValueTiming,
    /// Optional identity shared by a deliberate sequence of Dynamic starts. The first start
    /// creates one Programmer undo checkpoint and later starts with the same identity extend it.
    pub undo_group: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DynamicStartOutcome {
    pub runtime_instance_id: Uuid,
    pub controller_id: Uuid,
    pub targets: Vec<FixtureId>,
    pub started: bool,
}

struct ResolvedDynamicStart<'a> {
    identity: DynamicsIdentity,
    definition: &'a DynamicDefinition,
    targets: Vec<FixtureId>,
    stage_positions: HashMap<FixtureId, light_dynamics::SpatialPosition>,
    inherited_spatial_mapping: Option<SpatialSelectionMapping>,
    command: DynamicStartCommand,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DynamicOffCommand {
    pub controller_id: Uuid,
    pub timing: DynamicValueTiming,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DynamicControllerUpdate {
    pub controller_id: Uuid,
    pub size: Option<f32>,
    pub speed_multiplier: Option<f32>,
    pub phase_offset_degrees: Option<f32>,
    pub undo_group: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DynamicFixAtCommand {
    pub targets: Vec<FixtureId>,
    pub attribute: AttributeKey,
    pub value: f32,
    pub timing: DynamicValueTiming,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DynamicFixAtValue {
    pub fixture_id: FixtureId,
    pub attribute: AttributeKey,
    pub value: AttributeValue,
    /// Explicit whole/component mask. Rich families without this field become whole-family
    /// masks; a typed Focus mask must set it because its payload is also a legacy scalar.
    pub programming_mask: Option<light_dynamics::DynamicValueAddress>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DynamicFixAtBatchCommand {
    pub values: Vec<DynamicFixAtValue>,
    pub timing: DynamicValueTiming,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DynamicReleaseCommand {
    pub fixture_values: Vec<ReleaseProgrammerFixtureValue>,
    pub group_values: Vec<ReleaseProgrammerGroupValue>,
}

const DYNAMICS_REPLAY_LIMIT: usize = 1_024;

#[derive(Clone, Debug, PartialEq)]
enum DynamicsReplayAction {
    Toggle(DynamicStartCommand),
    Start(DynamicStartCommand),
    OffMatching(DynamicStartCommand),
    Off(DynamicOffCommand),
    Update(DynamicControllerUpdate),
    FixAt(DynamicFixAtCommand),
    FixAtBatch(DynamicFixAtBatchCommand),
    FixAtCapture(DynamicFixAtCaptureCommand),
    Release(DynamicReleaseCommand),
}

#[derive(Clone, Debug)]
enum DynamicsReplayOutcome {
    Start(DynamicStartOutcome),
    OptionalStart(Option<DynamicStartOutcome>),
    Unit,
    Applied(usize),
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct DynamicsReplayKey {
    desk_id: Uuid,
    session_id: Uuid,
    request_id: String,
}

struct DynamicsReplayEntry {
    action: DynamicsReplayAction,
    outcome: DynamicsReplayOutcome,
}

#[derive(Default)]
struct DynamicsReplayCache {
    entries: HashMap<DynamicsReplayKey, DynamicsReplayEntry>,
    order: VecDeque<DynamicsReplayKey>,
}

pub trait DynamicsPorts: Send + Sync {
    fn authorize(&self, context: &ActionContext) -> Result<(), ActionError>;
    fn supported_programming_contract(&self) -> u16 {
        light_core::programming::PROGRAMMING_CONTRACT_VERSION
    }
    fn snapshot(&self) -> Arc<EngineSnapshot>;
    /// One coherent composed intent frame, including live Target adoption inputs. This must
    /// not read fitted fixture channels or sample each target at a different instant.
    fn fix_at_environment(
        &self,
        _context: &ActionContext,
        _targets: &[FixtureId],
        _owner: light_core::programming::ProgrammingOwner,
    ) -> Result<DynamicFixAtEnvironment, ActionError> {
        Err(ActionError::new(
            ActionErrorKind::Unavailable,
            "coherent FixAT capture is unavailable",
        ))
    }
    /// Immutable original source model; never the selected destination fixture's model.
    fn fix_at_native_model(
        &self,
        _source: &light_core::NativeColorIdentity,
    ) -> Result<Arc<dyn light_core::programming::NativeColorEditModel + Send + Sync>, ActionError>
    {
        Err(ActionError::new(
            ActionErrorKind::Unavailable,
            "original Color source is unavailable",
        ))
    }
    fn now_millis(&self) -> u64;
    /// Off authors its source edit first. The next captured frame decides whether to mute a
    /// retained underlying On or retire an absent source; a response lookup must not stop it.
    fn runtime_controller_instance(&self, controller_id: Uuid) -> Option<Uuid>;
    /// Reconcile a successfully stored Live edit before publishing its event/checkpoint.
    fn reconcile_programmer_runtime(&self);
    fn runtime_controller_is_completed(&self, controller_id: Uuid) -> bool;
    fn start_runtime(&self, request: DynamicStartRequest) -> Result<Uuid, DynamicRuntimeError>;
    fn off_runtime_controller(
        &self,
        controller_id: Uuid,
        now_millis: u64,
        release_delay_millis: u64,
        release_duration_millis: u64,
    ) -> Result<(Uuid, bool), DynamicRuntimeError>;
    fn update_runtime_controller(
        &self,
        controller_id: Uuid,
        size: Option<f32>,
        speed_multiplier: Option<f32>,
        phase_offset_degrees: Option<f32>,
    ) -> Result<(), DynamicRuntimeError>;
    fn publish_runtime_change(&self, context: &ActionContext, change: crate::DynamicRuntimeChange);
}

#[derive(Clone)]
pub struct DynamicsService {
    programmers: ProgrammerRegistry,
    replay: Arc<Mutex<DynamicsReplayCache>>,
}

impl DynamicsService {
    pub fn new(programmers: ProgrammerRegistry) -> Self {
        Self {
            programmers,
            replay: Arc::default(),
        }
    }

    pub fn toggle(
        &self,
        context: &ActionContext,
        command: DynamicStartCommand,
        ports: &dyn DynamicsPorts,
    ) -> Result<DynamicStartOutcome, ActionError> {
        let identity = identity(context)?;
        ports.authorize(context)?;
        let replay_action = DynamicsReplayAction::Toggle(command.clone());
        if let Some(DynamicsReplayOutcome::Start(outcome)) =
            self.cached(context, identity.session, &replay_action)?
        {
            return Ok(outcome);
        }
        let snapshot = ports.snapshot();
        let (definition, targets, inherited_spatial_mapping) = definition_and_targets(
            context,
            ports,
            &self.programmers,
            identity.session,
            &snapshot,
            command.dynamic_id,
            &command.targets,
        )?;
        let stage_positions = snapshot.dynamic_stage_positions.as_ref().clone();
        if let Some(controller_id) = matching_programmer_controller(
            &self.programmers,
            identity.session,
            definition.id,
            &targets,
        ) && !ports.runtime_controller_is_completed(controller_id)
        {
            let preload = programmer_preload_active(&self.programmers, identity.session);
            let runtime_instance_id = if preload {
                controller_id
            } else {
                ports
                    .runtime_controller_instance(controller_id)
                    .unwrap_or(controller_id)
            };
            store_off(
                &self.programmers,
                identity.session,
                controller_id,
                command.timing,
            )?;
            if !preload {
                ports.reconcile_programmer_runtime();
            }
            let outcome = DynamicStartOutcome {
                runtime_instance_id,
                controller_id,
                targets,
                started: false,
            };
            publish_off_events(
                context,
                ports,
                Some(definition.id),
                &outcome,
                command.timing,
                preload,
            );
            self.remember(
                context,
                identity.session,
                replay_action,
                DynamicsReplayOutcome::Start(outcome.clone()),
            );
            return Ok(outcome);
        }
        let outcome = self.start_resolved(
            context,
            ResolvedDynamicStart {
                identity,
                definition,
                targets,
                stage_positions,
                inherited_spatial_mapping,
                command,
            },
            ports,
        )?;
        self.remember(
            context,
            identity.session,
            replay_action,
            DynamicsReplayOutcome::Start(outcome.clone()),
        );
        Ok(outcome)
    }

    pub fn start(
        &self,
        context: &ActionContext,
        command: DynamicStartCommand,
        ports: &dyn DynamicsPorts,
    ) -> Result<DynamicStartOutcome, ActionError> {
        let identity = identity(context)?;
        ports.authorize(context)?;
        let replay_action = DynamicsReplayAction::Start(command.clone());
        if let Some(DynamicsReplayOutcome::Start(outcome)) =
            self.cached(context, identity.session, &replay_action)?
        {
            return Ok(outcome);
        }
        let snapshot = ports.snapshot();
        let (definition, targets, inherited_spatial_mapping) = definition_and_targets(
            context,
            ports,
            &self.programmers,
            identity.session,
            &snapshot,
            command.dynamic_id,
            &command.targets,
        )?;
        let stage_positions = snapshot.dynamic_stage_positions.as_ref().clone();
        let outcome = self.start_resolved(
            context,
            ResolvedDynamicStart {
                identity,
                definition,
                targets,
                stage_positions,
                inherited_spatial_mapping,
                command,
            },
            ports,
        )?;
        self.remember(
            context,
            identity.session,
            replay_action,
            DynamicsReplayOutcome::Start(outcome.clone()),
        );
        Ok(outcome)
    }

    /// Stops the Programmer controller matching one Dynamic and its currently resolved scope.
    ///
    /// This is the idempotent pool-level Off operation used by OSC and operator pool surfaces:
    /// unlike `toggle`, an absent match never starts a new instance.
    pub fn off_matching(
        &self,
        context: &ActionContext,
        command: DynamicStartCommand,
        ports: &dyn DynamicsPorts,
    ) -> Result<Option<DynamicStartOutcome>, ActionError> {
        let identity = identity(context)?;
        ports.authorize(context)?;
        let replay_action = DynamicsReplayAction::OffMatching(command.clone());
        if let Some(DynamicsReplayOutcome::OptionalStart(outcome)) =
            self.cached(context, identity.session, &replay_action)?
        {
            return Ok(outcome);
        }
        let snapshot = ports.snapshot();
        let (definition, targets, _) = definition_and_targets(
            context,
            ports,
            &self.programmers,
            identity.session,
            &snapshot,
            command.dynamic_id,
            &command.targets,
        )?;
        let Some(controller_id) = matching_programmer_controller(
            &self.programmers,
            identity.session,
            definition.id,
            &targets,
        ) else {
            self.remember(
                context,
                identity.session,
                replay_action,
                DynamicsReplayOutcome::OptionalStart(None),
            );
            return Ok(None);
        };
        let preload = programmer_preload_active(&self.programmers, identity.session);
        let runtime_instance_id = if preload {
            controller_id
        } else {
            ports
                .runtime_controller_instance(controller_id)
                .unwrap_or(controller_id)
        };
        store_off(
            &self.programmers,
            identity.session,
            controller_id,
            command.timing,
        )?;
        if !preload {
            ports.reconcile_programmer_runtime();
        }
        let outcome = Some(DynamicStartOutcome {
            runtime_instance_id,
            controller_id,
            targets,
            started: false,
        });
        if let Some(outcome) = &outcome {
            publish_off_events(
                context,
                ports,
                Some(definition.id),
                outcome,
                command.timing,
                preload,
            );
        }
        self.remember(
            context,
            identity.session,
            replay_action,
            DynamicsReplayOutcome::OptionalStart(outcome.clone()),
        );
        Ok(outcome)
    }

    fn start_resolved(
        &self,
        context: &ActionContext,
        request: ResolvedDynamicStart<'_>,
        ports: &dyn DynamicsPorts,
    ) -> Result<DynamicStartOutcome, ActionError> {
        let ResolvedDynamicStart {
            identity,
            definition,
            targets,
            stage_positions,
            inherited_spatial_mapping,
            command,
        } = request;
        if definition.required_programming_contract() > ports.supported_programming_contract() {
            return Err(ActionError::new(
                ActionErrorKind::Invalid,
                "This Dynamic requires a programming contract that is not active on this desk",
            ));
        }
        let state = self.programmers.get(identity.session).ok_or_else(|| {
            ActionError::new(ActionErrorKind::NotFound, "Programmer is unavailable")
        })?;
        let authored_link = Uuid::new_v4();
        let controller_id =
            light_dynamics::programmer_dynamic_controller_id(state.id, authored_link);
        let now_millis = ports.now_millis();
        let controller = DynamicController {
            id: controller_id,
            source: DynamicControllerSource::Programmer {
                programmer_id: state.id.0,
                instance_link: Some(authored_link),
            },
            priority: state.priority,
            activated_at_millis: now_millis,
            size: command.overrides.size,
            speed_multiplier: command.overrides.speed_multiplier.factor() as f32,
            phase_offset_degrees: command.overrides.phase_offset_degrees,
            paused: false,
        };
        let preload = state.blind && state.preload_capture_programmer;
        let runtime_instance_id = if preload {
            // Preload owns a projected controller identity, but Live runtime/output must not
            // change until GO publishes the pending layer.
            controller_id
        } else {
            match ports.start_runtime(DynamicStartRequest {
                definition_id: definition.id,
                controller,
                target_scope: DynamicTargetScope {
                    ordered_targets: targets.clone(),
                },
                stage_positions,
                inherited_spatial_mapping,
                now_millis,
                activation_delay_millis: command.timing.delay_millis.unwrap_or_default(),
                activation_duration_millis: command.timing.fade_millis.unwrap_or_default(),
                activation_policy_override: None,
                reuse_matching_targetless: true,
            }) {
                Ok(instance_id) => instance_id,
                Err(error) => {
                    let message = error.to_string();
                    ports.publish_runtime_change(
                        context,
                        crate::DynamicRuntimeChange {
                            kind: crate::DynamicRuntimeEventKind::FailedDependency,
                            dynamic_id: Some(definition.id),
                            runtime_instance_id: None,
                            controller_id: Some(controller_id),
                            winning_controller_id: None,
                            occurred_at_millis: now_millis,
                            message: Some(message),
                        },
                    );
                    return Err(runtime_error(error));
                }
            }
        };
        let reference = DynamicReference {
            dynamic_id: Some(definition.id),
            last_known_pool_number: definition.pool_number,
            embedded_fallback: DynamicDefinitionSnapshot {
                definition: Arc::new(definition.clone()),
            },
        };
        let mutations = targets
            .iter()
            .flat_map(|fixture_id| {
                definition
                    .lanes
                    .iter()
                    .map(|lane| DynamicProgrammerValueMutation::Set {
                        fixture_id: *fixture_id,
                        attribute: lane.output_owner(),
                        value: DynamicSemanticValue::DynamicOn {
                            instance_link: authored_link,
                            dynamic: reference.clone(),
                            lane_id: lane.id,
                            overrides: command.overrides.clone(),
                            timing: command.timing,
                        },
                    })
            })
            .collect::<Vec<_>>();
        if !self.programmers.apply_dynamic_values(
            identity.session,
            &mutations,
            command.undo_group.as_deref(),
        ) {
            if !preload {
                let _ = ports.off_runtime_controller(controller_id, now_millis, 0, 0);
            }
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                "Dynamic start produced no Programmer change",
            ));
        }
        let outcome = DynamicStartOutcome {
            runtime_instance_id,
            controller_id,
            targets,
            started: true,
        };
        publish_start_events(context, ports, definition.id, &outcome, preload);
        Ok(outcome)
    }

    pub fn off(
        &self,
        context: &ActionContext,
        command: DynamicOffCommand,
        ports: &dyn DynamicsPorts,
    ) -> Result<DynamicStartOutcome, ActionError> {
        let identity = identity(context)?;
        ports.authorize(context)?;
        let replay_action = DynamicsReplayAction::Off(command.clone());
        if let Some(DynamicsReplayOutcome::Start(outcome)) =
            self.cached(context, identity.session, &replay_action)?
        {
            return Ok(outcome);
        }
        let state = self.programmers.get(identity.session).ok_or_else(|| {
            ActionError::new(ActionErrorKind::NotFound, "Programmer is unavailable")
        })?;
        let controller = resolve_programmer_dynamic_controller(&state, command.controller_id)
            .ok_or_else(|| {
                ActionError::new(
                    ActionErrorKind::NotFound,
                    "Dynamic controller is not present in this Programmer",
                )
            })?;
        let controller_id = controller.controller_id;
        let targets = controller.targets;
        let preload = programmer_preload_active(&self.programmers, identity.session);
        let dynamic_id = controller.dynamic_id;
        let runtime_instance_id = if preload {
            controller_id
        } else {
            ports
                .runtime_controller_instance(controller_id)
                .unwrap_or(controller_id)
        };
        store_off(
            &self.programmers,
            identity.session,
            controller_id,
            command.timing,
        )?;
        if !preload {
            ports.reconcile_programmer_runtime();
        }
        let outcome = DynamicStartOutcome {
            runtime_instance_id,
            controller_id,
            targets,
            started: false,
        };
        publish_off_events(
            context,
            ports,
            dynamic_id,
            &outcome,
            command.timing,
            preload,
        );
        self.remember(
            context,
            identity.session,
            replay_action,
            DynamicsReplayOutcome::Start(outcome.clone()),
        );
        Ok(outcome)
    }

    pub fn update_controller(
        &self,
        context: &ActionContext,
        command: DynamicControllerUpdate,
        ports: &dyn DynamicsPorts,
    ) -> Result<(), ActionError> {
        let identity = identity(context)?;
        ports.authorize(context)?;
        let replay_action = DynamicsReplayAction::Update(command.clone());
        if let Some(DynamicsReplayOutcome::Unit) =
            self.cached(context, identity.session, &replay_action)?
        {
            return Ok(());
        }
        let state = self.programmers.get(identity.session).ok_or_else(|| {
            ActionError::new(ActionErrorKind::NotFound, "Programmer is unavailable")
        })?;
        let speed_rational = command.speed_multiplier.map(factor_rational).transpose()?;
        let preload = state.blind && state.preload_capture_programmer;
        let controller = resolve_programmer_dynamic_controller(&state, command.controller_id)
            .ok_or_else(|| {
                ActionError::new(
                    ActionErrorKind::NotFound,
                    "Dynamic controller is not present in this Programmer",
                )
            })?;
        let mutations = effective_programmer_dynamic_values(&state)
            .into_iter()
            .filter_map(|stored| match &stored.value {
                DynamicSemanticValue::DynamicOn {
                    instance_link,
                    dynamic,
                    lane_id,
                    overrides,
                    timing,
                } if *instance_link == controller.authored_link => {
                    let mut overrides = overrides.clone();
                    if let Some(size) = command.size {
                        overrides.size = size;
                    }
                    if let Some(speed) = speed_rational {
                        overrides.speed_multiplier = speed;
                    }
                    if let Some(phase) = command.phase_offset_degrees {
                        overrides.phase_offset_degrees = phase;
                    }
                    Some(Ok(DynamicProgrammerValueMutation::Set {
                        fixture_id: stored.fixture_id,
                        attribute: stored.attribute.clone(),
                        value: DynamicSemanticValue::DynamicOn {
                            instance_link: *instance_link,
                            dynamic: dynamic.clone(),
                            lane_id: *lane_id,
                            overrides,
                            timing: *timing,
                        },
                    }))
                }
                _ => None,
            })
            .collect::<Result<Vec<_>, ActionError>>()?;
        if mutations.is_empty() {
            return Err(ActionError::new(
                ActionErrorKind::NotFound,
                "Dynamic controller is not present in this Programmer",
            ));
        }
        if !preload {
            ports
                .update_runtime_controller(
                    controller.controller_id,
                    command.size,
                    command.speed_multiplier,
                    command.phase_offset_degrees,
                )
                .map_err(runtime_error)?;
        }
        if !self.programmers.apply_dynamic_values(
            identity.session,
            &mutations,
            command.undo_group.as_deref(),
        ) {
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                "Dynamic controller update produced no Programmer change",
            ));
        }
        ports.publish_runtime_change(
            context,
            crate::DynamicRuntimeChange {
                kind: crate::DynamicRuntimeEventKind::ControllerUpdated,
                dynamic_id: controller.dynamic_id,
                runtime_instance_id: None,
                controller_id: Some(controller.controller_id),
                winning_controller_id: None,
                occurred_at_millis: ports.now_millis(),
                message: preload.then(|| "staged in Preload".into()),
            },
        );
        self.remember(
            context,
            identity.session,
            replay_action,
            DynamicsReplayOutcome::Unit,
        );
        Ok(())
    }

    pub fn release_values(
        &self,
        context: &ActionContext,
        command: DynamicReleaseCommand,
        ports: &dyn DynamicsPorts,
    ) -> Result<usize, ActionError> {
        let identity = identity(context)?;
        ports.authorize(context)?;
        let replay_action = DynamicsReplayAction::Release(command.clone());
        if let Some(DynamicsReplayOutcome::Unit) =
            self.cached(context, identity.session, &replay_action)?
        {
            return Ok(command.fixture_values.len());
        }
        if command.fixture_values.is_empty() && command.group_values.is_empty() {
            self.remember(
                context,
                identity.session,
                replay_action,
                DynamicsReplayOutcome::Unit,
            );
            return Ok(0);
        }
        // TL-552 follow-up: at contract ≥ 1 a native Position/Color channel release is stored
        // as the release of its semantic owner, never at a legacy address.
        let command = legacy_addresses::semantic_release_command(
            &command,
            ports.supported_programming_contract(),
        );
        validate_release_targets(&ports.snapshot(), &command.fixture_values)?;
        self.programmers.apply_release_values(
            identity.session,
            &command.fixture_values,
            &command.group_values,
        );
        self.remember(
            context,
            identity.session,
            replay_action,
            DynamicsReplayOutcome::Unit,
        );
        Ok(command.fixture_values.len())
    }

    fn cached(
        &self,
        context: &ActionContext,
        session: SessionId,
        action: &DynamicsReplayAction,
    ) -> Result<Option<DynamicsReplayOutcome>, ActionError> {
        let Some(request_id) = context.request_id.as_ref() else {
            return Ok(None);
        };
        let key = DynamicsReplayKey {
            desk_id: context.desk_id,
            session_id: session.0,
            request_id: request_id.clone(),
        };
        let replay = self.replay.lock();
        let Some(entry) = replay.entries.get(&key) else {
            return Ok(None);
        };
        if &entry.action != action {
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                "request_id was already used for a different Dynamic action",
            ));
        }
        Ok(Some(entry.outcome.clone()))
    }

    fn remember(
        &self,
        context: &ActionContext,
        session: SessionId,
        action: DynamicsReplayAction,
        outcome: DynamicsReplayOutcome,
    ) {
        let Some(request_id) = context.request_id.as_ref() else {
            return;
        };
        let key = DynamicsReplayKey {
            desk_id: context.desk_id,
            session_id: session.0,
            request_id: request_id.clone(),
        };
        let mut replay = self.replay.lock();
        if !replay.entries.contains_key(&key) {
            replay.order.push_back(key.clone());
        }
        replay
            .entries
            .insert(key, DynamicsReplayEntry { action, outcome });
        while replay.entries.len() > DYNAMICS_REPLAY_LIMIT {
            if let Some(oldest) = replay.order.pop_front() {
                replay.entries.remove(&oldest);
            }
        }
    }
}
