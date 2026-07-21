use super::*;
use crate::{ActionErrorKind, ProgrammingPriorityActionState};
use chrono::{TimeZone, Utc};
use light_core::{AttributeKey, AttributeValue, ManualClock};

#[test]
fn priority_is_lightweight_revisioned_replay_safe_and_stable_across_unrelated_values() {
    let started_at = Utc.with_ymd_and_hms(2026, 7, 21, 10, 0, 0).unwrap();
    let clock = Arc::new(ManualClock::new(started_at));
    let registry = ProgrammerRegistry::with_clock(clock.clone());
    let desk = Uuid::new_v4();
    let session = SessionId::new();
    let user = UserId::new();
    registry.start(session, user);
    let events = EventBus::new(8);
    let service = ProgrammingService::new(
        registry.clone(),
        events.clone(),
        Arc::new(HighlightRegistry::default()),
    );
    let ports = LivePorts::default();
    let context = ActionContext::operator(desk, user.0, session.0, ActionSource::Http);
    crate::programming::values_projection::reset_projection_read_count();

    clock.advance_millis(1_000);
    let first = service
        .handle_priority(
            ActionEnvelope {
                context: context.clone().with_request_id("priority-1"),
                command: ProgrammingPriorityRequest {
                    expected_revision: ProgrammingPriorityRevisionExpectation::Exact(0),
                    priority: 120,
                },
            },
            &ports,
        )
        .unwrap();
    assert_eq!(first.projection.revision, 1);
    assert_eq!(
        first.projection.changed_at,
        started_at + chrono::Duration::seconds(1)
    );
    assert!(matches!(
        first.outcome,
        ProgrammingPriorityActionState::Changed { event_sequence: 1 }
    ));
    assert_eq!(
        *ports.persisted_operations.lock(),
        vec!["programmer.priority"]
    );
    assert_eq!(
        crate::programming::values_projection::projection_read_count(),
        0
    );

    let replay = service
        .handle_priority(
            ActionEnvelope {
                context: context.clone().with_request_id("priority-1"),
                command: ProgrammingPriorityRequest {
                    expected_revision: ProgrammingPriorityRevisionExpectation::Exact(0),
                    priority: 120,
                },
            },
            &ports,
        )
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(events.latest_sequence(), 1);
    assert_eq!(ports.persisted_operations.lock().len(), 1);

    clock.advance_millis(1_000);
    assert!(registry.apply_normal_values(
        session,
        &[
            light_programmer::NormalProgrammerValueMutation::SetFixture {
                fixture_id: FixtureId::new(),
                attribute: AttributeKey::intensity(),
                value: AttributeValue::Normalized(0.5),
                timing: light_programmer::NormalProgrammerValueTiming::default(),
            }
        ],
    ));
    let snapshot = service.priority_snapshot(&context, &ports).unwrap();
    assert_eq!(snapshot.projection, first.projection);
    assert_eq!(snapshot.event_sequence, 1);
    assert_eq!(
        crate::programming::values_projection::projection_read_count(),
        0
    );

    let no_change = service
        .handle_priority(
            ActionEnvelope {
                context: context.clone().with_request_id("priority-2"),
                command: ProgrammingPriorityRequest {
                    expected_revision: ProgrammingPriorityRevisionExpectation::Exact(1),
                    priority: 120,
                },
            },
            &ports,
        )
        .unwrap();
    assert_eq!(no_change.outcome, ProgrammingPriorityActionState::NoChange);
    assert_eq!(events.latest_sequence(), 1);
    assert_eq!(ports.persisted_operations.lock().len(), 1);
}

#[test]
fn priority_snapshot_is_shared_between_user_desks_and_rejects_a_foreign_owner() {
    let registry = ProgrammerRegistry::default();
    let user = UserId::new();
    let first_session = SessionId::new();
    let second_session = SessionId::new();
    registry.start(first_session, user);
    registry.start(second_session, user);
    let service = ProgrammingService::new(
        registry,
        EventBus::new(8),
        Arc::new(HighlightRegistry::default()),
    );
    let ports = LivePorts::default();
    let first =
        ActionContext::operator(Uuid::new_v4(), user.0, first_session.0, ActionSource::Http);
    service
        .handle_priority(
            ActionEnvelope {
                context: first.clone().with_request_id("shared-priority"),
                command: ProgrammingPriorityRequest {
                    expected_revision: ProgrammingPriorityRevisionExpectation::Exact(0),
                    priority: 80,
                },
            },
            &ports,
        )
        .unwrap();
    let peer =
        ActionContext::operator(Uuid::new_v4(), user.0, second_session.0, ActionSource::Http);
    assert_eq!(
        service
            .priority_snapshot(&peer, &ports)
            .unwrap()
            .projection
            .priority,
        80
    );

    let foreign = ActionContext::operator(
        peer.desk_id,
        UserId::new().0,
        second_session.0,
        ActionSource::Http,
    );
    let error = service.priority_snapshot(&foreign, &ports).unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Forbidden);
}

#[test]
fn priority_revision_conflict_does_not_mutate_or_publish() {
    let setup = LiveSetup::new(8);
    let error = setup
        .service
        .handle_priority(
            ActionEnvelope {
                context: setup.context.clone().with_request_id("stale-priority"),
                command: ProgrammingPriorityRequest {
                    expected_revision: ProgrammingPriorityRevisionExpectation::Exact(99),
                    priority: 10,
                },
            },
            &setup.ports,
        )
        .unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert_eq!(error.current_revision, Some(0));
    assert_eq!(setup.events.latest_sequence(), 0);
    assert!(setup.ports.persisted_operations.lock().is_empty());
}
