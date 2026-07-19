use super::*;
use crate::{ActionErrorKind, EventObject};
use light_core::{AttributeKey, AttributeValue};
use light_programmer::SelectionReference;
use std::collections::{HashMap, HashSet};
use std::sync::Barrier;

#[derive(Default)]
struct ValuesPorts {
    environment: ProgrammingValuesEnvironment,
    persisted: Mutex<Vec<&'static str>>,
}

impl ProgrammingPorts for ValuesPorts {
    fn execute(
        &self,
        _programmers: &ProgrammerRegistry,
        _context: &ActionContext,
        _command: &str,
        _policy: ExecutionPolicy,
    ) -> ProgrammingExecution {
        panic!("values actions do not execute legacy commands")
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
        Ok(None)
    }
}

struct ValuesSetup {
    registry: ProgrammerRegistry,
    service: ProgrammingService,
    events: EventBus,
    session: SessionId,
    user: UserId,
    context: ActionContext,
    fixtures: [FixtureId; 3],
    ports: ValuesPorts,
}

impl ValuesSetup {
    fn new() -> Self {
        let registry = ProgrammerRegistry::default();
        let session = SessionId::new();
        let user = UserId::new();
        let desk = Uuid::new_v4();
        let fixtures = [FixtureId::new(), FixtureId::new(), FixtureId::new()];
        registry.start(session, user);
        registry.attach_command_context(session, SessionId(desk));
        let events = EventBus::new(32);
        let service = ProgrammingService::new(
            registry.clone(),
            events.clone(),
            Arc::new(HighlightRegistry::default()),
        );
        Self {
            registry,
            service,
            events,
            session,
            user,
            context: ActionContext::operator(desk, user.0, session.0, ActionSource::Http),
            fixtures,
            ports: ValuesPorts {
                environment: ProgrammingValuesEnvironment {
                    fixture_ids: fixtures.into_iter().collect(),
                    group_ids: HashSet::from(["front".into(), "back".into()]),
                },
                ..Default::default()
            },
        }
    }

    fn action(
        &self,
        request_id: &str,
        expected_revision: u64,
        command: ProgrammingValuesCommand,
    ) -> ActionEnvelope<ProgrammingValuesRequest> {
        self.action_with_capture(request_id, expected_revision, 0, command)
    }

    fn action_with_capture(
        &self,
        request_id: &str,
        expected_revision: u64,
        expected_capture_mode_revision: u64,
        command: ProgrammingValuesCommand,
    ) -> ActionEnvelope<ProgrammingValuesRequest> {
        ActionEnvelope {
            context: self
                .context
                .clone()
                .with_request_id(request_id)
                .with_expected_revision(expected_revision),
            command: ProgrammingValuesRequest {
                expected_capture_mode_revision,
                command,
            },
        }
    }

    fn handle(
        &self,
        request_id: &str,
        expected_revision: u64,
        command: ProgrammingValuesCommand,
    ) -> ProgrammingValuesResult {
        self.service
            .handle_values(
                self.action(request_id, expected_revision, command),
                &self.ports,
            )
            .unwrap()
    }

    fn values_events(&self, desk_id: Uuid, user_id: UserId) -> Vec<Arc<crate::EventEnvelope>> {
        let filter =
            EventFilter::for_desk(desk_id).with_object(EventObject::programming_values(user_id.0));
        let EventReplay::Events(events) = self.events.replay(0, &filter) else {
            panic!("values events should remain replayable")
        };
        events
    }
}

#[test]
fn capture_precondition_is_atomic_and_successful_replay_survives_mode_changes() {
    let setup = ValuesSetup::new();
    let command = ProgrammingValuesCommand::SetFixture {
        fixture_id: setup.fixtures[0],
        attribute: AttributeKey::intensity(),
        value: AttributeValue::Normalized(0.4),
        timing: Default::default(),
    };
    let original = setup.action_with_capture("capture-replay", 0, 0, command.clone());
    let first = setup
        .service
        .handle_values(original.clone(), &setup.ports)
        .unwrap();
    assert_eq!(first.capture_mode_revision, 0);

    let mode_change = setup
        .service
        .run_external_interaction(&setup.context, &setup.ports, || {
            setup.registry.arm_preload(setup.session, true)
        })
        .unwrap();
    assert_eq!(mode_change.capture_mode_event_sequence, Some(2));
    assert_eq!(setup.registry.capture_mode_revision(setup.user), 1);
    super::super::values_projection::reset_projection_read_count();

    let replay = setup.service.handle_values(original, &setup.ports).unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.capture_mode_revision, 0);
    assert_eq!(replay.outcome, first.outcome);
    assert_eq!(super::super::values_projection::projection_read_count(), 0);

    let reused = setup.service.handle_values(
        setup.action_with_capture("capture-replay", 0, 1, command.clone()),
        &setup.ports,
    );
    assert_eq!(reused.unwrap_err().kind, ActionErrorKind::Conflict);

    let stale_values = setup.service.handle_values(
        setup.action_with_capture("stale-values-first", 0, 0, command.clone()),
        &setup.ports,
    );
    let stale_values = stale_values.unwrap_err();
    assert_eq!(stale_values.current_revision, Some(1));
    assert_eq!(stale_values.current_related_revision, None);

    let stale_capture = setup.service.handle_values(
        setup.action_with_capture("stale-capture", 1, 0, command.clone()),
        &setup.ports,
    );
    let stale_capture = stale_capture.unwrap_err();
    assert_eq!(stale_capture.kind, ActionErrorKind::Conflict);
    assert_eq!(stale_capture.current_revision, Some(1));
    assert_eq!(stale_capture.current_related_revision, Some(1));

    let redirected = setup.service.handle_values(
        setup.action_with_capture(
            "redirected",
            1,
            1,
            ProgrammingValuesCommand::SetFixture {
                fixture_id: FixtureId::new(),
                attribute: AttributeKey::intensity(),
                value: AttributeValue::Normalized(0.6),
                timing: Default::default(),
            },
        ),
        &setup.ports,
    );
    let redirected = redirected.unwrap_err();
    assert_eq!(redirected.kind, ActionErrorKind::Conflict);
    assert_eq!(redirected.current_revision, Some(1));
    assert_eq!(redirected.current_related_revision, Some(1));
    assert_eq!(super::super::values_projection::projection_read_count(), 0);
    assert_eq!(setup.registry.normal_values_revision(setup.user), 1);

    setup
        .service
        .run_external_interaction(&setup.context, &setup.ports, || {
            setup.registry.arm_preload(setup.session, false)
        })
        .unwrap();
    let accepted = setup
        .service
        .handle_values(
            setup.action_with_capture("capture-allowed", 1, 2, command),
            &setup.ports,
        )
        .unwrap();
    assert_eq!(accepted.capture_mode_revision, 2);
    assert_eq!(
        accepted.outcome,
        ProgrammingValuesOutcome::NoChange { revision: 1 }
    );
    assert_eq!(super::super::values_projection::projection_read_count(), 0);
}

#[test]
fn concurrent_capture_transition_and_normal_write_have_one_serial_order() {
    let setup = ValuesSetup::new();
    let action = setup.action_with_capture(
        "capture-race",
        0,
        0,
        ProgrammingValuesCommand::SetFixture {
            fixture_id: setup.fixtures[0],
            attribute: AttributeKey::intensity(),
            value: AttributeValue::Normalized(0.4),
            timing: Default::default(),
        },
    );
    let barrier = Arc::new(Barrier::new(3));
    let (capture, values) = std::thread::scope(|scope| {
        let capture_barrier = Arc::clone(&barrier);
        let capture_service = &setup.service;
        let capture_context = &setup.context;
        let capture_ports = &setup.ports;
        let capture_registry = &setup.registry;
        let capture_session = setup.session;
        let capture = scope.spawn(move || {
            capture_barrier.wait();
            capture_service.run_external_interaction(capture_context, capture_ports, || {
                capture_registry.arm_preload(capture_session, true)
            })
        });
        let values_barrier = Arc::clone(&barrier);
        let values_service = &setup.service;
        let values_ports = &setup.ports;
        let values = scope.spawn(move || {
            values_barrier.wait();
            values_service.handle_values(action, values_ports)
        });
        barrier.wait();
        (capture.join().unwrap().unwrap(), values.join().unwrap())
    });

    assert!(capture.capture_mode_event_sequence.is_some());
    assert_eq!(setup.registry.capture_mode_revision(setup.user), 1);
    let capture_filter =
        EventFilter::default().with_object(EventObject::programming_capture_mode(setup.user.0));
    let EventReplay::Events(capture_events) = setup.events.replay(0, &capture_filter) else {
        panic!("capture events should remain replayable")
    };
    assert_eq!(capture_events.len(), 1);

    let value_events = setup.values_events(setup.context.desk_id, setup.user);
    match values {
        Ok(result) => {
            assert!(matches!(
                result.outcome,
                ProgrammingValuesOutcome::Changed { .. }
            ));
            assert_eq!(setup.registry.normal_values_revision(setup.user), 1);
            assert_eq!(setup.registry.get(setup.session).unwrap().values.len(), 1);
            assert_eq!(value_events.len(), 1);
        }
        Err(error) => {
            assert_eq!(error.kind, ActionErrorKind::Conflict);
            assert_eq!(error.current_revision, Some(0));
            assert_eq!(error.current_related_revision, Some(1));
            assert_eq!(setup.registry.normal_values_revision(setup.user), 0);
            assert!(setup.registry.get(setup.session).unwrap().values.is_empty());
            assert!(value_events.is_empty());
        }
    }
}

fn fixture_set(
    fixture_id: FixtureId,
    attribute: &str,
    value: f32,
    timing: ProgrammingValueTiming,
) -> ProgrammingValueMutation {
    ProgrammingValueMutation::SetFixture {
        fixture_id,
        attribute: AttributeKey(attribute.into()),
        value: AttributeValue::Normalized(value),
        timing,
    }
}

fn group_set(
    group_id: &str,
    attribute: &str,
    value: AttributeValue,
    timing: ProgrammingValueTiming,
) -> ProgrammingValueMutation {
    ProgrammingValueMutation::SetGroup {
        group_id: group_id.into(),
        attribute: AttributeKey(attribute.into()),
        value,
        timing,
    }
}

#[test]
fn values_batch_is_one_persisted_projection_event_and_undo_checkpoint() {
    let setup = ValuesSetup::new();
    let timing = ProgrammingValueTiming {
        fade: true,
        fade_millis: Some(1_500),
        delay_millis: Some(250),
    };
    super::super::values_projection::reset_projection_read_count();

    let result = setup.handle(
        "batch-1",
        0,
        ProgrammingValuesCommand::Batch {
            mutations: vec![
                fixture_set(setup.fixtures[0], "intensity", 0.25, Default::default()),
                fixture_set(setup.fixtures[1], "pan", 0.5, timing),
                group_set(
                    "front",
                    "tilt",
                    AttributeValue::Spread(vec![0.1, 0.9]),
                    timing,
                ),
            ],
        },
    );

    let ProgrammingValuesOutcome::Changed {
        projection,
        event_sequence,
    } = result.outcome
    else {
        panic!("the batch should change values")
    };
    assert_eq!(projection.revision, 1);
    assert_eq!(event_sequence, 1);
    assert_eq!(projection.fixture_values.len(), 2);
    assert_eq!(projection.group_values.len(), 1);
    assert!(
        projection.fixture_values[0].programmer_order
            < projection.fixture_values[1].programmer_order
    );
    assert!(
        projection.fixture_values[1].programmer_order < projection.group_values[0].programmer_order
    );
    assert!(projection.fixture_values[1].fade);
    assert_eq!(projection.fixture_values[1].fade_millis, Some(1_500));
    assert_eq!(projection.fixture_values[1].delay_millis, Some(250));
    assert_eq!(result.interaction_event_sequence, None);
    let events = setup.values_events(setup.context.desk_id, setup.user);
    assert_eq!(events.len(), 1);
    let ApplicationEvent::Programming(ProgrammingEvent::ValuesChanged(change)) = &events[0].payload
    else {
        panic!("expected the typed values event")
    };
    assert!(Arc::ptr_eq(&projection, &change.projection));
    assert_eq!(*setup.ports.persisted.lock(), vec!["programmer.values"]);
    assert_eq!(super::super::values_projection::projection_read_count(), 1);
    assert_eq!(setup.registry.get(setup.session).unwrap().undo.len(), 1);
    assert!(setup.registry.undo(setup.session));
    let undone = setup.registry.get(setup.session).unwrap();
    assert!(undone.values.is_empty());
    assert!(undone.group_values.is_empty());
}

#[test]
fn exact_and_interaction_only_actions_do_not_materialize_values() {
    let setup = ValuesSetup::new();
    let initial = ProgrammingValuesCommand::Batch {
        mutations: vec![
            fixture_set(setup.fixtures[0], "intensity", 0.5, Default::default()),
            group_set(
                "front",
                "pan",
                AttributeValue::Spread(vec![0.2, 0.8]),
                Default::default(),
            ),
        ],
    };
    setup.handle("initial", 0, initial.clone());
    super::super::values_projection::reset_projection_read_count();

    let exact = setup.handle("exact", 1, initial);
    assert_eq!(
        exact.outcome,
        ProgrammingValuesOutcome::NoChange { revision: 1 }
    );
    assert_eq!(setup.events.latest_sequence(), 1);
    assert_eq!(super::super::values_projection::projection_read_count(), 0);
    assert_eq!(*setup.ports.persisted.lock(), vec!["programmer.values"]);

    assert!(setup.registry.apply_selection_gesture(
        setup.session,
        vec![SelectionReference::Fixture {
            fixture_id: setup.fixtures[0],
        }],
        &HashMap::new(),
    ));
    let interaction_only = setup.handle(
        "interaction-only",
        1,
        ProgrammingValuesCommand::ReleaseFixture {
            fixture_id: setup.fixtures[2],
            attribute: AttributeKey::intensity(),
        },
    );
    assert_eq!(
        interaction_only.outcome,
        ProgrammingValuesOutcome::NoChange { revision: 1 }
    );
    assert_eq!(interaction_only.interaction_event_sequence, Some(2));
    assert_eq!(
        setup.values_events(setup.context.desk_id, setup.user).len(),
        1
    );
    assert_eq!(super::super::values_projection::projection_read_count(), 0);
}

#[test]
fn values_replay_precedes_revision_checks_and_failures_do_not_mutate() {
    let setup = ValuesSetup::new();
    let command = ProgrammingValuesCommand::SetFixture {
        fixture_id: setup.fixtures[0],
        attribute: AttributeKey::intensity(),
        value: AttributeValue::Normalized(0.4),
        timing: Default::default(),
    };
    let action = setup.action("set-1", 0, command.clone());
    let first = setup
        .service
        .handle_values(action.clone(), &setup.ports)
        .unwrap();
    let replayed = setup.service.handle_values(action, &setup.ports).unwrap();
    assert!(!first.replayed);
    assert!(replayed.replayed);
    assert_eq!(replayed.outcome, first.outcome);
    assert_eq!(setup.events.latest_sequence(), 1);
    assert_eq!(*setup.ports.persisted.lock(), vec!["programmer.values"]);

    let reused = setup.service.handle_values(
        setup.action("set-1", 0, ProgrammingValuesCommand::Clear),
        &setup.ports,
    );
    assert_eq!(reused.unwrap_err().kind, ActionErrorKind::Conflict);
    let stale = setup
        .service
        .handle_values(setup.action("stale", 0, command), &setup.ports);
    let stale = stale.unwrap_err();
    assert_eq!(stale.kind, ActionErrorKind::Conflict);
    assert_eq!(stale.current_revision, Some(1));

    let invalid = setup.service.handle_values(
        setup.action(
            "invalid",
            1,
            ProgrammingValuesCommand::Batch {
                mutations: vec![
                    fixture_set(setup.fixtures[1], "pan", 0.2, Default::default()),
                    fixture_set(setup.fixtures[1], "pan", 0.8, Default::default()),
                ],
            },
        ),
        &setup.ports,
    );
    assert_eq!(invalid.unwrap_err().kind, ActionErrorKind::Invalid);
    assert_eq!(
        setup.registry.normal_values_generation(setup.session),
        Some(1)
    );
    assert_eq!(setup.registry.get(setup.session).unwrap().undo.len(), 1);
    assert_eq!(setup.events.latest_sequence(), 1);
    assert_eq!(*setup.ports.persisted.lock(), vec!["programmer.values"]);
}

#[test]
fn release_and_clear_preserve_preload_transient_selection_and_modes() {
    let setup = ValuesSetup::new();
    setup.handle(
        "set",
        0,
        ProgrammingValuesCommand::Batch {
            mutations: vec![
                fixture_set(setup.fixtures[0], "intensity", 0.5, Default::default()),
                group_set(
                    "front",
                    "pan",
                    AttributeValue::Spread(vec![0.2, 0.8]),
                    Default::default(),
                ),
            ],
        },
    );
    let released_fixture = setup.handle(
        "release-fixture",
        1,
        ProgrammingValuesCommand::ReleaseFixture {
            fixture_id: setup.fixtures[0],
            attribute: AttributeKey::intensity(),
        },
    );
    assert_eq!(released_fixture.outcome.revision(), 2);
    let released_group = setup.handle(
        "release-group",
        2,
        ProgrammingValuesCommand::ReleaseGroup {
            group_id: "front".into(),
            attribute: AttributeKey("pan".into()),
        },
    );
    assert_eq!(released_group.outcome.revision(), 3);

    setup.handle(
        "set-again",
        3,
        ProgrammingValuesCommand::Batch {
            mutations: vec![
                fixture_set(setup.fixtures[0], "intensity", 0.7, Default::default()),
                group_set(
                    "front",
                    "tilt",
                    AttributeValue::Normalized(0.6),
                    Default::default(),
                ),
            ],
        },
    );
    setup.registry.select(setup.session, [setup.fixtures[2]]);
    assert!(setup.registry.arm_preload(setup.session, false));
    assert!(
        setup
            .registry
            .set_modes(setup.session, None, Some(true), Some(false), None,)
    );
    assert!(setup.registry.set_preload_group(
        setup.session,
        "back".into(),
        AttributeKey("pan".into()),
        AttributeValue::Normalized(0.3),
    ));
    assert!(
        setup
            .registry
            .set_transient_action(
                setup.session,
                "lamp-on".into(),
                [(
                    setup.fixtures[1],
                    AttributeKey("fixture-control".into()),
                    AttributeValue::RawDmxExact(255),
                )],
            )
            .is_some()
    );

    let cleared = setup.handle("clear", 4, ProgrammingValuesCommand::Clear);
    let ProgrammingValuesOutcome::Changed { projection, .. } = cleared.outcome else {
        panic!("clear should publish the empty authoritative projection")
    };
    assert_eq!(projection.revision, 5);
    assert!(projection.fixture_values.is_empty());
    assert!(projection.group_values.is_empty());
    let state = setup.registry.get(setup.session).unwrap();
    assert_eq!(state.selected, vec![setup.fixtures[2]]);
    assert!(state.blind);
    assert!(state.preview);
    assert_eq!(state.preload_group_pending.len(), 1);
    assert_eq!(state.transient_values.len(), 1);
    assert_eq!(
        setup.values_events(setup.context.desk_id, setup.user).len(),
        5
    );
}

#[test]
fn same_user_desks_share_values_while_other_users_and_forged_contexts_are_isolated() {
    let setup = ValuesSetup::new();
    let peer_session = SessionId::new();
    let peer_desk = Uuid::new_v4();
    setup.registry.start(peer_session, setup.user);
    setup
        .registry
        .attach_command_context(peer_session, SessionId(peer_desk));
    let peer_context =
        ActionContext::operator(peer_desk, setup.user.0, peer_session.0, ActionSource::Osc);
    setup.handle(
        "actor-set",
        0,
        ProgrammingValuesCommand::SetFixture {
            fixture_id: setup.fixtures[0],
            attribute: AttributeKey::intensity(),
            value: AttributeValue::Normalized(0.25),
            timing: Default::default(),
        },
    );
    setup
        .service
        .handle_values(
            ActionEnvelope {
                context: peer_context
                    .clone()
                    .with_request_id("peer-set")
                    .with_expected_revision(1),
                command: ProgrammingValuesRequest {
                    expected_capture_mode_revision: 0,
                    command: ProgrammingValuesCommand::SetGroup {
                        group_id: "front".into(),
                        attribute: AttributeKey("pan".into()),
                        value: AttributeValue::Normalized(0.5),
                        timing: Default::default(),
                    },
                },
            },
            &setup.ports,
        )
        .unwrap();
    let actor_snapshot = setup
        .service
        .values_snapshot(&setup.context, &setup.ports)
        .unwrap();
    assert_eq!(actor_snapshot.projection.revision, 2);
    assert_eq!(actor_snapshot.projection.fixture_values.len(), 1);
    assert_eq!(actor_snapshot.projection.group_values.len(), 1);
    assert_eq!(
        setup.values_events(setup.context.desk_id, setup.user).len(),
        2
    );
    assert_eq!(setup.values_events(peer_desk, setup.user).len(), 2);

    let other_user = UserId::new();
    let other_session = SessionId::new();
    let other_desk = Uuid::new_v4();
    setup.registry.start(other_session, other_user);
    setup
        .registry
        .attach_command_context(other_session, SessionId(other_desk));
    let other_context = ActionContext::operator(
        other_desk,
        other_user.0,
        other_session.0,
        ActionSource::Http,
    );
    setup
        .service
        .handle_values(
            ActionEnvelope {
                context: other_context
                    .clone()
                    .with_request_id("other-set")
                    .with_expected_revision(0),
                command: ProgrammingValuesRequest {
                    expected_capture_mode_revision: 0,
                    command: ProgrammingValuesCommand::SetFixture {
                        fixture_id: setup.fixtures[1],
                        attribute: AttributeKey::intensity(),
                        value: AttributeValue::Normalized(0.9),
                        timing: Default::default(),
                    },
                },
            },
            &setup.ports,
        )
        .unwrap();
    let other_snapshot = setup
        .service
        .values_snapshot(&other_context, &setup.ports)
        .unwrap();
    assert_eq!(other_snapshot.projection.revision, 1);
    assert_eq!(
        other_snapshot.projection.fixture_values[0].fixture_id,
        setup.fixtures[1]
    );
    assert_eq!(setup.values_events(other_desk, other_user).len(), 1);
    assert_eq!(setup.values_events(other_desk, setup.user).len(), 2);

    let forged = ActionContext::operator(
        other_desk,
        other_user.0,
        setup.session.0,
        ActionSource::Http,
    );
    let snapshot_error = setup
        .service
        .values_snapshot(&forged, &setup.ports)
        .unwrap_err();
    assert_eq!(snapshot_error.kind, ActionErrorKind::Forbidden);
    let action_error = setup
        .service
        .handle_values(
            ActionEnvelope {
                context: forged.with_request_id("forged").with_expected_revision(1),
                command: ProgrammingValuesRequest {
                    expected_capture_mode_revision: 0,
                    command: ProgrammingValuesCommand::Clear,
                },
            },
            &setup.ports,
        )
        .unwrap_err();
    assert_eq!(action_error.kind, ActionErrorKind::Forbidden);
    assert_eq!(setup.events.latest_sequence(), 3);
}
