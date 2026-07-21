use super::*;
use crate::EventSource;
use light_programmer::{GroupDefinition, SelectionExpression, SelectionRule};
use std::collections::HashMap;

#[test]
fn shared_selection_refresh_publishes_changed_desks_once_in_uuid_order() {
    let registry = ProgrammerRegistry::default();
    let low_desk = Uuid::from_u128(1);
    let high_desk = Uuid::from_u128(2);
    let low_session = SessionId::new();
    let high_session = SessionId::new();
    registry.start(low_session, UserId::new());
    registry.start(high_session, UserId::new());
    assert!(registry.attach_command_context(low_session, SessionId(low_desk)));
    assert!(registry.attach_command_context(high_session, SessionId(high_desk)));
    let first = FixtureId::new();
    let second = FixtureId::new();
    for session in [low_session, high_session] {
        registry.select_expression(
            session,
            vec![first],
            SelectionExpression::LiveGroup {
                group_id: "front".into(),
                rule: SelectionRule::All,
            },
        );
    }
    let events = EventBus::new(8);
    let service = ProgrammingService::new(
        registry.clone(),
        events.clone(),
        Arc::new(HighlightRegistry::default()),
    );
    let context = ActionContext::system(high_desk, ActionSource::System);
    let updated_groups = HashMap::from([(
        "front".into(),
        GroupDefinition {
            id: "front".into(),
            fixtures: vec![first, second],
            ..GroupDefinition::default()
        },
    )]);

    let result = service.run_selection_refresh(
        &context,
        [
            ProgrammingSelectionTarget {
                desk_id: high_desk,
                interaction_id: SessionId(high_desk),
            },
            ProgrammingSelectionTarget {
                desk_id: low_desk,
                interaction_id: SessionId(low_desk),
            },
            ProgrammingSelectionTarget {
                desk_id: high_desk,
                interaction_id: SessionId(high_desk),
            },
        ],
        || registry.refresh_live_selections(&updated_groups),
    );

    assert_eq!(
        result
            .events
            .iter()
            .map(|event| event.desk_id)
            .collect::<Vec<_>>(),
        vec![low_desk, high_desk]
    );
    assert_eq!(
        result
            .events
            .iter()
            .map(|event| event.event_sequence)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    let EventReplay::Events(published) = events.replay(
        0,
        &EventFilter::default().with_capability(EventCapability::Desk),
    ) else {
        panic!("selection refresh events should remain replayable")
    };
    assert_eq!(published.len(), 2);
    assert!(published.iter().all(|event| {
        event.correlation_id == Some(context.correlation_id)
            && event.source == EventSource::Action(ActionSource::System)
    }));
    for event in published {
        let ApplicationEvent::Programming(ProgrammingEvent::InteractionChanged(change)) =
            &event.payload
        else {
            panic!("expected a Programming interaction change")
        };
        assert_eq!(change.selection().unwrap().selected, vec![first, second]);
    }
    let EventReplay::Events(lifecycle) = events.replay(
        0,
        &EventFilter::default().with_object(EventObject::programming_lifecycle()),
    ) else {
        panic!("selection counts should publish lifecycle deltas")
    };
    assert_eq!(lifecycle.len(), 2);
    assert_eq!(lifecycle[0].sequence, 3);
    assert_eq!(lifecycle[1].sequence, 4);
}

#[test]
fn shared_selection_refresh_keeps_frozen_and_unchanged_desks_quiet() {
    let registry = ProgrammerRegistry::default();
    let live_desk = Uuid::from_u128(1);
    let frozen_desk = Uuid::from_u128(2);
    let live_session = SessionId::new();
    let frozen_session = SessionId::new();
    registry.start(live_session, UserId::new());
    registry.start(frozen_session, UserId::new());
    assert!(registry.attach_command_context(live_session, SessionId(live_desk)));
    assert!(registry.attach_command_context(frozen_session, SessionId(frozen_desk)));
    let first = FixtureId::new();
    let second = FixtureId::new();
    registry.select_expression(
        live_session,
        vec![first],
        SelectionExpression::LiveGroup {
            group_id: "front".into(),
            rule: SelectionRule::All,
        },
    );
    registry.select_expression(
        frozen_session,
        vec![first],
        SelectionExpression::FrozenGroup {
            group_id: "front".into(),
            source_revision: 1,
        },
    );
    let events = EventBus::new(8);
    let service = ProgrammingService::new(
        registry.clone(),
        events,
        Arc::new(HighlightRegistry::default()),
    );
    let groups = HashMap::from([(
        "front".into(),
        GroupDefinition {
            id: "front".into(),
            fixtures: vec![first, second],
            ..GroupDefinition::default()
        },
    )]);
    let result = service.run_selection_refresh(
        &ActionContext::system(live_desk, ActionSource::System),
        [
            ProgrammingSelectionTarget {
                desk_id: frozen_desk,
                interaction_id: SessionId(frozen_desk),
            },
            ProgrammingSelectionTarget {
                desk_id: live_desk,
                interaction_id: SessionId(live_desk),
            },
        ],
        || registry.refresh_live_selections(&groups),
    );

    assert_eq!(result.events.len(), 1);
    assert_eq!(result.events[0].desk_id, live_desk);
    assert_eq!(
        registry.selection(live_session).unwrap().selected,
        vec![first, second]
    );
    assert_eq!(
        registry.selection(frozen_session).unwrap().selected,
        vec![first]
    );
}

#[test]
fn shared_interaction_refresh_publishes_pending_choice_invalidation_once_per_desk() {
    let registry = ProgrammerRegistry::default();
    let low_desk = Uuid::from_u128(1);
    let high_desk = Uuid::from_u128(2);
    let low_session = SessionId::new();
    let high_session = SessionId::new();
    for (session, desk) in [(low_session, low_desk), (high_session, high_desk)] {
        registry.start(session, UserId::new());
        assert!(registry.attach_command_context(session, SessionId(desk)));
        registry.set_pending_command_choice(
            session,
            Some(CueMoveCopyChoice {
                choice_id: uuid::Uuid::from_u128(1),
                show_id: uuid::Uuid::from_u128(2),
                show_revision: 3,
                operation: CueTransferOperation::Copy,
                command: "COPY SET 1 CUE 1 AT SET 2 CUE 2".into(),
                options: Vec::new(),
                cancel_label: "Cancel".into(),
            }),
        );
    }
    let events = EventBus::new(8);
    let service = ProgrammingService::new(
        registry.clone(),
        events.clone(),
        Arc::new(HighlightRegistry::default()),
    );
    let result = service.run_selection_refresh(
        &ActionContext::system(high_desk, ActionSource::System),
        [
            ProgrammingSelectionTarget {
                desk_id: high_desk,
                interaction_id: SessionId(high_desk),
            },
            ProgrammingSelectionTarget {
                desk_id: low_desk,
                interaction_id: SessionId(low_desk),
            },
        ],
        || registry.clear_pending_command_choices_except_context(None),
    );

    assert_eq!(result.output, 2);
    assert_eq!(
        result
            .events
            .iter()
            .map(|event| event.desk_id)
            .collect::<Vec<_>>(),
        vec![low_desk, high_desk]
    );
    let EventReplay::Events(published) = events.replay(
        0,
        &EventFilter::default().with_capability(EventCapability::Desk),
    ) else {
        panic!("choice invalidation events should remain replayable")
    };
    assert_eq!(published.len(), 2);
    for event in published {
        let ApplicationEvent::Programming(ProgrammingEvent::InteractionChanged(change)) =
            &event.payload
        else {
            panic!("expected a Programming interaction change")
        };
        assert!(change.command_line().unwrap().pending_choice.is_none());
        assert!(change.selection().is_none());
    }
}

#[test]
fn owned_refresh_publishes_inside_outer_interaction_without_a_duplicate() {
    let setup = LiveSetup::new(8);
    let registry = setup.ports.registry.as_ref().unwrap();
    let session = SessionId(setup.context.session_id.unwrap());
    let desk_id = setup.context.desk_id;
    let first = FixtureId::new();
    let second = FixtureId::new();
    registry.select_expression(
        session,
        vec![first],
        SelectionExpression::LiveGroup {
            group_id: "front".into(),
            rule: SelectionRule::All,
        },
    );
    let groups = HashMap::from([(
        "front".into(),
        GroupDefinition {
            id: "front".into(),
            fixtures: vec![first, second],
            ..GroupDefinition::default()
        },
    )]);

    let completed = setup
        .service
        .run_external_interaction(&setup.context, &setup.ports, || {
            setup.service.run_selection_refresh_with_owned_target(
                &setup.context,
                ProgrammingSelectionTarget {
                    desk_id,
                    interaction_id: SessionId(desk_id),
                },
                [],
                || registry.refresh_live_selections(&groups),
            )
        })
        .unwrap();

    assert_eq!(completed.output.events.len(), 1);
    assert_eq!(completed.output.events[0].event_sequence, 1);
    assert_eq!(completed.event_sequence, None);
    assert_eq!(setup.events.latest_sequence(), 2);
    assert_eq!(
        registry.selection(session).unwrap().selected,
        vec![first, second]
    );
}
