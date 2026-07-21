use super::*;
use crate::{ActionErrorKind, EventSource};
use light_core::{AttributeKey, AttributeValue};
use std::collections::HashSet;

struct LifecyclePorts {
    fixture: FixtureId,
}

impl ProgrammingPorts for LifecyclePorts {
    fn execute(
        &self,
        _programmers: &ProgrammerRegistry,
        _context: &ActionContext,
        _command: &str,
        _policy: ExecutionPolicy,
    ) -> ProgrammingExecution {
        panic!("the lifecycle test does not execute legacy commands")
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
fn target_user_replacement_is_monotonic_exact_once_and_invalidates_old_values_replay() {
    let registry = ProgrammerRegistry::default();
    let target_user = UserId::new();
    let first_session = SessionId::new();
    let second_session = SessionId::new();
    let first_desk = Uuid::new_v4();
    let second_desk = Uuid::new_v4();
    registry.start(first_session, target_user);
    registry.start(second_session, target_user);
    registry.attach_command_context(first_session, SessionId(first_desk));
    registry.attach_command_context(second_session, SessionId(second_desk));

    let actor_user = UserId::new();
    let actor_session = SessionId::new();
    let actor_desk = Uuid::new_v4();
    registry.start(actor_session, actor_user);
    registry.attach_command_context(actor_session, SessionId(actor_desk));
    let target_context = ActionContext::operator(
        first_desk,
        target_user.0,
        first_session.0,
        ActionSource::Http,
    );
    let actor_context = ActionContext::operator(
        actor_desk,
        actor_user.0,
        actor_session.0,
        ActionSource::Http,
    );
    let fixture = FixtureId::new();
    let events = EventBus::new(16);
    let service = ProgrammingService::new(
        registry.clone(),
        events.clone(),
        Arc::new(HighlightRegistry::default()),
    );
    let ports = LifecyclePorts { fixture };
    let old_action = ActionEnvelope {
        context: target_context
            .clone()
            .with_request_id("before-replacement")
            .with_expected_revision(0),
        command: ProgrammingValuesRequest {
            expected_capture_mode_revision: 0,
            command: ProgrammingValuesCommand::SetFixture {
                fixture_id: fixture,
                attribute: AttributeKey::intensity(),
                value: AttributeValue::Normalized(0.5),
                timing: Default::default(),
            },
        },
    };
    service.handle_values(old_action.clone(), &ports).unwrap();
    service
        .run_external_interaction(&target_context, &ports, || {
            registry.arm_preload(second_session, true)
        })
        .unwrap();
    let old_programmer_id = registry.get(first_session).unwrap().id;
    let cursor = events.latest_sequence();

    let result = service
        .replace_user_programmer(
            &actor_context,
            &ports,
            ProgrammingLifecycleTarget::new(
                target_user,
                first_session,
                vec![second_desk, first_desk],
            ),
            || {
                assert!(registry.clear(first_session));
                registry.start(first_session, target_user);
                registry.start(second_session, target_user);
                ProgrammingLifecycleCompletion::new((), Some(second_session))
            },
        )
        .unwrap();

    assert_eq!(result.values_revision, 2);
    assert_eq!(result.capture_mode_revision, 2);
    assert_eq!(result.priority_revision, 1);
    assert!(result.values_event_sequence.is_some());
    assert!(result.capture_mode_event_sequence.is_some());
    assert!(result.priority_event_sequence.is_some());
    for session in [first_session, second_session] {
        let state = registry.get(session).unwrap();
        assert!(state.values.is_empty());
        assert!(state.group_values.is_empty());
        assert_eq!(registry.capture_mode(session), Some(Default::default()));
    }
    let EventReplay::Events(published) = events.replay(cursor, &EventFilter::default()) else {
        panic!("lifecycle events should remain replayable")
    };
    assert_eq!(published.len(), 4);
    assert!(published.iter().all(|event| event.desk_id.is_none()));
    assert!(
        published
            .iter()
            .all(|event| event.source == EventSource::Action(ActionSource::Http))
    );
    let lifecycle = published
        .iter()
        .filter_map(|event| match &event.payload {
            ApplicationEvent::Programming(ProgrammingEvent::LifecycleChanged(change)) => {
                Some(change)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(lifecycle.len(), 1);
    let priority = published
        .iter()
        .filter_map(|event| match &event.payload {
            ApplicationEvent::Programming(ProgrammingEvent::PriorityChanged(change)) => {
                Some(change)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(priority.len(), 1);
    let ProgrammingPriorityChange::Upsert { projection } = priority[0] else {
        panic!("replacement should publish the new priority authority")
    };
    assert_eq!(projection.user_id, target_user);
    assert_eq!(projection.revision, 1);
    assert_eq!(projection.priority, 100);
    let ProgrammingLifecycleDelta::Upsert { programmer } = &lifecycle[0].delta else {
        panic!("replacement should upsert the new Programmer identity")
    };
    assert_eq!(programmer.user_id, target_user);
    assert_ne!(programmer.programmer_id, old_programmer_id);
    assert_eq!(
        programmer.programmer_id,
        registry.get(first_session).unwrap().id
    );

    let stale = service.handle_values(old_action, &ports).unwrap_err();
    assert_eq!(stale.kind, ActionErrorKind::Conflict);
    assert_eq!(stale.current_revision, Some(2));
}

#[test]
fn target_user_removal_publishes_one_exact_priority_tombstone() {
    let registry = ProgrammerRegistry::default();
    let target_user = UserId::new();
    let target_session = SessionId::new();
    let target_desk = Uuid::new_v4();
    registry.start(target_session, target_user);
    registry.attach_command_context(target_session, SessionId(target_desk));
    let actor_user = UserId::new();
    let actor_session = SessionId::new();
    let actor_context = ActionContext::operator(
        Uuid::new_v4(),
        actor_user.0,
        actor_session.0,
        ActionSource::Http,
    );
    registry.start(actor_session, actor_user);
    let events = EventBus::new(8);
    let service = ProgrammingService::new(
        registry.clone(),
        events.clone(),
        Arc::new(HighlightRegistry::default()),
    );
    let ports = LifecyclePorts {
        fixture: FixtureId::new(),
    };

    let result = service
        .replace_user_programmer(
            &actor_context,
            &ports,
            ProgrammingLifecycleTarget::new(target_user, target_session, vec![target_desk]),
            || {
                assert!(registry.clear(target_session));
                ProgrammingLifecycleCompletion::new((), None)
            },
        )
        .unwrap();

    assert_eq!(result.priority_revision, 1);
    assert_eq!(result.priority_event_sequence, Some(1));
    let EventReplay::Events(events) = events.replay(
        0,
        &EventFilter::default().with_object(EventObject::programming_priority(target_user.0)),
    ) else {
        panic!("priority removal should remain replayable")
    };
    assert_eq!(events.len(), 1);
    let ApplicationEvent::Programming(ProgrammingEvent::PriorityChanged(
        ProgrammingPriorityChange::Remove { user_id, revision },
    )) = &events[0].payload
    else {
        panic!("removal should publish an exact priority tombstone")
    };
    assert_eq!((*user_id, *revision), (target_user, 1));
}
