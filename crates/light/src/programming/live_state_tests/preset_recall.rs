use super::*;
use crate::{
    ActionError, ActionErrorKind, ProgrammingPresetRecallEnvironment,
    ProgrammingPresetRecallOutcome, ProgrammingPresetRecallPorts, ProgrammingPresetRecallRequest,
    ProgrammingPresetRecallRevisionExpectation, ProgrammingPresetRecallTarget,
};
use chrono::{TimeZone, Utc};
use light_core::{AttributeKey, AttributeValue, ManualClock, ShowId};
use light_programmer::{GroupDefinition, Preset, PresetAddress, PresetFamily, SelectionReference};
use light_show::PortableShowRevision;
use std::collections::HashMap;

struct RecallPorts {
    environment: ProgrammingPresetRecallEnvironment,
    environment_reads: Mutex<usize>,
    persisted: Mutex<Vec<&'static str>>,
}

impl ProgrammingPresetRecallPorts for RecallPorts {
    fn authorize_preset_recall(&self, _context: &ActionContext) -> Result<(), ActionError> {
        Ok(())
    }

    fn preset_recall_environment(
        &self,
        _context: &ActionContext,
        _request: &ProgrammingPresetRecallRequest,
    ) -> Result<ProgrammingPresetRecallEnvironment, ActionError> {
        *self.environment_reads.lock() += 1;
        Ok(self.environment.clone())
    }

    fn persist_preset_recall(
        &self,
        _context: &ActionContext,
        operation: &'static str,
    ) -> Option<String> {
        self.persisted.lock().push(operation);
        None
    }
}

struct RecallSetup {
    clock: Arc<ManualClock>,
    registry: ProgrammerRegistry,
    service: ProgrammingService,
    events: EventBus,
    context: ActionContext,
    ports: RecallPorts,
    request: ProgrammingPresetRecallRequest,
    fixtures: [FixtureId; 2],
}

impl RecallSetup {
    fn new() -> Self {
        let started_at = Utc.with_ymd_and_hms(2026, 7, 21, 12, 0, 0).unwrap();
        let clock = Arc::new(ManualClock::new(started_at));
        let registry = ProgrammerRegistry::with_clock(clock.clone());
        let session = SessionId::new();
        let fixtures = [FixtureId::new(), FixtureId::new()];
        registry.start(session);
        let selection_revision = registry.select(session, [fixtures[1], fixtures[0]]);
        let events = EventBus::new(16);
        let service = ProgrammingService::new(
            registry.clone(),
            events.clone(),
            Arc::new(HighlightRegistry::default()),
        );
        let show_id = ShowId::new();
        let address = PresetAddress::new(PresetFamily::Mixed, 1).unwrap();
        let intensity = AttributeKey::intensity();
        let pan = AttributeKey("pan".into());
        let preset = Preset {
            name: "Look".into(),
            aim_at_fixture_number: None,
            family: PresetFamily::Mixed,
            number: 1,
            values: HashMap::from([
                (
                    fixtures[0],
                    HashMap::from([
                        (intensity.clone(), AttributeValue::Normalized(0.1)),
                        (pan, AttributeValue::Normalized(0.4)),
                    ]),
                ),
                (
                    fixtures[1],
                    HashMap::from([(intensity.clone(), AttributeValue::Normalized(0.2))]),
                ),
            ]),
            group_values: HashMap::from([(
                "5".into(),
                HashMap::from([(intensity, AttributeValue::Normalized(0.8))]),
            )]),
        };
        let raw_body = serde_json::json!({
            "name":"Look",
            "family":"Mixed",
            "number":1,
            "values":preset.values,
            "group_values":preset.group_values,
            "future_extension":{"retain":true},
        });
        let environment = ProgrammingPresetRecallEnvironment {
            show_id,
            show_revision: PortableShowRevision::from_value(11),
            object_id: address.storage_key(),
            object_revision: 7,
            address,
            raw_body: Arc::new(raw_body),
            preset: Arc::new(preset),
            groups: Arc::new(HashMap::from([(
                "5".into(),
                GroupDefinition {
                    id: "5".into(),
                    fixtures: fixtures.to_vec(),
                    ..GroupDefinition::default()
                },
            )])),
            selectable_targets: Arc::new(vec![fixtures[1], fixtures[0]]),
            target_expansions: Arc::new(HashMap::from([
                (fixtures[0], vec![fixtures[0]]),
                (fixtures[1], vec![fixtures[1]]),
            ])),
            programmer_fade_millis: 900,
        };
        Self {
            clock,
            registry,
            service,
            events,
            context: ActionContext::operator(Uuid::new_v4(), session.0, ActionSource::Http),
            ports: RecallPorts {
                environment,
                environment_reads: Mutex::new(0),
                persisted: Mutex::new(Vec::new()),
            },
            request: ProgrammingPresetRecallRequest {
                show_id,
                address,
                expected_preset_revision: exact(7),
                expected_show_revision: exact(11),
                expected_values_revision: exact(0),
                expected_preload_values_revision: exact(0),
                expected_capture_mode_revision: exact(0),
                expected_selection_revision: exact(selection_revision),
            },
            fixtures,
        }
    }

    fn apply(
        &self,
        request_id: &str,
        request: ProgrammingPresetRecallRequest,
    ) -> crate::ProgrammingPresetRecallResult {
        self.service
            .handle_preset_recall(
                ActionEnvelope {
                    context: self.context.clone().with_request_id(request_id),
                    command: request,
                },
                &self.ports,
            )
            .unwrap()
    }

    fn clear_selection_request(&mut self) -> ProgrammingPresetRecallRequest {
        let session = SessionId(self.context.session_id.unwrap());
        let revision = self.registry.select(session, []);
        self.request.expected_selection_revision = exact(revision);
        self.request.clone()
    }
}

#[test]
fn preset_recall_is_one_atomic_ordered_values_transition_with_one_timestamp_and_fade() {
    let setup = RecallSetup::new();
    setup.clock.advance_millis(2_000);

    let result = setup.apply("recall-1", setup.request.clone());

    let ProgrammingPresetRecallOutcome::Changed {
        values_revision,
        projection: Some(projection),
        values_event_sequence: Some(event_sequence),
    } = &result.outcome
    else {
        panic!("Preset recall should publish one complete values transition")
    };
    assert_eq!((*values_revision, *event_sequence), (1, 1));
    assert_eq!(result.interaction_event_sequence, None);
    assert_eq!(result.applied_fixtures, 2);
    assert_eq!(projection.fixture_values.len(), 3);
    assert_eq!(
        projection
            .fixture_values
            .iter()
            .map(|value| value.fixture_id)
            .collect::<Vec<_>>(),
        vec![setup.fixtures[1], setup.fixtures[0], setup.fixtures[0]]
    );
    let changed_at = setup.clock.advance_millis(0);
    let programmer = setup
        .registry
        .get(SessionId(setup.context.session_id.unwrap()))
        .unwrap();
    assert!(programmer.values.iter().all(|value| {
        value.changed_at == changed_at
            && value.fade
            && value.fade_millis == Some(900)
            && value.delay_millis.is_none()
    }));
    assert!(
        programmer
            .values
            .windows(2)
            .all(|pair| pair[0].programmer_order < pair[1].programmer_order)
    );
    assert_eq!(*setup.ports.environment_reads.lock(), 1);
    assert_eq!(*setup.ports.persisted.lock(), vec!["preset.apply"]);
    assert_eq!(setup.events.latest_sequence(), 1);
    assert_eq!(result.preset.raw_body["future_extension"]["retain"], true);

    let mut repeated_request = setup.request.clone();
    repeated_request.expected_values_revision = exact(1);
    let repeated = setup.apply("recall-1", repeated_request);
    assert!(matches!(
        repeated.outcome,
        ProgrammingPresetRecallOutcome::NoChange { .. }
    ));
    assert_eq!(repeated.interaction_event_sequence, None);
    assert_eq!(*setup.ports.environment_reads.lock(), 2);
    assert_eq!(setup.ports.persisted.lock().len(), 1);
    assert_eq!(setup.events.latest_sequence(), 1);
}

#[test]
fn gesture_close_is_one_sparse_interaction_transition_and_repeat_emits_nothing() {
    let setup = RecallSetup::new();
    setup.apply("recall-values", setup.request.clone());
    let session = SessionId(setup.context.session_id.unwrap());
    assert!(setup.registry.apply_selection_gesture(
        session,
        vec![
            SelectionReference::Fixture {
                fixture_id: setup.fixtures[1],
            },
            SelectionReference::Fixture {
                fixture_id: setup.fixtures[0],
            },
        ],
        &HashMap::new(),
    ));
    let open = setup.registry.selection(session).unwrap();
    assert!(open.gesture_open);
    let mut request = setup.request.clone();
    request.expected_values_revision = exact(1);
    request.expected_selection_revision = exact(open.revision);
    crate::programming::values_projection::reset_projection_read_count();

    let closed = setup.apply("recall-close-gesture", request.clone());

    assert!(matches!(
        closed.outcome,
        ProgrammingPresetRecallOutcome::Changed {
            values_revision: 1,
            projection: None,
            values_event_sequence: None,
        }
    ));
    assert_eq!(closed.interaction_event_sequence, Some(2));
    assert!(closed.selection_revision > open.revision);
    assert!(!setup.registry.selection(session).unwrap().gesture_open);
    assert_eq!(
        crate::programming::values_projection::projection_read_count(),
        0
    );
    assert_eq!(setup.events.latest_sequence(), 2);
    assert_eq!(setup.ports.persisted.lock().len(), 2);

    request.expected_selection_revision = exact(closed.selection_revision);
    let repeated = setup.apply("recall-close-gesture", request);
    assert_eq!(repeated.interaction_event_sequence, None);
    assert_eq!(repeated.selection_revision, closed.selection_revision);
    assert_eq!(
        crate::programming::values_projection::projection_read_count(),
        0
    );
    assert_eq!(setup.events.latest_sequence(), 2);
    assert_eq!(setup.ports.persisted.lock().len(), 2);
}

#[test]
fn empty_selection_first_tap_selects_targets_without_recalling_then_second_tap_recalls() {
    let mut setup = RecallSetup::new();
    let request = setup.clear_selection_request();
    let session = SessionId(setup.context.session_id.unwrap());

    let selected = setup.apply("select-targets", request);

    assert_eq!(
        selected.disposition,
        ProgrammingPresetRecallDisposition::TargetsSelected
    );
    assert!(matches!(
        selected.outcome,
        ProgrammingPresetRecallOutcome::Changed {
            values_revision: 0,
            projection: None,
            values_event_sequence: None,
        }
    ));
    assert_eq!(
        (selected.applied_fixtures, selected.selected_targets),
        (0, 2)
    );
    assert_eq!(selected.interaction_event_sequence, Some(1));
    assert_eq!(
        setup.registry.selection(session).unwrap().selected,
        vec![setup.fixtures[1], setup.fixtures[0]]
    );
    let programmer = setup.registry.get(session).unwrap();
    assert!(programmer.values.is_empty());
    assert_eq!(programmer.active_context, None);
    assert_eq!(*setup.ports.persisted.lock(), vec!["preset.select_targets"]);

    let mut recall = setup.request.clone();
    recall.expected_selection_revision = exact(selected.selection_revision);
    let recalled = setup.apply("recall-second-tap", recall);
    assert_eq!(
        recalled.disposition,
        ProgrammingPresetRecallDisposition::Recalled
    );
    assert_eq!(recalled.applied_fixtures, 2);
    assert_eq!(recalled.selected_targets, 0);
    assert!(matches!(
        recalled.outcome,
        ProgrammingPresetRecallOutcome::Changed {
            values_revision: 1,
            projection: Some(_),
            values_event_sequence: Some(_),
        }
    ));
}

#[test]
fn active_preload_recall_atomically_writes_pending_fixture_and_group_values_only() {
    let setup = RecallSetup::new();
    let session = SessionId(setup.context.session_id.unwrap());
    assert!(setup.registry.arm_preload(session, true));
    setup.registry.select_expression(
        session,
        setup.fixtures.to_vec(),
        light_programmer::SelectionExpression::LiveGroup {
            group_id: "5".into(),
            rule: light_programmer::SelectionRule::All,
        },
    );
    let mut request = setup.request.clone();
    request.expected_selection_revision =
        exact(setup.registry.selection(session).unwrap().revision);

    let result = setup.apply("recall-preload", request);

    assert_eq!(result.target, ProgrammingPresetRecallTarget::Preload);
    let ProgrammingPresetRecallOutcome::PreloadChanged {
        values_revision,
        preload_values_revision,
        projection: Some(projection),
        preload_values_event_sequence: Some(event_sequence),
    } = &result.outcome
    else {
        panic!("Preload recall should publish one complete pending-values transition")
    };
    assert_eq!(
        (*values_revision, *preload_values_revision, *event_sequence),
        (0, 1, 1)
    );
    assert_eq!(result.preload_values_revision, 1);
    assert_eq!(projection.fixture_values.len(), 3);
    assert_eq!(projection.group_values.len(), 1);
    assert!(projection.fixture_values.iter().all(|value| {
        value.fade && value.fade_millis == Some(900) && value.delay_millis.is_none()
    }));
    assert!(projection.group_values.iter().all(|value| {
        value.fade && value.fade_millis == Some(900) && value.delay_millis.is_none()
    }));
    let programmer = setup.registry.get(session).unwrap();
    assert!(programmer.values.is_empty());
    assert!(programmer.group_values.is_empty());
    assert!(programmer.preload_active.is_empty());
    assert!(programmer.preload_group_active.is_empty());
    assert_eq!(programmer.active_context, None);
    assert_eq!(*setup.ports.persisted.lock(), vec!["preset.apply_preload"]);
}

#[test]
fn active_preload_empty_selection_first_tap_selects_then_second_tap_writes_pending_values() {
    let mut setup = RecallSetup::new();
    let session = SessionId(setup.context.session_id.unwrap());
    assert!(setup.registry.arm_preload(session, true));
    let request = setup.clear_selection_request();

    let result = setup.apply("select-preload-targets", request);

    assert_eq!(result.target, ProgrammingPresetRecallTarget::Preload);
    assert_eq!(result.preload_values_revision, 0);
    assert!(matches!(
        result.outcome,
        ProgrammingPresetRecallOutcome::Changed {
            values_revision: 0,
            projection: None,
            values_event_sequence: None,
        }
    ));
    let programmer = setup.registry.get(session).unwrap();
    assert!(programmer.values.is_empty());
    assert!(programmer.preload_pending.is_empty());
    assert!(programmer.preload_active.is_empty());
    assert_eq!(setup.events.latest_sequence(), 1);

    let mut recall = setup.request.clone();
    recall.expected_selection_revision = exact(result.selection_revision);
    let recalled = setup.apply("recall-preload-second-tap", recall);
    let ProgrammingPresetRecallOutcome::PreloadChanged {
        values_revision: 0,
        preload_values_revision: 1,
        projection: Some(projection),
        preload_values_event_sequence: Some(event_sequence),
    } = recalled.outcome
    else {
        panic!("second Preload tap should publish pending values")
    };
    assert_eq!(event_sequence, 2);
    assert_eq!(projection.fixture_values.len(), 3);
    assert!(projection.group_values.is_empty());
    let programmer = setup.registry.get(session).unwrap();
    assert!(programmer.values.is_empty());
    assert!(programmer.group_values.is_empty());
    assert!(programmer.preload_active.is_empty());
    assert!(programmer.preload_group_active.is_empty());
    assert_eq!(setup.registry.normal_values_revision(), 0);
}

#[test]
fn color_position_and_mixed_presets_share_empty_selection_target_behavior() {
    for family in [
        PresetFamily::Color,
        PresetFamily::Position,
        PresetFamily::Mixed,
    ] {
        let mut setup = RecallSetup::new();
        let address = PresetAddress::new(family, 1).unwrap();
        setup.request.address = address;
        setup.ports.environment.address = address;
        Arc::make_mut(&mut setup.ports.environment.preset).family = family;
        let request = setup.clear_selection_request();

        let result = setup.apply("select-family-targets", request);

        assert_eq!(
            result.disposition,
            ProgrammingPresetRecallDisposition::TargetsSelected
        );
        assert_eq!(result.selected_targets, 2);
        assert_eq!(
            setup
                .registry
                .selection(SessionId(setup.context.session_id.unwrap()))
                .unwrap()
                .selected,
            vec![setup.fixtures[1], setup.fixtures[0]]
        );
    }
}

#[test]
fn missing_targets_are_skipped_with_warning_and_empty_preset_is_a_true_no_op() {
    let mut setup = RecallSetup::new();
    let missing = FixtureId::new();
    Arc::make_mut(&mut setup.ports.environment.preset)
        .values
        .insert(
            missing,
            HashMap::from([(AttributeKey::intensity(), AttributeValue::Normalized(0.5))]),
        );
    let request = setup.clear_selection_request();
    let selected = setup.apply("select-partial", request);
    assert_eq!(selected.selected_targets, 2);
    assert!(
        selected
            .warning
            .as_deref()
            .is_some_and(|warning| warning.contains("1 missing fixture target"))
    );

    let mut empty = RecallSetup::new();
    let preset = Arc::make_mut(&mut empty.ports.environment.preset);
    preset.values.clear();
    preset.group_values.clear();
    let request = empty.clear_selection_request();
    let revision_before = empty
        .registry
        .selection(SessionId(empty.context.session_id.unwrap()))
        .unwrap()
        .revision;
    let result = empty.apply("empty-preset", request);
    assert_eq!(
        result.disposition,
        ProgrammingPresetRecallDisposition::TargetsSelected
    );
    assert!(matches!(
        result.outcome,
        ProgrammingPresetRecallOutcome::NoChange { .. }
    ));
    assert_eq!(result.selected_targets, 0);
    assert_eq!(result.interaction_event_sequence, None);
    assert_eq!(
        empty
            .registry
            .selection(SessionId(empty.context.session_id.unwrap()))
            .unwrap()
            .revision,
        revision_before
    );
}

#[test]
fn active_context_only_recall_is_changed_but_sparse_then_exact_repeat_is_no_change() {
    let setup = RecallSetup::new();
    setup.apply("recall-values", setup.request.clone());
    let session = SessionId(setup.context.session_id.unwrap());
    setup.registry.set_modes(
        session,
        None,
        None,
        None,
        Some(Some("different-context".into())),
    );
    crate::programming::values_projection::reset_projection_read_count();
    let mut request = setup.request.clone();
    request.expected_values_revision = exact(1);

    let context_only = setup.apply("recall-context", request.clone());
    assert!(matches!(
        context_only.outcome,
        ProgrammingPresetRecallOutcome::Changed {
            values_revision: 1,
            projection: None,
            values_event_sequence: None,
        }
    ));
    assert_eq!(context_only.interaction_event_sequence, None);
    assert_eq!(
        crate::programming::values_projection::projection_read_count(),
        0
    );
    assert_eq!(setup.events.latest_sequence(), 1);
    assert_eq!(setup.ports.persisted.lock().len(), 2);

    let no_change = setup.apply("recall-no-change", request);
    assert!(matches!(
        no_change.outcome,
        ProgrammingPresetRecallOutcome::NoChange { values_revision: 1 }
    ));
    assert_eq!(no_change.interaction_event_sequence, None);
    assert_eq!(
        crate::programming::values_projection::projection_read_count(),
        0
    );
    assert_eq!(setup.events.latest_sequence(), 1);
    assert_eq!(setup.ports.persisted.lock().len(), 2);
}

#[test]
fn stale_preset_or_programmer_revision_is_rejected_before_mutation() {
    let setup = RecallSetup::new();
    let mut stale = setup.request.clone();
    stale.expected_values_revision = exact(9);
    let error = setup
        .service
        .handle_preset_recall(
            ActionEnvelope {
                context: setup.context.clone().with_request_id("stale-recall"),
                command: stale,
            },
            &setup.ports,
        )
        .unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert_eq!(error.current_revision, Some(0));
    assert_eq!(*setup.ports.environment_reads.lock(), 0);
    assert!(setup.ports.persisted.lock().is_empty());
    assert_eq!(setup.events.latest_sequence(), 0);
}

#[test]
fn stale_preload_revision_is_rejected_before_resolving_or_mutating_the_preset() {
    let setup = RecallSetup::new();
    let session = SessionId(setup.context.session_id.unwrap());
    assert!(setup.registry.arm_preload(session, true));
    let mut stale = setup.request.clone();
    stale.expected_preload_values_revision = exact(9);

    let error = setup
        .service
        .handle_preset_recall(
            ActionEnvelope {
                context: setup
                    .context
                    .clone()
                    .with_request_id("stale-preload-recall"),
                command: stale,
            },
            &setup.ports,
        )
        .unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert_eq!(error.current_revision, Some(0));
    assert_eq!(*setup.ports.environment_reads.lock(), 0);
    assert!(
        setup
            .registry
            .get(session)
            .unwrap()
            .preload_pending
            .is_empty()
    );
    assert_eq!(setup.events.latest_sequence(), 0);
}

#[test]
fn a_preset_recall_reaches_every_surface_of_the_desk() {
    let setup = RecallSetup::new();
    let second_screen = SessionId::new();
    let legacy_session = SessionId::new();
    setup.registry.start(second_screen);
    // A connection arriving under an identity from before the collapse joins the same Programmer.
    setup.registry.start(legacy_session);

    setup.apply("recall-shared", setup.request.clone());

    for (desk, session) in [
        (Uuid::new_v4(), second_screen),
        (Uuid::new_v4(), legacy_session),
    ] {
        let context = ActionContext::operator(desk, session.0, ActionSource::Http);
        let snapshot = setup
            .service
            .values_snapshot(&context, &LivePorts::default())
            .unwrap();
        assert_eq!(snapshot.projection.revision, 1);
        assert_eq!(snapshot.projection.fixture_values.len(), 3);
    }

    // Authentication still gates a recall. A system context operates no surface.
    let system = ActionContext::system(Uuid::new_v4(), ActionSource::System)
        .with_request_id("recall-unauthenticated");
    let error = setup
        .service
        .handle_preset_recall(
            ActionEnvelope {
                context: system,
                command: setup.request.clone(),
            },
            &setup.ports,
        )
        .unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Unauthorized);
    assert_eq!(setup.events.latest_sequence(), 1);
}

const fn exact(revision: u64) -> ProgrammingPresetRecallRevisionExpectation {
    ProgrammingPresetRecallRevisionExpectation::Exact(revision)
}
