use super::*;
use crate::{
    ActionSource, ActiveShowPorts, ActiveShowUnitOfWork, ApplicationEvent, BackupIdentity,
    EventFilter, EventReplay, ProgrammingCueActivationPolicy, ProgrammingCueCapturePolicy,
    ProgrammingCuePageSlot, ProgrammingCueProjections, ProgrammingCueRecordOperation,
    ProgrammingCueRecordRequest, ProgrammingCueRecordTarget, ProgrammingCueRecordTiming,
    ProgrammingCueRecordingEnvironment, ProgrammingCueResolvedTarget,
    ProgrammingCueShowRevisionExpectation, ShowEvent,
};
use light_core::{AttributeKey, AttributeValue, CueListId, FixtureId, ShowId};
use light_engine::EngineSnapshot;
use light_playback::{Cue, CueList, CueListMode, IntensityPriorityMode, RestartMode, WrapMode};
use light_programmer::{CueRecordingCapture, CueRecordingCapturedSource, CueRecordingFixtureValue};
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
fn empty_page_slot_creates_one_lossless_three_object_transaction_and_event() {
    let rig = TestRig::new();
    rig.seed(
        "playback_page",
        "1",
        json!({"number":1,"name":"Main","slots":{},"future":{"columns":10}}),
    );
    let before = rig.document().revision();
    let commit = rig.commit(
        ProgrammingCueRecordTarget::PageSlot { page: 1, slot: 3 },
        ProgrammingCueResolvedTarget::EmptyPageSlot(ProgrammingCuePageSlot { page: 1, slot: 3 }),
        ProgrammingCueRecordOperation::Overwrite,
        None,
        capture(0.5),
    );

    let result = rig
        .service
        .commit_programming_cue(&rig.context(), &commit, &rig.ports)
        .unwrap();

    assert!(result.changed);
    assert_eq!(result.show_revision.value(), before.value() + 1);
    assert_eq!(result.concrete_playback_number, Some(1));
    assert_eq!(result.projections.cue_list.object_revision, 1);
    assert_eq!(
        result
            .projections
            .playback
            .as_ref()
            .unwrap()
            .object_revision,
        1
    );
    assert_eq!(result.projections.page.as_ref().unwrap().object_revision, 2);
    assert_eq!(
        result.projections.page.as_ref().unwrap().raw_body["future"]["columns"],
        10
    );
    assert_eq!(
        rig.steps(),
        [
            "begin",
            "prepare",
            "backup",
            "commit",
            "install",
            "reconcile"
        ]
    );

    let EventReplay::Events(events) = rig.service.events().replay(0, &EventFilter::default())
    else {
        panic!("expected one Cue-recording event")
    };
    assert_eq!(events.len(), 1);
    let ApplicationEvent::Show(ShowEvent::ObjectsChanged(change)) = &events[0].payload else {
        panic!("expected one ShowObjects event")
    };
    assert_eq!(change.changes.len(), 3);
    assert_eq!(
        change
            .changes
            .iter()
            .map(|change| change.kind)
            .collect::<Vec<_>>(),
        [
            ActiveShowObjectKind::CueList,
            ActiveShowObjectKind::Playback,
            ActiveShowObjectKind::PlaybackPage,
        ]
    );
}

#[test]
fn merge_active_without_existing_topology_creates_the_first_pool_cue() {
    let rig = TestRig::new();
    let commit = rig.commit(
        ProgrammingCueRecordTarget::Pool { playback_number: 8 },
        ProgrammingCueResolvedTarget::Playback {
            playback_number: 8,
            page_slot: None,
        },
        ProgrammingCueRecordOperation::Merge,
        None,
        capture(0.5),
    );

    let result = rig
        .service
        .commit_programming_cue(&rig.context(), &commit, &rig.ports)
        .unwrap();

    assert!(result.changed);
    assert_eq!(result.recorded_cue.number.value(), 1.0);
    assert_eq!(result.concrete_playback_number, Some(8));
    assert!(result.projections.playback.is_some());
    assert!(result.projections.page.is_none());
}

#[test]
fn identical_overwrite_is_no_change_before_prepare_backup_commit_and_event() {
    let rig = TestRig::new();
    let cue_list_id = CueListId::new();
    let cue_id = Uuid::new_v4();
    let mut body = cue_list(cue_list_id, cue_id, 0.5);
    body["future_list"] = json!({"keep":true});
    body["cues"][0]["future_cue"] = json!([1, 2, 3]);
    rig.seed("cue_list", &cue_list_id.0.to_string(), body.clone());
    let commit = rig.commit(
        ProgrammingCueRecordTarget::CueList { cue_list_id },
        ProgrammingCueResolvedTarget::CueList { cue_list_id },
        ProgrammingCueRecordOperation::Overwrite,
        Some(1.0),
        capture(0.5),
    );

    let result = rig
        .service
        .commit_programming_cue(&rig.context(), &commit, &rig.ports)
        .unwrap();

    assert!(!result.changed);
    assert_eq!(result.event_sequence, None);
    assert_eq!(result.projections.cue_list.raw_body.as_ref(), &body);
    assert_eq!(rig.steps(), ["begin"]);
    assert_eq!(rig.service.events().latest_sequence(), 0);
}

#[test]
fn changed_cue_preserves_unknown_list_and_keyed_cue_fields() {
    let rig = TestRig::new();
    let cue_list_id = CueListId::new();
    let cue_id = Uuid::new_v4();
    let mut body = cue_list(cue_list_id, cue_id, 0.5);
    body["future_list"] = json!({"keep":true});
    body["cues"][0]["future_cue"] = json!({"keep":"yes"});
    rig.seed("cue_list", &cue_list_id.0.to_string(), body);
    let commit = rig.commit(
        ProgrammingCueRecordTarget::CueList { cue_list_id },
        ProgrammingCueResolvedTarget::CueList { cue_list_id },
        ProgrammingCueRecordOperation::Overwrite,
        Some(1.0),
        capture(0.8),
    );

    let result = rig
        .service
        .commit_programming_cue(&rig.context(), &commit, &rig.ports)
        .unwrap();
    let body = result.projections.cue_list.raw_body.as_ref();
    assert_eq!(body["future_list"]["keep"], true);
    assert_eq!(body["cues"][0]["future_cue"]["keep"], "yes");
    let level = body["cues"][0]["changes"][0]["value"]["value"]
        .as_f64()
        .unwrap();
    assert!((level - 0.8).abs() < 0.000_001);
    assert!(result.projections.playback.is_none());
    assert!(result.projections.page.is_none());
}

#[test]
fn merge_missing_cue_and_stale_show_stop_before_prepare_or_side_effects() {
    let rig = TestRig::new();
    let cue_list_id = CueListId::new();
    rig.seed(
        "cue_list",
        &cue_list_id.0.to_string(),
        cue_list(cue_list_id, Uuid::new_v4(), 0.5),
    );
    let merge = rig.commit(
        ProgrammingCueRecordTarget::CueList { cue_list_id },
        ProgrammingCueResolvedTarget::CueList { cue_list_id },
        ProgrammingCueRecordOperation::Merge,
        Some(2.0),
        capture(0.8),
    );
    let error = rig
        .service
        .commit_programming_cue(&rig.context(), &merge, &rig.ports)
        .unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::NotFound);
    assert_eq!(rig.steps(), ["begin"]);

    rig.ports.steps.lock().clear();
    let mut stale = merge;
    stale.expected_show_revision = ProgrammingCueShowRevisionExpectation::Exact(
        light_show::PortableShowRevision::from_value(0),
    );
    let error = rig
        .service
        .commit_programming_cue(&rig.context(), &stale, &rig.ports)
        .unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert_eq!(
        error.current_revision,
        Some(rig.document().revision().value())
    );
    assert_eq!(rig.steps(), ["begin"]);
    assert_eq!(rig.service.events().latest_sequence(), 0);
}

#[test]
fn empty_subtract_deletes_the_exact_cue_but_not_the_cuelist() {
    let rig = TestRig::new();
    let cue_list_id = CueListId::new();
    let first = Uuid::new_v4();
    let mut list: CueList = serde_json::from_value(cue_list(cue_list_id, first, 0.5)).unwrap();
    let second = Cue::new(2.0);
    let second_id = second.id;
    list.cues.push(second);
    rig.seed(
        "cue_list",
        &cue_list_id.0.to_string(),
        serde_json::to_value(list).unwrap(),
    );
    let commit = rig.commit(
        ProgrammingCueRecordTarget::CueList { cue_list_id },
        ProgrammingCueResolvedTarget::CueList { cue_list_id },
        ProgrammingCueRecordOperation::Subtract,
        Some(2.0),
        empty_capture(),
    );

    let result = rig
        .service
        .commit_programming_cue(&rig.context(), &commit, &rig.ports)
        .unwrap();
    assert!(result.recorded_cue.deleted);
    assert_eq!(result.recorded_cue.id, second_id);
    assert_eq!(
        result.projections.cue_list.raw_body["cues"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn runtime_prepare_failure_rolls_back_before_backup_commit_install_and_event() {
    let rig = TestRig::with_prepare_failure();
    let commit = rig.commit(
        ProgrammingCueRecordTarget::Pool { playback_number: 7 },
        ProgrammingCueResolvedTarget::Playback {
            playback_number: 7,
            page_slot: None,
        },
        ProgrammingCueRecordOperation::Overwrite,
        None,
        capture(0.5),
    );

    let error = rig
        .service
        .commit_programming_cue(&rig.context(), &commit, &rig.ports)
        .unwrap_err();

    assert_eq!(error.kind, ActionErrorKind::Unavailable);
    assert_eq!(rig.steps(), ["begin", "prepare"]);
    assert_eq!(rig.document().objects_of_kind("cue_list").count(), 0);
    assert_eq!(rig.document().objects_of_kind("playback").count(), 0);
    assert_eq!(rig.service.events().latest_sequence(), 0);
}

struct TestRig {
    service: ActiveShowService,
    ports: TestPorts,
    show_id: ShowId,
}

impl TestRig {
    fn new() -> Self {
        Self::with_prepare_mode(false)
    }

    fn with_prepare_failure() -> Self {
        Self::with_prepare_mode(true)
    }

    fn with_prepare_mode(fail_prepare: bool) -> Self {
        let path =
            std::env::temp_dir().join(format!("light-cue-recording-{}.sqlite", Uuid::new_v4()));
        let (store, show_id) = ShowStore::create(&path, "Cue recording test").unwrap();
        drop(store);
        Self {
            service: ActiveShowService::new(EventBus::new(16)),
            ports: TestPorts {
                path,
                show_id,
                steps: Arc::default(),
                installed: Arc::default(),
                fail_prepare,
            },
            show_id,
        }
    }

    fn context(&self) -> ActionContext {
        ActionContext::operator(
            Uuid::from_u128(1),
            Uuid::from_u128(2),
            Uuid::from_u128(3),
            ActionSource::Http,
        )
        .with_request_id("cue-test")
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

    fn steps(&self) -> Vec<&'static str> {
        self.ports.steps.lock().clone()
    }

    fn commit(
        &self,
        target: ProgrammingCueRecordTarget,
        resolved: ProgrammingCueResolvedTarget,
        operation: ProgrammingCueRecordOperation,
        cue_number: Option<f64>,
        capture: CueRecordingCapture,
    ) -> ProgrammingCueCommit {
        ProgrammingCueCommit::new(
            ProgrammingCueRecordRequest {
                show_id: self.show_id,
                target,
                operation,
                cue_number: cue_number.map(crate::CueNumber::new),
                timing: ProgrammingCueRecordTiming::default(),
                cue_only: false,
                name: None,
                capture_policy: ProgrammingCueCapturePolicy::CurrentCapture,
                activation_policy: ProgrammingCueActivationPolicy::Hold,
                expected_show_revision: ProgrammingCueShowRevisionExpectation::Current,
            },
            ProgrammingCueRecordingEnvironment {
                target: resolved,
                active_cue: None,
            },
            capture,
        )
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
    installed: Arc<Mutex<Option<EngineSnapshot>>>,
    fail_prepare: bool,
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
        unreachable!("Cue recording does not use object Undo")
    }

    fn prepare_runtime(&self, snapshot: EngineSnapshot) -> Result<EngineSnapshot, ActionError> {
        self.steps.lock().push("prepare");
        if self.fail_prepare {
            return Err(ActionError::new(
                ActionErrorKind::Unavailable,
                "runtime preparation failed",
            ));
        }
        snapshot
            .validate()
            .map_err(|error| ActionError::new(ActionErrorKind::Invalid, error.to_string()))?;
        Ok(snapshot)
    }

    fn install_runtime(&self, _context: &ActionContext, prepared: EngineSnapshot) {
        self.steps.lock().push("install");
        *self.installed.lock() = Some(prepared);
    }
}

impl ProgrammingCueActiveShowPorts for TestPorts {
    fn reconcile_programming_cue(&self, _projections: &ProgrammingCueProjections) {
        self.steps.lock().push("reconcile");
    }
}

fn capture(level: f32) -> CueRecordingCapture {
    CueRecordingCapture {
        source: CueRecordingCapturedSource::Normal,
        fixture_values: vec![CueRecordingFixtureValue {
            fixture_id: FixtureId(Uuid::from_u128(10)),
            attribute: AttributeKey::intensity(),
            value: AttributeValue::Normalized(level),
            programmer_order: 1,
            fade: false,
            fade_millis: None,
            delay_millis: None,
        }],
        group_values: Vec::new(),
    }
}

fn empty_capture() -> CueRecordingCapture {
    CueRecordingCapture {
        source: CueRecordingCapturedSource::Normal,
        fixture_values: Vec::new(),
        group_values: Vec::new(),
    }
}

fn cue_list(id: CueListId, cue_id: Uuid, level: f32) -> Value {
    let mut cue = Cue::new(1.0);
    cue.id = cue_id;
    cue.changes = vec![light_playback::CueChange::set(
        FixtureId(Uuid::from_u128(10)),
        AttributeKey::intensity(),
        AttributeValue::Normalized(level),
    )];
    serde_json::to_value(CueList {
        id,
        name: "Existing".into(),
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
        cues: vec![cue],
    })
    .unwrap()
}
