use super::*;
use crate::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ActionSource,
    ActiveShowObjectChange, ActiveShowObjectKind, ActiveShowPorts, ActiveShowUnitOfWork,
    ApplicationEvent, BackupIdentity, EventBus, EventFilter, EventReplay, ShowEvent,
};
use light_core::{CueListId, ShowId};
use light_engine::EngineSnapshot;
use light_playback::{
    Cue, CueList, CueListMode, FlashReleaseMode, IntensityPriorityMode, PlaybackDefinition,
    PlaybackTarget, RestartMode, WrapMode,
};
use light_show::{
    PortableShowCommit, PortableShowDocument, PortableShowObjectUndo, PortableShowTransaction,
    ShowStore,
};
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

#[test]
fn save_cue_list_is_lossless_and_semantic_repetition_is_no_change() {
    let rig = TestRig::new();
    let cue_list_id = CueListId::new();
    let mut raw = serde_json::to_value(cue_list(cue_list_id, "Original")).unwrap();
    raw["future_list"] = json!({"owner":"newer-desk"});
    raw["cues"][0]["future_cue"] = json!({"shape":7});
    rig.seed("cue_list", &cue_list_id.0.to_string(), &raw);
    let mut changed: CueList = serde_json::from_value(raw.clone()).unwrap();
    changed.name = "Act One".into();
    let mut request = raw;
    request["name"] = json!(changed.name);

    let result = rig
        .handle(
            "save-1",
            rig.show_revision(),
            PlaybackTopologyAction::SaveCueList {
                cue_list_id,
                expected_revision: 1,
                cue_list: changed.clone(),
                raw_body: Arc::new(request),
            },
        )
        .unwrap();

    assert!(matches!(
        result.outcome,
        PlaybackTopologyOutcome::Changed { .. }
    ));
    let projection = result.outcome.objects()[0].raw_body().unwrap();
    assert_eq!(projection["future_list"]["owner"], "newer-desk");
    assert_eq!(projection["cues"][0]["future_cue"]["shape"], 7);
    assert_eq!(rig.steps(), mutation_steps());
    assert_one_event(&rig, 1);

    rig.clear_steps();
    let result = rig
        .handle(
            "save-2",
            rig.show_revision(),
            PlaybackTopologyAction::SaveCueList {
                cue_list_id,
                expected_revision: 2,
                cue_list: changed,
                raw_body: projection.clone(),
            },
        )
        .unwrap();
    assert!(matches!(
        result.outcome,
        PlaybackTopologyOutcome::NoChange { .. }
    ));
    assert_eq!(rig.steps(), ["authorize", "begin"]);
    assert_one_event(&rig, 1);
}

#[test]
fn configure_empty_slot_allocates_once_and_commits_playback_and_page_together() {
    let rig = TestRig::new();
    rig.seed(
        "playback_page",
        "legacy-page-one",
        &json!({"number":1,"name":"Main","slots":{},"future":{"columns":12}}),
    );

    let result = rig
        .handle(
            "configure-1",
            rig.show_revision(),
            PlaybackTopologyAction::ConfigureSlot {
                page: 1,
                slot: 4,
                expected_page_revision: 1,
                expected_playback_revision: 0,
                playback: playback(999, "House"),
            },
        )
        .unwrap();

    assert_eq!(
        result.outcome.resolution(),
        PlaybackTopologyResolution::PageSlot {
            page: 1,
            slot: 4,
            playback_number: Some(1),
        }
    );
    assert_eq!(result.outcome.objects().len(), 2);
    assert_eq!(
        result.outcome.objects()[0].kind(),
        ActiveShowObjectKind::Playback
    );
    assert_eq!(result.outcome.objects()[0].raw_body().unwrap()["number"], 1);
    assert_eq!(result.outcome.objects()[1].object_id(), "legacy-page-one");
    assert_eq!(
        result.outcome.objects()[1].raw_body().unwrap()["future"]["columns"],
        12
    );
    assert_eq!(rig.steps(), mutation_steps());
    assert_one_event(&rig, 2);
}

#[test]
fn legacy_semantic_configure_is_no_change_without_normalizing_raw_json() {
    let rig = TestRig::new();
    let legacy = json!({
        "number":7,
        "name":"Legacy",
        "target":{"type":"grand_master"},
        "future":{"keep":true}
    });
    rig.seed("playback", "legacy-seven", &legacy);
    rig.seed(
        "playback_page",
        "1",
        &json!({"number":1,"name":"Main","slots":{"2":7}}),
    );
    let decoded: PlaybackDefinition = serde_json::from_value(legacy.clone()).unwrap();

    let result = rig
        .handle(
            "legacy-noop",
            rig.show_revision(),
            PlaybackTopologyAction::ConfigureSlot {
                page: 1,
                slot: 2,
                expected_page_revision: 1,
                expected_playback_revision: 1,
                playback: decoded,
            },
        )
        .unwrap();

    assert!(matches!(
        result.outcome,
        PlaybackTopologyOutcome::NoChange { .. }
    ));
    let playback = result
        .outcome
        .objects()
        .iter()
        .find(|projection| projection.kind() == ActiveShowObjectKind::Playback)
        .unwrap()
        .raw_body()
        .unwrap();
    assert_eq!(playback.as_ref(), &legacy);
    assert!(playback.get("buttons").is_none());
    assert_eq!(rig.steps(), ["authorize", "begin"]);
    assert_one_event(&rig, 0);
}

#[test]
fn changed_playback_preserves_nested_extensions_and_returns_unchanged_page_authority() {
    let rig = TestRig::new();
    let mut raw = serde_json::to_value(playback(7, "Before")).unwrap();
    raw["future_playback"] = json!({"keep":true});
    raw["target"]["future_target"] = json!([1, 2, 3]);
    rig.seed("playback", "legacy-seven", &raw);
    rig.seed(
        "playback_page",
        "page-one",
        &json!({"number":1,"name":"Main","slots":{"2":7},"future_page":true}),
    );
    let mut changed: PlaybackDefinition = serde_json::from_value(raw).unwrap();
    changed.name = "After".into();

    let result = rig
        .handle(
            "change-playback",
            rig.show_revision(),
            PlaybackTopologyAction::ConfigureSlot {
                page: 1,
                slot: 2,
                expected_page_revision: 1,
                expected_playback_revision: 1,
                playback: changed,
            },
        )
        .unwrap();

    assert_eq!(result.outcome.objects().len(), 2);
    let playback = result.outcome.objects()[0].raw_body().unwrap();
    assert_eq!(playback["name"], "After");
    assert_eq!(playback["future_playback"]["keep"], true);
    assert_eq!(playback["target"]["future_target"], json!([1, 2, 3]));
    assert_eq!(result.outcome.objects()[1].object_id(), "page-one");
    assert_eq!(
        result.outcome.objects()[1].raw_body().unwrap()["future_page"],
        true
    );
    assert_one_event(&rig, 1);
}

#[test]
fn clear_removes_one_playback_from_every_page_in_one_event() {
    let rig = TestRig::new();
    let cue_list_id = CueListId::new();
    rig.seed(
        "cue_list",
        &cue_list_id.0.to_string(),
        &serde_json::to_value(cue_list(cue_list_id, "Keep source")).unwrap(),
    );
    let mut removed = playback(9, "Clear me");
    removed.target = PlaybackTarget::CueList { cue_list_id };
    removed.buttons = PlaybackDefinition::default_buttons(&removed.target);
    removed.fader = PlaybackDefinition::default_fader(&removed.target);
    rig.seed(
        "playback",
        "legacy-nine",
        &serde_json::to_value(removed).unwrap(),
    );
    rig.seed(
        "playback",
        "3",
        &serde_json::to_value(playback(3, "Keep me")).unwrap(),
    );
    rig.seed(
        "playback_page",
        "page-a",
        &json!({"number":1,"name":"One","slots":{"1":9,"2":3},"future":"a"}),
    );
    rig.seed(
        "playback_page",
        "page-b",
        &json!({"number":2,"name":"Two","slots":{"7":9},"future":"b"}),
    );
    let before = rig.show_revision();

    let result = rig
        .handle(
            "clear-1",
            before,
            PlaybackTopologyAction::ClearMappedPlayback {
                page: 1,
                slot: 1,
                expected_page_revision: 1,
                expected_playback_revision: 1,
            },
        )
        .unwrap();

    assert_eq!(result.outcome.show_revision().value(), before + 1);
    assert_eq!(result.outcome.objects().len(), 3);
    assert!(result.outcome.objects().iter().any(|projection| matches!(
        projection,
        PlaybackTopologyObjectProjection::Deleted {
            kind: ActiveShowObjectKind::Playback,
            object_id,
            object_revision: 2,
        } if object_id == "legacy-nine"
    )));
    let document = rig.document();
    assert!(document.object("playback", "legacy-nine").is_none());
    assert!(
        document
            .object("cue_list", &cue_list_id.0.to_string())
            .is_some()
    );
    for page in document.objects_of_kind("playback_page") {
        let body: light_playback::PlaybackPage =
            serde_json::from_value(page.body().clone()).unwrap();
        assert!(!body.slots.values().any(|number| *number == 9));
        assert!(page.body().get("future").is_some());
    }
    assert_eq!(rig.steps(), mutation_steps());
    assert_one_event(&rig, 3);
}

#[test]
fn empty_clear_is_no_change_and_does_not_prepare_or_emit() {
    let rig = TestRig::new();
    rig.seed(
        "playback_page",
        "1",
        &json!({"number":1,"name":"Main","slots":{}}),
    );

    let result = rig
        .handle(
            "clear-empty",
            rig.show_revision(),
            PlaybackTopologyAction::ClearMappedPlayback {
                page: 1,
                slot: 2,
                expected_page_revision: 1,
                expected_playback_revision: 0,
            },
        )
        .unwrap();

    assert!(matches!(
        result.outcome,
        PlaybackTopologyOutcome::NoChange { .. }
    ));
    assert_eq!(result.outcome.event_sequence(), None);
    assert_eq!(rig.steps(), ["authorize", "begin"]);
    assert_one_event(&rig, 0);
}

#[test]
fn exact_replay_returns_original_authority_without_a_second_transaction_or_event() {
    let rig = TestRig::new();
    rig.seed(
        "playback_page",
        "1",
        &json!({"number":1,"name":"Main","slots":{}}),
    );
    let action = PlaybackTopologyAction::ConfigureSlot {
        page: 1,
        slot: 1,
        expected_page_revision: 1,
        expected_playback_revision: 0,
        playback: playback(0, "Replay"),
    };
    let expected = rig.show_revision();
    let first = rig.handle("replay", expected, action.clone()).unwrap();
    rig.clear_steps();

    let replay = rig.handle("replay", expected, action).unwrap();

    assert!(replay.replayed);
    assert_eq!(replay.outcome, first.outcome);
    assert_eq!(replay.correlation_id, first.correlation_id);
    assert_eq!(rig.steps(), ["authorize"]);
    assert_one_event(&rig, 2);
}

#[test]
fn show_object_and_request_conflicts_stop_before_side_effects() {
    let rig = TestRig::new();
    rig.seed(
        "playback_page",
        "1",
        &json!({"number":1,"name":"Main","slots":{}}),
    );
    let action = PlaybackTopologyAction::ConfigureSlot {
        page: 1,
        slot: 1,
        expected_page_revision: 0,
        expected_playback_revision: 0,
        playback: playback(0, "Conflict"),
    };
    let error = rig
        .handle("object-conflict", rig.show_revision(), action)
        .unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert_eq!(error.current_revision, Some(rig.show_revision()));
    assert_eq!(error.current_related_revision, Some(1));
    assert_eq!(rig.steps(), ["authorize", "begin"]);

    rig.clear_steps();
    let stale = rig
        .handle(
            "show-conflict",
            0,
            PlaybackTopologyAction::ClearMappedPlayback {
                page: 1,
                slot: 1,
                expected_page_revision: 1,
                expected_playback_revision: 0,
            },
        )
        .unwrap_err();
    assert_eq!(stale.kind, ActionErrorKind::Conflict);
    assert_eq!(stale.current_revision, Some(rig.show_revision()));
    assert_eq!(rig.steps(), ["authorize", "begin"]);

    rig.clear_steps();
    let current = rig.show_revision();
    let noop = PlaybackTopologyAction::ClearMappedPlayback {
        page: 1,
        slot: 1,
        expected_page_revision: 1,
        expected_playback_revision: 0,
    };
    rig.handle("collision", current, noop.clone()).unwrap();
    rig.clear_steps();
    let collision = rig
        .handle(
            "collision",
            current,
            PlaybackTopologyAction::ClearMappedPlayback {
                page: 1,
                slot: 2,
                expected_page_revision: 1,
                expected_playback_revision: 0,
            },
        )
        .unwrap_err();
    assert_eq!(collision.kind, ActionErrorKind::Conflict);
    assert_eq!(rig.steps(), ["authorize"]);
}

#[test]
fn replay_identity_isolated_by_user_desk_and_session() {
    let rig = TestRig::new();
    rig.seed(
        "playback_page",
        "1",
        &json!({"number":1,"name":"Main","slots":{}}),
    );
    let action = PlaybackTopologyAction::ClearMappedPlayback {
        page: 1,
        slot: 1,
        expected_page_revision: 1,
        expected_playback_revision: 0,
    };
    let show_revision = rig.show_revision();
    for (user, desk, session) in [(2, 1, 3), (20, 1, 3), (2, 10, 3), (2, 1, 30)] {
        let result = rig
            .handle_as(
                "shared-id",
                show_revision,
                action.clone(),
                user,
                desk,
                session,
            )
            .unwrap();
        assert!(!result.replayed);
    }
    assert_eq!(
        rig.steps(),
        [
            "authorize",
            "begin",
            "authorize",
            "begin",
            "authorize",
            "begin",
            "authorize",
            "begin"
        ]
    );
}

#[test]
fn request_actor_session_and_exact_show_revision_are_required_before_opening_show() {
    let rig = TestRig::new();
    let action = PlaybackTopologyAction::ClearMappedPlayback {
        page: 1,
        slot: 1,
        expected_page_revision: 0,
        expected_playback_revision: 0,
    };
    let operator = ActionContext::operator(
        Uuid::from_u128(1),
        Uuid::from_u128(2),
        Uuid::from_u128(3),
        ActionSource::Http,
    );
    let missing_request = rig
        .handle_context(operator.clone(), action.clone())
        .unwrap_err();
    assert_eq!(missing_request.kind, ActionErrorKind::Invalid);
    rig.clear_steps();
    let missing_revision = rig
        .handle_context(operator.with_request_id("missing-revision"), action.clone())
        .unwrap_err();
    assert_eq!(missing_revision.kind, ActionErrorKind::Invalid);
    rig.clear_steps();
    let missing_actor = rig
        .handle_context(
            ActionContext::system(Uuid::from_u128(1), ActionSource::System)
                .with_request_id("system")
                .with_expected_revision(0),
            action,
        )
        .unwrap_err();
    assert_eq!(missing_actor.kind, ActionErrorKind::Unauthorized);
    assert_eq!(rig.steps(), ["authorize"]);
}

fn mutation_steps() -> [&'static str; 7] {
    [
        "authorize",
        "begin",
        "prepare",
        "backup",
        "commit",
        "install",
        "reconcile",
    ]
}

fn assert_one_event(rig: &TestRig, object_changes: usize) {
    let EventReplay::Events(events) = rig.service.events().replay(0, &EventFilter::default())
    else {
        panic!("expected retained events")
    };
    if object_changes == 0 {
        assert!(events.is_empty());
        return;
    }
    assert_eq!(events.len(), 1);
    let ApplicationEvent::Show(ShowEvent::ObjectsChanged(change)) = &events[0].payload else {
        panic!("expected Show Objects event")
    };
    assert_eq!(change.changes.len(), object_changes);
}

struct TestRig {
    service: PlaybackTopologyService,
    ports: TestPorts,
    show_id: ShowId,
}

impl TestRig {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("light-playback-topology-{}.sqlite", Uuid::new_v4()));
        let (store, show_id) = ShowStore::create(&path, "Playback topology test").unwrap();
        drop(store);
        let active_show = crate::ActiveShowService::new(EventBus::new(32));
        Self {
            service: PlaybackTopologyService::new(active_show),
            ports: TestPorts {
                path,
                show_id,
                steps: Arc::default(),
            },
            show_id,
        }
    }

    fn seed(&self, kind: &str, id: &str, body: &Value) {
        ShowStore::open(&self.ports.path)
            .unwrap()
            .put_object(kind, id, body, 0)
            .unwrap();
    }

    fn document(&self) -> PortableShowDocument {
        ShowStore::open(&self.ports.path)
            .unwrap()
            .portable_document()
            .unwrap()
    }

    fn show_revision(&self) -> u64 {
        self.document().revision().value()
    }

    fn handle(
        &self,
        request_id: &str,
        show_revision: u64,
        action: PlaybackTopologyAction,
    ) -> Result<PlaybackTopologyResult, ActionError> {
        self.handle_as(request_id, show_revision, action, 2, 1, 3)
    }

    fn handle_as(
        &self,
        request_id: &str,
        show_revision: u64,
        action: PlaybackTopologyAction,
        user: u128,
        desk: u128,
        session: u128,
    ) -> Result<PlaybackTopologyResult, ActionError> {
        self.handle_context(
            ActionContext::operator(
                Uuid::from_u128(desk),
                Uuid::from_u128(user),
                Uuid::from_u128(session),
                ActionSource::Http,
            )
            .with_request_id(request_id)
            .with_expected_revision(show_revision),
            action,
        )
    }

    fn handle_context(
        &self,
        context: ActionContext,
        action: PlaybackTopologyAction,
    ) -> Result<PlaybackTopologyResult, ActionError> {
        self.service.handle(
            ActionEnvelope {
                context,
                command: PlaybackTopologyCommand {
                    show_id: self.show_id,
                    action,
                },
            },
            &self.ports,
        )
    }

    fn steps(&self) -> Vec<&'static str> {
        self.ports.steps.lock().clone()
    }

    fn clear_steps(&self) {
        self.ports.steps.lock().clear();
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
            return Err(ActionError::new(ActionErrorKind::NotFound, "inactive show"));
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
        unreachable!("Playback topology does not use object Undo")
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

impl PlaybackTopologyPorts for TestPorts {
    fn authorize_playback_topology(&self, _context: &ActionContext) -> Result<(), ActionError> {
        self.steps.lock().push("authorize");
        Ok(())
    }

    fn reconcile_playback_topology(&self, _changes: &[ActiveShowObjectChange]) {
        self.steps.lock().push("reconcile");
    }
}

fn playback(number: u16, name: &str) -> PlaybackDefinition {
    let target = PlaybackTarget::GrandMaster;
    PlaybackDefinition {
        number,
        name: name.into(),
        buttons: PlaybackDefinition::default_buttons(&target),
        fader: PlaybackDefinition::default_fader(&target),
        target,
        button_count: 3,
        has_fader: true,
        go_activates: true,
        auto_off: true,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: FlashReleaseMode::ReleaseAll,
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
    }
}

fn cue_list(id: CueListId, name: &str) -> CueList {
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
        cues: vec![Cue::new(1.0)],
    }
}
