use light_application as application;
use light_wire::v2::programmer_priority as wire;

pub(super) fn outcome(
    result: application::ProgrammingPriorityResult,
) -> wire::ProgrammerPriorityActionOutcome {
    let outcome = match result.outcome {
        application::ProgrammingPriorityActionState::Changed { event_sequence } => {
            wire::ProgrammerPriorityActionState::Changed { event_sequence }
        }
        application::ProgrammingPriorityActionState::NoChange => {
            wire::ProgrammerPriorityActionState::NoChange
        }
    };
    wire::ProgrammerPriorityActionOutcome {
        request_id: result.request_id,
        correlation_id: result.context.correlation_id,
        projection: projection(&result.projection),
        outcome,
        replayed: result.replayed,
        warning: result.warning,
    }
}

pub(super) fn snapshot(
    snapshot: application::ProgrammingPrioritySnapshot,
) -> wire::ProgrammerPrioritySnapshot {
    wire::ProgrammerPrioritySnapshot {
        cursor: light_wire::v2::events::EventSnapshotCursor {
            sequence: snapshot.event_sequence,
        },
        projection: projection(&snapshot.projection),
    }
}

pub(in crate::runtime) fn change(
    change: &application::ProgrammingPriorityChange,
) -> wire::ProgrammerPriorityChange {
    match change {
        application::ProgrammingPriorityChange::Upsert { projection: value } => {
            wire::ProgrammerPriorityChange::Upsert {
                projection: projection(value),
            }
        }
        application::ProgrammingPriorityChange::Remove { user_id, revision } => {
            wire::ProgrammerPriorityChange::Remove {
                user_id: user_id.0,
                revision: *revision,
            }
        }
    }
}

fn projection(
    projection: &application::ProgrammingPriorityProjection,
) -> wire::ProgrammerPriorityProjection {
    wire::ProgrammerPriorityProjection {
        user_id: projection.user_id.0,
        revision: projection.revision,
        priority: projection.priority,
        changed_at: projection.changed_at.to_rfc3339(),
    }
}
