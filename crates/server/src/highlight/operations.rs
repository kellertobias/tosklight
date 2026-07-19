use super::model::{
    HighlightAction, HighlightError, HighlightFixture, HighlightMode, HighlightSelectionWrite,
    HighlightState,
};
use super::selection::resolve_remembered;
use super::state::{HighlightRuntime, OperatorState};
use light_core::{FixtureId, UserId};
use light_programmer::{GroupDefinition, SelectionExpression};
use std::collections::HashMap;
use uuid::Uuid;

pub(super) struct ActionContext<'a> {
    pub(super) desk_id: Uuid,
    pub(super) user_id: UserId,
    pub(super) valid_fixtures: &'a [HighlightFixture],
    pub(super) groups: &'a HashMap<String, GroupDefinition>,
    pub(super) capture_only: bool,
}

pub(super) fn apply_action(
    runtime: &mut HighlightRuntime,
    operator: &mut OperatorState,
    action: HighlightAction,
    context: &ActionContext<'_>,
) -> Result<Option<HighlightSelectionWrite>, HighlightError> {
    match action {
        HighlightAction::On => enable_highlight(runtime, operator, context)?,
        HighlightAction::Off => disable_highlight(runtime, operator, context),
        HighlightAction::Toggle if operator.active => {
            disable_highlight(runtime, operator, context);
        }
        HighlightAction::Toggle => enable_highlight(runtime, operator, context)?,
        HighlightAction::Next | HighlightAction::Previous => {
            return Ok(Some(step_selection(operator, action, context)));
        }
        HighlightAction::All => return Ok(restore_selection(operator, context)),
    }
    Ok(None)
}

fn acquire_output_owner(
    runtime: &mut HighlightRuntime,
    desk_id: Uuid,
    user_id: UserId,
) -> Result<(), HighlightError> {
    if let Some(owner) = runtime.output_owners.get(&desk_id)
        && *owner != user_id
    {
        return Err(HighlightError::OwnedByAnotherUser(*owner));
    }
    runtime.output_owners.insert(desk_id, user_id);
    Ok(())
}

fn enable_highlight(
    runtime: &mut HighlightRuntime,
    operator: &mut OperatorState,
    context: &ActionContext<'_>,
) -> Result<(), HighlightError> {
    operator.active = true;
    if context.capture_only {
        operator.output_enabled = false;
        operator.message = Some(blind_message().into());
    } else {
        acquire_output_owner(runtime, context.desk_id, context.user_id)?;
        operator.output_enabled = true;
    }
    Ok(())
}

fn disable_highlight(
    runtime: &mut HighlightRuntime,
    operator: &mut OperatorState,
    context: &ActionContext<'_>,
) {
    operator.active = false;
    operator.output_enabled = false;
    if runtime.output_owners.get(&context.desk_id) == Some(&context.user_id) {
        runtime.output_owners.remove(&context.desk_id);
    }
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
        expression: operator.remembered_expression.clone(),
    })
}

fn static_selection(selected: Vec<FixtureId>) -> HighlightSelectionWrite {
    HighlightSelectionWrite {
        selected,
        expression: Some(SelectionExpression::Static),
    }
}

pub(super) fn reconcile_capture_mode(
    runtime: &mut HighlightRuntime,
    operator: &mut OperatorState,
    context: &ActionContext<'_>,
) {
    if context.capture_only && operator.output_enabled {
        operator.output_enabled = false;
        if runtime.output_owners.get(&context.desk_id) == Some(&context.user_id) {
            runtime.output_owners.remove(&context.desk_id);
        }
        operator.message = Some(blind_message().into());
    }
}

pub(super) fn restore_live_output(
    runtime: &mut HighlightRuntime,
    operator: &mut OperatorState,
    context: &ActionContext<'_>,
) {
    if !operator.active || context.capture_only || operator.output_enabled {
        return;
    }
    if runtime
        .output_owners
        .get(&context.desk_id)
        .is_none_or(|owner| *owner == context.user_id)
    {
        runtime
            .output_owners
            .insert(context.desk_id, context.user_id);
        operator.output_enabled = true;
        operator.message = None;
    } else {
        operator.message = Some("Highlight output is active for another user on this desk".into());
    }
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

pub(super) fn response(
    operator: &OperatorState,
    fixtures: &[HighlightFixture],
    capture_only: bool,
    owner_user_id: Option<UserId>,
    owner_user_name: Option<String>,
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
    let can_step = !operator.remembered.is_empty();
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
        owner_user_id,
        owner_user_name,
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
