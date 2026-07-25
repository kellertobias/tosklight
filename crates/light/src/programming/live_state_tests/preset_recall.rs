use super::*;
use crate::{
    ActionError, ActionErrorKind, ProgrammingPresetRecallEnvironment,
    ProgrammingPresetRecallOutcome, ProgrammingPresetRecallPorts, ProgrammingPresetRecallRequest,
    ProgrammingPresetRecallRevisionExpectation,
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
        let user = UserId::new();
        let fixtures = [FixtureId::new(), FixtureId::new()];
        registry.start(session, user);
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
            programmer_fade_millis: 900,
        };
        Self {
            clock,
            registry,
            service,
            events,
            context: ActionContext::operator(Uuid::new_v4(), user.0, session.0, ActionSource::Http),
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

    let replay = setup.apply("recall-1", setup.request.clone());
    assert!(replay.replayed);
    assert_eq!(replay.interaction_event_sequence, None);
    assert_eq!(*setup.ports.environment_reads.lock(), 1);
    assert_eq!(setup.ports.persisted.lock().len(), 1);
    assert_eq!(setup.events.latest_sequence(), 1);
}

#[test]
fn gesture_close_is_one_sparse_interaction_transition_and_replay_emits_nothing() {
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

    let replay = setup.apply("recall-close-gesture", request);
    assert!(replay.replayed);
    assert_eq!(replay.interaction_event_sequence, Some(2));
    assert_eq!(replay.selection_revision, closed.selection_revision);
    assert_eq!(
        crate::programming::values_projection::projection_read_count(),
        0
    );
    assert_eq!(setup.events.latest_sequence(), 2);
    assert_eq!(setup.ports.persisted.lock().len(), 2);
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
fn preset_recall_is_shared_between_user_desks_and_isolated_from_another_user() {
    let setup = RecallSetup::new();
    let user = UserId(setup.context.user_id.unwrap());
    let peer_session = SessionId::new();
    let foreign_user = UserId::new();
    let foreign_session = SessionId::new();
    setup.registry.start(peer_session, user);
    setup.registry.start(foreign_session, foreign_user);

    setup.apply("recall-shared", setup.request.clone());

    let peer_context =
        ActionContext::operator(Uuid::new_v4(), user.0, peer_session.0, ActionSource::Http);
    let peer = setup
        .service
        .values_snapshot(&peer_context, &LivePorts::default())
        .unwrap();
    assert_eq!(peer.projection.revision, 1);
    assert_eq!(peer.projection.fixture_values.len(), 3);
    let foreign_context = ActionContext::operator(
        Uuid::new_v4(),
        foreign_user.0,
        foreign_session.0,
        ActionSource::Http,
    );
    let foreign = setup
        .service
        .values_snapshot(&foreign_context, &LivePorts::default())
        .unwrap();
    assert_eq!(foreign.projection.revision, 0);
    assert!(foreign.projection.fixture_values.is_empty());

    let forged = ActionContext::operator(
        peer_context.desk_id,
        foreign_user.0,
        peer_session.0,
        ActionSource::Http,
    )
    .with_request_id("recall-forged-owner");
    let error = setup
        .service
        .handle_preset_recall(
            ActionEnvelope {
                context: forged,
                command: setup.request.clone(),
            },
            &setup.ports,
        )
        .unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Forbidden);
    assert_eq!(setup.events.latest_sequence(), 1);
}

const fn exact(revision: u64) -> ProgrammingPresetRecallRevisionExpectation {
    ProgrammingPresetRecallRevisionExpectation::Exact(revision)
}
