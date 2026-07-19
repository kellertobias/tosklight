use light_application::{
    ProgrammingAction, ProgrammingCommand, ProgrammingOutcome, ProgrammingResult,
    SelectionGestureSource,
};
use light_wire::v2::command_line::{
    ProgrammerSelectionRule, ProgrammingSelectionAcceptedAction, ProgrammingSelectionAction,
    ProgrammingSelectionActionOutcome, ProgrammingSelectionGestureSource,
};

use super::super::ApiError;

const JAVASCRIPT_MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

pub(super) fn selection_command(
    action: ProgrammingSelectionAction,
) -> Result<ProgrammingCommand, ApiError> {
    Ok(match action {
        ProgrammingSelectionAction::Replace {
            fixtures,
            expected_revision,
        } => ProgrammingCommand::ReplaceSelection {
            fixtures: fixtures.into_iter().map(light_core::FixtureId).collect(),
            expected_revision,
        },
        ProgrammingSelectionAction::Gesture { source, remove } => {
            ProgrammingCommand::ApplySelectionGesture {
                source: selection_source(source),
                remove,
            }
        }
        ProgrammingSelectionAction::SelectGroup {
            group_id,
            frozen,
            rule,
            expected_revision,
        } => ProgrammingCommand::SelectGroup {
            group_id,
            frozen,
            rule: selection_rule(rule)?,
            expected_revision,
        },
        ProgrammingSelectionAction::ApplyRule { rule } => ProgrammingCommand::ApplySelectionRule {
            rule: selection_rule(rule)?,
        },
    })
}

pub(super) fn selection_response(
    request_id: String,
    result: ProgrammingResult,
) -> Result<ProgrammingSelectionActionOutcome, ApiError> {
    let ProgrammingOutcome::Accepted {
        action,
        applied,
        warning,
    } = result.outcome
    else {
        return Err(ApiError::internal(
            "selection service returned a non-accepted outcome",
        ));
    };
    let selection = result
        .selection
        .as_ref()
        .ok_or_else(|| ApiError::internal("selection service omitted its projection"))?;
    Ok(ProgrammingSelectionActionOutcome {
        request_id,
        correlation_id: result.context.correlation_id,
        action: selection_action(action)?,
        applied: applied.unwrap_or_default().try_into().map_err(|_| {
            ApiError::internal("selection target count exceeds the wire integer width")
        })?,
        selection: super::interaction_wire::selection_projection(selection),
        event_sequence: result.interaction_event_sequence.ok_or_else(|| {
            ApiError::internal("selection service omitted its authoritative event sequence")
        })?,
        replayed: result.replayed,
        warning,
    })
}

fn selection_source(source: ProgrammingSelectionGestureSource) -> SelectionGestureSource {
    match source {
        ProgrammingSelectionGestureSource::Fixture { fixture_id } => {
            SelectionGestureSource::Fixture {
                fixture_id: light_core::FixtureId(fixture_id),
            }
        }
        ProgrammingSelectionGestureSource::LiveGroup { group_id } => {
            SelectionGestureSource::LiveGroup { group_id }
        }
        ProgrammingSelectionGestureSource::DereferencedGroup { group_id } => {
            SelectionGestureSource::DereferencedGroup { group_id }
        }
    }
}

fn selection_rule(
    rule: ProgrammerSelectionRule,
) -> Result<light_programmer::SelectionRule, ApiError> {
    Ok(match rule {
        ProgrammerSelectionRule::All => light_programmer::SelectionRule::All,
        ProgrammerSelectionRule::Odd => light_programmer::SelectionRule::Odd,
        ProgrammerSelectionRule::Even => light_programmer::SelectionRule::Even,
        ProgrammerSelectionRule::EveryNth { n, offset } => {
            light_programmer::SelectionRule::EveryNth {
                n: selection_rule_integer(n, "n")?,
                offset: selection_rule_integer(offset, "offset")?,
            }
        }
    })
}

fn selection_rule_integer(value: u64, field: &str) -> Result<usize, ApiError> {
    if value > JAVASCRIPT_MAX_SAFE_INTEGER {
        return Err(ApiError::bad_request(format!(
            "selection rule {field} exceeds the public safe-integer limit"
        )));
    }
    value
        .try_into()
        .map_err(|_| ApiError::bad_request(format!("selection rule {field} is too large")))
}

fn selection_action(
    action: ProgrammingAction,
) -> Result<ProgrammingSelectionAcceptedAction, ApiError> {
    match action {
        ProgrammingAction::SelectionReplaced => Ok(ProgrammingSelectionAcceptedAction::Replaced),
        ProgrammingAction::SelectionGestureApplied => {
            Ok(ProgrammingSelectionAcceptedAction::GestureApplied)
        }
        ProgrammingAction::GroupSelected => Ok(ProgrammingSelectionAcceptedAction::GroupSelected),
        ProgrammingAction::SelectionRuleApplied => {
            Ok(ProgrammingSelectionAcceptedAction::RuleApplied)
        }
        _ => Err(ApiError::internal(
            "selection service returned a command-line action",
        )),
    }
}
