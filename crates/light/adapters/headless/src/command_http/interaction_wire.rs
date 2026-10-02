use light_application as application;
use light_programmer::{SelectionExpression, SelectionReference, SelectionRule};
use light_wire::v2::{command_line as wire, events::EventSnapshotCursor};

use super::wire::command_line_from_state;

pub(super) fn interaction_snapshot(
    snapshot: application::ProgrammingLiveSnapshot,
) -> wire::ProgrammingInteractionSnapshot {
    wire::ProgrammingInteractionSnapshot {
        cursor: EventSnapshotCursor {
            sequence: snapshot.event_sequence,
        },
        projection: interaction_projection(&snapshot.interaction),
    }
}

pub(in crate::runtime) fn interaction_projection(
    projection: &application::ProgrammingInteractionProjection,
) -> wire::ProgrammingInteractionProjection {
    wire::ProgrammingInteractionProjection {
        desk_id: projection.desk_id,
        command_line: command_line_from_state(projection.command_line.clone()),
        selection: selection_projection(&projection.selection),
        alignment: alignment_projection(&projection.alignment),
    }
}

pub(in crate::runtime) fn interaction_change(
    change: &application::ProgrammingInteractionChange,
) -> wire::ProgrammingInteractionChange {
    match (
        change.command_line(),
        change.selection(),
        change.alignment(),
    ) {
        (Some(command_line), Some(selection), Some(alignment)) => {
            wire::ProgrammingInteractionChange::All {
                desk_id: change.desk_id(),
                command_line: command_line_from_state(command_line.clone()),
                selection: selection_projection(selection),
                alignment: alignment_projection(alignment),
            }
        }
        (Some(command_line), None, Some(alignment)) => {
            wire::ProgrammingInteractionChange::CommandLineAlignment {
                desk_id: change.desk_id(),
                command_line: command_line_from_state(command_line.clone()),
                alignment: alignment_projection(alignment),
            }
        }
        (None, Some(selection), Some(alignment)) => {
            wire::ProgrammingInteractionChange::SelectionAlignment {
                desk_id: change.desk_id(),
                selection: selection_projection(selection),
                alignment: alignment_projection(alignment),
            }
        }
        (None, None, Some(alignment)) => wire::ProgrammingInteractionChange::Alignment {
            desk_id: change.desk_id(),
            alignment: alignment_projection(alignment),
        },
        (Some(command_line), Some(selection), None) => wire::ProgrammingInteractionChange::Both {
            desk_id: change.desk_id(),
            command_line: command_line_from_state(command_line.clone()),
            selection: selection_projection(selection),
        },
        (Some(command_line), None, None) => wire::ProgrammingInteractionChange::CommandLine {
            desk_id: change.desk_id(),
            command_line: command_line_from_state(command_line.clone()),
        },
        (None, Some(selection), None) => wire::ProgrammingInteractionChange::Selection {
            desk_id: change.desk_id(),
            selection: selection_projection(selection),
        },
        (None, None, None) => unreachable!("application Programming changes are non-empty"),
    }
}

pub(in crate::runtime) fn alignment_projection(
    value: &light_programmer::ProgrammerAlignmentProjection,
) -> wire::ProgrammingAlignmentProjection {
    use super::intent_wire::ToIntentWire;
    use light_programmer::{
        ProgrammerAlignmentLane as Lane, ProgrammerAlignmentMode as Mode,
        ProgrammerAlignmentProjectionBinding as Binding,
    };
    use light_wire::v2::live_action::ProgrammingAlignMode as WireMode;
    wire::ProgrammingAlignmentProjection {
        revision: value.revision,
        fixture_count: value.fixture_count,
        mode: match value.mode {
            None => WireMode::Off,
            Some(Mode::Left) => WireMode::Left,
            Some(Mode::Right) => WireMode::Right,
            Some(Mode::Out) => WireMode::Out,
            Some(Mode::In) => WireMode::In,
        },
        binding: value.binding.as_ref().map(|binding| match binding {
            Binding::Attribute { attribute } => wire::ProgrammingAlignmentBinding::Attribute {
                attribute: attribute.0.to_string(),
            },
            Binding::Family {
                component,
                lane,
                group_id,
            } => wire::ProgrammingAlignmentBinding::Family {
                component: component.to_intent_wire(),
                lane: match lane {
                    Lane::Normal => wire::ProgrammingAlignmentLane::Normal,
                    Lane::Preload => wire::ProgrammingAlignmentLane::Preload,
                },
                group_id: group_id.clone(),
            },
        }),
    }
}

pub(super) fn selection_projection(
    selection: &light_programmer::ProgrammerSelection,
) -> wire::ProgrammerSelectionProjection {
    wire::ProgrammerSelectionProjection {
        selected: selection
            .selected
            .iter()
            .map(|fixture_id| fixture_id.0)
            .collect(),
        expression: selection.expression.as_ref().map(expression),
        revision: selection.revision,
        gesture_open: selection.gesture_open,
    }
}

fn expression(value: &SelectionExpression) -> wire::ProgrammerSelectionExpression {
    match value {
        SelectionExpression::Static => wire::ProgrammerSelectionExpression::Static,
        SelectionExpression::LiveGroup { group_id, rule } => live_group(group_id, rule),
        SelectionExpression::PlaybackContents { items } => playback_contents(items),
        SelectionExpression::Sources { items } => selection_sources(items),
    }
}

fn live_group(group_id: &str, rule: &SelectionRule) -> wire::ProgrammerSelectionExpression {
    wire::ProgrammerSelectionExpression::LiveGroup {
        group_id: group_id.to_owned(),
        rule: selection_rule(rule),
    }
}

fn playback_contents(items: &[SelectionReference]) -> wire::ProgrammerSelectionExpression {
    wire::ProgrammerSelectionExpression::PlaybackContents {
        items: items.iter().map(selection_reference).collect(),
    }
}

fn selection_sources(items: &[SelectionReference]) -> wire::ProgrammerSelectionExpression {
    wire::ProgrammerSelectionExpression::Sources {
        items: items.iter().map(selection_reference).collect(),
    }
}

fn selection_rule(value: &SelectionRule) -> wire::ProgrammerSelectionRule {
    match value {
        SelectionRule::All => wire::ProgrammerSelectionRule::All,
        SelectionRule::Odd => wire::ProgrammerSelectionRule::Odd,
        SelectionRule::Even => wire::ProgrammerSelectionRule::Even,
        SelectionRule::EveryNth { n, offset } => wire::ProgrammerSelectionRule::EveryNth {
            n: (*n)
                .try_into()
                .expect("usize fits in the wire revision width"),
            offset: (*offset)
                .try_into()
                .expect("usize fits in the wire revision width"),
        },
    }
}

fn selection_reference(value: &SelectionReference) -> wire::ProgrammerSelectionReference {
    match value {
        SelectionReference::Fixture { fixture_id } => fixture_reference(fixture_id.0, false),
        SelectionReference::LiveGroup { group_id } => group_reference(group_id, false),
        SelectionReference::RemoveFixture { fixture_id } => fixture_reference(fixture_id.0, true),
        SelectionReference::RemoveLiveGroup { group_id } => group_reference(group_id, true),
    }
}

fn fixture_reference(fixture_id: uuid::Uuid, remove: bool) -> wire::ProgrammerSelectionReference {
    if remove {
        wire::ProgrammerSelectionReference::RemoveFixture { fixture_id }
    } else {
        wire::ProgrammerSelectionReference::Fixture { fixture_id }
    }
}

fn group_reference(group_id: &str, remove: bool) -> wire::ProgrammerSelectionReference {
    if remove {
        wire::ProgrammerSelectionReference::RemoveLiveGroup {
            group_id: group_id.to_owned(),
        }
    } else {
        wire::ProgrammerSelectionReference::LiveGroup {
            group_id: group_id.to_owned(),
        }
    }
}
