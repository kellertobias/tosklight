use super::*;
use crate::{
    ActionError, ActionErrorKind, ProgrammingPresetCommit, ProgrammingPresetCommitResult,
    ProgrammingPresetProjection, ProgrammingPresetRecordOutcome, ProgrammingPresetRecordRequest,
    ProgrammingPresetRecordingPorts, ProgrammingPresetRevisionExpectation,
};
use light_core::{AttributeKey, AttributeValue, Revision, ShowId};
use light_programmer::{Preset, PresetAddress, PresetFamily, PresetStoreMode};
use light_show::PortableShowRevision;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize};
use std::time::Duration;

#[derive(Clone)]
struct StoredPreset {
    revision: Revision,
    preset: Preset,
}

struct FakeShow {
    show_id: ShowId,
    revision: Revision,
    presets: HashMap<PresetAddress, StoredPreset>,
}

struct PresetPorts {
    show: Mutex<FakeShow>,
    calls: AtomicUsize,
    event_sequence: AtomicU64,
}

impl PresetPorts {
    fn new(show_id: ShowId) -> Self {
        Self {
            show: Mutex::new(FakeShow {
                show_id,
                revision: 0,
                presets: HashMap::new(),
            }),
            calls: AtomicUsize::new(0),
            event_sequence: AtomicU64::new(0),
        }
    }
}

impl ProgrammingPresetRecordingPorts for PresetPorts {
    fn authorize_preset_recording(&self, _context: &ActionContext) -> Result<(), ActionError> {
        Ok(())
    }

    fn commit_preset(
        &self,
        _context: &ActionContext,
        commit: &ProgrammingPresetCommit,
    ) -> Result<ProgrammingPresetCommitResult, ActionError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        let mut show = self.show.lock();
        validate_fake_show(&show, commit)?;
        let current = show.presets.get(&commit.address).cloned();
        validate_fake_object(current.as_ref(), commit.expected_object_revision)?;
        let preset = commit.merged_with(current.as_ref().map(|stored| &stored.preset))?;
        let changed = current
            .as_ref()
            .is_none_or(|stored| stored.preset != preset);
        let object_revision =
            current.as_ref().map_or(0, |stored| stored.revision) + u64::from(changed);
        let event_sequence = changed.then(|| {
            show.revision += 1;
            show.presets.insert(
                commit.address,
                StoredPreset {
                    revision: object_revision,
                    preset: preset.clone(),
                },
            );
            self.event_sequence.fetch_add(1, Ordering::Relaxed) + 1
        });
        let raw_body = serde_json::to_value(&preset).unwrap();
        Ok(ProgrammingPresetCommitResult {
            changed,
            projection: ProgrammingPresetProjection {
                show_id: show.show_id,
                object_id: commit.address.storage_key(),
                address: commit.address,
                object_revision,
                raw_body: Arc::new(raw_body),
            },
            show_revision: PortableShowRevision::from_value(show.revision),
            event_sequence,
        })
    }
}

fn validate_fake_show(
    show: &FakeShow,
    commit: &ProgrammingPresetCommit,
) -> Result<(), ActionError> {
    if show.show_id != commit.show_id {
        return Err(ActionError::new(
            ActionErrorKind::NotFound,
            "show is not active",
        ));
    }
    if let Some(expected) = commit.expected_show_revision
        && expected.value() != show.revision
    {
        return Err(
            ActionError::new(ActionErrorKind::Conflict, "show revision conflict")
                .at_related_revision(show.revision),
        );
    }
    Ok(())
}

fn validate_fake_object(
    current: Option<&StoredPreset>,
    expected: ProgrammingPresetRevisionExpectation,
) -> Result<(), ActionError> {
    let actual = current.map_or(0, |stored| stored.revision);
    if matches!(expected, ProgrammingPresetRevisionExpectation::Current)
        || expected == ProgrammingPresetRevisionExpectation::Exact(actual)
    {
        Ok(())
    } else {
        Err(
            ActionError::new(ActionErrorKind::Conflict, "Preset revision conflict")
                .at_revision(actual),
        )
    }
}

struct PresetSetup {
    registry: ProgrammerRegistry,
    service: ProgrammingService,
    ports: Arc<PresetPorts>,
    show_id: ShowId,
    user_id: UserId,
    session_id: SessionId,
    context: ActionContext,
}

impl PresetSetup {
    fn new() -> Self {
        let registry = ProgrammerRegistry::default();
        let user_id = UserId::new();
        let session_id = SessionId::new();
        let desk_id = Uuid::new_v4();
        let show_id = ShowId::new();
        registry.start(session_id, user_id);
        registry.attach_command_context(session_id, SessionId(desk_id));
        Self {
            service: ProgrammingService::new(
                registry.clone(),
                EventBus::default(),
                Arc::new(HighlightRegistry::default()),
            ),
            registry,
            ports: Arc::new(PresetPorts::new(show_id)),
            show_id,
            user_id,
            session_id,
            context: ActionContext::operator(desk_id, user_id.0, session_id.0, ActionSource::Http),
        }
    }

    fn request(
        &self,
        address: PresetAddress,
        mode: PresetStoreMode,
        expectation: ProgrammingPresetRevisionExpectation,
    ) -> ProgrammingPresetRecordRequest {
        ProgrammingPresetRecordRequest {
            show_id: self.show_id,
            address,
            name: format!("Preset {}", address.storage_key()),
            mode,
            expected_object_revision: expectation,
            expected_show_revision: None,
        }
    }

    fn action(
        &self,
        request_id: &str,
        request: ProgrammingPresetRecordRequest,
    ) -> ActionEnvelope<ProgrammingPresetRecordRequest> {
        ActionEnvelope {
            context: self.context.clone().with_request_id(request_id),
            command: request,
        }
    }
}

#[test]
fn fixture_and_group_capture_returns_one_authoritative_projection() {
    let setup = PresetSetup::new();
    let fixture = FixtureId::new();
    setup.registry.set_faded_with_timing(
        setup.session_id,
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.7),
        Some(1_000),
        Some(250),
    );
    setup.registry.set_group(
        setup.session_id,
        "front".into(),
        AttributeKey("pan".into()),
        AttributeValue::Normalized(0.3),
    );
    let address = PresetAddress::new(PresetFamily::Mixed, 4).unwrap();
    let request = setup.request(
        address,
        PresetStoreMode::Overwrite,
        ProgrammingPresetRevisionExpectation::Exact(0),
    );

    let result = setup
        .service
        .handle_preset_recording(setup.action("record-values", request), setup.ports.as_ref())
        .unwrap();
    let ProgrammingPresetRecordOutcome::Changed {
        projection,
        show_revision,
        event_sequence,
    } = result.outcome
    else {
        panic!("first recording must change the Preset")
    };
    let projected: Preset = serde_json::from_value(projection.raw_body.as_ref().clone()).unwrap();
    assert_eq!(
        projected.values[&fixture][&AttributeKey::intensity()],
        AttributeValue::Normalized(0.7)
    );
    assert_eq!(
        projected.group_values["front"][&AttributeKey("pan".into())],
        AttributeValue::Normalized(0.3)
    );
    assert_eq!(projection.object_revision, 1);
    assert_eq!(show_revision.value(), 1);
    assert_eq!(event_sequence, 1);
    assert_eq!(projection.raw_body["number"], 4);
}

#[test]
fn empty_family_capture_is_rejected_before_the_port() {
    let setup = PresetSetup::new();
    setup.registry.set(
        setup.session_id,
        FixtureId::new(),
        AttributeKey("pan".into()),
        AttributeValue::Normalized(0.2),
    );
    let request = setup.request(
        PresetAddress::new(PresetFamily::Color, 1).unwrap(),
        PresetStoreMode::Overwrite,
        ProgrammingPresetRevisionExpectation::Current,
    );
    let error = setup
        .service
        .handle_preset_recording(setup.action("empty", request), setup.ports.as_ref())
        .unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Invalid);
    assert_eq!(setup.ports.calls.load(Ordering::Relaxed), 0);
}

#[test]
fn invalid_show_address_and_name_are_rejected_before_the_port() {
    let setup = PresetSetup::new();
    setup.registry.set(
        setup.session_id,
        FixtureId::new(),
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.5),
    );
    let valid = setup.request(
        PresetAddress::new(PresetFamily::Intensity, 1).unwrap(),
        PresetStoreMode::Overwrite,
        ProgrammingPresetRevisionExpectation::Current,
    );
    let mut requests = Vec::new();
    let mut invalid_show = valid.clone();
    invalid_show.show_id = ShowId(Uuid::nil());
    requests.push(invalid_show);
    let mut invalid_address = valid.clone();
    invalid_address.address.number = 0;
    requests.push(invalid_address);
    let mut empty_name = valid.clone();
    empty_name.name.clear();
    requests.push(empty_name);
    let mut control_name = valid.clone();
    control_name.name = "bad\nname".into();
    requests.push(control_name);
    let mut long_name = valid;
    long_name.name = "x".repeat(257);
    requests.push(long_name);

    for (index, request) in requests.into_iter().enumerate() {
        let error = setup
            .service
            .handle_preset_recording(
                setup.action(&format!("invalid-{index}"), request),
                setup.ports.as_ref(),
            )
            .unwrap_err();
        assert_eq!(error.kind, ActionErrorKind::Invalid);
    }
    assert_eq!(setup.ports.calls.load(Ordering::Relaxed), 0);
}

#[test]
fn replay_no_change_and_changed_request_reuse_are_distinct() {
    let setup = PresetSetup::new();
    setup.registry.set(
        setup.session_id,
        FixtureId::new(),
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.5),
    );
    let address = PresetAddress::new(PresetFamily::Intensity, 2).unwrap();
    let request = setup.request(
        address,
        PresetStoreMode::Merge,
        ProgrammingPresetRevisionExpectation::Exact(0),
    );
    let action = setup.action("stable-request", request.clone());
    let first = setup
        .service
        .handle_preset_recording(action.clone(), setup.ports.as_ref())
        .unwrap();
    let replay = setup
        .service
        .handle_preset_recording(action, setup.ports.as_ref())
        .unwrap();
    assert!(!first.replayed);
    assert!(replay.replayed);
    assert_eq!(replay.outcome, first.outcome);
    assert_eq!(setup.ports.calls.load(Ordering::Relaxed), 1);

    let mut changed_request = request;
    changed_request.name = "Changed reuse".into();
    let reused = setup.service.handle_preset_recording(
        setup.action("stable-request", changed_request),
        setup.ports.as_ref(),
    );
    assert_eq!(reused.unwrap_err().kind, ActionErrorKind::Conflict);
    assert_eq!(setup.ports.calls.load(Ordering::Relaxed), 1);

    let no_change = setup
        .service
        .handle_preset_recording(
            setup.action(
                "no-change",
                setup.request(
                    address,
                    PresetStoreMode::Merge,
                    ProgrammingPresetRevisionExpectation::Exact(1),
                ),
            ),
            setup.ports.as_ref(),
        )
        .unwrap();
    assert!(matches!(
        no_change.outcome,
        ProgrammingPresetRecordOutcome::NoChange { .. }
    ));
    assert_eq!(setup.ports.calls.load(Ordering::Relaxed), 2);
}

#[test]
fn exact_revision_conflicts_while_current_resolves_under_the_port_transaction() {
    let setup = PresetSetup::new();
    setup.registry.set(
        setup.session_id,
        FixtureId::new(),
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.5),
    );
    let address = PresetAddress::new(PresetFamily::Intensity, 12).unwrap();
    setup
        .service
        .handle_preset_recording(
            setup.action(
                "create",
                setup.request(
                    address,
                    PresetStoreMode::Overwrite,
                    ProgrammingPresetRevisionExpectation::Exact(0),
                ),
            ),
            setup.ports.as_ref(),
        )
        .unwrap();

    let stale = setup.service.handle_preset_recording(
        setup.action(
            "stale",
            setup.request(
                address,
                PresetStoreMode::Overwrite,
                ProgrammingPresetRevisionExpectation::Exact(0),
            ),
        ),
        setup.ports.as_ref(),
    );
    assert_eq!(stale.unwrap_err().current_revision, Some(1));

    let current = setup
        .service
        .handle_preset_recording(
            setup.action(
                "current",
                setup.request(
                    address,
                    PresetStoreMode::Overwrite,
                    ProgrammingPresetRevisionExpectation::Current,
                ),
            ),
            setup.ports.as_ref(),
        )
        .unwrap();
    assert!(matches!(
        current.outcome,
        ProgrammingPresetRecordOutcome::NoChange { .. }
    ));
}

#[test]
fn already_gated_interaction_uses_the_non_reentrant_recording_bridge() {
    let setup = PresetSetup::new();
    setup.registry.set(
        setup.session_id,
        FixtureId::new(),
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.5),
    );
    let action = setup.action(
        "nested-command",
        setup.request(
            PresetAddress::new(PresetFamily::Intensity, 13).unwrap(),
            PresetStoreMode::Overwrite,
            ProgrammingPresetRevisionExpectation::Current,
        ),
    );
    let completed = setup
        .service
        .run_external_interaction(&setup.context, &LivePorts::default(), || {
            setup
                .service
                .record_preset_within_interaction(action, setup.ports.as_ref())
        })
        .unwrap();
    assert!(completed.output.unwrap().outcome.event_sequence().is_some());
}

#[test]
fn owner_and_preload_capture_are_rejected_before_commit() {
    let setup = PresetSetup::new();
    setup.registry.set(
        setup.session_id,
        FixtureId::new(),
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.5),
    );
    let address = PresetAddress::new(PresetFamily::Intensity, 3).unwrap();
    let request = setup.request(
        address,
        PresetStoreMode::Overwrite,
        ProgrammingPresetRevisionExpectation::Current,
    );
    let mut forged = setup.action("forged", request.clone());
    forged.context.user_id = Some(Uuid::new_v4());
    let error = setup
        .service
        .handle_preset_recording(forged, setup.ports.as_ref())
        .unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Forbidden);

    assert!(setup.registry.arm_preload(setup.session_id, true));
    let error = setup
        .service
        .handle_preset_recording(setup.action("preload", request), setup.ports.as_ref())
        .unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert_eq!(setup.ports.calls.load(Ordering::Relaxed), 0);
}

#[test]
fn replay_identity_isolated_by_user_desk_and_session() {
    let setup = PresetSetup::new();
    let fixture = FixtureId::new();
    setup.registry.set(
        setup.session_id,
        fixture,
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.5),
    );
    let peer_session = SessionId::new();
    let peer_desk = Uuid::new_v4();
    setup.registry.start(peer_session, setup.user_id);
    setup
        .registry
        .attach_command_context(peer_session, SessionId(peer_desk));
    let request = setup.request(
        PresetAddress::new(PresetFamily::Intensity, 8).unwrap(),
        PresetStoreMode::Overwrite,
        ProgrammingPresetRevisionExpectation::Current,
    );
    setup
        .service
        .handle_preset_recording(
            setup.action("shared-id", request.clone()),
            setup.ports.as_ref(),
        )
        .unwrap();
    let peer = ActionEnvelope {
        context: ActionContext::operator(
            peer_desk,
            setup.user_id.0,
            peer_session.0,
            ActionSource::Osc,
        )
        .with_request_id("shared-id"),
        command: request.clone(),
    };
    assert!(
        !setup
            .service
            .handle_preset_recording(peer, setup.ports.as_ref())
            .unwrap()
            .replayed
    );
    let mut changed_peer_request = request;
    changed_peer_request.name = "Changed peer reuse".into();
    let changed_peer = ActionEnvelope {
        context: ActionContext::operator(
            peer_desk,
            setup.user_id.0,
            peer_session.0,
            ActionSource::Osc,
        )
        .with_request_id("shared-id"),
        command: changed_peer_request,
    };
    assert_eq!(
        setup
            .service
            .handle_preset_recording(changed_peer, setup.ports.as_ref())
            .unwrap_err()
            .kind,
        ActionErrorKind::Conflict
    );

    let other_user = UserId::new();
    let other_session = SessionId::new();
    let other_desk = Uuid::new_v4();
    setup.registry.start(other_session, other_user);
    setup.registry.set(
        other_session,
        FixtureId::new(),
        AttributeKey("pan".into()),
        AttributeValue::Normalized(0.4),
    );
    let other = ActionEnvelope {
        context: ActionContext::operator(
            other_desk,
            other_user.0,
            other_session.0,
            ActionSource::Http,
        )
        .with_request_id("shared-id"),
        command: setup.request(
            PresetAddress::new(PresetFamily::Position, 9).unwrap(),
            PresetStoreMode::Overwrite,
            ProgrammingPresetRevisionExpectation::Current,
        ),
    };
    assert!(
        !setup
            .service
            .handle_preset_recording(other, setup.ports.as_ref())
            .unwrap()
            .replayed
    );
    assert_eq!(setup.ports.calls.load(Ordering::Relaxed), 3);
}

struct BlockingPorts {
    inner: PresetPorts,
    entered: std::sync::mpsc::Sender<()>,
    release: Mutex<std::sync::mpsc::Receiver<()>>,
    blocked: AtomicBool,
}

impl ProgrammingPresetRecordingPorts for BlockingPorts {
    fn authorize_preset_recording(&self, context: &ActionContext) -> Result<(), ActionError> {
        self.inner.authorize_preset_recording(context)
    }

    fn commit_preset(
        &self,
        context: &ActionContext,
        commit: &ProgrammingPresetCommit,
    ) -> Result<ProgrammingPresetCommitResult, ActionError> {
        self.entered.send(()).unwrap();
        if !self.blocked.swap(true, Ordering::Relaxed) {
            self.release.lock().recv().unwrap();
        }
        self.inner.commit_preset(context, commit)
    }
}

#[test]
fn same_user_two_desk_recordings_share_one_serial_order() {
    let setup = PresetSetup::new();
    setup.registry.set(
        setup.session_id,
        FixtureId::new(),
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.5),
    );
    let peer_session = SessionId::new();
    let peer_desk = Uuid::new_v4();
    setup.registry.start(peer_session, setup.user_id);
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let ports = Arc::new(BlockingPorts {
        inner: PresetPorts::new(setup.show_id),
        entered: entered_tx,
        release: Mutex::new(release_rx),
        blocked: AtomicBool::new(false),
    });
    let first = recording_thread(
        setup.service.clone(),
        setup.context.clone(),
        setup.request(
            PresetAddress::new(PresetFamily::Intensity, 10).unwrap(),
            PresetStoreMode::Overwrite,
            ProgrammingPresetRevisionExpectation::Current,
        ),
        "first",
        Arc::clone(&ports),
    );
    entered_rx.recv().unwrap();
    let peer_context = ActionContext::operator(
        peer_desk,
        setup.user_id.0,
        peer_session.0,
        ActionSource::Osc,
    );
    let second = recording_thread(
        setup.service.clone(),
        peer_context,
        setup.request(
            PresetAddress::new(PresetFamily::Intensity, 11).unwrap(),
            PresetStoreMode::Overwrite,
            ProgrammingPresetRevisionExpectation::Current,
        ),
        "second",
        Arc::clone(&ports),
    );
    assert!(entered_rx.recv_timeout(Duration::from_millis(75)).is_err());
    release_tx.send(()).unwrap();
    first.join().unwrap().unwrap();
    entered_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    second.join().unwrap().unwrap();
    assert_eq!(ports.inner.calls.load(Ordering::Relaxed), 2);
}

fn recording_thread(
    service: ProgrammingService,
    context: ActionContext,
    request: ProgrammingPresetRecordRequest,
    request_id: &'static str,
    ports: Arc<BlockingPorts>,
) -> std::thread::JoinHandle<Result<super::ProgrammingPresetRecordResult, ActionError>> {
    std::thread::spawn(move || {
        service.handle_preset_recording(
            ActionEnvelope {
                context: context.with_request_id(request_id),
                command: request,
            },
            ports.as_ref(),
        )
    })
}

#[test]
fn core_owned_store_modes_cover_fixture_and_group_semantics() {
    let fixture = FixtureId::new();
    let added = FixtureId::new();
    let existing = preset_with_values(fixture, 0.2, "old");
    let incoming = incoming_preset(fixture, added);
    let request = ProgrammingPresetRecordRequest {
        show_id: ShowId::new(),
        address: PresetAddress::new(PresetFamily::Mixed, 1).unwrap(),
        name: "Incoming".into(),
        mode: PresetStoreMode::Merge,
        expected_object_revision: ProgrammingPresetRevisionExpectation::Current,
        expected_show_revision: None,
    };

    let merged = ProgrammingPresetCommit::new(&request, incoming.clone())
        .merged_with(Some(&existing))
        .unwrap();
    assert_eq!(
        merged.values[&fixture][&AttributeKey::intensity()],
        AttributeValue::Normalized(0.8)
    );
    assert!(merged.values[&fixture].contains_key(&AttributeKey("pan".into())));
    assert!(merged.values.contains_key(&added));
    assert_eq!(merged.group_values["front"].len(), 2);

    let mut overwrite_request = request.clone();
    overwrite_request.mode = PresetStoreMode::Overwrite;
    let overwritten = ProgrammingPresetCommit::new(&overwrite_request, incoming.clone())
        .merged_with(Some(&existing))
        .unwrap();
    assert_eq!(overwritten.values[&fixture].len(), 1);
    assert_eq!(overwritten.group_values["front"].len(), 1);

    let mut missing_request = request;
    missing_request.mode = PresetStoreMode::AddMissingFixtures;
    let missing = ProgrammingPresetCommit::new(&missing_request, incoming)
        .merged_with(Some(&existing))
        .unwrap();
    assert_eq!(missing.values[&fixture], existing.values[&fixture]);
    assert!(missing.values.contains_key(&added));
    assert_eq!(
        missing.group_values["front"],
        existing.group_values["front"]
    );
}

fn preset_with_values(fixture: FixtureId, intensity: f32, group_value: &str) -> Preset {
    Preset {
        name: "Existing".into(),
        family: PresetFamily::Mixed,
        number: 1,
        values: HashMap::from([(
            fixture,
            HashMap::from([
                (
                    AttributeKey::intensity(),
                    AttributeValue::Normalized(intensity),
                ),
                (AttributeKey("pan".into()), AttributeValue::Normalized(0.4)),
            ]),
        )]),
        group_values: HashMap::from([(
            "front".into(),
            HashMap::from([(
                AttributeKey("custom.old".into()),
                AttributeValue::Discrete(group_value.into()),
            )]),
        )]),
    }
}

fn incoming_preset(fixture: FixtureId, added: FixtureId) -> Preset {
    let mut preset = preset_with_values(fixture, 0.8, "new");
    preset.name = "Incoming".into();
    preset
        .values
        .get_mut(&fixture)
        .unwrap()
        .remove(&AttributeKey("pan".into()));
    preset.values.insert(
        added,
        HashMap::from([(
            AttributeKey("gobo.1".into()),
            AttributeValue::Discrete("dots".into()),
        )]),
    );
    preset.group_values = HashMap::from([(
        "front".into(),
        HashMap::from([(
            AttributeKey("custom.new".into()),
            AttributeValue::Discrete("new".into()),
        )]),
    )]);
    preset
}
