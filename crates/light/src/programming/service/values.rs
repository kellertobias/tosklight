use super::{ProgrammingService, state::interaction_change, support::Snapshot};
use crate::{
    ActionEnvelope, ActionError, ActionErrorKind, ProgrammingPorts, ProgrammingValueIntent,
    ProgrammingValueMutation, ProgrammingValueOperation, ProgrammingValueTiming,
    ProgrammingValuesCommand, ProgrammingValuesEnvironment, ProgrammingValuesOutcome,
    ProgrammingValuesRequest, ProgrammingValuesResult,
};
use light_core::{AttributeValue, SessionId, UserId};
use light_programmer::{
    NormalProgrammerValueMutation, NormalProgrammerValueTiming, ProgrammerAlignmentBase,
    ProgrammerAlignmentPlan,
};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use super::values_replay_fingerprint::{RequestFingerprint, values_request_fingerprint};
use super::values_validation::{validate_request_id, validate_value_mutations};

impl ProgrammingService {
    pub fn handle_values(
        &self,
        action: ActionEnvelope<ProgrammingValuesRequest>,
        ports: &dyn ProgrammingPorts,
    ) -> Result<ProgrammingValuesResult, ActionError> {
        let (session, user_id, request_id, expected_revision) = values_context(&action)?;
        self.with_user_and_desk_gate(action.context.desk_id, user_id, || {
            ports.authorize(&action.context)?;
            let user_id = self.operated_values_owner(session, user_id)?;
            let fingerprint = values_request_fingerprint(expected_revision, &action.command);
            if let Some(cached) = self.cached_values(
                user_id,
                action.context.desk_id,
                session,
                &request_id,
                fingerprint,
            )? {
                return Ok(cached);
            }
            self.assert_values_revision(expected_revision)?;
            let capture_mode_revision = self.assert_capture_mode_precondition(
                session,
                action.command.expected_capture_mode_revision,
            )?;
            let result = self.apply_values_action(
                &action,
                ports,
                session,
                user_id,
                expected_revision,
                capture_mode_revision,
            )?;
            self.remember_values(
                user_id,
                action.context.desk_id,
                session,
                request_id,
                fingerprint,
                result.clone(),
            );
            Ok(result)
        })
    }

    fn apply_values_action(
        &self,
        action: &ActionEnvelope<ProgrammingValuesRequest>,
        ports: &dyn ProgrammingPorts,
        session: SessionId,
        user_id: UserId,
        revision_before: u64,
        capture_mode_revision: u64,
    ) -> Result<ProgrammingValuesResult, ActionError> {
        let lifecycle_before = self.active_lifecycle_programmer(user_id);
        let before = Snapshot::read(&self.programmers, action.context.desk_id, session, user_id)?;
        let environment = (!action.command.command.is_clear())
            .then(|| ports.values_environment(&action.context))
            .transpose()?;
        let planned;
        let mut alignment_plan = None;
        let mutations = if let Some(intent) = action.command.command.intent() {
            let active_content = self
                .programmers
                .get(session)
                .map(|state| state.update_content())
                .unwrap_or_default();
            let active_values = active_content
                .fixture_values
                .iter()
                .map(|value| {
                    (
                        (value.fixture_id, value.attribute.clone()),
                        value.value.clone(),
                    )
                })
                .collect::<HashMap<_, _>>();
            let active_group_values = active_content
                .group_values
                .iter()
                .map(|value| {
                    (
                        (value.group_id.clone(), value.attribute.clone()),
                        value.value.clone(),
                    )
                })
                .collect::<HashMap<_, _>>();
            let values_environment = environment
                .as_ref()
                .expect("non-clear actions load a values environment");
            if let Some((aligned, plan)) = self.plan_aligned_value_intent(
                &action.context,
                ports,
                session,
                intent,
                values_environment,
                &active_values,
            )? {
                planned = aligned;
                alignment_plan = Some(plan);
            } else {
                planned = plan_value_intent(
                    intent,
                    values_environment,
                    active_values,
                    active_group_values,
                )?;
            }
            std::borrow::Cow::Owned(planned)
        } else {
            if action.command.command.is_clear() {
                self.programmers.deactivate_alignment(session);
            }
            action.command.command.mutations()
        };
        if !mutations.is_empty() {
            validate_value_mutations(
                mutations.as_ref(),
                environment
                    .as_ref()
                    .expect("non-clear actions load a values environment"),
            )?;
        }
        let changed =
            self.mutate_normal_values(session, &action.command.command, mutations.as_ref());
        if let Some(plan) = alignment_plan {
            self.programmers
                .commit_alignment_plan(session, plan)
                .map_err(super::alignment::alignment_error)?;
        }
        let explicit_fixture_attributes = mutations
            .iter()
            .filter_map(|mutation| match mutation {
                ProgrammingValueMutation::SetFixture {
                    fixture_id,
                    attribute,
                    ..
                } => Some((*fixture_id, attribute.clone())),
                ProgrammingValueMutation::ReleaseFixture { .. }
                | ProgrammingValueMutation::SetGroup { .. }
                | ProgrammingValueMutation::ReleaseGroup { .. } => None,
            })
            .collect::<Vec<_>>();
        if !explicit_fixture_attributes.is_empty() {
            ports.mark_highlight_explicit_fixture_attributes(
                &action.context,
                &explicit_fixture_attributes,
            );
        }
        let warning = changed
            .then(|| ports.persist(&action.context, "programmer.values"))
            .flatten();
        let after = Snapshot::read(&self.programmers, action.context.desk_id, session, user_id)?;
        let interaction = interaction_change(
            &self.programmers,
            action.context.desk_id,
            session,
            &before,
            &after,
        );
        let values = self.values_change(user_id, &before.values_content, &after.values_content)?;
        let interaction_event_sequence = self.publish_interaction(&action.context, interaction);
        let outcome = self.values_outcome(&action.context, values, revision_before);
        self.publish_lifecycle_for_context(&action.context, lifecycle_before);
        Ok(ProgrammingValuesResult {
            context: action.context.clone(),
            outcome,
            capture_mode_revision,
            interaction_event_sequence,
            replayed: false,
            warning,
        })
    }

    fn plan_aligned_value_intent(
        &self,
        context: &crate::ActionContext,
        ports: &dyn ProgrammingPorts,
        session: SessionId,
        intent: &ProgrammingValueIntent,
        environment: &ProgrammingValuesEnvironment,
        active_values: &HashMap<(light_core::FixtureId, light_core::AttributeKey), AttributeValue>,
    ) -> Result<Option<(Vec<ProgrammingValueMutation>, ProgrammerAlignmentPlan)>, ActionError> {
        let Some(alignment) = self.programmers.alignment(session) else {
            return Ok(None);
        };
        let ProgrammingValueOperation::RelativeStep(delta) = intent.operation else {
            self.programmers.deactivate_alignment(session);
            return Ok(None);
        };
        if alignment
            .binding
            .as_ref()
            .is_some_and(|binding| binding.attribute != intent.attribute)
        {
            self.programmers.deactivate_alignment(session);
            return Ok(None);
        }
        let bases = alignment.binding.as_ref().map_or_else(
            || {
                alignment_bases_from_environment(
                    ports,
                    context,
                    &alignment.fixtures,
                    &intent.attribute,
                    environment,
                    active_values,
                )
            },
            |_| Vec::new(),
        );
        let plan = self
            .programmers
            .plan_alignment_delta(session, intent.attribute.clone(), delta, &bases)
            .map_err(super::alignment::alignment_error)?;
        let mut mutations = Vec::new();
        let mut addresses = HashSet::new();
        for aligned in &plan.values {
            push_intent_value(
                &mut mutations,
                &mut addresses,
                aligned.fixture_id,
                intent.attribute.clone(),
                AttributeValue::Normalized(aligned.value),
                intent.timing,
            );
            for linked in environment
                .activation_links
                .get(&intent.attribute)
                .into_iter()
                .flatten()
            {
                let address = (aligned.fixture_id, linked.clone());
                if active_values.contains_key(&address)
                    || !environment
                        .supported_attributes
                        .get(&aligned.fixture_id)
                        .is_some_and(|attributes| attributes.contains(linked))
                {
                    continue;
                }
                let Some(value) = environment.current_values.get(&address).cloned() else {
                    continue;
                };
                push_intent_value(
                    &mut mutations,
                    &mut addresses,
                    aligned.fixture_id,
                    linked.clone(),
                    value,
                    intent.timing,
                );
            }
        }
        Ok(Some((mutations, plan)))
    }

    fn mutate_normal_values(
        &self,
        session: SessionId,
        command: &ProgrammingValuesCommand,
        mutations: &[ProgrammingValueMutation],
    ) -> bool {
        if command.is_clear() {
            self.programmers.clear_normal_values(session)
        } else if let Some(intent) = command.intent() {
            let mutations = mutations.iter().map(domain_mutation).collect::<Vec<_>>();
            self.programmers.apply_normal_values_grouped(
                session,
                &mutations,
                intent.undo_group.as_deref(),
            )
        } else {
            let mutations = mutations.iter().map(domain_mutation).collect::<Vec<_>>();
            self.programmers.apply_normal_values(session, &mutations)
        }
    }

    fn values_outcome(
        &self,
        context: &crate::ActionContext,
        change: Option<crate::ProgrammingValuesChange>,
        revision_before: u64,
    ) -> ProgrammingValuesOutcome {
        let Some(change) = change else {
            return ProgrammingValuesOutcome::NoChange {
                revision: revision_before,
            };
        };
        let projection = Arc::clone(&change.projection);
        let event_sequence = self
            .publish_values(context, Some(change))
            .expect("a values change always publishes one event");
        ProgrammingValuesOutcome::Changed {
            projection,
            event_sequence,
        }
    }

    /// The Programmer this session operates, given whatever identity it presented.
    fn operated_values_owner(
        &self,
        session: SessionId,
        user_id: UserId,
    ) -> Result<UserId, ActionError> {
        self.programmers
            .operated_desk_user(session, user_id)
            .ok_or_else(|| {
                ActionError::new(
                    ActionErrorKind::NotFound,
                    "Programmer values are unavailable",
                )
            })
    }

    fn assert_values_revision(&self, expected: u64) -> Result<(), ActionError> {
        let actual = self.programmers.normal_values_revision();
        if expected == actual {
            Ok(())
        } else {
            Err(ActionError::new(
                ActionErrorKind::Conflict,
                format!(
                    "Programmer values revision conflict: expected {expected}, actual {actual}"
                ),
            )
            .at_revision(actual))
        }
    }

    fn assert_capture_mode_precondition(
        &self,
        session: SessionId,
        expected: u64,
    ) -> Result<u64, ActionError> {
        let actual = self.programmers.capture_mode_revision();
        let values_revision = self.programmers.normal_values_revision();
        if expected != actual {
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                format!(
                    "Programmer capture-mode revision conflict: expected {expected}, actual {actual}"
                ),
            )
            .at_revision(values_revision)
            .at_related_revision(actual));
        }
        let mode = self.programmers.capture_mode(session).ok_or_else(|| {
            ActionError::new(
                ActionErrorKind::NotFound,
                "Programmer values are unavailable",
            )
        })?;
        if mode.redirects_normal_values_to_preload() {
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                "normal Programmer values cannot be changed while Programmer capture is redirected to Preload",
            )
            .at_revision(values_revision)
            .at_related_revision(actual));
        }
        Ok(actual)
    }

    fn cached_values(
        &self,
        user_id: UserId,
        desk_id: uuid::Uuid,
        session_id: SessionId,
        request_id: &str,
        fingerprint: RequestFingerprint,
    ) -> Result<Option<ProgrammingValuesResult>, ActionError> {
        self.values_replay
            .lock()
            .get(user_id, desk_id, session_id, request_id, fingerprint)
    }

    fn remember_values(
        &self,
        user_id: UserId,
        desk_id: uuid::Uuid,
        session_id: SessionId,
        request_id: String,
        fingerprint: RequestFingerprint,
        result: ProgrammingValuesResult,
    ) {
        self.values_replay.lock().insert(
            user_id,
            desk_id,
            session_id,
            request_id,
            fingerprint,
            result,
        );
    }
}

fn alignment_bases_from_environment(
    ports: &dyn ProgrammingPorts,
    context: &crate::ActionContext,
    fixtures: &[light_core::FixtureId],
    attribute: &light_core::AttributeKey,
    environment: &ProgrammingValuesEnvironment,
    active_values: &HashMap<(light_core::FixtureId, light_core::AttributeKey), AttributeValue>,
) -> Vec<ProgrammerAlignmentBase> {
    fixtures
        .iter()
        .filter(|fixture_id| {
            environment
                .supported_attributes
                .get(fixture_id)
                .is_some_and(|attributes| attributes.contains(attribute))
        })
        .filter_map(|fixture_id| {
            let address = (*fixture_id, attribute.clone());
            active_values
                .get(&address)
                .or_else(|| environment.current_values.get(&address))
                .or_else(|| environment.default_values.get(&address))
                .and_then(AttributeValue::normalized)
                .map(|value| ProgrammerAlignmentBase {
                    fixture_id: *fixture_id,
                    value,
                    wraps: ports.programmer_attribute_wraps(context, *fixture_id, attribute),
                })
        })
        .collect()
}

// @tour value-spreading:40 Expand once at the application boundary
// The application maps one validated spread over ordered fixture IDs and prepares the complete
// mutation set before any Programmer state changes.
pub(super) fn plan_value_intent(
    intent: &ProgrammingValueIntent,
    environment: &ProgrammingValuesEnvironment,
    active_values: HashMap<(light_core::FixtureId, light_core::AttributeKey), AttributeValue>,
    active_group_values: HashMap<(String, light_core::AttributeKey), AttributeValue>,
) -> Result<Vec<ProgrammingValueMutation>, ActionError> {
    let count = validate_value_intent(intent, environment)?;
    if let Some(group_id) = intent.group_id.as_ref() {
        return plan_group_value_intent(
            intent,
            environment,
            group_id,
            active_values,
            active_group_values,
        );
    }
    plan_fixture_value_intent(intent, environment, count, active_values)
}

fn validate_value_intent(
    intent: &ProgrammingValueIntent,
    environment: &ProgrammingValuesEnvironment,
) -> Result<usize, ActionError> {
    if intent
        .undo_group
        .as_ref()
        .is_some_and(|group| group.is_empty() || group.len() > 128)
    {
        return Err(ActionError::new(
            ActionErrorKind::Invalid,
            "encoder undo_group must contain 1-128 bytes",
        ));
    }
    if let ProgrammingValueOperation::RelativeStep(delta) = intent.operation
        && (!delta.is_finite() || delta == 0.0)
    {
        return Err(ActionError::new(
            ActionErrorKind::Invalid,
            "relative Programmer value step must be finite and non-zero",
        ));
    }
    let targets_group = intent.group_id.is_some();
    if targets_group != intent.fixture_ids.is_empty() {
        return Err(ActionError::new(
            ActionErrorKind::Invalid,
            "Programmer value intent requires either ordered fixture_ids or one group_id",
        ));
    }
    let count = intent
        .group_id
        .as_ref()
        .and_then(|group_id| {
            environment
                .group_rank_counts
                .get(group_id)
                .or_else(|| environment.group_memberships.get(group_id))
                .copied()
        })
        .unwrap_or(intent.fixture_ids.len());
    if let ProgrammingValueOperation::AbsoluteSet(AttributeValue::Spread(points)) =
        &intent.operation
        && (points.len() < 2
            || points
                .iter()
                .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
            || (points.len() > 2 && points.len() > count))
    {
        let capacity = if targets_group {
            format!("the Group has only {count} ranks")
        } else {
            format!("there are only {count} selected items")
        };
        return Err(ActionError::new(
            ActionErrorKind::Invalid,
            format!("spread has {} control points but {capacity}", points.len(),),
        ));
    }
    Ok(count)
}

fn plan_group_value_intent(
    intent: &ProgrammingValueIntent,
    environment: &ProgrammingValuesEnvironment,
    group_id: &str,
    active_values: HashMap<(light_core::FixtureId, light_core::AttributeKey), AttributeValue>,
    active_group_values: HashMap<(String, light_core::AttributeKey), AttributeValue>,
) -> Result<Vec<ProgrammingValueMutation>, ActionError> {
    let requested = match &intent.operation {
        ProgrammingValueOperation::AbsoluteSet(value) => value.clone(),
        ProgrammingValueOperation::RelativeStep(delta) => {
            let current = active_group_values
                .get(&(group_id.to_owned(), intent.attribute.clone()))
                .cloned()
                .or_else(|| {
                    environment.group_members.get(group_id).and_then(|members| {
                        group_current_value(environment, members, &intent.attribute)
                    })
                })
                .ok_or_else(|| {
                    ActionError::new(
                        ActionErrorKind::Invalid,
                        "relative Group value requires a current normalized value",
                    )
                })?;
            shift_attribute_value(current, *delta)?
        }
    };
    let mut mutations = vec![ProgrammingValueMutation::SetGroup {
        group_id: group_id.to_owned(),
        attribute: intent.attribute.clone(),
        value: requested,
        timing: intent.timing,
    }];
    let mut addresses = HashSet::new();
    let linked_group_values = environment
        .activation_links
        .get(&intent.attribute)
        .into_iter()
        .flatten()
        .filter(|linked| {
            !active_group_values.contains_key(&(group_id.to_owned(), (*linked).clone()))
        })
        .cloned()
        .collect::<Vec<_>>();
    if let Some(members) = environment.group_members.get(group_id) {
        for fixture_id in members {
            for linked in &linked_group_values {
                let address = (*fixture_id, linked.clone());
                if active_values.contains_key(&address)
                    || !environment
                        .supported_attributes
                        .get(fixture_id)
                        .is_some_and(|attributes| attributes.contains(linked))
                {
                    continue;
                }
                let Some(value) = environment.current_values.get(&address).cloned() else {
                    continue;
                };
                push_intent_value(
                    &mut mutations,
                    &mut addresses,
                    *fixture_id,
                    linked.clone(),
                    value,
                    intent.timing,
                );
            }
        }
    }
    Ok(mutations)
}

fn plan_fixture_value_intent(
    intent: &ProgrammingValueIntent,
    environment: &ProgrammingValuesEnvironment,
    count: usize,
    active_values: HashMap<(light_core::FixtureId, light_core::AttributeKey), AttributeValue>,
) -> Result<Vec<ProgrammingValueMutation>, ActionError> {
    let mut mutations = Vec::new();
    let mut addresses = HashSet::new();
    for (index, fixture_id) in intent.fixture_ids.iter().copied().enumerate() {
        let requested = match &intent.operation {
            ProgrammingValueOperation::AbsoluteSet(AttributeValue::Spread(points)) => {
                AttributeValue::Normalized(light_core::spread_position(
                    points.as_slice(),
                    index,
                    count,
                ))
            }
            ProgrammingValueOperation::AbsoluteSet(value) => value.clone(),
            ProgrammingValueOperation::RelativeStep(delta) => {
                let current = active_values
                    .get(&(fixture_id, intent.attribute.clone()))
                    .or_else(|| {
                        environment
                            .current_values
                            .get(&(fixture_id, intent.attribute.clone()))
                    })
                    .or_else(|| {
                        environment
                            .default_values
                            .get(&(fixture_id, intent.attribute.clone()))
                    })
                    .and_then(AttributeValue::normalized)
                    .ok_or_else(|| {
                        ActionError::new(
                            ActionErrorKind::Invalid,
                            format!(
                                "Cannot adjust {}: a selected fixture has no normalized current or profile-default value. Set an absolute value or remove that fixture from the selection.",
                                intent.attribute.0
                            ),
                        )
                    })?;
                AttributeValue::Normalized(shift_normalized(current, *delta))
            }
        };
        push_intent_value(
            &mut mutations,
            &mut addresses,
            fixture_id,
            intent.attribute.clone(),
            requested,
            intent.timing,
        );
        for linked in environment
            .activation_links
            .get(&intent.attribute)
            .into_iter()
            .flatten()
        {
            let address = (fixture_id, linked.clone());
            if active_values.contains_key(&address)
                || !environment
                    .supported_attributes
                    .get(&fixture_id)
                    .is_some_and(|attributes| attributes.contains(linked))
            {
                continue;
            }
            let Some(value) = environment.current_values.get(&address).cloned() else {
                continue;
            };
            push_intent_value(
                &mut mutations,
                &mut addresses,
                fixture_id,
                linked.clone(),
                value,
                intent.timing,
            );
        }
    }
    Ok(mutations)
}

fn group_current_value(
    environment: &ProgrammingValuesEnvironment,
    members: &[light_core::FixtureId],
    attribute: &light_core::AttributeKey,
) -> Option<AttributeValue> {
    let values = members
        .iter()
        .map(|fixture_id| {
            environment
                .current_values
                .get(&(*fixture_id, attribute.clone()))
                .or_else(|| {
                    environment
                        .default_values
                        .get(&(*fixture_id, attribute.clone()))
                })
                .and_then(AttributeValue::normalized)
        })
        .collect::<Option<Vec<_>>>()?;
    match values.as_slice() {
        [] => None,
        [value] => Some(AttributeValue::Normalized(*value)),
        values => Some(AttributeValue::Spread(values.to_vec())),
    }
}

fn shift_attribute_value(value: AttributeValue, delta: f32) -> Result<AttributeValue, ActionError> {
    match value {
        AttributeValue::Normalized(value) => {
            Ok(AttributeValue::Normalized(shift_normalized(value, delta)))
        }
        AttributeValue::Spread(values) => Ok(AttributeValue::Spread(
            values
                .into_iter()
                .map(|value| shift_normalized(value, delta))
                .collect(),
        )),
        AttributeValue::Discrete(_)
        | AttributeValue::ColorXyz(_)
        | AttributeValue::RawDmx(_)
        | AttributeValue::RawDmxExact(_) => Err(ActionError::new(
            ActionErrorKind::Invalid,
            "relative Group value requires a current normalized value",
        )),
    }
}

fn shift_normalized(value: f32, delta: f32) -> f32 {
    const PRECISION: f32 = 1_000_000.0;
    (((value + delta) * PRECISION).round() / PRECISION).clamp(0.0, 1.0)
}

fn push_intent_value(
    mutations: &mut Vec<ProgrammingValueMutation>,
    addresses: &mut HashSet<(light_core::FixtureId, light_core::AttributeKey)>,
    fixture_id: light_core::FixtureId,
    attribute: light_core::AttributeKey,
    value: AttributeValue,
    timing: ProgrammingValueTiming,
) {
    if addresses.insert((fixture_id, attribute.clone())) {
        mutations.push(ProgrammingValueMutation::SetFixture {
            fixture_id,
            attribute,
            value,
            timing,
        });
    }
}

fn values_context(
    action: &ActionEnvelope<ProgrammingValuesRequest>,
) -> Result<(SessionId, UserId, String, u64), ActionError> {
    let session = action.context.session_id.map(SessionId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Programmer values actions require an operator session",
        )
    })?;
    let user_id = action.context.user_id.map(UserId).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Unauthorized,
            "Programmer values actions require an authenticated user",
        )
    })?;
    let request_id = action.context.request_id.as_deref().ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Invalid,
            "Programmer values actions require a request_id",
        )
    })?;
    validate_request_id(request_id)?;
    let expected_revision = action.context.expected_revision.ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Invalid,
            "Programmer values actions require an expected revision",
        )
    })?;
    Ok((session, user_id, request_id.to_owned(), expected_revision))
}

fn domain_mutation(mutation: &ProgrammingValueMutation) -> NormalProgrammerValueMutation {
    match mutation {
        ProgrammingValueMutation::SetFixture {
            fixture_id,
            attribute,
            value,
            timing,
        } => NormalProgrammerValueMutation::SetFixture {
            fixture_id: *fixture_id,
            attribute: attribute.clone(),
            value: value.clone(),
            timing: domain_timing(*timing),
        },
        ProgrammingValueMutation::ReleaseFixture {
            fixture_id,
            attribute,
        } => NormalProgrammerValueMutation::ReleaseFixture {
            fixture_id: *fixture_id,
            attribute: attribute.clone(),
        },
        ProgrammingValueMutation::SetGroup {
            group_id,
            attribute,
            value,
            timing,
        } => NormalProgrammerValueMutation::SetGroup {
            group_id: group_id.clone(),
            attribute: attribute.clone(),
            value: value.clone(),
            timing: domain_timing(*timing),
        },
        ProgrammingValueMutation::ReleaseGroup {
            group_id,
            attribute,
        } => NormalProgrammerValueMutation::ReleaseGroup {
            group_id: group_id.clone(),
            attribute: attribute.clone(),
        },
    }
}

fn domain_timing(timing: ProgrammingValueTiming) -> NormalProgrammerValueTiming {
    NormalProgrammerValueTiming {
        fade: timing.fade,
        fade_millis: timing.fade_millis,
        delay_millis: timing.delay_millis,
    }
}

#[cfg(test)]
mod tests {
    use super::shift_normalized;

    #[test]
    fn repeated_operator_steps_do_not_drift_below_the_displayed_value() {
        let value = (0..10).fold(0.0, |current, _| shift_normalized(current, 0.01));
        assert_eq!(value, 0.1);
    }
}
