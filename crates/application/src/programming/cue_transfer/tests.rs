use super::*;
use crate::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ActionSource, ActiveShowPorts,
    ActiveShowService, ActiveShowUnitOfWork, ApplicationEvent, BackupIdentity, EventBus,
    EventFilter, EventReplay, ProgrammingService, ShowEvent,
};
use light_core::{AttributeKey, AttributeValue, CueListId, FixtureId, SessionId, ShowId, UserId};
use light_engine::EngineSnapshot;
use light_playback::{
    Cue, CueChange, CueList, CueListMode, FlashReleaseMode, GroupCueChange, IntensityPriorityMode,
    PlaybackButtonAction, PlaybackDefinition, PlaybackFaderMode, PlaybackTarget, RestartMode,
    WrapMode,
};
use light_programmer::{HighlightRegistry, ProgrammerRegistry};
use light_show::{
    PortableShowCommit, PortableShowDocument, PortableShowObjectUndo, PortableShowTransaction,
    ShowStore,
};
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

#[test]
fn same_list_plain_copy_preserves_source_identity_and_lossless_fields() {
    let rig = TestRig::standard();
    let before = rig.object_revision(rig.source_id);

    let outcome = rig.transfer(
        CueTransferOperation::Copy,
        ProgrammingCueTransferMode::Plain,
        1,
        2.0,
        1,
        4.0,
    );

    assert_eq!(outcome.projections.len(), 1);
    assert_eq!(outcome.projections[0].object_revision, before + 1);
    assert_ne!(outcome.summary.destination_cue_id, rig.source_cue_id);
    let body = outcome.projections[0].raw_body.as_ref();
    assert_eq!(body["future_list"]["owner"], "source");
    assert_eq!(
        future_cue_owner(body, outcome.summary.destination_cue_id),
        "source-cue"
    );
    let list = decoded_list(body);
    let source = cue(&list, rig.source_cue_id);
    let copied = cue(&list, outcome.summary.destination_cue_id);
    assert_eq!(source.number, 2.0);
    assert_eq!(copied.number, 4.0);
    assert_eq!(copied.changes, source.changes);
    assert_eq!(copied.group_changes, source.group_changes);
    rig.assert_one_show_event(1);
    rig.assert_committed_once();
}

#[test]
fn same_list_status_move_preserves_id_and_materializes_tracked_state() {
    let rig = TestRig::standard();

    let outcome = rig.transfer(
        CueTransferOperation::Move,
        ProgrammingCueTransferMode::Status,
        1,
        2.0,
        1,
        4.0,
    );

    assert_eq!(outcome.projections.len(), 1);
    assert_eq!(outcome.summary.destination_cue_id, rig.source_cue_id);
    let body = outcome.projections[0].raw_body.as_ref();
    assert_eq!(future_cue_owner(body, rig.source_cue_id), "source-cue");
    let list = decoded_list(body);
    assert!(!list.cues.iter().any(|cue| cue.number == 2.0));
    let moved = cue(&list, rig.source_cue_id);
    assert_eq!(moved.number, 4.0);
    assert_eq!(moved.name, "Transfer me");
    assert_eq!(moved.changes.len(), 2);
    assert!(moved.changes.iter().all(|change| {
        change.value.is_some() && change.fade_millis.is_none() && change.delay_millis.is_none()
    }));
    assert_eq!(moved.group_changes.len(), 1);
    assert_eq!(moved.group_changes[0].group_id, "2");
    rig.assert_one_show_event(1);
}

#[test]
fn cross_list_status_copy_changes_only_destination_with_exact_revision() {
    let rig = TestRig::standard();
    let source_before = rig.object_body(rig.source_id);
    let source_revision = rig.object_revision(rig.source_id);
    let destination_revision = rig.object_revision(rig.destination_id);

    let outcome = rig.transfer(
        CueTransferOperation::Copy,
        ProgrammingCueTransferMode::Status,
        1,
        2.0,
        2,
        4.0,
    );

    assert_eq!(outcome.projections.len(), 1);
    let projection = &outcome.projections[0];
    assert_eq!(projection.cue_list_id, rig.destination_id);
    assert_eq!(projection.object_revision, destination_revision + 1);
    assert_eq!(
        projection.object_revision,
        rig.object_revision(rig.destination_id)
    );
    assert_eq!(rig.object_revision(rig.source_id), source_revision);
    assert_eq!(rig.object_body(rig.source_id), source_before);
    assert_eq!(projection.raw_body["future_list"]["owner"], "destination");
    assert_eq!(
        future_cue_owner(&projection.raw_body, outcome.summary.destination_cue_id),
        "source-cue"
    );
    rig.assert_one_show_event(1);
}

#[test]
fn cross_list_plain_move_commits_two_projections_in_one_event() {
    let rig = TestRig::standard();
    let source_revision = rig.object_revision(rig.source_id);
    let destination_revision = rig.object_revision(rig.destination_id);

    let outcome = rig.transfer(
        CueTransferOperation::Move,
        ProgrammingCueTransferMode::Plain,
        1,
        2.0,
        2,
        4.0,
    );

    assert_eq!(outcome.summary.destination_cue_id, rig.source_cue_id);
    assert_eq!(outcome.projections.len(), 2);
    assert_eq!(outcome.projections[0].cue_list_id, rig.source_id);
    assert_eq!(outcome.projections[0].object_revision, source_revision + 1);
    assert_eq!(outcome.projections[1].cue_list_id, rig.destination_id);
    assert_eq!(
        outcome.projections[1].object_revision,
        destination_revision + 1
    );
    let source = decoded_list(&outcome.projections[0].raw_body);
    let destination = decoded_list(&outcome.projections[1].raw_body);
    assert!(!source.cues.iter().any(|cue| cue.id == rig.source_cue_id));
    let moved = cue(&destination, rig.source_cue_id);
    assert_eq!(moved.number, 4.0);
    assert_eq!(moved.changes.len(), 1);
    assert_eq!(moved.changes[0].fixture_id, FixtureId(Uuid::from_u128(12)));
    assert_eq!(
        future_cue_owner(&outcome.projections[1].raw_body, rig.source_cue_id),
        "source-cue"
    );
    rig.assert_one_show_event(2);
    rig.assert_committed_once();
}

#[test]
fn sole_cue_cross_list_move_is_rejected_without_side_effects() {
    let rig = TestRig::with_source_cues(vec![first_cue()]);
    let before = rig.document().revision();
    rig.ports.steps.lock().clear();

    let error = rig
        .active_show
        .commit_current_programming_cue_transfer(
            &rig.context(),
            &rig.request(CueTransferOperation::Move, 1, 1.0, 2, 4.0),
            ProgrammingCueTransferMode::Plain,
            &rig.ports,
        )
        .unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Invalid);
    assert_eq!(rig.document().revision(), before);
    assert_eq!(rig.ports.steps.lock().as_slice(), ["begin"]);
    assert_eq!(rig.events.latest_sequence(), 0);
}

#[test]
fn prepared_choice_rejects_a_changed_show_at_the_exact_revision() {
    let rig = TestRig::standard();
    let request = rig.request(CueTransferOperation::Copy, 1, 2.0, 2, 4.0);
    let authority = rig
        .active_show
        .prepare_programming_cue_transfer_choice(&rig.context(), &request, &rig.ports)
        .unwrap();
    rig.rewrite_object(rig.destination_id);
    let current = rig.document().revision();
    rig.ports.steps.lock().clear();

    let error = rig
        .active_show
        .commit_programming_cue_transfer(
            &rig.context()
                .with_expected_revision(authority.show_revision.value()),
            &authority,
            ProgrammingCueTransferMode::Plain,
            &rig.ports,
        )
        .unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert_eq!(error.current_revision, Some(current.value()));
    assert_eq!(rig.ports.steps.lock().as_slice(), ["begin"]);
    assert_eq!(rig.events.latest_sequence(), 0);
}

#[test]
fn doubly_stale_request_reports_the_current_show_revision() {
    let rig = TestRig::standard();
    let authority = rig
        .active_show
        .prepare_programming_cue_transfer_choice(
            &rig.context(),
            &rig.request(CueTransferOperation::Copy, 1, 2.0, 2, 4.0),
            &rig.ports,
        )
        .unwrap();
    rig.rewrite_object(rig.destination_id);
    let current = rig.document().revision();
    rig.ports.steps.lock().clear();

    let error = rig
        .active_show
        .commit_programming_cue_transfer(
            &rig.context()
                .with_expected_revision(authority.show_revision.value().saturating_sub(1)),
            &authority,
            ProgrammingCueTransferMode::Plain,
            &rig.ports,
        )
        .unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert_eq!(error.current_revision, Some(current.value()));
    assert_eq!(rig.ports.steps.lock().as_slice(), ["begin"]);
    assert_eq!(rig.events.latest_sequence(), 0);
}

#[test]
fn identical_request_replays_without_a_second_commit_or_event() {
    let rig = TestRig::standard();
    let registry = ProgrammerRegistry::default();
    let session = SessionId::new();
    let user = UserId::new();
    let desk = Uuid::from_u128(90);
    registry.start(session, user);
    assert!(registry.attach_command_context(session, SessionId(desk)));
    let service = ProgrammingService::new(
        registry.clone(),
        rig.events.clone(),
        Arc::new(HighlightRegistry::default()),
    );
    let context = ActionContext::operator(desk, user.0, session.0, ActionSource::Http);
    let choice = service
        .prepare_cue_transfer_choice_within_interaction(
            &context,
            rig.request(CueTransferOperation::Copy, 1, 2.0, 2, 4.0),
            &rig.active_show,
            &rig.ports,
        )
        .unwrap();
    let command_line = registry
        .set_pending_command_choice(session, Some(choice.clone()))
        .unwrap();
    let envelope = ActionEnvelope {
        context: context
            .with_request_id("cue-transfer-replay")
            .with_expected_revision(choice.show_revision),
        command: ProgrammingCueTransferRequest {
            show_id: rig.show_id,
            choice_id: choice.choice_id,
            mode: ProgrammingCueTransferMode::Plain,
            expected_command_line_revision: command_line.revision,
        },
    };
    rig.ports.steps.lock().clear();

    let first = service
        .handle_cue_transfer(envelope.clone(), &rig.active_show, &rig.ports)
        .unwrap();
    let sequence = rig.events.latest_sequence();
    let replay = service
        .handle_cue_transfer(envelope, &rig.active_show, &rig.ports)
        .unwrap();

    assert!(!first.replayed);
    assert!(replay.replayed);
    assert_eq!(replay.outcome, first.outcome);
    assert_eq!(rig.events.latest_sequence(), sequence);
    assert_eq!(rig.ports.steps.lock().as_slice(), COMMIT_STEPS);
    assert_eq!(show_event_count(&rig.events), 1);
}

const COMMIT_STEPS: [&str; 6] = [
    "begin",
    "prepare",
    "backup",
    "commit",
    "install",
    "reconcile",
];

struct TestRig {
    active_show: ActiveShowService,
    events: EventBus,
    ports: TestPorts,
    show_id: ShowId,
    source_id: CueListId,
    destination_id: CueListId,
    source_cue_id: Uuid,
}

impl TestRig {
    fn standard() -> Self {
        Self::with_source_cues(vec![first_cue(), transfer_cue(), final_cue()])
    }

    fn with_source_cues(source_cues: Vec<Cue>) -> Self {
        let source_cue_id = source_cues.get(1).unwrap_or(&source_cues[0]).id;
        let source_id = CueListId::new();
        let destination_id = CueListId::new();
        let path =
            std::env::temp_dir().join(format!("light-cue-transfer-{}.sqlite", Uuid::new_v4()));
        let (store, show_id) = ShowStore::create(&path, "Cue transfer test").unwrap();
        drop(store);
        let events = EventBus::new(32);
        let rig = Self {
            active_show: ActiveShowService::new(events.clone()),
            events,
            ports: TestPorts::new(path, show_id),
            show_id,
            source_id,
            destination_id,
            source_cue_id,
        };
        rig.seed_list(cue_list(source_id, "Source", source_cues), "source");
        rig.seed_list(
            cue_list(destination_id, "Destination", vec![destination_cue()]),
            "destination",
        );
        rig.seed(
            "playback",
            "1",
            serde_json::to_value(playback(1, source_id)).unwrap(),
        );
        rig.seed(
            "playback",
            "2",
            serde_json::to_value(playback(2, destination_id)).unwrap(),
        );
        rig.ports.steps.lock().clear();
        rig
    }

    fn context(&self) -> ActionContext {
        ActionContext::operator(
            Uuid::from_u128(1),
            Uuid::from_u128(2),
            Uuid::from_u128(3),
            ActionSource::Http,
        )
        .with_request_id("cue-transfer-test")
    }

    fn request(
        &self,
        operation: CueTransferOperation,
        source_playback: u16,
        source_cue: f64,
        destination_playback: u16,
        destination_cue: f64,
    ) -> ProgrammingCueTransferChoiceRequest {
        ProgrammingCueTransferChoiceRequest {
            show_id: self.show_id,
            operation,
            source: endpoint(source_playback, source_cue),
            destination: endpoint(destination_playback, destination_cue),
            command: "transfer".into(),
            plain_command: "transfer plain".into(),
            status_command: "transfer status".into(),
        }
    }

    fn transfer(
        &self,
        operation: CueTransferOperation,
        mode: ProgrammingCueTransferMode,
        source_playback: u16,
        source_cue: f64,
        destination_playback: u16,
        destination_cue: f64,
    ) -> ProgrammingCueTransferOutcome {
        self.active_show
            .commit_current_programming_cue_transfer(
                &self.context(),
                &self.request(
                    operation,
                    source_playback,
                    source_cue,
                    destination_playback,
                    destination_cue,
                ),
                mode,
                &self.ports,
            )
            .unwrap()
    }

    fn seed_list(&self, list: CueList, owner: &str) {
        let mut body = serde_json::to_value(list).unwrap();
        body["future_list"] = json!({"owner": owner});
        let index = usize::from(owner == "source" && body["cues"].as_array().unwrap().len() > 1);
        body["cues"][index]["future_cue"] = json!({"owner": format!("{owner}-cue")});
        let object_id = body["id"].as_str().unwrap().to_owned();
        self.seed("cue_list", &object_id, body);
    }

    fn seed(&self, kind: &str, id: &str, body: Value) {
        ShowStore::open(&self.ports.path)
            .unwrap()
            .put_object(kind, id, &body, 0)
            .unwrap();
    }

    fn document(&self) -> PortableShowDocument {
        ShowStore::open(&self.ports.path)
            .unwrap()
            .portable_document()
            .unwrap()
    }

    fn object_body(&self, id: CueListId) -> Value {
        self.document()
            .object("cue_list", &id.0.to_string())
            .unwrap()
            .body()
            .clone()
    }

    fn object_revision(&self, id: CueListId) -> u64 {
        self.document()
            .object("cue_list", &id.0.to_string())
            .unwrap()
            .revision()
    }

    fn rewrite_object(&self, id: CueListId) {
        let document = self.document();
        let object = document.object("cue_list", &id.0.to_string()).unwrap();
        ShowStore::open(&self.ports.path)
            .unwrap()
            .put_object(
                "cue_list",
                &id.0.to_string(),
                object.body(),
                object.revision(),
            )
            .unwrap();
    }

    fn assert_committed_once(&self) {
        assert_eq!(self.ports.steps.lock().as_slice(), COMMIT_STEPS);
    }

    fn assert_one_show_event(&self, change_count: usize) {
        assert_eq!(show_event_count(&self.events), 1);
        let EventReplay::Events(events) = self.events.replay(0, &EventFilter::default()) else {
            panic!("expected retained transfer event");
        };
        let ApplicationEvent::Show(ShowEvent::ObjectsChanged(change)) = &events[0].payload else {
            panic!("expected Show objects event");
        };
        assert_eq!(change.changes.len(), change_count);
    }
}

impl Drop for TestRig {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{}", self.ports.path.display(), suffix));
        }
    }
}

struct TestPorts {
    path: PathBuf,
    show_id: ShowId,
    steps: Arc<Mutex<Vec<&'static str>>>,
}

impl TestPorts {
    fn new(path: PathBuf, show_id: ShowId) -> Self {
        Self {
            path,
            show_id,
            steps: Arc::default(),
        }
    }
}

struct TestUnit {
    store: ShowStore,
    document: PortableShowDocument,
    steps: Arc<Mutex<Vec<&'static str>>>,
}

impl ActiveShowUnitOfWork for TestUnit {
    fn document(&self) -> &PortableShowDocument {
        &self.document
    }

    fn backup(&mut self, _identity: &BackupIdentity) -> Result<(), ActionError> {
        self.steps.lock().push("backup");
        Ok(())
    }

    fn commit(
        &mut self,
        transaction: PortableShowTransaction,
    ) -> Result<PortableShowCommit, ActionError> {
        self.steps.lock().push("commit");
        self.store
            .apply_portable_transaction(transaction)
            .map_err(|error| ActionError::new(ActionErrorKind::Internal, error.to_string()))
    }
}

impl ActiveShowPorts for TestPorts {
    type UnitOfWork = TestUnit;
    type PreparedRuntime = EngineSnapshot;

    fn begin_active_show(
        &self,
        _context: &ActionContext,
        show_id: ShowId,
    ) -> Result<Self::UnitOfWork, ActionError> {
        self.steps.lock().push("begin");
        if show_id != self.show_id {
            return Err(ActionError::new(ActionErrorKind::NotFound, "inactive Show"));
        }
        let store = ShowStore::open(&self.path)
            .map_err(|error| ActionError::new(ActionErrorKind::Internal, error.to_string()))?;
        let document = store
            .portable_document()
            .map_err(|error| ActionError::new(ActionErrorKind::Internal, error.to_string()))?;
        Ok(TestUnit {
            store,
            document,
            steps: Arc::clone(&self.steps),
        })
    }

    fn prepare_object_undo(
        &self,
        _unit: &Self::UnitOfWork,
        _kind: &str,
        _object_id: &str,
        _expected_object_revision: u64,
    ) -> Result<PortableShowObjectUndo, ActionError> {
        unreachable!("Cue transfer does not use object Undo")
    }

    fn prepare_runtime(&self, snapshot: EngineSnapshot) -> Result<EngineSnapshot, ActionError> {
        self.steps.lock().push("prepare");
        snapshot
            .validate()
            .map_err(|error| ActionError::new(ActionErrorKind::Invalid, error.to_string()))?;
        Ok(snapshot)
    }

    fn install_runtime(&self, _context: &ActionContext, _prepared: EngineSnapshot) {
        self.steps.lock().push("install");
    }
}

impl ProgrammingCueTransferPorts for TestPorts {
    fn authorize_cue_transfer(&self, _context: &ActionContext) -> Result<(), ActionError> {
        Ok(())
    }

    fn reconcile_cue_transfer(&self, _changes: &[crate::ActiveShowObjectChange]) {
        self.steps.lock().push("reconcile");
    }
}

fn endpoint(playback_number: u16, cue_number: f64) -> ProgrammingCueTransferEndpoint {
    ProgrammingCueTransferEndpoint {
        address: ProgrammingCueTransferAddress::Pool { playback_number },
        cue_number: crate::CueNumber::new(cue_number),
    }
}

fn first_cue() -> Cue {
    let mut cue = Cue::new(1.0);
    cue.changes.push(CueChange::set(
        FixtureId(Uuid::from_u128(11)),
        AttributeKey::intensity(),
        AttributeValue::Normalized(1.0),
    ));
    cue.group_changes.push(group_set("1", 0.25));
    cue
}

fn transfer_cue() -> Cue {
    let mut cue = Cue::new(2.0);
    cue.name = "Transfer me".into();
    cue.fade_millis = 700;
    cue.delay_millis = 20;
    cue.cue_only = true;
    cue.changes.push(CueChange::set(
        FixtureId(Uuid::from_u128(12)),
        AttributeKey::intensity(),
        AttributeValue::Normalized(0.5),
    ));
    cue.group_changes.push(group_release("1"));
    cue.group_changes.push(group_set("2", 0.75));
    cue
}

fn final_cue() -> Cue {
    let mut cue = Cue::new(3.0);
    cue.changes.push(CueChange {
        fixture_id: FixtureId(Uuid::from_u128(11)),
        attribute: AttributeKey::intensity(),
        value: None,
        automatic_restore: false,
        fade_millis: None,
        delay_millis: None,
    });
    cue
}

fn destination_cue() -> Cue {
    let mut cue = Cue::new(1.0);
    cue.changes.push(CueChange::set(
        FixtureId(Uuid::from_u128(13)),
        AttributeKey::intensity(),
        AttributeValue::Normalized(1.0),
    ));
    cue
}

fn group_set(group_id: &str, value: f32) -> GroupCueChange {
    GroupCueChange {
        group_id: group_id.into(),
        attribute: AttributeKey::intensity(),
        value: Some(AttributeValue::Normalized(value)),
        automatic_restore: false,
        fade_millis: Some(500),
        delay_millis: Some(10),
    }
}

fn group_release(group_id: &str) -> GroupCueChange {
    GroupCueChange {
        value: None,
        ..group_set(group_id, 0.0)
    }
}

fn cue_list(id: CueListId, name: &str, cues: Vec<Cue>) -> CueList {
    CueList {
        id,
        name: name.into(),
        priority: 0,
        mode: CueListMode::Sequence,
        looped: false,
        chaser_step_millis: 1_000,
        speed_group: None,
        intensity_priority_mode: IntensityPriorityMode::Htp,
        wrap_mode: Some(WrapMode::Off),
        restart_mode: RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_multiplier: 1.0,
        cues,
    }
}

fn playback(number: u16, cue_list_id: CueListId) -> PlaybackDefinition {
    PlaybackDefinition {
        number,
        name: format!("Cuelist {number}"),
        target: PlaybackTarget::CueList { cue_list_id },
        buttons: [PlaybackButtonAction::None; 3],
        button_count: 3,
        fader: PlaybackFaderMode::Master,
        has_fader: true,
        go_activates: true,
        auto_off: false,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: FlashReleaseMode::ReleaseAll,
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
    }
}

fn decoded_list(body: &Value) -> CueList {
    serde_json::from_value(body.clone()).unwrap()
}

fn cue(list: &CueList, id: Uuid) -> &Cue {
    list.cues.iter().find(|cue| cue.id == id).unwrap()
}

fn future_cue_owner(body: &Value, id: Uuid) -> &str {
    body["cues"]
        .as_array()
        .unwrap()
        .iter()
        .find(|cue| cue["id"] == id.to_string())
        .unwrap()["future_cue"]["owner"]
        .as_str()
        .unwrap()
}

fn show_event_count(events: &EventBus) -> usize {
    let EventReplay::Events(events) = events.replay(0, &EventFilter::default()) else {
        panic!("expected retained events");
    };
    events
        .iter()
        .filter(|event| matches!(event.payload, ApplicationEvent::Show(_)))
        .count()
}
