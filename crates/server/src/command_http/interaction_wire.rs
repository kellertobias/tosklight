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
    }
}

pub(in crate::runtime) fn interaction_change(
    change: &application::ProgrammingInteractionChange,
) -> wire::ProgrammingInteractionChange {
    match (change.command_line(), change.selection()) {
        (Some(command_line), Some(selection)) => wire::ProgrammingInteractionChange::Both {
            desk_id: change.desk_id(),
            command_line: command_line_from_state(command_line.clone()),
            selection: selection_projection(selection),
        },
        (Some(command_line), None) => wire::ProgrammingInteractionChange::CommandLine {
            desk_id: change.desk_id(),
            command_line: command_line_from_state(command_line.clone()),
        },
        (None, Some(selection)) => wire::ProgrammingInteractionChange::Selection {
            desk_id: change.desk_id(),
            selection: selection_projection(selection),
        },
        (None, None) => unreachable!("application Programming changes are non-empty"),
    }
}

fn selection_projection(
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
    }
}

fn expression(value: &SelectionExpression) -> wire::ProgrammerSelectionExpression {
    match value {
        SelectionExpression::Static => wire::ProgrammerSelectionExpression::Static,
        SelectionExpression::LiveGroup { group_id, rule } => live_group(group_id, rule),
        SelectionExpression::FrozenGroup {
            group_id,
            source_revision,
        } => frozen_group(group_id, *source_revision),
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

fn frozen_group(group_id: &str, source_revision: u64) -> wire::ProgrammerSelectionExpression {
    wire::ProgrammerSelectionExpression::FrozenGroup {
        group_id: group_id.to_owned(),
        source_revision,
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
