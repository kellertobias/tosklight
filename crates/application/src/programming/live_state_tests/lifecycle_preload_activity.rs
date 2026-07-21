use super::*;
use light_core::{AttributeKey, AttributeValue};

#[test]
fn preload_take_live_and_release_publish_one_boolean_transition_each() {
    let registry = ProgrammerRegistry::default();
    let user = UserId::new();
    let context = ActionContext::operator(
        Uuid::new_v4(),
        user.0,
        SessionId::new().0,
        ActionSource::Http,
    );
    let session = SessionId(context.session_id.unwrap());
    registry.start(session, user);
    registry.attach_command_context(session, SessionId(context.desk_id));
    let events = EventBus::new(32);
    let service = ProgrammingService::new(
        registry.clone(),
        events.clone(),
        Arc::new(HighlightRegistry::default()),
    );
    let ports = LivePorts::default();

    external(&service, &context, &ports, || {
        registry.arm_preload(session, true)
    });
    external(&service, &context, &ports, || {
        registry.set(
            session,
            FixtureId::new(),
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.5),
        )
    });
    external(&service, &context, &ports, || {
        registry.queue_preload_playback_action(
            session,
            1,
            None,
            light_programmer::PreloadPlaybackQueueAction::Go,
            light_programmer::PreloadPlaybackQueueSurface::Physical,
        )
    });
    assert!(lifecycle_events(&events).is_empty());

    external(&service, &context, &ports, || {
        registry.activate_preload(session)
    });
    assert_preload_lifecycle(&events, &[(1, true)]);
    external(&service, &context, &ports, || {
        registry.activate_preload(session)
    });
    assert_preload_lifecycle(&events, &[(1, true)]);

    external(&service, &context, &ports, || {
        registry.set_preload_group(
            session,
            "7".into(),
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.7),
        )
    });
    external(&service, &context, &ports, || {
        registry.activate_preload(session)
    });
    assert_preload_lifecycle(&events, &[(1, true)]);

    external(&service, &context, &ports, || {
        registry.release_preload(session)
    });
    assert_preload_lifecycle(&events, &[(1, true), (2, false)]);
    external(&service, &context, &ports, || {
        registry.release_preload(session)
    });
    assert_preload_lifecycle(&events, &[(1, true), (2, false)]);

    external(&service, &context, &ports, || {
        registry.set_preload_group(
            session,
            "8".into(),
            AttributeKey::intensity(),
            AttributeValue::Normalized(0.8),
        )
    });
    assert_preload_lifecycle(&events, &[(1, true), (2, false)]);
    external(&service, &context, &ports, || {
        registry.activate_preload(session)
    });
    external(&service, &context, &ports, || {
        registry.release_preload(session)
    });
    assert_preload_lifecycle(&events, &[(1, true), (2, false), (3, true), (4, false)]);
}

fn external<T>(
    service: &ProgrammingService,
    context: &ActionContext,
    ports: &dyn ProgrammingPorts,
    operation: impl FnOnce() -> T,
) {
    service
        .run_external_interaction(context, ports, operation)
        .unwrap();
}

fn lifecycle_events(events: &EventBus) -> Vec<Arc<crate::EventEnvelope>> {
    let EventReplay::Events(events) = events.replay(
        0,
        &EventFilter::default().with_object(EventObject::programming_lifecycle()),
    ) else {
        panic!("lifecycle events should remain replayable")
    };
    events
}

fn assert_preload_lifecycle(events: &EventBus, expected: &[(u64, bool)]) {
    let actual = lifecycle_events(events)
        .iter()
        .map(|event| {
            let ApplicationEvent::Programming(ProgrammingEvent::LifecycleChanged(change)) =
                &event.payload
            else {
                panic!("expected a Programmer lifecycle change")
            };
            let ProgrammingLifecycleDelta::Upsert { programmer } = &change.delta else {
                panic!("Preload activity keeps the lifecycle row connected")
            };
            (change.revision, programmer.preload_active)
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}
