//! Server-authoritative Dynamic instance and FAT application operations.

mod conversion;
mod helpers;

use crate::{ActionContext, ActionError, ActionErrorKind};
use conversion::{factor_rational, runtime_error};
use helpers::*;
use light_core::{AttributeKey, AttributeValue, FixtureId, SessionId, UserId};
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
    Release(DynamicReleaseCommand),
}

#[derive(Clone, Debug)]
enum DynamicsReplayOutcome {
    Start(DynamicStartOutcome),
    OptionalStart(Option<DynamicStartOutcome>),
    Unit,
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
    fn snapshot(&self) -> Arc<EngineSnapshot>;
    fn now_millis(&self) -> u64;
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
                    .off_runtime_controller(
                        controller_id,
                        ports.now_millis(),
                        command.timing.delay_millis.unwrap_or_default(),
                        command.timing.fade_millis.unwrap_or_default(),
                    )
                    .map_err(runtime_error)?
                    .0
            };
            store_off(
                &self.programmers,
                identity.session,
                controller_id,
                command.timing,
            )?;
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
                .off_runtime_controller(
                    controller_id,
                    ports.now_millis(),
                    command.timing.delay_millis.unwrap_or_default(),
                    command.timing.fade_millis.unwrap_or_default(),
                )
                .map_err(runtime_error)?
                .0
        };
        store_off(
            &self.programmers,
            identity.session,
            controller_id,
            command.timing,
        )?;
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
        let state = self.programmers.get(identity.session).ok_or_else(|| {
            ActionError::new(ActionErrorKind::NotFound, "Programmer is unavailable")
        })?;
        if state.user_id != identity.user {
            return Err(ActionError::new(
                ActionErrorKind::Forbidden,
                "the Programmer session belongs to another user",
            ));
        }
        let controller_id = Uuid::new_v4();
        let now_millis = ports.now_millis();
        let controller = DynamicController {
            id: controller_id,
            source: DynamicControllerSource::Programmer {
                programmer_id: state.id.0,
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
                        attribute: lane.attribute.clone(),
                        value: DynamicSemanticValue::DynamicOn {
                            instance_link: controller_id,
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
        let targets =
            controller_targets(&self.programmers, identity.session, command.controller_id);
        if targets.is_empty() {
            return Err(ActionError::new(
                ActionErrorKind::NotFound,
                "Dynamic controller is not present in this Programmer",
            ));
        }
        let preload = programmer_preload_active(&self.programmers, identity.session);
        let dynamic_id =
            controller_dynamic_id(&self.programmers, identity.session, command.controller_id);
        let runtime_instance_id = if preload {
            command.controller_id
        } else {
            ports
                .off_runtime_controller(
                    command.controller_id,
                    ports.now_millis(),
                    command.timing.delay_millis.unwrap_or_default(),
                    command.timing.fade_millis.unwrap_or_default(),
                )
                .map_err(runtime_error)?
                .0
        };
        store_off(
            &self.programmers,
            identity.session,
            command.controller_id,
            command.timing,
        )?;
        let outcome = DynamicStartOutcome {
            runtime_instance_id,
            controller_id: command.controller_id,
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
        let mut source_values = state
            .dynamic_values
            .iter()
            .map(|stored| {
                (
                    (stored.fixture_id, stored.attribute.clone()),
                    stored.clone(),
                )
            })
            .collect::<HashMap<_, _>>();
        if preload {
            for stored in state.preload_dynamic_pending.iter() {
                source_values.insert(
                    (stored.fixture_id, stored.attribute.clone()),
                    stored.clone(),
                );
            }
        }
        let mutations = source_values
            .values()
            .filter_map(|stored| match &stored.value {
                DynamicSemanticValue::DynamicOn {
                    instance_link,
                    dynamic,
                    lane_id,
                    overrides,
                    timing,
                } if *instance_link == command.controller_id => {
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
                    command.controller_id,
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
                dynamic_id: controller_dynamic_id(
                    &self.programmers,
                    identity.session,
                    command.controller_id,
                ),
                runtime_instance_id: None,
                controller_id: Some(command.controller_id),
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

    pub fn fix_at(
        &self,
        context: &ActionContext,
        command: DynamicFixAtCommand,
        ports: &dyn DynamicsPorts,
    ) -> Result<(), ActionError> {
        let identity = identity(context)?;
        ports.authorize(context)?;
        let replay_action = DynamicsReplayAction::FixAt(command.clone());
        if let Some(DynamicsReplayOutcome::Unit) =
            self.cached(context, identity.session, &replay_action)?
        {
            return Ok(());
        }
        if !command.value.is_finite() {
            return Err(ActionError::new(
                ActionErrorKind::Invalid,
                "FixAT value must be finite",
            ));
        }
        let targets = if command.targets.is_empty() {
            self.programmers
                .selection(identity.session)
                .map(|selection| selection.selected)
                .unwrap_or_default()
        } else {
            command.targets
        };
        if targets.is_empty() {
            return Err(ActionError::new(
                ActionErrorKind::Invalid,
                "FixAT requires a fixture selection",
            ));
        }
        validate_fix_at_targets(&ports.snapshot(), &targets, &command.attribute)?;
        let mutations = targets
            .into_iter()
            .map(|fixture_id| DynamicProgrammerValueMutation::Set {
                fixture_id,
                attribute: command.attribute.clone(),
                value: DynamicSemanticValue::FixAt {
                    value: command.value,
                    timing: command.timing,
                },
            })
            .collect::<Vec<_>>();
        if !self
            .programmers
            .apply_dynamic_values(identity.session, &mutations, None)
        {
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                "FixAT produced no Programmer change",
            ));
        }
        self.remember(
            context,
            identity.session,
            replay_action,
            DynamicsReplayOutcome::Unit,
        );
        Ok(())
    }

    pub fn fix_at_batch(
        &self,
        context: &ActionContext,
        command: DynamicFixAtBatchCommand,
        ports: &dyn DynamicsPorts,
    ) -> Result<usize, ActionError> {
        let identity = identity(context)?;
        ports.authorize(context)?;
        let replay_action = DynamicsReplayAction::FixAtBatch(command.clone());
        if let Some(DynamicsReplayOutcome::Unit) =
            self.cached(context, identity.session, &replay_action)?
        {
            return Ok(command.values.len());
        }
        if command.values.is_empty() {
            return Err(ActionError::new(
                ActionErrorKind::Invalid,
                "FixAT Preset contains no applicable scalar values",
            ));
        }
        let snapshot = ports.snapshot();
        for value in &command.values {
            validate_release_targets(
                &snapshot,
                &[ReleaseProgrammerFixtureValue {
                    fixture_id: value.fixture_id,
                    attribute: value.attribute.clone(),
                }],
            )?;
        }
        let mutations = command
            .values
            .iter()
            .map(|value| DynamicProgrammerValueMutation::Set {
                fixture_id: value.fixture_id,
                attribute: value.attribute.clone(),
                value: DynamicSemanticValue::Static {
                    value: value.value.clone(),
                    timing: command.timing,
                },
            })
            .collect::<Vec<_>>();
        if !self
            .programmers
            .apply_dynamic_values(identity.session, &mutations, None)
        {
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                "FixAT Preset produced no Programmer change",
            ));
        }
        self.remember(
            context,
            identity.session,
            replay_action,
            DynamicsReplayOutcome::Unit,
        );
        Ok(command.values.len())
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
        if command.fixture_values.is_empty() {
            return Err(ActionError::new(
                ActionErrorKind::Invalid,
                "RELEASE requires at least one supported fixture attribute",
            ));
        }
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
