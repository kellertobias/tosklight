use super::*;
use crate::{
    ActionEnvelope, ActionErrorKind, ActionSource, ActiveShowService, ActiveShowUnitOfWork,
    BackupIdentity, EventBus, EventFilter, EventReplay, ProgrammingService,
};
use light_core::{SessionId, UserId};
use light_engine::EngineSnapshot;
use light_playback::{
    Cue, CueList, CueListMode, FlashReleaseMode, IntensityPriorityMode, PlaybackButtonAction,
    PlaybackDefinition, PlaybackFaderMode, PlaybackPage, PlaybackTarget, RestartMode, WrapMode,
};
use light_programmer::{HighlightRegistry, ProgrammerRegistry};
use light_show::{
    PortableShowCommit, PortableShowDocument, PortableShowObjectUndo, PortableShowTransaction,
    ShowStore,
};
use parking_lot::Mutex;
use serde_json::json;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering},
};

#[test]
fn deletion_is_lossless_and_commits_one_runtime_install_and_show_event() {
    let rig = TestRig::new(3);
    let request = rig.exact_request(
        2.0,
        ProgrammingCueDeletionAddress::Pool { playback_number: 1 },
    );
    let deleted_id = rig.cue_id(2.0);

    let result = rig.handle("delete-lossless", request, rig.show_revision());

    assert!(!result.replayed);
    assert_eq!(result.outcome.deleted_cue.id, deleted_id);
    assert_eq!(result.outcome.cue_list.object_id, "legacy-cuelist-key");
    assert_eq!(result.outcome.cue_list.raw_body["future_list"], "preserved");
    assert_eq!(
        result.outcome.cue_list.raw_body["cues"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        result.outcome.cue_list.raw_body["cues"][1]["future_cue"],
        "cue-3"
    );
    assert_eq!(rig.ports.steps.lock().as_slice(), COMMIT_STEPS);
    assert_eq!(rig.show_event_count(), 1);
}

#[test]
fn replay_precedes_page_lock_and_show_reads_and_if_match_is_part_of_identity() {
    let rig = TestRig::new(3);
    let request = rig.exact_request(
        2.0,
        ProgrammingCueDeletionAddress::CurrentPage {
            expected_page: 1,
            slot: 1,
        },
    );
    let revision = rig.show_revision();
    let first = rig.handle("delete-replay", request.clone(), revision);
    let reads = rig.ports.page_reads.load(Ordering::Relaxed);
    let steps = rig.ports.steps.lock().clone();
    rig.ports.page.store(2, Ordering::Relaxed);
    rig.ports.locked.store(true, Ordering::Relaxed);

    let replay = rig.handle("delete-replay", request.clone(), revision);

    assert!(replay.replayed);
    assert_eq!(replay.outcome, first.outcome);
    assert_eq!(rig.ports.page_reads.load(Ordering::Relaxed), reads);
    assert_eq!(*rig.ports.steps.lock(), steps);
    assert_eq!(rig.show_event_count(), 1);
    let collision = rig
        .try_handle("delete-replay", request, revision + 1)
        .unwrap_err();
    assert_eq!(collision.kind, ActionErrorKind::Conflict);
    assert_eq!(rig.ports.page_reads.load(Ordering::Relaxed), reads);
}

#[test]
fn missing_stale_and_sole_cue_rejections_are_atomic() {
    let missing = TestRig::new(3);
    let before = missing.show_revision();
    let error = missing
        .try_handle("delete-missing", missing.current_request(99.0), before)
        .unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::NotFound);
    missing.assert_uncommitted(before);

    let stale = TestRig::new(3);
    let before = stale.show_revision();
    let mut request = stale.exact_request(
        2.0,
        ProgrammingCueDeletionAddress::Pool { playback_number: 1 },
    );
    let ProgrammingCueDeletionExpectation::Exact(authority) = &mut request.expectation else {
        unreachable!()
    };
    authority.object_revision = authority.object_revision.saturating_sub(1);
    let error = stale
        .try_handle("delete-stale", request, before)
        .unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Conflict);
    assert_eq!(error.current_related_revision, Some(1));
    stale.assert_uncommitted(before);

    let sole = TestRig::new(1);
    let before = sole.show_revision();
    let error = sole
        .try_handle("delete-sole", sole.current_request(1.0), before)
        .unwrap_err();
    assert_eq!(error.kind, ActionErrorKind::Invalid);
    sole.assert_uncommitted(before);
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
    service: ProgrammingService,
    active_show: ActiveShowService,
    events: EventBus,
    ports: TestPorts,
    context: ActionContext,
    show_id: ShowId,
    cue_list_id: CueListId,
}

impl TestRig {
    fn new(cue_count: usize) -> Self {
        let path = std::env::temp_dir().join(format!("light-cue-delete-{}.sqlite", Uuid::new_v4()));
        let (store, show_id) = ShowStore::create(&path, "Cue delete test").unwrap();
        let cue_list_id = CueListId::new();
        seed_show(&store, cue_list_id, cue_count);
        drop(store);
        let events = EventBus::new(32);
        let programmers = ProgrammerRegistry::default();
        let session = SessionId::new();
        let user = UserId::new();
        let desk = Uuid::from_u128(44);
        programmers.start(session, user);
        assert!(programmers.attach_command_context(session, SessionId(desk)));
        Self {
            service: ProgrammingService::new(
                programmers,
                events.clone(),
                Arc::new(HighlightRegistry::default()),
            ),
            active_show: ActiveShowService::new(events.clone()),
            events,
            ports: TestPorts::new(path, show_id),
            context: ActionContext::operator(desk, user.0, session.0, ActionSource::Http),
            show_id,
            cue_list_id,
        }
    }

    fn exact_request(
        &self,
        cue_number: f64,
        address: ProgrammingCueDeletionAddress,
    ) -> ProgrammingCueDeletionRequest {
        ProgrammingCueDeletionRequest {
            show_id: self.show_id,
            address,
            cue_number: crate::CueNumber::new(cue_number),
            expectation: ProgrammingCueDeletionExpectation::Exact(
                ProgrammingCueDeletionAuthority {
                    playback_number: 1,
                    cue_list_id: self.cue_list_id,
                    object_id: "legacy-cuelist-key".into(),
                    object_revision: 1,
                    cue_id: self.cue_id(cue_number),
                },
            ),
        }
    }

    fn current_request(&self, cue_number: f64) -> ProgrammingCueDeletionRequest {
        ProgrammingCueDeletionRequest {
            show_id: self.show_id,
            address: ProgrammingCueDeletionAddress::Pool { playback_number: 1 },
            cue_number: crate::CueNumber::new(cue_number),
            expectation: ProgrammingCueDeletionExpectation::Current,
        }
    }

    fn handle(
        &self,
        request_id: &str,
        request: ProgrammingCueDeletionRequest,
        revision: u64,
    ) -> ProgrammingCueDeletionResult {
        self.try_handle(request_id, request, revision).unwrap()
    }

    fn try_handle(
        &self,
        request_id: &str,
        request: ProgrammingCueDeletionRequest,
        revision: u64,
    ) -> Result<ProgrammingCueDeletionResult, crate::ActionError> {
        self.service.handle_cue_deletion(
            ActionEnvelope {
                context: self
                    .context
                    .clone()
                    .with_request_id(request_id)
                    .with_expected_revision(revision),
                command: request,
            },
            &self.active_show,
            &self.ports,
        )
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

    fn cue_id(&self, number: f64) -> Uuid {
        let document = self.document();
        let object = document.object("cue_list", "legacy-cuelist-key").unwrap();
        let list: CueList = serde_json::from_value(object.body().clone()).unwrap();
        list.cues
            .iter()
            .find(|cue| cue.number == number)
            .unwrap_or(&list.cues[0])
            .id
    }

    fn show_event_count(&self) -> usize {
        let EventReplay::Events(events) = self.events.replay(0, &EventFilter::default()) else {
            return usize::MAX;
        };
        events
            .iter()
            .filter(|event| matches!(event.payload, crate::ApplicationEvent::Show(_)))
            .count()
    }

    fn assert_uncommitted(&self, revision: u64) {
        assert_eq!(self.show_revision(), revision);
        assert_eq!(self.show_event_count(), 0);
        assert!(!self.ports.steps.lock().contains(&"commit"));
    }
}

impl Drop for TestRig {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{}", self.ports.path.display(), suffix));
        }
    }
}

fn seed_show(store: &ShowStore, cue_list_id: CueListId, cue_count: usize) {
    let cues = (1..=cue_count)
        .map(|number| Cue::new(number as f64))
        .collect::<Vec<_>>();
    let mut body = serde_json::to_value(cue_list(cue_list_id, cues)).unwrap();
    body["future_list"] = json!("preserved");
    for (index, cue) in body["cues"].as_array_mut().unwrap().iter_mut().enumerate() {
        cue["future_cue"] = json!(format!("cue-{}", index + 1));
    }
    store
        .put_object("cue_list", "legacy-cuelist-key", &body, 0)
        .unwrap();
    let playback = PlaybackDefinition {
        number: 1,
        name: "Cuelist 1".into(),
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
    };
    store
        .put_object("playback", "1", &serde_json::to_value(playback).unwrap(), 0)
        .unwrap();
    let page = PlaybackPage {
        number: 1,
        name: "Main".into(),
        slots: [(1, 1)].into_iter().collect(),
    };
    store
        .put_object(
            "playback_page",
            "1",
            &serde_json::to_value(page).unwrap(),
            0,
        )
        .unwrap();
}

fn cue_list(id: CueListId, cues: Vec<Cue>) -> CueList {
    CueList {
        id,
        name: "Lossless".into(),
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

struct TestPorts {
    path: PathBuf,
    show_id: ShowId,
    steps: Arc<Mutex<Vec<&'static str>>>,
    page: AtomicU8,
    page_reads: AtomicUsize,
    locked: AtomicBool,
}

impl TestPorts {
    fn new(path: PathBuf, show_id: ShowId) -> Self {
        Self {
            path,
            show_id,
            steps: Arc::default(),
            page: AtomicU8::new(1),
            page_reads: AtomicUsize::new(0),
            locked: AtomicBool::new(false),
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

    fn backup(&mut self, _identity: &BackupIdentity) -> Result<(), crate::ActionError> {
        self.steps.lock().push("backup");
        Ok(())
    }

    fn commit(
        &mut self,
        transaction: PortableShowTransaction,
    ) -> Result<PortableShowCommit, crate::ActionError> {
        self.steps.lock().push("commit");
        self.store
            .apply_portable_transaction(transaction)
            .map_err(|error| crate::ActionError::new(ActionErrorKind::Internal, error.to_string()))
    }
}

impl crate::ActiveShowPorts for TestPorts {
    type UnitOfWork = TestUnit;
    type PreparedRuntime = EngineSnapshot;

    fn authorize_mutation(&self, _context: &ActionContext) -> Result<(), crate::ActionError> {
        if self.locked.load(Ordering::Relaxed) {
            Err(crate::ActionError::new(ActionErrorKind::Conflict, "locked"))
        } else {
            Ok(())
        }
    }

    fn begin_active_show(
        &self,
        _context: &ActionContext,
        show_id: ShowId,
    ) -> Result<Self::UnitOfWork, crate::ActionError> {
        self.steps.lock().push("begin");
        if show_id != self.show_id {
            return Err(crate::ActionError::new(
                ActionErrorKind::NotFound,
                "inactive Show",
            ));
        }
        let store = ShowStore::open(&self.path).unwrap();
        let document = store.portable_document().unwrap();
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
        _expected_object_revision: Revision,
    ) -> Result<PortableShowObjectUndo, crate::ActionError> {
        unreachable!("Cue deletion does not use object Undo")
    }

    fn prepare_runtime(
        &self,
        snapshot: EngineSnapshot,
    ) -> Result<EngineSnapshot, crate::ActionError> {
        self.steps.lock().push("prepare");
        snapshot.validate().map_err(|error| {
            crate::ActionError::new(ActionErrorKind::Invalid, error.to_string())
        })?;
        Ok(snapshot)
    }

    fn install_runtime(&self, _context: &ActionContext, _prepared: EngineSnapshot) {
        self.steps.lock().push("install");
    }
}

impl ProgrammingCueDeletionPorts for TestPorts {
    fn authorize_cue_deletion_identity(
        &self,
        _context: &ActionContext,
    ) -> Result<(), crate::ActionError> {
        Ok(())
    }

    fn current_cue_deletion_page(
        &self,
        _context: &ActionContext,
        show_id: ShowId,
    ) -> Result<u8, crate::ActionError> {
        assert_eq!(show_id, self.show_id);
        self.page_reads.fetch_add(1, Ordering::Relaxed);
        Ok(self.page.load(Ordering::Relaxed))
    }

    fn reconcile_cue_deletion(&self, _changes: &[crate::ActiveShowObjectChange]) {
        self.steps.lock().push("reconcile");
    }
}
