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
    PortableShowCommit, PortableShowDocument, PortableShowObjectRedo, PortableShowObjectUndo,
    PortableShowTransaction, ShowStore,
};
use parking_lot::Mutex;
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

#[path = "tests/canonical_assignments.rs"]
mod canonical_assignments;
#[path = "tests/group_master_assignment.rs"]
mod group_master_assignment;
#[path = "tests/page_actions.rs"]
mod page_actions;

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
                expected_object_id: Some(cue_list_id.0.to_string()),
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
                expected_object_id: Some(cue_list_id.0.to_string()),
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
fn save_cue_list_out_timing_is_forward_compatible_and_reversible() {
    let rig = TestRig::new();
    let cue_list_id = CueListId::new();
    let mut stored = serde_json::to_value(cue_list(cue_list_id, "Original")).unwrap();
    stored["future_list"] = json!({"owner":"newer-desk"});
    stored["cues"][0]["future_cue"] = json!({"shape":7});
    stored["cues"][0]["out_fade_millis"] = json!(900);
    stored["cues"][0]["out_delay_millis"] = json!(300);
    rig.seed("cue_list", &cue_list_id.0.to_string(), &stored);

    let mut omitted = stored.clone();
    omitted["name"] = json!("Older Desk Rename");
    omitted["cues"][0]
        .as_object_mut()
        .unwrap()
        .remove("out_fade_millis");
    omitted["cues"][0]
        .as_object_mut()
        .unwrap()
        .remove("out_delay_millis");
    let omitted_typed: CueList = serde_json::from_value(omitted.clone()).unwrap();
    let saved = rig
        .handle(
            "timing-preserve",
            rig.show_revision(),
            PlaybackTopologyAction::SaveCueList {
                cue_list_id,
                expected_revision: 1,
                expected_object_id: Some(cue_list_id.0.to_string()),
                cue_list: omitted_typed,
                raw_body: Arc::new(omitted),
            },
        )
        .unwrap();
    let preserved = saved.outcome.objects()[0].raw_body().unwrap();
    assert_eq!(preserved["cues"][0]["out_fade_millis"], 900);
    assert_eq!(preserved["cues"][0]["out_delay_millis"], 300);
    assert_eq!(preserved["cues"][0]["future_cue"]["shape"], 7);

    let mut tri_state = preserved.as_ref().clone();
    tri_state["cues"][0]["out_fade_millis"] = Value::Null;
    tri_state["cues"][0]["out_delay_millis"] = json!(0);
    let tri_state_typed: CueList = serde_json::from_value(tri_state.clone()).unwrap();
    let changed = rig
        .handle(
            "timing-tristate",
            rig.show_revision(),
            PlaybackTopologyAction::SaveCueList {
                cue_list_id,
                expected_revision: 2,
                expected_object_id: Some(cue_list_id.0.to_string()),
                cue_list: tri_state_typed,
                raw_body: Arc::new(tri_state),
            },
        )
        .unwrap();
    let changed_body = changed.outcome.objects()[0].raw_body().unwrap();
    assert!(changed_body["cues"][0].get("out_fade_millis").is_none());
    assert_eq!(changed_body["cues"][0]["out_delay_millis"], 0);

    let undone = rig
        .handle(
            "timing-undo",
            rig.show_revision(),
            PlaybackTopologyAction::UndoCueList {
                cue_list_id,
                expected_revision: 3,
                expected_object_id: cue_list_id.0.to_string(),
            },
        )
        .unwrap();
    let undone_body = undone.outcome.objects()[0].raw_body().unwrap();
    assert_eq!(undone_body["cues"][0]["out_fade_millis"], 900);
    assert_eq!(undone_body["cues"][0]["out_delay_millis"], 300);

    let redone = rig
        .handle(
            "timing-redo",
            rig.show_revision(),
            PlaybackTopologyAction::RedoCueList {
                cue_list_id,
                expected_revision: 4,
                expected_object_id: cue_list_id.0.to_string(),
            },
        )
        .unwrap();
    let redone_body = redone.outcome.objects()[0].raw_body().unwrap();
    assert!(redone_body["cues"][0].get("out_fade_millis").is_none());
    assert_eq!(redone_body["cues"][0]["out_delay_millis"], 0);
    assert_eq!(redone_body["future_list"]["owner"], "newer-desk");
    let EventReplay::Events(events) = rig.service.events().replay(0, &EventFilter::default())
    else {
        panic!("expected retained events")
    };
    assert_eq!(events.len(), 4);
    assert!(events.iter().all(|event| matches!(
        &event.payload,
        ApplicationEvent::Show(ShowEvent::ObjectsChanged(change)) if change.changes.len() == 1
    )));
}

#[test]
fn exact_replay_returns_original_authority_without_a_second_transaction_or_event() {
    let rig = TestRig::new();
    rig.seed(
        "playback_page",
        "1",
        &json!({"number":1,"name":"Main","assignments":{}}),
    );
    let action = PlaybackTopologyAction::AssignPlayback {
        address: address(1, 1),
        expected_page_revision: 1,
        expected_page_object_id: Some("1".into()),

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
    assert_one_event(&rig, 1);
}

#[test]
fn show_object_and_request_conflicts_stop_before_side_effects() {
    let rig = TestRig::new();
    rig.seed(
        "playback_page",
        "1",
        &json!({"number":1,"name":"Main","assignments":{}}),
    );
    let action = PlaybackTopologyAction::AssignPlayback {
        address: address(1, 1),
        expected_page_revision: 0,
        expected_page_object_id: Some("1".into()),

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
            PlaybackTopologyAction::ClearPlayback {
                address: address(1, 1),
                expected_page_revision: 1,
                expected_page_object_id: Some("1".into()),
            },
        )
        .unwrap_err();
    assert_eq!(stale.kind, ActionErrorKind::Conflict);
    assert_eq!(stale.current_revision, Some(rig.show_revision()));
    assert_eq!(rig.steps(), ["authorize", "begin"]);

    rig.clear_steps();
    let current = rig.show_revision();
    let noop = PlaybackTopologyAction::ClearPlayback {
        address: address(1, 1),
        expected_page_revision: 1,
        expected_page_object_id: Some("1".into()),
    };
    rig.handle("collision", current, noop.clone()).unwrap();
    rig.clear_steps();
    let collision = rig
        .handle(
            "collision",
            current,
            PlaybackTopologyAction::ClearPlayback {
                address: address(1, 2),
                expected_page_revision: 1,
                expected_page_object_id: Some("1".into()),
            },
        )
        .unwrap_err();
    assert_eq!(collision.kind, ActionErrorKind::Conflict);
    assert_eq!(rig.steps(), ["authorize"]);
}

#[test]
fn replay_identity_isolated_by_desk_and_session() {
    let rig = TestRig::new();
    rig.seed(
        "playback_page",
        "1",
        &json!({"number":1,"name":"Main","assignments":{}}),
    );
    let action = PlaybackTopologyAction::ClearPlayback {
        address: address(1, 1),
        expected_page_revision: 1,
        expected_page_object_id: Some("1".into()),
    };
    let show_revision = rig.show_revision();
    for (desk, session) in [(1, 3), (10, 3), (1, 30)] {
        let result = rig
            .handle_as("shared-id", show_revision, action.clone(), desk, session)
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
            "begin"
        ]
    );
}

#[test]
fn request_actor_session_and_exact_show_revision_are_required_before_opening_show() {
    let rig = TestRig::new();
    let action = PlaybackTopologyAction::ClearPlayback {
        address: address(1, 1),
        expected_page_revision: 0,
        expected_page_object_id: None,
    };
    let operator =
        ActionContext::operator(Uuid::from_u128(1), Uuid::from_u128(3), ActionSource::Http);
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
        let scratch = PathBuf::from(std::env::var_os("LIGHT_TMP_DIR").unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../..")
                .join(".artifacts/tmp")
                .into_os_string()
        }));
        std::fs::create_dir_all(&scratch).unwrap();
        let path = scratch.join(format!("light-playback-topology-{}.sqlite", Uuid::new_v4()));
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
        self.handle_as(request_id, show_revision, action, 1, 3)
    }

    fn handle_as(
        &self,
        request_id: &str,
        show_revision: u64,
        action: PlaybackTopologyAction,
        desk: u128,
        session: u128,
    ) -> Result<PlaybackTopologyResult, ActionError> {
        self.handle_context(
            ActionContext::operator(
                Uuid::from_u128(desk),
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
        unit: &Self::UnitOfWork,
        kind: &str,
        object_id: &str,
        expected_object_revision: u64,
    ) -> Result<PortableShowObjectUndo, ActionError> {
        unit.store
            .prepare_object_undo(kind, object_id, expected_object_revision)
            .map_err(|error| ActionError::new(ActionErrorKind::Invalid, error.to_string()))
    }

    fn prepare_object_redo(
        &self,
        unit: &Self::UnitOfWork,
        kind: &str,
        object_id: &str,
        expected_object_revision: u64,
    ) -> Result<PortableShowObjectRedo, ActionError> {
        unit.store
            .prepare_object_redo(kind, object_id, expected_object_revision)
            .map_err(|error| ActionError::new(ActionErrorKind::Invalid, error.to_string()))
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

fn playback(_number: u16, name: &str) -> PlaybackDefinition {
    let target = PlaybackTarget::GrandMaster;
    PlaybackDefinition {
        name: name.into(),
        buttons: PlaybackDefinition::default_buttons(&target),
        fader: PlaybackDefinition::default_fader(&target),
        target,
        button_count: 3,
        has_fader: true,
        footprint: light_playback::PlaybackFootprint::Normal,
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

fn cue_list_playback(number: u16, cue_list_id: CueListId) -> PlaybackDefinition {
    let mut value = playback(number, "Cuelist");
    value.target = PlaybackTarget::CueList { cue_list_id };
    value.buttons = PlaybackDefinition::default_buttons(&value.target);
    value.fader = PlaybackDefinition::default_fader(&value.target);
    value
}

fn address(page: u8, position: u16) -> light_playback::PlaybackAddress {
    light_playback::PlaybackAddress::new(page, position).unwrap()
}
fn assigned(rig: &TestRig, page: u8, position: u16) -> PlaybackDefinition {
    let document = rig.document();
    let page = document
        .objects_of_kind("playback_page")
        .find(|object| object.body()["number"] == page)
        .unwrap();
    serde_json::from_value(page.body()["assignments"][position.to_string()].clone()).unwrap()
}
fn assignment(
    page: u8,
    position: u16,
    playback: PlaybackDefinition,
    revision: u64,
    id: Option<&str>,
) -> PlaybackTopologyAction {
    PlaybackTopologyAction::AssignPlayback {
        address: address(page, position),
        playback,
        expected_page_revision: revision,
        expected_page_object_id: id.map(str::to_owned),
    }
}
fn cue_list(id: CueListId, name: &str) -> CueList {
    CueList {
        pool_number: None,
        legacy_pool_aliases: Vec::new(),
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
        auto_off_at_zero: false,
        auto_off_flash_release: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_multiplier: 1.0,
        cues: vec![Cue::new(
            crate::CueNumber::try_from_legacy_f64(1.0).unwrap(),
        )],
    }
}
