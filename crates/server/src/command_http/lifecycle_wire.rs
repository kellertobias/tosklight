use light_application as application;
use light_wire::v2::{events::EventSnapshotCursor, programmer_lifecycle as wire};

pub(super) fn lifecycle_snapshot(
    snapshot: application::ProgrammingLifecycleSnapshot,
) -> wire::ProgrammingLifecycleSnapshot {
    wire::ProgrammingLifecycleSnapshot {
        cursor: EventSnapshotCursor {
            sequence: snapshot.event_sequence,
        },
        projection: lifecycle_projection(snapshot.projection),
    }
}

pub(in crate::runtime) fn lifecycle_change(
    change: &application::ProgrammingLifecycleChange,
) -> wire::ProgrammingLifecycleChange {
    wire::ProgrammingLifecycleChange {
        revision: change.revision,
        delta: match &change.delta {
            application::ProgrammingLifecycleDelta::Upsert { programmer } => {
                wire::ProgrammingLifecycleDelta::Upsert {
                    programmer: lifecycle_programmer(programmer),
                }
            }
            application::ProgrammingLifecycleDelta::Remove { programmer_id } => {
                wire::ProgrammingLifecycleDelta::Remove {
                    programmer_id: programmer_id.0,
                }
            }
        },
    }
}

fn lifecycle_projection(
    projection: application::ProgrammingLifecycleProjection,
) -> wire::ProgrammingLifecycleProjection {
    wire::ProgrammingLifecycleProjection {
        revision: projection.revision,
        programmers: projection
            .programmers
            .iter()
            .map(lifecycle_programmer)
            .collect(),
    }
}

fn lifecycle_programmer(
    programmer: &application::ProgrammingLifecycleProgrammer,
) -> wire::ProgrammingLifecycleProgrammer {
    wire::ProgrammingLifecycleProgrammer {
        programmer_id: programmer.programmer_id.0,
        user_id: programmer.user_id.0,
        connected: programmer.connected,
        selected_fixture_count: programmer.selected_fixture_count,
        normal_value_count: programmer.normal_value_count,
        preload_active: programmer.preload_active,
        sessions: programmer
            .sessions
            .iter()
            .map(|session| wire::ProgrammingLifecycleSession {
                session_id: session.session_id.0,
            })
            .collect(),
    }
}
