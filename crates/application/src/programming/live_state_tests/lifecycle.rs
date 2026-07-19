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
    assert!(result.values_event_sequence.is_some());
    assert!(result.capture_mode_event_sequence.is_some());
    for session in [first_session, second_session] {
        let state = registry.get(session).unwrap();
        assert!(state.values.is_empty());
        assert!(state.group_values.is_empty());
        assert_eq!(registry.capture_mode(session), Some(Default::default()));
    }
    let EventReplay::Events(published) = events.replay(cursor, &EventFilter::default()) else {
        panic!("lifecycle events should remain replayable")
    };
    assert_eq!(published.len(), 2);
    assert!(published.iter().all(|event| event.desk_id.is_none()));
    assert!(published.iter().all(|event| {
        event.source == EventSource::Action(ActionSource::Http)
            && event
                .object
                .as_ref()
                .and_then(EventObject::programming_user_id)
                == Some(target_user.0)
    }));

    let stale = service.handle_values(old_action, &ports).unwrap_err();
    assert_eq!(stale.kind, ActionErrorKind::Conflict);
    assert_eq!(stale.current_revision, Some(2));
}
