use super::*;
use crate::{ActionErrorKind, EventObject};
use light_core::{AttributeKey, AttributeValue};
use light_programmer::{ProgrammerAlignmentMode, SelectionReference};
use std::collections::{HashMap, HashSet};
use std::sync::Barrier;

#[derive(Default)]
struct ValuesPorts {
    environment: ProgrammingValuesEnvironment,
    persisted: Mutex<Vec<&'static str>>,
    highlight_explicit: Mutex<Vec<Vec<(FixtureId, AttributeKey)>>>,
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

    fn mark_highlight_explicit_fixture_attributes(
        &self,
        _context: &ActionContext,
        touched: &[(FixtureId, AttributeKey)],
    ) {
        self.highlight_explicit.lock().push(touched.to_vec());
    }

    fn reconcile(&self, _context: &ActionContext, _reason: ProgrammingReconciliation) {}

    fn commit_preload(&self, _context: &ActionContext) -> Result<Option<String>, String> {
        Ok(None)
    }
}

#[test]
fn explicit_fixture_sets_notify_highlight_even_when_the_programmer_value_is_unchanged() {
    let setup = ValuesSetup::new();
    let fixture_id = setup.fixtures[0];
    let command = ProgrammingValuesCommand::SetFixture {
        fixture_id,
        attribute: AttributeKey::intensity(),
        value: AttributeValue::Normalized(0.4),
        timing: Default::default(),
    };
    setup.handle("highlight-explicit-first", 0, command.clone());
    setup.handle("highlight-explicit-same", 1, command);
    setup.handle(
        "highlight-release",
        1,
        ProgrammingValuesCommand::ReleaseFixture {
            fixture_id,
            attribute: AttributeKey::intensity(),
        },
    );

    assert_eq!(
        *setup.ports.highlight_explicit.lock(),
        vec![
            vec![(fixture_id, AttributeKey::intensity())],
            vec![(fixture_id, AttributeKey::intensity())],
        ]
    );
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
                    group_memberships: HashMap::from([("front".into(), 3), ("back".into(), 3)]),
                    group_members: HashMap::from([
                        ("front".into(), fixtures.to_vec()),
                        ("back".into(), fixtures.to_vec()),
                    ]),
                    ..Default::default()
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
fn align_modifies_future_relative_steps_and_reanchors_without_mutating_on_activation() {
    let mut setup = ValuesSetup::new();
    let pan = AttributeKey("pan".into());
    let tilt = AttributeKey("tilt".into());
    setup.registry.select(setup.session, setup.fixtures);
    for fixture in setup.fixtures {
        setup
            .ports
            .environment
            .supported_attributes
            .entry(fixture)
            .or_default()
            .extend([pan.clone(), tilt.clone()]);
        setup
            .ports
            .environment
            .current_values
            .insert((fixture, pan.clone()), AttributeValue::Normalized(0.4));
        setup
            .ports
            .environment
            .current_values
            .insert((fixture, tilt.clone()), AttributeValue::Normalized(0.2));
    }

    let activated = setup
        .service
        .set_alignment(
            &setup.context,
            &setup.ports,
            Some(ProgrammerAlignmentMode::Left),
        )
        .unwrap()
        .expect("Align should activate");
    assert_eq!(activated.fixtures, setup.fixtures);
    assert_eq!(setup.registry.normal_values_revision(setup.user), 0);
    assert!(setup.registry.get(setup.session).unwrap().values.is_empty());

    let request = setup.action(
        "aligned-pan",
        0,
        ProgrammingValuesCommand::ApplyIntent {
            intent: ProgrammingValueIntent {
                fixture_ids: vec![setup.fixtures[0]],
                group_id: None,
                attribute: pan.clone(),
                operation: ProgrammingValueOperation::RelativeStep(0.2),
                undo_group: Some("encoder-pan".into()),
                timing: Default::default(),
            },
        },
    );
    let first = setup
        .service
        .handle_values(request.clone(), &setup.ports)
        .unwrap();
    let ProgrammingValuesOutcome::Changed { projection, .. } = &first.outcome else {
        panic!("the aligned encoder step should change Programmer values")
    };
    let values = projection
        .fixture_values
        .iter()
        .map(|value| value.value.normalized().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(values, vec![0.4, 0.5, 0.6]);

    let replay = setup.service.handle_values(request, &setup.ports).unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.outcome, first.outcome);
    assert_eq!(setup.registry.alignment(setup.session).unwrap().revision, 2);

    setup
        .service
        .set_alignment(
            &setup.context,
            &setup.ports,
            Some(ProgrammerAlignmentMode::Out),
        )
        .unwrap();
    setup.handle(
        "aligned-pan-out",
        1,
        ProgrammingValuesCommand::ApplyIntent {
            intent: ProgrammingValueIntent {
                fixture_ids: setup.fixtures.to_vec(),
                group_id: None,
                attribute: pan,
                operation: ProgrammingValueOperation::RelativeStep(0.2),
                undo_group: Some("encoder-pan-out".into()),
                timing: Default::default(),
            },
        },
    );
    let content = setup.registry.get(setup.session).unwrap().update_content();
    let values = content
        .fixture_values
        .iter()
        .filter_map(|value| {
            value
                .value
                .normalized()
                .map(|normalized| (value.fixture_id, normalized))
        })
        .collect::<HashMap<_, _>>();
    assert_eq!(values.get(&setup.fixtures[0]), Some(&0.6));
    assert_eq!(values.get(&setup.fixtures[1]), Some(&0.5));
    assert_eq!(values.get(&setup.fixtures[2]), Some(&0.8));

    setup.handle(
        "different-attribute",
        2,
        ProgrammingValuesCommand::ApplyIntent {
            intent: ProgrammingValueIntent {
                fixture_ids: vec![setup.fixtures[0]],
                group_id: None,
                attribute: tilt,
                operation: ProgrammingValueOperation::RelativeStep(0.1),
                undo_group: None,
                timing: Default::default(),
            },
        },
    );
    assert!(setup.registry.alignment(setup.session).is_none());
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
    assert_eq!(mode_change.capture_mode_event_sequence, Some(3));
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
fn value_intent_identity_and_injected_activation_expansion_are_atomic() {
    let mut setup = ValuesSetup::new();
    let fixture = setup.fixtures[0];
    let intensity = AttributeKey::intensity();
    let pan = AttributeKey("pan".into());
    setup.ports.environment.current_values = light_engine::ResolvedValues::from_iter([(
        (fixture, pan.clone()),
        AttributeValue::Normalized(0.7),
    )]);
    setup.ports.environment.default_values = light_engine::ResolvedValues::from_iter([(
        (fixture, intensity.clone()),
        AttributeValue::Normalized(0.4),
    )]);
    setup.ports.environment.supported_attributes =
        HashMap::from([(fixture, HashSet::from([intensity.clone(), pan.clone()]))]);

    let absolute = setup.handle(
        "intent-absolute",
        0,
        ProgrammingValuesCommand::ApplyIntent {
            intent: ProgrammingValueIntent {
                fixture_ids: vec![fixture],
                group_id: None,
                attribute: intensity.clone(),
                operation: ProgrammingValueOperation::AbsoluteSet(AttributeValue::Normalized(0.25)),
                undo_group: None,
                timing: Default::default(),
            },
        },
    );
    let ProgrammingValuesOutcome::Changed { projection, .. } = absolute.outcome else {
        panic!("the identity intent should change one value")
    };
    assert_eq!(projection.fixture_values.len(), 1);
    assert_eq!(projection.fixture_values[0].attribute, intensity);
    assert_eq!(setup.registry.get(setup.session).unwrap().undo.len(), 1);
    assert!(setup.registry.undo(setup.session));

    setup
        .ports
        .environment
        .activation_links
        .insert(intensity.clone(), vec![pan.clone()]);
    let relative_action = setup.action(
        "intent-relative",
        1,
        ProgrammingValuesCommand::ApplyIntent {
            intent: ProgrammingValueIntent {
                fixture_ids: vec![fixture],
                group_id: None,
                attribute: intensity.clone(),
                operation: ProgrammingValueOperation::RelativeStep(0.1),
                undo_group: None,
                timing: Default::default(),
            },
        },
    );
    let relative = setup
        .service
        .handle_values(relative_action.clone(), &setup.ports)
        .unwrap();
    let ProgrammingValuesOutcome::Changed {
        projection,
        event_sequence,
    } = &relative.outcome
    else {
        panic!("the relative intent should change linked values")
    };
    assert_eq!(projection.revision, 2);
    assert_eq!(*event_sequence, 3);
    assert_eq!(projection.fixture_values.len(), 2);
    assert_eq!(
        projection
            .fixture_values
            .iter()
            .find(|value| value.attribute == intensity)
            .and_then(|value| value.value.normalized()),
        Some(0.5)
    );
    assert_eq!(
        projection
            .fixture_values
            .iter()
            .find(|value| value.attribute == pan)
            .and_then(|value| value.value.normalized()),
        Some(0.7)
    );
    assert_eq!(setup.registry.get(setup.session).unwrap().undo.len(), 1);
    assert_eq!(
        *setup.ports.persisted.lock(),
        vec!["programmer.values", "programmer.values"]
    );

    setup
        .ports
        .environment
        .current_values
        .insert((fixture, intensity), AttributeValue::Normalized(0.9));
    let replay = setup
        .service
        .handle_values(relative_action, &setup.ports)
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.outcome, relative.outcome);
    assert_eq!(setup.registry.get(setup.session).unwrap().undo.len(), 1);

    assert!(setup.registry.undo(setup.session));
    let undone = setup.registry.get(setup.session).unwrap();
    assert!(undone.values.is_empty());
}

#[test]
fn relative_fixture_intent_starts_from_the_owned_programmer_value() {
    let mut setup = ValuesSetup::new();
    let fixture = setup.fixtures[0];
    let intensity = AttributeKey::intensity();
    setup.ports.environment.current_values = light_engine::ResolvedValues::from_iter([(
        (fixture, intensity.clone()),
        AttributeValue::Normalized(0.0),
    )]);

    setup.handle(
        "owned-relative-base",
        0,
        ProgrammingValuesCommand::ApplyIntent {
            intent: ProgrammingValueIntent {
                fixture_ids: vec![fixture],
                group_id: None,
                attribute: intensity.clone(),
                operation: ProgrammingValueOperation::AbsoluteSet(AttributeValue::Normalized(0.5)),
                undo_group: None,
                timing: Default::default(),
            },
        },
    );
    let relative = setup.handle(
        "owned-relative-step",
        1,
        ProgrammingValuesCommand::ApplyIntent {
            intent: ProgrammingValueIntent {
                fixture_ids: vec![fixture],
                group_id: None,
                attribute: intensity,
                operation: ProgrammingValueOperation::RelativeStep(0.001),
                undo_group: None,
                timing: Default::default(),
            },
        },
    );

    let ProgrammingValuesOutcome::Changed { projection, .. } = relative.outcome else {
        panic!("the relative intent should change the owned Programmer value")
    };
    assert_eq!(
        projection.fixture_values[0].value,
        AttributeValue::Normalized(0.501)
    );
}

#[test]
fn relative_fixture_intent_without_a_resolvable_base_is_actionable_and_atomic() {
    let mut setup = ValuesSetup::new();
    let fixture = setup.fixtures[0];
    let focus = AttributeKey("focus".into());
    setup
        .ports
        .environment
        .supported_attributes
        .insert(fixture, HashSet::from([focus.clone()]));

    let rejected = setup
        .service
        .handle_values(
            setup.action(
                "missing-relative-base",
                0,
                ProgrammingValuesCommand::ApplyIntent {
                    intent: ProgrammingValueIntent {
                        fixture_ids: vec![fixture],
                        group_id: None,
                        attribute: focus,
                        operation: ProgrammingValueOperation::RelativeStep(0.01),
                        undo_group: Some("encoder-focus".into()),
                        timing: Default::default(),
                    },
                },
            ),
            &setup.ports,
        )
        .unwrap_err();

    assert_eq!(rejected.kind, ActionErrorKind::Invalid);
    assert!(rejected.message.contains("Cannot adjust focus"));
    assert!(rejected.message.contains("Set an absolute value"));
    assert!(setup.registry.get(setup.session).unwrap().values.is_empty());
    assert!(setup.ports.persisted.lock().is_empty());
}

#[test]
fn relative_group_intent_preserves_live_group_scope_and_mixed_member_values() {
    let mut setup = ValuesSetup::new();
    let attribute = AttributeKey::intensity();
    for (fixture, value) in setup.fixtures.into_iter().zip([0.1, 0.4, 0.95]) {
        setup.ports.environment.current_values.insert(
            (fixture, attribute.clone()),
            AttributeValue::Normalized(value),
        );
    }
    let result = setup.handle(
        "group-relative",
        0,
        ProgrammingValuesCommand::ApplyIntent {
            intent: ProgrammingValueIntent {
                fixture_ids: Vec::new(),
                group_id: Some("front".into()),
                attribute: attribute.clone(),
                operation: ProgrammingValueOperation::RelativeStep(0.1),
                undo_group: None,
                timing: Default::default(),
            },
        },
    );
    let ProgrammingValuesOutcome::Changed { projection, .. } = result.outcome else {
        panic!("the Group intent should change one live Group value")
    };
    assert!(projection.fixture_values.is_empty());
    assert_eq!(projection.group_values.len(), 1);
    assert_eq!(projection.group_values[0].group_id, "front");
    assert_eq!(
        projection.group_values[0].value,
        AttributeValue::Spread(vec![0.2, 0.5, 1.0])
    );
}

#[test]
fn group_intent_captures_supported_linked_values_per_fixture_in_one_undo_step() {
    let mut setup = ValuesSetup::new();
    let red = AttributeKey("color.red".into());
    let green = AttributeKey("color.green".into());
    setup
        .ports
        .environment
        .activation_links
        .insert(red.clone(), vec![green.clone()]);
    for (index, fixture) in setup.fixtures.into_iter().enumerate() {
        setup.ports.environment.current_values.insert(
            (fixture, green.clone()),
            AttributeValue::Normalized(index as f32 / 10.0),
        );
        if index < 2 {
            setup
                .ports
                .environment
                .supported_attributes
                .entry(fixture)
                .or_default()
                .extend([red.clone(), green.clone()]);
        }
    }

    let result = setup.handle(
        "group-linked",
        0,
        ProgrammingValuesCommand::ApplyIntent {
            intent: ProgrammingValueIntent {
                fixture_ids: Vec::new(),
                group_id: Some("front".into()),
                attribute: red,
                operation: ProgrammingValueOperation::AbsoluteSet(AttributeValue::Normalized(0.8)),
                undo_group: None,
                timing: Default::default(),
            },
        },
    );
    let ProgrammingValuesOutcome::Changed { projection, .. } = result.outcome else {
        panic!("the Group intent should capture linked fixture values")
    };
    assert_eq!(projection.group_values.len(), 1);
    assert_eq!(projection.fixture_values.len(), 2);
    assert!(
        projection
            .fixture_values
            .iter()
            .all(|value| value.attribute == green)
    );
    assert_eq!(setup.registry.get(setup.session).unwrap().undo.len(), 1);
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
    assert_eq!(setup.events.latest_sequence(), 2);
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
    assert_eq!(interaction_only.interaction_event_sequence, Some(3));
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
    assert_eq!(setup.events.latest_sequence(), 2);
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
    assert_eq!(setup.events.latest_sequence(), 2);
    assert_eq!(*setup.ports.persisted.lock(), vec!["programmer.values"]);
}

#[test]
fn group_spread_with_more_control_points_than_ranks_is_rejected_without_mutation() {
    let mut setup = ValuesSetup::new();
    let rejected = setup
        .service
        .handle_values(
            setup.action(
                "overfull",
                0,
                ProgrammingValuesCommand::SetGroup {
                    group_id: "front".into(),
                    attribute: AttributeKey::intensity(),
                    value: AttributeValue::Spread(vec![1.0, 0.0, 1.0, 0.0]),
                    timing: Default::default(),
                },
            ),
            &setup.ports,
        )
        .unwrap_err();
    assert_eq!(rejected.kind, ActionErrorKind::Invalid);
    assert!(rejected.message.contains("control points"));
    assert!(
        setup
            .registry
            .get(setup.session)
            .unwrap()
            .group_values
            .is_empty()
    );
    assert!(setup.ports.persisted.lock().is_empty());

    setup
        .ports
        .environment
        .group_rank_counts
        .insert("front".into(), 2);
    let tied_ranks = setup
        .service
        .handle_values(
            setup.action(
                "tied-ranks",
                0,
                ProgrammingValuesCommand::SetGroup {
                    group_id: "front".into(),
                    attribute: AttributeKey::intensity(),
                    value: AttributeValue::Spread(vec![1.0, 0.0, 1.0]),
                    timing: Default::default(),
                },
            ),
            &setup.ports,
        )
        .unwrap_err();
    assert!(tied_ranks.message.contains("only 2 ranks"));

    let accepted = setup.handle(
        "two-point",
        0,
        ProgrammingValuesCommand::SetGroup {
            group_id: "front".into(),
            attribute: AttributeKey::intensity(),
            value: AttributeValue::Spread(vec![1.0, 0.0]),
            timing: Default::default(),
        },
    );
    assert_eq!(accepted.outcome.revision(), 1);
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
    assert_eq!(setup.events.latest_sequence(), 6);
}
