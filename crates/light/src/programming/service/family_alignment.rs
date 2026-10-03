//! Typed Align shares complete anchors and mutation assembly between Normal and Preload.
use super::{ProgrammingService, alignment::alignment_error, family_values};
use crate::{
    ActionError, ActionErrorKind, ProgrammingFamilyContext, ProgrammingValueIntent,
    ProgrammingValueMutation, ProgrammingValueOperation, ProgrammingValuesEnvironment,
};
use light_core::{AttributeKey, AttributeValue, FixtureId, SessionId, programming::*};
use light_programmer::{
    FamilyAlignmentInput, ProgrammerAlignmentLane, ProgrammerFamilyAlignmentBase,
    ProgrammerFamilyAlignmentInitial, ProgrammerFamilyAlignmentPlan,
    ProgrammerFamilyAlignmentTarget,
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

type FixtureValues = HashMap<(FixtureId, AttributeKey), AttributeValue>;
type GroupValues = HashMap<(String, AttributeKey), AttributeValue>;

impl ProgrammingService {
    /// Fixture Align operates on its frozen selection, even when one surface addresses only
    /// one member. A bound Align retains its original bases and adoption across later samples.
    pub(super) fn family_alignment_fixture_cohort(
        &self,
        session: SessionId,
        lane: ProgrammerAlignmentLane,
        intent: &ProgrammingValueIntent,
        environment: &ProgrammingValuesEnvironment,
        active: &FixtureValues,
    ) -> Option<Vec<FixtureId>> {
        if intent.group_id.is_some() || intent.fixture_ids.is_empty() {
            return None;
        }
        let state = self.programmers.alignment(session)?;
        let ProgrammingValueOperation::ComponentEdits(edits) = &intent.operation else {
            return None;
        };
        let [edit] = edits.as_slice() else {
            return None;
        };
        let (component, _) = FamilyAlignmentInput::from_edit(edit)?;
        if state.binding.is_some() {
            return None;
        }
        if let Some(binding) = &state.family_binding {
            return (binding.component == component
                && binding.lane == lane
                && binding.group_id == intent.group_id)
                .then(|| binding.bases.iter().map(|base| base.fixture_id).collect());
        }
        let members = initial_fixture_members(&state, intent, environment, active);
        // An empty Align falls through to the ordinary requested fixture edit.
        (!members.is_empty()).then_some(members)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn plan_aligned_family_intent(
        &self,
        session: SessionId,
        lane: ProgrammerAlignmentLane,
        intent: &ProgrammingValueIntent,
        environment: &ProgrammingValuesEnvironment,
        active: &FixtureValues,
        groups: &GroupValues,
    ) -> Result<Option<(Vec<ProgrammingValueMutation>, ProgrammerFamilyAlignmentPlan)>, ActionError>
    {
        if intent.group_id.is_none() && intent.fixture_ids.is_empty() {
            return Ok(None);
        }
        let Some(state) = self.programmers.alignment(session) else {
            return Ok(None);
        };
        let ProgrammingValueOperation::ComponentEdits(edits) = &intent.operation else {
            return Ok(None);
        };
        let [edit] = edits.as_slice() else {
            return Ok(None);
        };
        let Some((component, delta)) = FamilyAlignmentInput::from_edit(edit) else {
            return Ok(None);
        };
        if state.binding.is_some()
            || state.family_binding.as_ref().is_some_and(|binding| {
                binding.component != component
                    || binding.lane != lane
                    || binding.group_id != intent.group_id
            })
        {
            return Ok(None);
        }
        let current_members = intent
            .group_id
            .as_ref()
            .map(|id| {
                environment
                    .group_members
                    .get(id)
                    .map(|ids| ids.iter().copied().collect::<HashSet<_>>())
                    .ok_or_else(|| invalid("Group Align requires current authoritative membership"))
            })
            .transpose()?;
        let initial = if state.family_binding.is_none() {
            match initial_family_alignment(
                &state,
                intent,
                edits,
                component,
                environment,
                active,
                groups,
            )? {
                InitialAlignment::Held => return Ok(Some((Vec::new(), held_initial_plan(state)))),
                InitialAlignment::Empty => return Ok(None),
                InitialAlignment::Ready(initial) => Some(initial),
            }
        } else {
            None
        };
        let target = match (&intent.group_id, &current_members) {
            (Some(id), Some(members)) => ProgrammerFamilyAlignmentTarget::Group {
                id,
                current_members: members,
            },
            _ => ProgrammerFamilyAlignmentTarget::Fixtures,
        };
        let plan = self
            .programmers
            .plan_family_alignment_delta(session, component, lane, target, delta, initial)
            .map_err(alignment_error)?;
        let mutations = aligned_mutations(intent, &plan, active, groups)?;
        Ok(Some((mutations, plan)))
    }

    pub(super) fn reanchor_aligned_family(
        &self,
        context: &crate::ActionContext,
        ports: &dyn crate::ProgrammingPorts,
        session: SessionId,
        mode: light_programmer::ProgrammerAlignmentMode,
        binding: &light_programmer::ProgrammerFamilyAlignmentBinding,
    ) -> Result<light_programmer::ProgrammerAlignmentState, ActionError> {
        let environment = ports.values_environment(context)?;
        let state = self
            .programmers
            .get(session)
            .ok_or_else(|| invalid("Programmer Align is unavailable"))?;
        let (active, groups): (FixtureValues, GroupValues) = match binding.lane {
            ProgrammerAlignmentLane::Normal => {
                let content = state.update_content();
                (
                    content
                        .fixture_values
                        .into_iter()
                        .map(|v| ((v.fixture_id, v.attribute), v.value))
                        .collect(),
                    content
                        .group_values
                        .into_iter()
                        .map(|v| ((v.group_id, v.attribute), v.value))
                        .collect(),
                )
            }
            ProgrammerAlignmentLane::Preload => {
                let content = self
                    .programmers
                    .preload_pending_values(session)
                    .ok_or_else(|| invalid("Preload Align is unavailable"))?;
                (
                    content
                        .fixture_values
                        .into_iter()
                        .map(|v| ((v.fixture_id, v.attribute), v.value))
                        .collect(),
                    content
                        .group_values
                        .into_iter()
                        .map(|v| ((v.group_id, v.attribute), v.value))
                        .collect(),
                )
            }
        };
        let members = binding
            .group_id
            .as_ref()
            .map(|id| {
                environment
                    .group_members
                    .get(id)
                    .map(|members| members.iter().copied().collect::<HashSet<_>>())
                    .ok_or_else(|| invalid("Group Align requires current authoritative membership"))
            })
            .transpose()?;
        let key = binding.component.owner().key();
        let mut bases = Vec::with_capacity(binding.bases.len());
        for previous in binding.bases.iter() {
            if members
                .as_ref()
                .is_some_and(|members| !members.contains(&previous.fixture_id))
            {
                bases.push(previous.clone());
                continue;
            }
            let address = (previous.fixture_id, key.clone());
            let seed = if let Some(group) = &binding.group_id {
                match groups.get(&(group.clone(), key.clone())) {
                    Some(AttributeValue::GroupFamily(assignment)) => {
                        assignment.for_member(previous.fixture_id)
                    }
                    Some(value) => value,
                    None => &previous.value,
                }
            } else {
                active
                    .get(&address)
                    .or_else(|| environment.current_values.get(&address))
                    .or_else(|| environment.default_values.get(&address))
                    .unwrap_or(&previous.value)
            };
            let mut captured = previous.context.as_ref().clone();
            if matches!(seed, AttributeValue::Position(p) if matches!(p.as_ref(),PositionIntent::Target {..}))
            {
                captured.solved_angles = environment
                    .family_contexts
                    .get(&previous.fixture_id)
                    .and_then(|context| context.solved_angles)
                    .or(captured.solved_angles);
            }
            let value = materialize(seed, binding.rank_count, previous.rank, &captured)?;
            bases.push(ProgrammerFamilyAlignmentBase {
                fixture_id: previous.fixture_id,
                rank: previous.rank,
                value,
                context: Arc::new(captured),
            });
        }
        self.programmers
            .reanchor_family_alignment(session, mode, bases)
            .map_err(alignment_error)
    }
}

/// How a first Align step starts: held unchanged, not an Align at all, or from its bases.
enum InitialAlignment {
    Held,
    Empty,
    Ready(ProgrammerFamilyAlignmentInitial),
}

/// The complete anchors a first Align step captures from its members' current values.
fn initial_family_alignment(
    state: &light_programmer::ProgrammerAlignmentState,
    intent: &ProgrammingValueIntent,
    edits: &[ComponentEdit],
    component: ProgrammingComponent,
    environment: &ProgrammingValuesEnvironment,
    active: &FixtureValues,
    groups: &GroupValues,
) -> Result<InitialAlignment, ActionError> {
    if intent.group_id.is_some()
        && matches!(
            component,
            ProgrammingComponent::Pan | ProgrammingComponent::Tilt
        )
        && !family_values::position_missing_seed_available(intent, environment, active, groups)
    {
        return Ok(InitialAlignment::Held);
    }
    let selected = state.fixtures.iter().copied().collect::<HashSet<_>>();
    let sequential_ranks = intent.group_id.as_ref().map(|id| {
        environment.group_members[id]
            .iter()
            .enumerate()
            .map(|(rank, id)| (*id, rank))
            .collect::<HashMap<_, _>>()
    });
    let group_seed = intent
        .group_id
        .as_ref()
        .map(|id| family_values::adopt_group_family(intent, edits, environment, active, groups, id))
        .transpose()?;
    let fixtures = match intent.group_id.as_ref() {
        Some(id) => environment.group_members[id]
            .iter()
            .copied()
            .filter(|id| selected.contains(id))
            .collect::<Vec<_>>(),
        None => initial_fixture_members(state, intent, environment, active),
    };
    if fixtures.is_empty() {
        return Ok(InitialAlignment::Empty);
    }
    let rank_count = intent
        .group_id
        .as_ref()
        .map(|id| {
            environment
                .group_rank_counts
                .get(id)
                .copied()
                .unwrap_or(environment.group_members[id].len())
        })
        .unwrap_or(fixtures.len());
    let mut bases = Vec::with_capacity(fixtures.len());
    for (ordinal, id) in fixtures.iter().enumerate() {
        let context = captured_context(environment, *id, intent.group_id.as_deref());
        let rank = alignment_rank(
            environment,
            intent.group_id.as_ref(),
            *id,
            ordinal,
            rank_count,
            sequential_ranks.as_ref(),
        )?;
        let address = (*id, intent.attribute.clone());
        let seed = group_seed
            .as_ref()
            .map(|seed| seed.for_member(*id))
            .or_else(|| active.get(&address))
            .or_else(|| environment.current_values.get(&address))
            .or_else(|| environment.default_values.get(&address));
        if seed.is_none()
            && matches!(
                component,
                ProgrammingComponent::Pan | ProgrammingComponent::Tilt
            )
            && context.position_adoption_attempted
            && context.solved_angles.is_none()
        {
            return Ok(InitialAlignment::Held);
        }
        let seed =
            seed.ok_or_else(|| invalid("Align requires a complete current or default family"))?;
        let value = materialize(seed, rank_count, rank, &context)?;
        bases.push(ProgrammerFamilyAlignmentBase {
            fixture_id: *id,
            rank,
            value,
            context,
        });
    }
    if matches!(
        component,
        ProgrammingComponent::Pan | ProgrammingComponent::Tilt
    ) && bases.iter().any(|base| {
        matches!(&base.value, AttributeValue::Position(position)
                if matches!(position.as_ref(), PositionIntent::Target { .. }))
            && base.context.solved_angles.is_none()
    }) {
        // Returning None would fall through to an ordinary edit of the addressed
        // subset. An empty unchanged plan holds the complete Align operation without
        // establishing a binding, moving its input, or creating an Undo checkpoint.
        return Ok(InitialAlignment::Held);
    }
    Ok(InitialAlignment::Ready(ProgrammerFamilyAlignmentInitial {
        rank_count,
        bases,
        group_seed,
    }))
}

/// A member's rank: its authoritative Group rank, its sequential Group position, or its order.
fn alignment_rank(
    environment: &ProgrammingValuesEnvironment,
    group: Option<&String>,
    id: FixtureId,
    ordinal: usize,
    rank_count: usize,
    sequential_ranks: Option<&HashMap<FixtureId, usize>>,
) -> Result<usize, ActionError> {
    let Some(group) = group else {
        return Ok(ordinal);
    };
    // A missing map is allowed only for legacy/test sequential membership.
    // A partial supplied map is invalid; it must never collapse equal ranks.
    match environment.group_ranks.get(group) {
        Some(ranks) => Ok(*ranks
            .get(&id)
            .ok_or_else(|| invalid("Group Align member is missing its authoritative rank"))?),
        None if rank_count == environment.group_members[group].len() => {
            Ok(sequential_ranks.expect("Group ranks")[&id])
        }
        None => Err(invalid(
            "spatial Group Align requires authoritative member ranks",
        )),
    }
}

fn held_initial_plan(
    state: light_programmer::ProgrammerAlignmentState,
) -> ProgrammerFamilyAlignmentPlan {
    ProgrammerFamilyAlignmentPlan {
        expected_revision: state.revision,
        next_state: state,
        values: Vec::new(),
    }
}

fn initial_fixture_members(
    state: &light_programmer::ProgrammerAlignmentState,
    intent: &ProgrammingValueIntent,
    environment: &ProgrammingValuesEnvironment,
    active: &FixtureValues,
) -> Vec<FixtureId> {
    let position = intent.attribute == ProgrammingOwner::Position.key();
    let pan = AttributeKey("pan".into());
    let tilt = AttributeKey("tilt".into());
    state
        .fixtures
        .iter()
        .copied()
        .filter(|id| {
            let address = (*id, intent.attribute.clone());
            active.contains_key(&address)
                || environment.current_values.contains_key(&address)
                || environment.default_values.contains_key(&address)
                || environment
                    .supported_attributes
                    .get(id)
                    .is_some_and(|keys| {
                        keys.contains(&intent.attribute)
                        // A first edit has no typed seed yet. Standard native Pan/Tilt
                        // identifies the complete Position owner before pose capture; aliased
                        // profiles expose their cold-compiled semantic owner key instead.
                        || (position && keys.contains(&pan) && keys.contains(&tilt))
                    })
        })
        .collect()
}

fn captured_context(
    environment: &ProgrammingValuesEnvironment,
    fixture: FixtureId,
    group: Option<&str>,
) -> Arc<ProgrammingFamilyContext> {
    Arc::new(
        environment
            .family_contexts
            .get(&fixture)
            .or_else(|| group.and_then(|id| environment.group_family_contexts.get(id)))
            .cloned()
            .unwrap_or_default(),
    )
}

fn materialize(
    value: &AttributeValue,
    rank_count: usize,
    rank: usize,
    context: &ProgrammingFamilyContext,
) -> Result<AttributeValue, ActionError> {
    compile_programming_ranks(value, rank_count, &[rank], &context.borrowed())
        .map_err(|e| invalid(&e.0))
        .map(|values| values.at_rank(0).expect("one rank compiled").clone())
}

fn aligned_mutations(
    intent: &ProgrammingValueIntent,
    plan: &ProgrammerFamilyAlignmentPlan,
    active: &FixtureValues,
    groups: &GroupValues,
) -> Result<Vec<ProgrammingValueMutation>, ActionError> {
    let mut mutations = vec![];
    if plan.values.is_empty() {
        return Ok(mutations);
    }
    let binding = plan
        .next_state
        .family_binding
        .as_ref()
        .expect("typed plan has binding");
    let owner = binding.component.owner();
    if let Some(group_id) = &intent.group_id {
        if plan.values.iter().all(|value| value.preserves_target) {
            return Ok(mutations);
        }
        let mut assignment = binding
            .group_seed
            .as_ref()
            .expect("Group binding has seed")
            .as_ref()
            .clone();
        // Use the latest same-lane exceptions so a removed member retains its last authored
        // result. Never rebuild the future-member template from fitted or sampled fixture data.
        if let Some(AttributeValue::GroupFamily(current)) =
            groups.get(&(group_id.clone(), intent.attribute.clone()))
        {
            assignment.members.extend(current.members.clone());
        }
        for result in &plan.values {
            assignment
                .members
                .insert(result.fixture_id.0, result.value.clone());
        }
        assignment.remove_redundant_exceptions();
        assignment.validate().map_err(|e| invalid(&e.0))?;
        let mut releases = groups
            .keys()
            .filter(|(id, key)| id == group_id && independent_programming_component(key, owner))
            .map(|(_, key)| key.clone())
            .collect::<Vec<_>>();
        releases.sort();
        mutations.extend(releases.into_iter().map(|attribute| {
            ProgrammingValueMutation::ReleaseGroup {
                group_id: group_id.clone(),
                attribute,
            }
        }));
        let value = if assignment.members.is_empty() {
            assignment.template
        } else {
            AttributeValue::GroupFamily(Arc::new(assignment))
        };
        mutations.push(ProgrammingValueMutation::SetGroup {
            group_id: group_id.clone(),
            attribute: intent.attribute.clone(),
            value,
            timing: intent.timing,
        });
    } else {
        for result in &plan.values {
            if result.preserves_target {
                continue;
            }
            family_values::append_component_releases(
                &mut mutations,
                result.fixture_id,
                owner,
                active,
            );
            mutations.push(ProgrammingValueMutation::SetFixture {
                fixture_id: result.fixture_id,
                attribute: intent.attribute.clone(),
                value: result.value.clone(),
                timing: intent.timing,
            });
        }
    }
    Ok(mutations)
}

fn invalid(message: &str) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}
