use super::*;
use crate::{ActionErrorKind, EventSource};
use light_core::{AttributeKey, AttributeValue};
use std::collections::{HashMap, HashSet};

struct LifecyclePublicationPorts {
    fixture: FixtureId,
    deny: bool,
}

impl ProgrammingPorts for LifecyclePublicationPorts {
    fn authorize(&self, _context: &ActionContext) -> Result<(), crate::ActionError> {
        if self.deny {
            Err(crate::ActionError::new(
                ActionErrorKind::Unauthorized,
                "authentication required",
            ))
        } else {
            Ok(())
        }
    }

    fn execute(
        &self,
        _programmers: &ProgrammerRegistry,
        _context: &ActionContext,
        _command: &str,
        _policy: ExecutionPolicy,
    ) -> ProgrammingExecution {
        panic!("the lifecycle publication test does not execute commands")
    }

    fn values_environment(
        &self,
        _context: &ActionContext,
    ) -> Result<ProgrammingValuesEnvironment, crate::ActionError> {
        Ok(ProgrammingValuesEnvironment {
            fixture_ids: HashSet::from([self.fixture]),
            ..Default::default()
        })
    }

    fn persist(&self, _context: &ActionContext, _operation: &'static str) -> Option<String> {
        None
    }

    fn reconcile(&self, _context: &ActionContext, _reason: ProgrammingReconciliation) {}

    fn commit_preload(&self, _context: &ActionContext) -> Result<Option<String>, String> {
        Ok(None)
    }
}

#[test]
fn session_lifecycle_is_one_safe_per_user_delta_and_snapshot_is_authenticated() {
    let registry = ProgrammerRegistry::default();
    let events = EventBus::new(16);
    let service = ProgrammingService::new(
        registry.clone(),
        events.clone(),
        Arc::new(HighlightRegistry::default()),
    );
    let first_user = UserId::new();
    let foreign_user = UserId::new();
    let first = lifecycle_context(first_user);
    let second = lifecycle_context(first_user);
    let foreign = lifecycle_context(foreign_user);

    start_session(&service, &registry, &first, first_user);
    start_session(&service, &registry, &second, first_user);
    start_session(&service, &registry, &foreign, foreign_user);
    disconnect_session(&service, &registry, &second, first_user);
    disconnect_session(&service, &registry, &first, first_user);

    let published = lifecycle_events(&events);
    assert_eq!(published.len(), 5);
    assert_eq!(
        published
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 5]
    );
    assert!(published.iter().all(|event| {
        event.delivery == crate::DeliveryPolicy::Lossless
            && event.desk_id.is_none()
            && event.object.as_ref() == Some(&EventObject::programming_lifecycle())
    }));
    let session_counts = published
        .iter()
        .filter_map(|event| lifecycle_upsert(event).map(|row| row.sessions.len()))
        .collect::<Vec<_>>();
    assert_eq!(session_counts, vec![1, 2, 1, 1]);
    assert!(matches!(
        lifecycle_change(&published[4]).delta,
        ProgrammingLifecycleDelta::Remove { .. }
    ));

    let ports = LifecyclePublicationPorts {
        fixture: FixtureId::new(),
        deny: false,
    };
    let snapshot = service.lifecycle_snapshot(&foreign, &ports).unwrap();
    assert_eq!(snapshot.event_sequence, 5);
    assert_eq!(snapshot.projection.revision, 5);
    assert_eq!(snapshot.projection.programmers.len(), 1);
    let row = &snapshot.projection.programmers[0];
    assert_eq!(row.user_id, foreign_user);
    assert_eq!(row.normal_value_count, 0);
    assert_eq!(row.sessions.len(), 1);

    let denied = LifecyclePublicationPorts {
        fixture: FixtureId::new(),
        deny: true,
    };
    assert_eq!(
        service
            .lifecycle_snapshot(&foreign, &denied)
            .unwrap_err()
            .kind,
        ActionErrorKind::Unauthorized
    );
}

#[test]
fn count_changes_publish_after_authoritative_events_while_same_count_and_replay_stay_quiet() {
    let registry = ProgrammerRegistry::default();
    let user = UserId::new();
    let context = lifecycle_context(user);
    let session = SessionId(context.session_id.unwrap());
    registry.start(session, user);
    registry.attach_command_context(session, SessionId(context.desk_id));
    let events = EventBus::new(32);
    let highlight = Arc::new(HighlightRegistry::default());
    let service = ProgrammingService::new(registry.clone(), events.clone(), Arc::clone(&highlight));
    let fixture = FixtureId::new();
    let ports = LifecyclePublicationPorts {
        fixture,
        deny: false,
    };
    let first = values_action(&context, "set", 0, fixture, 0.4);

    service.handle_values(first.clone(), &ports).unwrap();
    assert_lifecycle_sequences(&events, &[2]);
    service
        .handle_values(values_action(&context, "replace", 1, fixture, 0.7), &ports)
        .unwrap();
    assert_lifecycle_sequences(&events, &[2]);
    assert!(service.handle_values(first, &ports).unwrap().replayed);
    assert_lifecycle_sequences(&events, &[2]);

    let selected = [FixtureId::new(), FixtureId::new()];
    service
        .run_external_interaction(&context, &ports, || registry.select(session, selected))
        .unwrap();
    assert_lifecycle_sequences(&events, &[2, 5]);
    service
        .run_external_interaction(&context, &ports, || {
            registry.select(session, [FixtureId::new(), FixtureId::new()])
        })
        .unwrap();
    assert_lifecycle_sequences(&events, &[2, 5]);
    service
        .run_external_interaction(&context, &ports, || registry.arm_preload(session, true))
        .unwrap();
    assert_lifecycle_sequences(&events, &[2, 5]);

    service
        .run_external_interaction(&context, &ports, || {
            registry.set_command_line(session, "FIXTURE 1 AT 50".into())
        })
        .unwrap();
    service
        .run_external_interaction(&context, &ports, || registry.set_priority(session, 42))
        .unwrap();
    service
        .run_external_interaction(&context, &ports, || {
            registry.set_modes(
                session,
                Some(true),
                Some(true),
                Some(true),
                Some(Some("preview".into())),
            )
        })
        .unwrap();
    let transient = service
        .run_external_interaction(&context, &ports, || {
            registry.set_transient_action(
                session,
                "test-control".into(),
                [(
                    fixture,
                    AttributeKey::intensity(),
                    AttributeValue::Normalized(1.0),
                )],
            )
        })
        .unwrap()
        .output
        .unwrap();
    service
        .run_external_interaction(&context, &ports, || {
            registry.release_transient_action(session, "test-control", Some(transient))
        })
        .unwrap();
    service
        .run_external_interaction(&context, &ports, || {
            registry.set_preload_group(
                session,
                "7".into(),
                AttributeKey::intensity(),
                AttributeValue::Normalized(0.5),
            )
        })
        .unwrap();
    service
        .run_external_interaction(&context, &ports, || {
            registry.queue_preload_playback_action(
                session,
                1,
                None,
                light_programmer::PreloadPlaybackQueueAction::Go,
                light_programmer::PreloadPlaybackQueueSurface::Physical,
            )
        })
        .unwrap();
    service
        .run_external_interaction(&context, &ports, || registry.activate_preload(session))
        .unwrap();
    let selection = registry.selection(session).unwrap();
    service
        .run_external_interaction(&context, &ports, || {
            highlight.action(
                context.desk_id,
                user,
                None,
                light_programmer::HighlightAction::On,
                &selection,
                &[light_programmer::HighlightFixture {
                    fixture_id: selected[0],
                    name: None,
                    number: None,
                }],
                &HashMap::new(),
                false,
            )
        })
        .unwrap();
    assert_lifecycle_sequences(&events, &[2, 5]);
}

#[test]
fn lifecycle_snapshot_cursor_repairs_a_retention_gap() {
    let registry = ProgrammerRegistry::default();
    let events = EventBus::new(2);
    let service = ProgrammingService::new(
        registry.clone(),
        events.clone(),
        Arc::new(HighlightRegistry::default()),
    );
    let mut contexts = Vec::new();
    for _ in 0..3 {
        let user = UserId::new();
        let context = lifecycle_context(user);
        start_session(&service, &registry, &context, user);
        contexts.push(context);
    }
    let subscription = events.subscribe(
        EventFilter::default().with_object(EventObject::programming_lifecycle()),
        SubscriptionOptions {
            after_sequence: Some(0),
            ..Default::default()
        },
    );
    assert!(matches!(
        subscription.try_next(),
        Some(SubscriptionDelivery::Gap(_))
    ));

    let ports = LifecyclePublicationPorts {
        fixture: FixtureId::new(),
        deny: false,
    };
    let snapshot = service.lifecycle_snapshot(&contexts[0], &ports).unwrap();
    assert_eq!(snapshot.event_sequence, 3);
    assert_eq!(snapshot.projection.revision, 3);
    subscription
        .repair_from_snapshot(snapshot.event_sequence)
        .unwrap();

    let next_user = UserId::new();
    let next = lifecycle_context(next_user);
    start_session(&service, &registry, &next, next_user);
    let Some(SubscriptionDelivery::Event(event)) = subscription.try_next() else {
        panic!("delivery should resume after the lifecycle snapshot cursor")
    };
    assert_eq!(event.sequence, 4);
}

fn lifecycle_context(user: UserId) -> ActionContext {
    ActionContext::operator(
        Uuid::new_v4(),
        user.0,
        SessionId::new().0,
        ActionSource::Http,
    )
}

fn start_session(
    service: &ProgrammingService,
    registry: &ProgrammerRegistry,
    context: &ActionContext,
    user: UserId,
) {
    let session = SessionId(context.session_id.unwrap());
    service.run_lifecycle_transition(context, user, || {
        registry.start(session, user);
        registry.attach_command_context(session, SessionId(context.desk_id));
    });
}

fn disconnect_session(
    service: &ProgrammingService,
    registry: &ProgrammerRegistry,
    context: &ActionContext,
    user: UserId,
) {
    service.run_lifecycle_transition(context, user, || {
        registry.disconnect(SessionId(context.session_id.unwrap()));
    });
}

fn values_action(
    context: &ActionContext,
    request_id: &str,
    revision: u64,
    fixture_id: FixtureId,
    value: f32,
) -> ActionEnvelope<ProgrammingValuesRequest> {
    ActionEnvelope {
        context: context
            .clone()
            .with_request_id(request_id)
            .with_expected_revision(revision),
        command: ProgrammingValuesRequest {
            expected_capture_mode_revision: 0,
            command: ProgrammingValuesCommand::SetFixture {
                fixture_id,
                attribute: AttributeKey::intensity(),
                value: AttributeValue::Normalized(value),
                timing: Default::default(),
            },
        },
    }
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

fn lifecycle_change(event: &crate::EventEnvelope) -> &ProgrammingLifecycleChange {
    let ApplicationEvent::Programming(ProgrammingEvent::LifecycleChanged(change)) = &event.payload
    else {
        panic!("expected a Programmer lifecycle change")
    };
    change
}

fn lifecycle_upsert(event: &crate::EventEnvelope) -> Option<&ProgrammingLifecycleProgrammer> {
    match &lifecycle_change(event).delta {
        ProgrammingLifecycleDelta::Upsert { programmer } => Some(programmer),
        ProgrammingLifecycleDelta::Remove { .. } => None,
    }
}

fn assert_lifecycle_sequences(events: &EventBus, expected: &[u64]) {
    assert_eq!(
        lifecycle_events(events)
            .iter()
            .map(|event| event.sequence)
            .collect::<Vec<_>>(),
        expected
    );
    assert!(lifecycle_events(events).iter().all(|event| {
        event.source == EventSource::Action(ActionSource::Http)
            && event.delivery == crate::DeliveryPolicy::Lossless
    }));
}
