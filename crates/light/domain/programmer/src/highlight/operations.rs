use super::model::{
    HighlightAction, HighlightError, HighlightFixture, HighlightMode, HighlightOutputLayer,
    HighlightOutputRole, HighlightSelectionWrite, HighlightState,
};
use super::selection::resolve_remembered;
use super::state::OperatorState;
use crate::{GroupDefinition, SelectionExpression};
use light_core::FixtureId;
use std::collections::HashMap;

pub(super) struct ActionContext<'a> {
    pub(super) valid_fixtures: &'a [HighlightFixture],
    pub(super) groups: &'a HashMap<String, GroupDefinition>,
    pub(super) capture_only: bool,
}

pub(super) fn apply_action(
    operator: &mut OperatorState,
    action: HighlightAction,
    context: &ActionContext<'_>,
) -> Result<Option<HighlightSelectionWrite>, HighlightError> {
    match action {
        HighlightAction::On => enable_highlight(operator, context)?,
        HighlightAction::Off => disable_highlight(operator),
        HighlightAction::Toggle if operator.active => {
            disable_highlight(operator);
        }
        HighlightAction::Toggle => enable_highlight(operator, context)?,
        HighlightAction::Next | HighlightAction::Previous if operator.active => {
            return Ok(Some(step_selection(operator, action, context)));
        }
        HighlightAction::All if operator.active => return Ok(restore_selection(operator, context)),
        HighlightAction::Next | HighlightAction::Previous | HighlightAction::All => {}
    }
    Ok(None)
}

fn enable_highlight(
    operator: &mut OperatorState,
    context: &ActionContext<'_>,
) -> Result<(), HighlightError> {
    if !operator.active {
        operator.explicit_attributes.clear();
    }
    operator.active = true;
    if context.capture_only {
        operator.output_enabled = false;
        operator.message = Some(blind_message().into());
    } else {
        operator.output_enabled = true;
    }
    Ok(())
}

fn disable_highlight(operator: &mut OperatorState) {
    operator.active = false;
    operator.output_enabled = false;
    operator.explicit_attributes.clear();
    operator.observed_selection_revision = None;
}

fn step_selection(
    operator: &mut OperatorState,
    action: HighlightAction,
    context: &ActionContext<'_>,
) -> HighlightSelectionWrite {
    operator.remembered = resolve_remembered(operator, context.valid_fixtures, context.groups);
    if operator.remembered.is_empty() {
        operator.stepping = true;
        operator.active_fixture = None;
        operator.message = Some("The remembered selection has no valid items".into());
        return static_selection(Vec::new());
    }

    let index = step_index(operator, action);
    let fixture = operator.remembered[index];
    operator.stepping = true;
    operator.active_fixture = Some(fixture);
    static_selection(vec![fixture])
}

fn step_index(operator: &OperatorState, action: HighlightAction) -> usize {
    if !operator.stepping {
        return if action == HighlightAction::Next {
            0
        } else {
            operator.remembered.len() - 1
        };
    }
    operator
        .active_fixture
        .and_then(|active| {
            operator
                .remembered
                .iter()
                .position(|fixture| *fixture == active)
        })
        .map(|index| adjacent_index(index, operator.remembered.len(), action))
        .unwrap_or_else(|| {
            if action == HighlightAction::Next {
                0
            } else {
                operator.remembered.len() - 1
            }
        })
}

fn adjacent_index(index: usize, len: usize, action: HighlightAction) -> usize {
    match action {
        HighlightAction::Next => (index + 1) % len,
        HighlightAction::Previous => (index + len - 1) % len,
        _ => unreachable!(),
    }
}

fn restore_selection(
    operator: &mut OperatorState,
    context: &ActionContext<'_>,
) -> Option<HighlightSelectionWrite> {
    operator.remembered = resolve_remembered(operator, context.valid_fixtures, context.groups);
    if !operator.stepping {
        return None;
    }
    operator.stepping = false;
    operator.active_fixture = None;
    Some(HighlightSelectionWrite {
        selected: operator.remembered.clone(),
        expression: if operator.active {
            Some(SelectionExpression::Static)
        } else {
            operator.remembered_expression.clone()
        },
    })
}

fn static_selection(selected: Vec<FixtureId>) -> HighlightSelectionWrite {
    HighlightSelectionWrite {
        selected,
        expression: Some(SelectionExpression::Static),
    }
}

pub(super) fn reconcile_capture_mode(operator: &mut OperatorState, context: &ActionContext<'_>) {
    if context.capture_only && operator.output_enabled {
        operator.output_enabled = false;
        operator.message = Some(blind_message().into());
    }
}

pub(super) fn restore_live_output(operator: &mut OperatorState, context: &ActionContext<'_>) {
    if !operator.active || context.capture_only || operator.output_enabled {
        return;
    }
    // Nothing to contend with: leaving Blind hands live output back to the desk.
    operator.output_enabled = true;
    operator.message = None;
}

fn blind_message() -> &'static str {
    "Highlight prepared in Blind/Preview; live output is suppressed"
}

pub(super) fn output_fixture_ids(operator: &OperatorState) -> Vec<FixtureId> {
    if !operator.active || !operator.output_enabled {
        return Vec::new();
    }
    if operator.stepping {
        operator.active_fixture.into_iter().collect()
    } else {
        operator.remembered.clone()
    }
}

pub(super) fn output_layers(operator: &OperatorState) -> Vec<HighlightOutputLayer> {
    if !operator.active || !operator.output_enabled {
        return Vec::new();
    }
    operator
        .remembered
        .iter()
        .copied()
        .map(|fixture_id| HighlightOutputLayer {
            fixture_id,
            role: if !operator.stepping || operator.active_fixture == Some(fixture_id) {
                HighlightOutputRole::Highlight
            } else {
                HighlightOutputRole::LowLight
            },
            suppressed_attributes: operator
                .explicit_attributes
                .get(&fixture_id)
                .cloned()
                .unwrap_or_default(),
        })
        .collect()
}

pub(super) fn response(
    operator: &OperatorState,
    fixtures: &[HighlightFixture],
    capture_only: bool,
) -> HighlightState {
    let by_id = fixtures
        .iter()
        .map(|fixture| (fixture.fixture_id, fixture.clone()))
        .collect::<HashMap<_, _>>();
    let remembered = operator
        .remembered
        .iter()
        .filter_map(|fixture| by_id.get(fixture).cloned())
        .collect::<Vec<_>>();
    let active_fixture = operator
        .stepping
        .then_some(operator.active_fixture)
        .flatten()
        .and_then(|fixture| by_id.get(&fixture).cloned());
    let can_step = operator.active && !operator.remembered.is_empty();
    HighlightState {
        active: operator.active,
        mode: highlight_mode(operator),
        output_enabled: operator.output_enabled,
        capture_only,
        remembered,
        active_index: active_index(operator),
        active_fixture,
        can_previous: can_step,
        can_next: can_step,
        message: operator.message.clone(),
    }
}

fn active_index(operator: &OperatorState) -> Option<usize> {
    operator.stepping.then_some(())?;
    let active = operator.active_fixture?;
    operator
        .remembered
        .iter()
        .position(|fixture| *fixture == active)
}

fn highlight_mode(operator: &OperatorState) -> HighlightMode {
    if operator.stepping {
        HighlightMode::Step
    } else {
        HighlightMode::Selection
    }
}
