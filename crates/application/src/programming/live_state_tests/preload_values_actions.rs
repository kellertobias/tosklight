use super::*;
use crate::{ActionErrorKind, EventObject};
use light_core::{AttributeKey, AttributeValue};
use std::collections::HashSet;

#[derive(Default)]
struct PreloadValuesPorts {
    environment: ProgrammingValuesEnvironment,
    persisted: Mutex<Vec<&'static str>>,
    registry: Option<ProgrammerRegistry>,
    session: Option<SessionId>,
}

impl ProgrammingPorts for PreloadValuesPorts {
    fn execute(
        &self,
        _programmers: &ProgrammerRegistry,
        _context: &ActionContext,
        _command: &str,
        _policy: ExecutionPolicy,
    ) -> ProgrammingExecution {
        panic!("Preload values actions do not execute legacy commands")
    }

    fn values_environment(
        &self,
        _context: &ActionContext,
    ) -> Result<ProgrammingValuesEnvironment, crate::ActionError> {
        Ok(self.environment.clone())
    }

    fn persist(&self, _context: &ActionContext, operation: &'static str) -> Option<String> {
        self.persisted.lock().push(operation);
        None
    }

    fn reconcile(&self, _context: &ActionContext, _reason: ProgrammingReconciliation) {}

    fn commit_preload(&self, _context: &ActionContext) -> Result<Option<String>, String> {
        let registry = self.registry.as_ref().ok_or("registry is unavailable")?;
        let session = self.session.ok_or("session is unavailable")?;
        registry.activate_preload(session);
        Ok(None)
    }
}

struct PreloadValuesSetup {
    registry: ProgrammerRegistry,
    service: ProgrammingService,
    events: EventBus,
    session: SessionId,
    user: UserId,
    context: ActionContext,
    fixtures: [FixtureId; 3],
    ports: PreloadValuesPorts,
}

impl PreloadValuesSetup {
    fn new() -> Self {
        let registry = ProgrammerRegistry::default();
        let session = SessionId::new();
        let user = UserId::new();
        let desk = Uuid::new_v4();
        let fixtures = [FixtureId::new(), FixtureId::new(), FixtureId::new()];
        registry.start(session, user);
        registry.attach_command_context(session, SessionId(desk));
        let events = EventBus::new(64);
        let service = ProgrammingService::new(
            registry.clone(),
            events.clone(),
            Arc::new(HighlightRegistry::default()),
        );
        Self {
            registry: registry.clone(),
            service,
            events,
            session,
            user,
            context: ActionContext::operator(desk, user.0, session.0, ActionSource::Http),
            fixtures,
            ports: PreloadValuesPorts {
                environment: ProgrammingValuesEnvironment {
                    fixture_ids: fixtures.into_iter().collect(),
                    group_ids: HashSet::from(["front".into(), "back".into()]),
                },
                registry: Some(registry),
                session: Some(session),
                ..Default::default()
            },
        }
    }

    fn enter_capture(&self) -> u64 {
        self.service
            .run_external_interaction(&self.context, &self.ports, || {
                self.registry.arm_preload(self.session, true)
            })
            .unwrap();
        self.registry.capture_mode_revision(self.user)
    }

    fn action(
        &self,
        request_id: &str,
        expected_revision: u64,
        expected_capture_mode_revision: u64,
        command: ProgrammingPreloadValuesCommand,
    ) -> ActionEnvelope<ProgrammingPreloadValuesRequest> {
        ActionEnvelope {
            context: self
                .context
                .clone()
                .with_request_id(request_id)
                .with_expected_revision(expected_revision),
            command: ProgrammingPreloadValuesRequest {
                expected_capture_mode_revision,
                command,
            },
        }
    }

    fn values_events(&self) -> Vec<Arc<crate::EventEnvelope>> {
        let filter = EventFilter::default()
            .with_object(EventObject::programming_preload_values(self.user.0));
        let EventReplay::Events(events) = self.events.replay(0, &filter) else {
            panic!("Preload values events should remain replayable")
        };
        events
    }
}

fn fixture_set(
    fixture_id: FixtureId,
    attribute: &str,
    value: f32,
    timing: ProgrammingPreloadValueTiming,
) -> ProgrammingPreloadValueMutation {
    ProgrammingPreloadValueMutation::SetFixture {
        fixture_id,
        attribute: AttributeKey(attribute.into()),
        value: AttributeValue::Normalized(value),
        timing,
    }
}

#[test]
fn typed_batch_is_one_pending_projection_event_persist_and_checkpoint() {
    let setup = PreloadValuesSetup::new();
    let capture_revision = setup.enter_capture();
    let undo_before = setup.registry.get(setup.session).unwrap().undo.len();
    let timing = ProgrammingPreloadValueTiming {
        fade: true,
        fade_millis: Some(1_500),
        delay_millis: Some(250),
    };
    super::super::preload_values_projection::reset_projection_read_count();

    let result = setup
        .service
        .handle_preload_values(
            setup.action(
                "pending-batch",
                0,
                capture_revision,
                ProgrammingPreloadValuesCommand::Batch {
                    mutations: vec![
                        fixture_set(setup.fixtures[0], "intensity", 0.25, Default::default()),
                        fixture_set(setup.fixtures[1], "pan", 0.5, timing),
                        ProgrammingPreloadValueMutation::SetGroup {
                            group_id: "front".into(),
                            attribute: AttributeKey("tilt".into()),
                            value: AttributeValue::Spread(vec![0.1, 0.9]),
                            timing,
                        },
                    ],
                },
            ),
            &setup.ports,
        )
        .unwrap();

    let ProgrammingPreloadValuesOutcome::Changed {
        projection,
        event_sequence,
    } = result.outcome
    else {
        panic!("the pending batch should change values")
    };
    assert_eq!(projection.revision, 1);
    assert_eq!(projection.fixture_values.len(), 2);
    assert_eq!(projection.group_values.len(), 1);
    assert!(
        projection.fixture_values[0].programmer_order
            < projection.fixture_values[1].programmer_order
    );
    assert!(
        projection.fixture_values[1].programmer_order < projection.group_values[0].programmer_order
    );
    assert_eq!(projection.fixture_values[1].fade_millis, Some(1_500));
    assert_eq!(projection.fixture_values[1].delay_millis, Some(250));
    assert_eq!(event_sequence, 2);
    assert_eq!(setup.values_events().len(), 1);
    assert_eq!(
        *setup.ports.persisted.lock(),
        vec!["programmer.preload_values"]
    );
    assert_eq!(
        super::super::preload_values_projection::projection_read_count(),
        1
    );
    assert_eq!(
        setup.registry.get(setup.session).unwrap().undo.len(),
        undo_before + 1
    );
}

#[test]
fn no_op_replay_and_preconditions_do_not_materialize_pending_projection() {
    let setup = PreloadValuesSetup::new();
    let capture_revision = setup.enter_capture();
    let command = ProgrammingPreloadValuesCommand::SetFixture {
        fixture_id: setup.fixtures[0],
        attribute: AttributeKey::intensity(),
        value: AttributeValue::Normalized(0.4),
        timing: Default::default(),
    };
    let original = setup.action("pending-set", 0, capture_revision, command.clone());
    let first = setup
        .service
        .handle_preload_values(original.clone(), &setup.ports)
        .unwrap();
    super::super::preload_values_projection::reset_projection_read_count();

    let replay = setup
        .service
        .handle_preload_values(original, &setup.ports)
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.outcome, first.outcome);
    assert_eq!(
        super::super::preload_values_projection::projection_read_count(),
        0
    );

    let reused = setup.service.handle_preload_values(
        setup.action(
            "pending-set",
            0,
            capture_revision,
            ProgrammingPreloadValuesCommand::SetFixture {
                fixture_id: setup.fixtures[0],
                attribute: AttributeKey::intensity(),
                value: AttributeValue::Normalized(0.8),
                timing: Default::default(),
            },
        ),
        &setup.ports,
    );
    assert_eq!(reused.unwrap_err().kind, ActionErrorKind::Conflict);
    assert_eq!(setup.registry.preload_values_revision(setup.user), 1);

    let exact = setup
        .service
        .handle_preload_values(
            setup.action("pending-exact", 1, capture_revision, command.clone()),
            &setup.ports,
        )
        .unwrap();
    assert_eq!(
        exact.outcome,
        ProgrammingPreloadValuesOutcome::NoChange { revision: 1 }
    );
    assert_eq!(setup.values_events().len(), 1);
    assert_eq!(
        super::super::preload_values_projection::projection_read_count(),
        0
    );

    let stale = setup.service.handle_preload_values(
        setup.action("pending-stale", 0, capture_revision, command.clone()),
        &setup.ports,
    );
    assert_eq!(stale.unwrap_err().current_revision, Some(1));

    setup
        .service
        .run_external_interaction(&setup.context, &setup.ports, || {
            setup.registry.arm_preload(setup.session, false)
        })
        .unwrap();
    let wrong_capture = setup.service.handle_preload_values(
        setup.action("pending-capture", 1, capture_revision, command.clone()),
        &setup.ports,
    );
    let error = wrong_capture.unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert_eq!(error.current_revision, Some(1));
    assert_eq!(error.current_related_revision, Some(capture_revision + 1));

    let not_redirected = setup.service.handle_preload_values(
        setup.action("pending-not-redirected", 1, capture_revision + 1, command),
        &setup.ports,
    );
    assert_eq!(not_redirected.unwrap_err().kind, ActionErrorKind::Conflict);
    assert_eq!(
        super::super::preload_values_projection::projection_read_count(),
        0
    );
}

#[test]
fn rejected_batch_does_not_advance_pending_generation_or_checkpoint() {
    let setup = PreloadValuesSetup::new();
    let capture_revision = setup.enter_capture();
    let generation_before = setup
        .registry
        .preload_values_generation(setup.session)
        .unwrap();
    let undo_before = setup.registry.get(setup.session).unwrap().undo.len();
    let event_before = setup.events.latest_sequence();

    let rejected = setup.service.handle_preload_values(
        setup.action(
            "duplicate-address",
            0,
            capture_revision,
            ProgrammingPreloadValuesCommand::Batch {
                mutations: vec![
                    fixture_set(setup.fixtures[0], "intensity", 0.2, Default::default()),
                    fixture_set(setup.fixtures[0], "intensity", 0.8, Default::default()),
                ],
            },
        ),
        &setup.ports,
    );

    assert_eq!(rejected.unwrap_err().kind, ActionErrorKind::Invalid);
    assert_eq!(
        setup
            .registry
            .preload_values_generation(setup.session)
            .unwrap(),
        generation_before
    );
    assert_eq!(
        setup.registry.get(setup.session).unwrap().undo.len(),
        undo_before
    );
    assert_eq!(setup.events.latest_sequence(), event_before);
    assert!(setup.ports.persisted.lock().is_empty());
}

#[test]
fn same_user_desks_share_pending_values_and_foreign_identity_is_rejected() {
    let setup = PreloadValuesSetup::new();
    let capture_revision = setup.enter_capture();
    let peer_session = SessionId::new();
    let peer_desk = Uuid::new_v4();
    setup.registry.start(peer_session, setup.user);
    setup
        .registry
        .attach_command_context(peer_session, SessionId(peer_desk));
    let peer_context =
        ActionContext::operator(peer_desk, setup.user.0, peer_session.0, ActionSource::Http);
    setup
        .service
        .handle_preload_values(
            setup.action(
                "shared-set",
                0,
                capture_revision,
                ProgrammingPreloadValuesCommand::SetGroup {
                    group_id: "front".into(),
                    attribute: AttributeKey::intensity(),
                    value: AttributeValue::Spread(vec![0.2, 0.8]),
                    timing: Default::default(),
                },
            ),
            &setup.ports,
        )
        .unwrap();

    let peer = setup
        .service
        .preload_values_snapshot(&peer_context, &setup.ports)
        .unwrap();
    assert_eq!(peer.projection.revision, 1);
    assert_eq!(peer.projection.group_values.len(), 1);

    let foreign = ActionContext::operator(
        peer_desk,
        Uuid::new_v4(),
        peer_session.0,
        ActionSource::Http,
    );
    assert_eq!(
        setup
            .service
            .preload_values_snapshot(&foreign, &setup.ports)
            .unwrap_err()
            .kind,
        ActionErrorKind::Forbidden
    );
}

#[test]
fn legacy_clear_go_release_undo_and_lifecycle_publish_pending_transitions_once() {
    let setup = PreloadValuesSetup::new();
    setup.enter_capture();
    let legacy_set = setup
        .service
        .run_external_interaction(&setup.context, &setup.ports, || {
            setup.registry.set(
                setup.session,
                setup.fixtures[0],
                AttributeKey::intensity(),
                AttributeValue::Normalized(0.6),
            )
        })
        .unwrap();
    assert!(legacy_set.preload_values_event_sequence.is_some());

    let clear = setup
        .service
        .handle(
            ActionEnvelope {
                context: setup.context.clone(),
                command: ProgrammingCommand::ClearStep,
            },
            &setup.ports,
        )
        .unwrap();
    assert!(clear.preload_values_event_sequence.is_some());
    let undo = setup
        .service
        .handle(
            ActionEnvelope {
                context: setup.context.clone(),
                command: ProgrammingCommand::Undo,
            },
            &setup.ports,
        )
        .unwrap();
    assert!(undo.preload_values_event_sequence.is_some());

    let revision_before_redo = setup.registry.preload_values_revision(setup.user);
    let events_before_redo = setup.values_events().len();
    let redo = setup
        .service
        .run_external_interaction(&setup.context, &setup.ports, || {
            setup.registry.redo(setup.session)
        })
        .unwrap();
    assert!(redo.output);
    assert!(redo.preload_values_event_sequence.is_some());
    assert_eq!(
        setup.registry.preload_values_revision(setup.user),
        revision_before_redo + 1
    );
    assert_eq!(setup.values_events().len(), events_before_redo + 1);

    let restore_after_redo = setup
        .service
        .run_external_interaction(&setup.context, &setup.ports, || {
            setup.registry.undo(setup.session)
        })
        .unwrap();
    assert!(restore_after_redo.output);
    assert!(restore_after_redo.preload_values_event_sequence.is_some());

    let go = setup
        .service
        .handle(
            ActionEnvelope {
                context: setup.context.clone(),
                command: ProgrammingCommand::Preload {
                    capture_programmer: true,
                },
            },
            &setup.ports,
        )
        .unwrap();
    assert!(go.preload_values_event_sequence.is_some());

    setup
        .service
        .run_external_interaction(&setup.context, &setup.ports, || {
            setup.registry.arm_preload(setup.session, true);
            setup.registry.set(
                setup.session,
                setup.fixtures[1],
                AttributeKey::intensity(),
                AttributeValue::Normalized(0.4),
            );
        })
        .unwrap();
    let release = setup
        .service
        .run_external_interaction(&setup.context, &setup.ports, || {
            setup.registry.release_preload(setup.session)
        })
        .unwrap();
    assert!(release.preload_values_event_sequence.is_some());

    setup
        .service
        .run_external_interaction(&setup.context, &setup.ports, || {
            setup.registry.arm_preload(setup.session, true);
            setup.registry.set(
                setup.session,
                setup.fixtures[2],
                AttributeKey::intensity(),
                AttributeValue::Normalized(0.2),
            );
        })
        .unwrap();
    let target =
        ProgrammingLifecycleTarget::new(setup.user, setup.session, vec![setup.context.desk_id]);
    let lifecycle = setup
        .service
        .replace_user_programmer(&setup.context, &setup.ports, target, || {
            setup.registry.clear(setup.session);
            ProgrammingLifecycleCompletion::new((), None)
        })
        .unwrap();
    assert!(lifecycle.preload_values_event_sequence.is_some());
    assert_eq!(setup.values_events().len(), 10);
}
