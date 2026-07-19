use super::{ProgrammingService, support};
use crate::{
    ActionContext, ActionError, ActionErrorKind,
    programming::{
        ProgrammingAction, ProgrammingCommand, ProgrammingOutcome, ProgrammingPorts,
        ProgrammingSelectionEnvironment, ProgrammingSelectionQuery, SelectionGestureSource,
    },
};
use light_core::{FixtureId, SessionId};
use light_programmer::{
    SelectionExpression, SelectionReference, SelectionReplaceError, SelectionRule,
    apply_selection_rule, resolve_group,
};
use std::collections::HashSet;
use support::{accepted, selection_replace_error, unknown_programmer};

impl ProgrammingService {
    pub(super) fn apply_selection(
        &self,
        session: SessionId,
        command: &ProgrammingCommand,
        context: &ActionContext,
        ports: &dyn ProgrammingPorts,
    ) -> Result<ProgrammingOutcome, ActionError> {
        SelectionOperation {
            service: self,
            session,
            context,
            ports,
        }
        .apply(command)
    }
}

struct SelectionOperation<'a> {
    service: &'a ProgrammingService,
    session: SessionId,
    context: &'a ActionContext,
    ports: &'a dyn ProgrammingPorts,
}

impl SelectionOperation<'_> {
    fn apply(&self, command: &ProgrammingCommand) -> Result<ProgrammingOutcome, ActionError> {
        match command {
            ProgrammingCommand::ReplaceSelection {
                fixtures,
                expected_revision,
            } => self.replace_selection(fixtures, *expected_revision),
            ProgrammingCommand::ApplySelectionGesture { source, remove } => {
                self.apply_selection_gesture(source, *remove)
            }
            ProgrammingCommand::SelectGroup {
                group_id,
                frozen,
                rule,
                expected_revision,
            } => self.select_group(group_id, *frozen, rule, *expected_revision),
            ProgrammingCommand::ApplySelectionRule { rule } => self.apply_rule(rule),
            _ => unreachable!("selection dispatch accepts only selection commands"),
        }
    }

    fn replace_selection(
        &self,
        fixtures: &[FixtureId],
        expected_revision: u64,
    ) -> Result<ProgrammingOutcome, ActionError> {
        let environment =
            self.environment(ProgrammingSelectionQuery::Fixtures(fixtures.to_vec()))?;
        let fixtures = expand_fixtures(fixtures, &environment)?;
        self.service
            .programmers
            .replace_selection_if_revision(
                self.session,
                expected_revision,
                fixtures,
                SelectionExpression::Static,
            )
            .map_err(selection_replace_error)?;
        self.accept(
            ProgrammingAction::SelectionReplaced,
            "programmer.selection.replace",
        )
    }

    fn apply_selection_gesture(
        &self,
        source: &SelectionGestureSource,
        remove: bool,
    ) -> Result<ProgrammingOutcome, ActionError> {
        let environment = self.environment(gesture_query(source))?;
        let references = gesture_references(source, remove, &environment)?;
        if !self.service.programmers.apply_selection_gesture(
            self.session,
            references,
            &environment.groups,
        ) {
            return Err(unknown_programmer());
        }
        self.accept(
            ProgrammingAction::SelectionGestureApplied,
            "programmer.selection.gesture",
        )
    }

    fn select_group(
        &self,
        group_id: &str,
        frozen: bool,
        rule: &SelectionRule,
        expected_revision: u64,
    ) -> Result<ProgrammingOutcome, ActionError> {
        validate_rule(rule)?;
        let environment =
            self.environment(ProgrammingSelectionQuery::Groups(vec![group_id.to_owned()]))?;
        let (selected, expression) = group_selection(group_id, frozen, rule, &environment)?;
        self.service
            .programmers
            .replace_selection_if_revision(self.session, expected_revision, selected, expression)
            .map_err(selection_replace_error)?;
        self.accept(
            ProgrammingAction::GroupSelected,
            "programmer.selection.group",
        )
    }

    fn apply_rule(&self, rule: &SelectionRule) -> Result<ProgrammingOutcome, ActionError> {
        validate_rule(rule)?;
        const MAX_RETRIES: usize = 8;
        for attempt in 0..MAX_RETRIES {
            let (revision, selected, expression) = self.rule_candidate(rule)?;
            let replaced = self.service.programmers.replace_selection_if_revision(
                self.session,
                revision,
                selected,
                expression,
            );
            match replaced {
                Ok(_) => {
                    return self.accept(
                        ProgrammingAction::SelectionRuleApplied,
                        "programmer.selection.rule",
                    );
                }
                Err(SelectionReplaceError::RevisionConflict { .. })
                    if attempt + 1 < MAX_RETRIES => {}
                Err(error) => return Err(selection_replace_error(error)),
            }
        }
        unreachable!("the final selection-rule conflict returns from the retry loop")
    }

    fn rule_candidate(
        &self,
        rule: &SelectionRule,
    ) -> Result<(u64, Vec<FixtureId>, SelectionExpression), ActionError> {
        let current = self
            .service
            .programmers
            .selection(self.session)
            .ok_or_else(unknown_programmer)?;
        let environment = match &current.expression {
            Some(SelectionExpression::LiveGroup { .. }) => {
                let Some(SelectionExpression::LiveGroup { group_id, .. }) = &current.expression
                else {
                    unreachable!("the live Group expression was matched above")
                };
                self.environment(ProgrammingSelectionQuery::Groups(vec![group_id.clone()]))?
            }
            _ => ProgrammingSelectionEnvironment::default(),
        };
        let (base, expression) = rule_base(&current, rule, &environment)?;
        Ok((
            current.revision,
            apply_selection_rule(&base, rule),
            expression,
        ))
    }

    fn environment(
        &self,
        query: ProgrammingSelectionQuery,
    ) -> Result<ProgrammingSelectionEnvironment, ActionError> {
        self.ports.selection_environment(self.context, &query)
    }

    fn accept(
        &self,
        action: ProgrammingAction,
        operation: &'static str,
    ) -> Result<ProgrammingOutcome, ActionError> {
        selection_accepted(
            &self.service.programmers,
            self.session,
            action,
            self.ports.persist(self.context, operation),
        )
    }
}

fn group_selection(
    group_id: &str,
    frozen: bool,
    rule: &SelectionRule,
    environment: &ProgrammingSelectionEnvironment,
) -> Result<(Vec<FixtureId>, SelectionExpression), ActionError> {
    let fixtures = resolve_group(group_id, &environment.groups).map_err(invalid)?;
    let selected = apply_selection_rule(&fixtures, rule);
    let expression = if frozen {
        SelectionExpression::FrozenGroup {
            group_id: group_id.to_owned(),
            source_revision: environment.show_revision,
        }
    } else {
        SelectionExpression::LiveGroup {
            group_id: group_id.to_owned(),
            rule: rule.clone(),
        }
    };
    Ok((selected, expression))
}

fn gesture_query(source: &SelectionGestureSource) -> ProgrammingSelectionQuery {
    match source {
        SelectionGestureSource::Fixture { fixture_id } => {
            ProgrammingSelectionQuery::Fixtures(vec![*fixture_id])
        }
        SelectionGestureSource::LiveGroup { group_id }
        | SelectionGestureSource::DereferencedGroup { group_id } => {
            ProgrammingSelectionQuery::Groups(vec![group_id.clone()])
        }
    }
}

fn expand_fixtures(
    fixtures: &[FixtureId],
    environment: &ProgrammingSelectionEnvironment,
) -> Result<Vec<FixtureId>, ActionError> {
    let mut expanded = Vec::new();
    let mut seen = HashSet::new();
    for fixture in fixtures {
        let selectable = environment
            .selectable_fixtures
            .get(fixture)
            .ok_or_else(|| invalid("fixture does not exist"))?;
        for fixture in selectable {
            if seen.insert(*fixture) {
                expanded.push(*fixture);
            }
        }
    }
    Ok(expanded)
}

fn gesture_references(
    source: &SelectionGestureSource,
    remove: bool,
    environment: &ProgrammingSelectionEnvironment,
) -> Result<Vec<SelectionReference>, ActionError> {
    match source {
        SelectionGestureSource::Fixture { fixture_id } => environment
            .selectable_fixtures
            .get(fixture_id)
            .ok_or_else(|| invalid("fixture does not exist"))
            .map(|fixtures| {
                fixtures
                    .iter()
                    .map(|id| fixture_reference(*id, remove))
                    .collect()
            }),
        SelectionGestureSource::LiveGroup { group_id } => {
            resolve_group(group_id, &environment.groups).map_err(invalid)?;
            Ok(vec![group_reference(group_id, remove)])
        }
        SelectionGestureSource::DereferencedGroup { group_id } => {
            resolve_group(group_id, &environment.groups)
                .map_err(invalid)
                .map(|fixtures| {
                    fixtures
                        .into_iter()
                        .map(|fixture| fixture_reference(fixture, remove))
                        .collect()
                })
        }
    }
}

fn fixture_reference(fixture_id: FixtureId, remove: bool) -> SelectionReference {
    if remove {
        SelectionReference::RemoveFixture { fixture_id }
    } else {
        SelectionReference::Fixture { fixture_id }
    }
}

fn group_reference(group_id: &str, remove: bool) -> SelectionReference {
    if remove {
        SelectionReference::RemoveLiveGroup {
            group_id: group_id.to_owned(),
        }
    } else {
        SelectionReference::LiveGroup {
            group_id: group_id.to_owned(),
        }
    }
}

fn rule_base(
    current: &light_programmer::ProgrammerSelection,
    rule: &SelectionRule,
    environment: &ProgrammingSelectionEnvironment,
) -> Result<(Vec<FixtureId>, SelectionExpression), ActionError> {
    match &current.expression {
        Some(SelectionExpression::LiveGroup { group_id, .. }) => Ok((
            resolve_group(group_id, &environment.groups).map_err(invalid)?,
            SelectionExpression::LiveGroup {
                group_id: group_id.clone(),
                rule: rule.clone(),
            },
        )),
        _ => Ok((current.selected.clone(), SelectionExpression::Static)),
    }
}

fn validate_rule(rule: &SelectionRule) -> Result<(), ActionError> {
    rule.validate().map_err(invalid)
}

fn selection_accepted(
    programmers: &light_programmer::ProgrammerRegistry,
    session: SessionId,
    action: ProgrammingAction,
    warning: Option<String>,
) -> Result<ProgrammingOutcome, ActionError> {
    let applied = programmers
        .selection(session)
        .ok_or_else(unknown_programmer)?
        .selected
        .len();
    Ok(accepted(action, Some(applied), warning))
}

fn invalid(error: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, error.into())
}
