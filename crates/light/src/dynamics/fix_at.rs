//! FixAT records complete intent separately from the component it holds. Capture is cold
//! application work; clocks and fixture fitting remain in the engine.
use super::*;
use light_core::programming::*;
use light_dynamics::{
    CompiledDynamicValueAddress, DynamicValue, DynamicValueAddress, ProgrammingFamilyFixAt,
};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq)]
pub struct DynamicFixAtCaptureCommand {
    pub targets: Vec<FixtureId>,
    pub owner: ProgrammingOwner,
    pub component: Option<ProgrammingComponent>,
    /// Empty means Current. Values are already materialized for this action; selection/group
    /// spread planning belongs before this boundary, as it does for preset batches.
    pub edits: Vec<ComponentEdit>,
    pub timing: DynamicValueTiming,
}

#[derive(Clone, Debug, Default)]
pub struct DynamicFixAtEnvironment {
    pub families: HashMap<FixtureId, AttributeValue>,
    pub contexts: HashMap<FixtureId, OwnedFamilyEditContext>,
}

impl DynamicFixAtValue {
    pub fn programming(fixture_id: FixtureId, mask: ProgrammingFamilyFixAt) -> Self {
        Self {
            fixture_id,
            attribute: mask.address.owner().key(),
            value: mask.family,
            programming_mask: Some(mask.address),
        }
    }

    fn mask(&self) -> Result<Option<ProgrammingFamilyFixAt>, ActionError> {
        let address = match &self.programming_mask {
            Some(address) => address.clone(),
            None => {
                let owner = match self.value {
                    AttributeValue::Position(_) => ProgrammingOwner::Position,
                    AttributeValue::ColorProgram(_) => ProgrammingOwner::Color,
                    AttributeValue::Zoom(_) => ProgrammingOwner::Zoom,
                    _ => return Ok(None),
                };
                DynamicValueAddress::whole_family(owner, &self.value).map_err(invalid)?
            }
        };
        if address.owner().key() != self.attribute {
            return Err(invalid("FixAT mask and attribute have different owners"));
        }
        let mask = ProgrammingFamilyFixAt {
            address,
            family: self.value.clone(),
        };
        mask.validate().map_err(invalid)?;
        Ok(Some(mask))
    }
}

impl DynamicsService {
    pub fn fix_at(
        &self,
        context: &ActionContext,
        command: DynamicFixAtCommand,
        ports: &dyn DynamicsPorts,
    ) -> Result<usize, ActionError> {
        let identity = identity(context)?;
        ports.authorize(context)?;
        let action = DynamicsReplayAction::FixAt(command.clone());
        if let Some(DynamicsReplayOutcome::Applied(count)) =
            self.cached(context, identity.session, &action)?
        {
            return Ok(count);
        }
        if !command.value.is_finite() {
            return Err(invalid("FixAT value must be finite"));
        }
        let targets = self.fix_at_targets(identity.session, &command.targets);
        if !targets.is_empty() {
            validate_fix_at_targets(&ports.snapshot(), &targets, &command.attribute)?;
        }
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
        super::legacy_addresses::refuse_legacy_dynamic_values(
            &mutations,
            ports.supported_programming_contract(),
        )?;
        self.commit_fix_at(context, identity.session, action, &mutations)
    }

    pub fn fix_at_batch(
        &self,
        context: &ActionContext,
        command: DynamicFixAtBatchCommand,
        ports: &dyn DynamicsPorts,
    ) -> Result<usize, ActionError> {
        let identity = identity(context)?;
        ports.authorize(context)?;
        let action = DynamicsReplayAction::FixAtBatch(command.clone());
        if let Some(DynamicsReplayOutcome::Applied(count)) =
            self.cached(context, identity.session, &action)?
        {
            return Ok(count);
        }
        // Empty selection is a no-op even on a runtime without the semantic contract.
        if command.values.is_empty() {
            return self.commit_fix_at(context, identity.session, action, &[]);
        }
        Self::validate_fix_at_contract(&command.values, ports.supported_programming_contract())?;
        validate_targeted_programming_entries(
            ProgrammingValueScope::Fixture,
            command
                .values
                .iter()
                .map(|value| (value.fixture_id, &value.attribute, &value.value)),
        )
        .map_err(invalid)?;
        let snapshot = ports.snapshot();
        let mut tracks = HashSet::new();
        let mut mutations = Vec::with_capacity(command.values.len());
        for value in &command.values {
            let mask = value.mask()?;
            let component = mask.as_ref().and_then(|mask| mask.address.component);
            if !tracks.insert((value.fixture_id, value.attribute.clone(), component)) {
                return Err(invalid("FixAT batch contains duplicate mask addresses"));
            }
            let semantic = if let Some(mask) = mask {
                validate_semantic_targets(&snapshot, &[value.fixture_id])?;
                let model =
                    if let light_dynamics::DynamicFamilyRepresentation::DirectColor { source } =
                        &mask.address.representation
                    {
                        Some(ports.fix_at_native_model(source)?)
                    } else {
                        None
                    };
                validate_mask_model(&mask, model)?;
                DynamicSemanticValue::ProgrammingFixAt {
                    mask,
                    timing: command.timing,
                }
            } else {
                value
                    .value
                    .validate_programming_address(&value.attribute)
                    .map_err(invalid)?;
                validate_release_targets(
                    &snapshot,
                    &[ReleaseProgrammerFixtureValue {
                        fixture_id: value.fixture_id,
                        attribute: value.attribute.clone(),
                    }],
                )?;
                DynamicSemanticValue::Static {
                    value: value.value.clone(),
                    timing: command.timing,
                }
            };
            mutations.push(DynamicProgrammerValueMutation::Set {
                fixture_id: value.fixture_id,
                attribute: value.attribute.clone(),
                value: semantic,
            });
        }
        super::legacy_addresses::refuse_legacy_dynamic_values(
            &mutations,
            ports.supported_programming_contract(),
        )?;
        self.commit_fix_at(context, identity.session, action, &mutations)
    }

    /// Current and typed component commands share this path. The adapter freezes all target
    /// intents/adoption inputs together; no ordinary static assignment is installed as a side effect.
    pub fn fix_at_capture(
        &self,
        context: &ActionContext,
        command: DynamicFixAtCaptureCommand,
        ports: &dyn DynamicsPorts,
    ) -> Result<usize, ActionError> {
        let identity = identity(context)?;
        ports.authorize(context)?;
        let action = DynamicsReplayAction::FixAtCapture(command.clone());
        if let Some(DynamicsReplayOutcome::Applied(count)) =
            self.cached(context, identity.session, &action)?
        {
            return Ok(count);
        }
        validate_component_edits(&command.edits).map_err(invalid)?;
        if let Some(component) = command.component {
            if match component {
                ProgrammingComponent::NativeColor(binding) => {
                    binding.channel_id.is_nil() || binding.function_id.is_nil()
                }
                _ => !component.descriptor().dynamics,
            } {
                return Err(invalid(
                    "FixAT component requires a continuous typed mask address",
                ));
            }
        }
        if command
            .component
            .is_some_and(|component| component.owner() != command.owner)
            || command
                .edits
                .iter()
                .any(|edit| edit.owner() != command.owner)
        {
            return Err(invalid("FixAT must address one programming owner"));
        }
        let targets = self.fix_at_targets(identity.session, &command.targets);
        if targets.is_empty() {
            return self.commit_fix_at(context, identity.session, action, &[]);
        }
        validate_contract(
            PROGRAMMING_CONTRACT_VERSION,
            ports.supported_programming_contract(),
        )?;
        validate_semantic_targets(&ports.snapshot(), &targets)?;
        let environment = ports.fix_at_environment(context, &targets, command.owner)?;
        let mut mutations = Vec::with_capacity(targets.len());
        for target in targets {
            let base = environment.families.get(&target).ok_or_else(|| {
                ActionError::new(
                    ActionErrorKind::Unavailable,
                    "complete Current intent is unavailable for FixAT",
                )
            })?;
            base.validate_programming_address(&command.owner.key())
                .map_err(invalid)?;
            let fallback = OwnedFamilyEditContext::default();
            let family_context = environment.contexts.get(&target).unwrap_or(&fallback);
            let mut edits = command.edits.clone();
            // Holding a joint while tracking adopts the solved unwrapped Angle pair once.
            if matches!(
                command.component,
                Some(ProgrammingComponent::Pan | ProgrammingComponent::Tilt)
            ) && matches!(base, AttributeValue::Position(position) if matches!(position.as_ref(), PositionIntent::Target { .. }))
                && !edits.contains(&ComponentEdit::ActivateAngles)
            {
                edits.insert(0, ComponentEdit::ActivateAngles);
            }
            let family = edit_family(base, &edits, &family_context.borrowed()).map_err(invalid)?;
            let mask =
                ProgrammingFamilyFixAt::from_family(command.owner, command.component, family)
                    .map_err(invalid)?;
            validate_mask_model(&mask, family_context.native_model.clone())?;
            mutations.push(DynamicProgrammerValueMutation::Set {
                fixture_id: target,
                attribute: command.owner.key(),
                value: DynamicSemanticValue::ProgrammingFixAt {
                    mask,
                    timing: command.timing,
                },
            });
        }
        self.commit_fix_at(context, identity.session, action, &mutations)
    }

    /// Command parsing uses the same gate before any interaction/selection changes.
    pub fn validate_fix_at_contract(
        values: &[DynamicFixAtValue],
        supported: u16,
    ) -> Result<(), ActionError> {
        let mut required = 0;
        for value in values {
            required = required.max(value.value.required_programming_contract());
            if let Some(mask) = value.mask()? {
                required = required.max(mask.required_programming_contract());
            }
        }
        validate_contract(required, supported)
    }

    fn fix_at_targets(&self, session: SessionId, explicit: &[FixtureId]) -> Vec<FixtureId> {
        if explicit.is_empty() {
            self.programmers
                .selection(session)
                .map(|selection| selection.selected)
                .unwrap_or_default()
        } else {
            explicit.to_vec()
        }
    }

    fn commit_fix_at(
        &self,
        context: &ActionContext,
        session: SessionId,
        action: DynamicsReplayAction,
        mutations: &[DynamicProgrammerValueMutation],
    ) -> Result<usize, ActionError> {
        if !self.programmers.knows_session(session) {
            return Err(ActionError::new(
                ActionErrorKind::NotFound,
                "programmer does not exist",
            ));
        }
        let applied = if mutations.is_empty() {
            0
        } else if self
            .programmers
            .apply_dynamic_values(session, mutations, None)
        {
            mutations.len()
        } else {
            0
        };
        self.remember(
            context,
            session,
            action,
            DynamicsReplayOutcome::Applied(applied),
        );
        Ok(applied)
    }
}

fn validate_contract(required: u16, supported: u16) -> Result<(), ActionError> {
    if required > supported {
        Err(invalid(format!(
            "FixAT requires programming contract {required}; this runtime supports {supported}"
        )))
    } else {
        Ok(())
    }
}

fn validate_mask_model(
    mask: &ProgrammingFamilyFixAt,
    model: Option<Arc<dyn NativeColorEditModel + Send + Sync>>,
) -> Result<(), ActionError> {
    let whole =
        DynamicValueAddress::whole_family(mask.address.owner(), &mask.family).map_err(invalid)?;
    CompiledDynamicValueAddress::new(whole, model.clone())
        .map_err(invalid)?
        .validate_source_value(&DynamicValue::Family(mask.family.clone()))
        .map_err(invalid)?;
    CompiledDynamicValueAddress::new(mask.address.clone(), model).map_err(invalid)?;
    Ok(())
}

fn validate_semantic_targets(
    snapshot: &EngineSnapshot,
    targets: &[FixtureId],
) -> Result<(), ActionError> {
    let mut unique = HashSet::new();
    for target in targets {
        if target.0.is_nil() || !unique.insert(*target) {
            return Err(invalid("FixAT requires unique stable fixture identities"));
        }
        if !snapshot.fixtures.iter().any(|fixture| {
            fixture.fixture_id == *target
                || fixture
                    .logical_heads
                    .iter()
                    .any(|head| head.fixture_id == *target)
        }) {
            return Err(ActionError::new(
                ActionErrorKind::NotFound,
                format!("FixAT fixture {} does not exist", target.0),
            ));
        }
    }
    Ok(())
}

fn invalid(error: impl std::fmt::Display) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, error.to_string())
}

#[cfg(test)]
mod tests;
