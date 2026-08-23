use super::*;
use crate::EventSource;
use light_programmer::{GroupDefinition, SelectionExpression, SelectionReference, SelectionRule};
use std::collections::HashMap;

/// The desk's one interaction context, settled by whichever surface connected first.
fn desk_context(registry: &ProgrammerRegistry) -> Uuid {
    registry
        .desk_interaction_context()
        .expect("a started session settles the desk's interaction context")
        .0
}

#[test]
fn a_refresh_publishes_the_desks_selection_once_however_many_surfaces_are_connected() {
    let registry = ProgrammerRegistry::default();
    let main_window = SessionId::new();
    let second_screen = SessionId::new();
    registry.start(main_window, UserId::new());
    registry.start(second_screen, UserId::new());
    // Both surfaces are the same desk, so both are already on its one command line.
    assert!(registry.attach_command_context(main_window, SessionId(Uuid::from_u128(1))));
    assert!(registry.attach_command_context(second_screen, SessionId(Uuid::from_u128(2))));
    let desk = desk_context(&registry);
    let first = FixtureId::new();
    let second = FixtureId::new();
    registry.select_expression(
        main_window,
        vec![first],
        SelectionExpression::LiveGroup {
            group_id: "front".into(),
            rule: SelectionRule::All,
        },
    );
    let events = EventBus::new(8);
    let service = ProgrammingService::new(
        registry.clone(),
        events.clone(),
        Arc::new(HighlightRegistry::default()),
    );
    let context = ActionContext::system(desk, ActionSource::System);
    let updated_groups = HashMap::from([(
        "front".into(),
        GroupDefinition {
            id: "front".into(),
            fixtures: vec![first, second],
            ..GroupDefinition::default()
        },
    )]);

    let result = service.run_selection_refresh(&context, || {
        registry.refresh_live_selections(&updated_groups)
    });

    assert_eq!(
        result
            .events
            .iter()
            .map(|event| event.desk_id)
            .collect::<Vec<_>>(),
        vec![desk],
        "one desk publishes one selection change"
    );
    assert_eq!(result.events[0].event_sequence, 1);
    let EventReplay::Events(published) = events.replay(
        0,
        &EventFilter::default().with_capability(EventCapability::Desk),
    ) else {
        panic!("selection refresh events should remain replayable")
    };
    assert_eq!(published.len(), 1);
    assert!(published.iter().all(|event| {
        event.correlation_id == Some(context.correlation_id)
            && event.source == EventSource::Action(ActionSource::System)
    }));
    let ApplicationEvent::Programming(ProgrammingEvent::InteractionChanged(change)) =
        &published[0].payload
    else {
        panic!("expected a Programming interaction change")
    };
    assert_eq!(change.selection().unwrap().selected, vec![first, second]);
    // Both surfaces observe the same refreshed selection, because it is the same selection.
    for session in [main_window, second_screen] {
        assert_eq!(
            registry.selection(session).unwrap().selected,
            vec![first, second]
        );
    }
    let EventReplay::Events(lifecycle) = events.replay(
        0,
        &EventFilter::default().with_object(EventObject::programming_lifecycle()),
    ) else {
        panic!("selection counts should publish lifecycle deltas")
    };
    assert_eq!(lifecycle.len(), 1);
    assert_eq!(lifecycle[0].sequence, 2);
}

#[test]
fn a_refresh_that_changes_nothing_stays_quiet() {
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    registry.start(session, UserId::new());
    let desk = desk_context(&registry);
    let first = FixtureId::new();
    let second = FixtureId::new();
    // Selected by source rather than by live Group, so a Group membership change cannot move it.
    registry.select_expression(
        session,
        vec![first],
        SelectionExpression::Sources {
            items: vec![SelectionReference::Fixture { fixture_id: first }],
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

    let result = service
        .run_selection_refresh(&ActionContext::system(desk, ActionSource::System), || {
            registry.refresh_live_selections(&groups)
        });

    assert!(result.events.is_empty());
    assert_eq!(registry.selection(session).unwrap().selected, vec![first]);
}

#[test]
fn a_refresh_publishes_pending_choice_invalidation_once_for_the_desk() {
    let registry = ProgrammerRegistry::default();
    let main_window = SessionId::new();
    let second_screen = SessionId::new();
    for session in [main_window, second_screen] {
        registry.start(session, UserId::new());
    }
    let desk = desk_context(&registry);
    registry.set_pending_command_choice(
        main_window,
        Some(PendingCommandChoice::CueMoveCopy(CueMoveCopyChoice {
            choice_id: uuid::Uuid::from_u128(1),
            show_id: uuid::Uuid::from_u128(2),
            show_revision: 3,
            operation: CueTransferOperation::Copy,
            command: "COPY CUELIST 1 CUE 1 AT CUELIST 2 CUE 2".into(),
            options: Vec::new(),
            cancel_label: "Cancel".into(),
        })),
    );
    let events = EventBus::new(8);
    let service = ProgrammingService::new(
        registry.clone(),
        events.clone(),
        Arc::new(HighlightRegistry::default()),
    );

    let result = service
        .run_selection_refresh(&ActionContext::system(desk, ActionSource::System), || {
            registry.clear_pending_command_choices_except_context(None)
        });

    assert_eq!(
        result.output, 1,
        "the desk has one command line, so one pending choice to clear"
    );
    assert_eq!(
        result
            .events
            .iter()
            .map(|event| event.desk_id)
            .collect::<Vec<_>>(),
        vec![desk]
    );
    let EventReplay::Events(published) = events.replay(
        0,
        &EventFilter::default().with_capability(EventCapability::Desk),
    ) else {
        panic!("choice invalidation events should remain replayable")
    };
    assert_eq!(published.len(), 1);
    let ApplicationEvent::Programming(ProgrammingEvent::InteractionChanged(change)) =
        &published[0].payload
    else {
        panic!("expected a Programming interaction change")
    };
    assert!(change.command_line().unwrap().pending_choice.is_none());
    assert!(change.selection().is_none());
}

#[test]
fn a_refresh_inside_an_outer_interaction_publishes_without_a_duplicate() {
    let setup = LiveSetup::new(8);
    let registry = setup.ports.registry.as_ref().unwrap();
    let session = SessionId(setup.context.session_id.unwrap());
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
            setup
                .service
                .run_selection_refresh_within_interaction(&setup.context, || {
                    registry.refresh_live_selections(&groups)
                })
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
