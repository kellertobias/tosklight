use super::model::{HighlightFixture, HighlightSelectionWrite};
use super::state::OperatorState;
use light_core::FixtureId;
use light_programmer::{
    GroupDefinition, ProgrammerSelection, SelectionExpression, apply_selection_rule, resolve_group,
    resolve_selection_references,
};
use std::collections::{HashMap, HashSet};

fn valid_selection(selection: &[FixtureId], valid_fixtures: &[HighlightFixture]) -> Vec<FixtureId> {
    let valid = valid_fixtures
        .iter()
        .map(|fixture| fixture.fixture_id)
        .collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    selection
        .iter()
        .copied()
        .filter(|fixture| valid.contains(fixture) && seen.insert(*fixture))
        .collect()
}

fn resolve_expression(
    expression: &SelectionExpression,
    snapshot: &[FixtureId],
    groups: &HashMap<String, GroupDefinition>,
) -> Vec<FixtureId> {
    match expression {
        SelectionExpression::LiveGroup { group_id, rule } => resolve_group(group_id, groups)
            .map(|fixtures| apply_selection_rule(&fixtures, rule))
            .unwrap_or_default(),
        SelectionExpression::PlaybackContents { items }
        | SelectionExpression::Sources { items } => resolve_selection_references(items, groups),
        // A frozen selection and an explicitly static selection retain the exact resolved order
        // present when stepping began.
        SelectionExpression::Static | SelectionExpression::FrozenGroup { .. } => snapshot.to_vec(),
    }
}

pub(super) fn resolve_remembered(
    operator: &OperatorState,
    valid_fixtures: &[HighlightFixture],
    groups: &HashMap<String, GroupDefinition>,
) -> Vec<FixtureId> {
    let resolved = operator
        .remembered_expression
        .as_ref()
        .map(|expression| resolve_expression(expression, &operator.remembered, groups))
        .unwrap_or_else(|| operator.remembered.clone());
    valid_selection(&resolved, valid_fixtures)
}

fn reset_basis(
    operator: &mut OperatorState,
    current_selection: &ProgrammerSelection,
    valid_fixtures: &[HighlightFixture],
    groups: &HashMap<String, GroupDefinition>,
) {
    let snapshot = valid_selection(&current_selection.selected, valid_fixtures);
    let remembered = current_selection
        .expression
        .as_ref()
        .map(|expression| resolve_expression(expression, &snapshot, groups))
        .unwrap_or(snapshot);
    operator.remembered = valid_selection(&remembered, valid_fixtures);
    operator.remembered_expression = current_selection.expression.clone();
    operator.stepping = false;
    operator.active_fixture = None;
    operator.message = None;
    operator.observed_selection_revision = Some(current_selection.revision);
}

pub(super) fn synchronize_actual_selection(
    operator: &mut OperatorState,
    current_selection: &ProgrammerSelection,
    valid_fixtures: &[HighlightFixture],
    groups: &HashMap<String, GroupDefinition>,
) -> Option<HighlightSelectionWrite> {
    if operator.observed_selection_revision != Some(current_selection.revision) {
        reset_basis(operator, current_selection, valid_fixtures, groups);
        return None;
    }

    let previous = operator.remembered.clone();
    let previous_index = operator
        .active_fixture
        .and_then(|active| previous.iter().position(|candidate| *candidate == active));
    operator.remembered = resolve_remembered(operator, valid_fixtures, groups);
    if !operator.stepping
        || operator
            .active_fixture
            .is_some_and(|active| operator.remembered.contains(&active))
    {
        return None;
    }
    operator.active_fixture = previous_index.and_then(|index| {
        (!operator.remembered.is_empty())
            .then(|| operator.remembered[index.min(operator.remembered.len() - 1)])
    });
    operator.message = Some("Removed selection item skipped while stepping".into());
    Some(HighlightSelectionWrite {
        selected: operator.active_fixture.into_iter().collect(),
        expression: Some(SelectionExpression::Static),
    })
}
