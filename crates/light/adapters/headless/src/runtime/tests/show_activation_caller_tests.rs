//! TL-589: all five production activation callers through their real v2 routes.
//!
//! Every test pauses the owned worker at a per-AppState probe, drops the HTTP request at an
//! exact admission phase, releases the worker and joins its terminal completion (the completed
//! probe plus reacquiring show-change serialization) before inspecting state or tearing down.
use super::show_activation_cancellation_tests::ArmedProbe;
use super::*;
use light_fixture::{ChannelFunction, ChannelFunctionBehavior, ChannelResolution, FixtureProfile};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Caller {
    Open,
    CleanDefault,
    Rollback,
    RevisionCopy,
    Mvr,
}

impl Caller {
    fn transition(self) -> &'static str {
        match self {
            Self::Open | Self::Rollback => "safe_blackout",
            Self::CleanDefault | Self::RevisionCopy => "timed_fade",
            // MVR open-after has no operator transition choice.
            Self::Mvr => "hold_current",
        }
    }

    fn completion_kind(self) -> &'static str {
        match self {
            Self::Rollback => "show_rolled_back",
            Self::Mvr => "mvr_imported",
            _ => "show_opened",
        }
    }
}

/// Alpha is opened first, then Live. Durable previous-active is therefore Alpha, the Rollback
/// destination is Alpha and the revision-copy source is Alpha, distinct from Live (current).
pub(super) struct CallerScenario {
    pub(super) caller: Caller,
    pub(super) state: AppState,
    pub(super) app: Router,
    pub(super) token: String,
    pub(super) data_dir: PathBuf,
    pub(super) alpha: String,
    pub(super) live: String,
    mvr_preview: Option<serde_json::Value>,
}

const REVISION_NAME: &str = "Caller baseline";

impl CallerScenario {
    pub(super) async fn new(caller: Caller) -> Self {
        let (state, data_dir) = test_state();
        let app = router(state.clone());
        let (token, _) = login(&app, "Caller activation").await;
        let alpha = create_show(&app, &token, "Caller alpha").await["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let live = create_show(&app, &token, "Caller live").await["id"]
            .as_str()
            .unwrap()
            .to_owned();
        for show in [&alpha, &live] {
            let response = app
                .clone()
                .oneshot(open_show_request(&token, show))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        }
        if caller == Caller::RevisionCopy {
            let response = app
                .clone()
                .oneshot(save_show_revision_request(&token, &alpha, REVISION_NAME))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        }
        let mvr_preview = if caller == Caller::Mvr {
            Some(caller_mvr_preview(&app, &token).await)
        } else {
            None
        };
        let scenario = Self {
            caller,
            state,
            app,
            token,
            data_dir,
            alpha,
            live,
            mvr_preview,
        };
        assert_eq!(scenario.current_id(), scenario.live);
        assert_eq!(scenario.durable_id(), scenario.live);
        assert_eq!(scenario.previous_id(), Some(scenario.alpha.clone()));
        scenario
    }

    pub(super) fn request(&self) -> Request<Body> {
        let transition = self.caller.transition();
        let millis = (transition == "timed_fade").then_some(100);
        let action = match self.caller {
            Caller::Open => {
                return open_show_with_transition_request(
                    &self.token,
                    &self.alpha,
                    transition,
                    millis,
                );
            }
            Caller::CleanDefault => serde_json::json!({
                "type": "open_default", "transition": transition, "transition_millis": millis
            }),
            Caller::Rollback => serde_json::json!({
                "type": "rollback", "transition": transition, "transition_millis": millis
            }),
            Caller::RevisionCopy => serde_json::json!({
                "type": "open_revision", "show_id": self.alpha, "revision": 1,
                "transition": transition, "transition_millis": millis
            }),
            Caller::Mvr => serde_json::json!({
                "type": "apply_mvr",
                "token": self.mvr_preview.as_ref().unwrap()["token"],
                "destination": {"type": "new_show", "name": "Caller MVR", "open_after_import": true},
                "resolutions": []
            }),
        };
        show_action_request(&self.token, action)
    }

    pub(super) fn current_id(&self) -> String {
        self.state.active_show.current().unwrap().id.0.to_string()
    }

    pub(super) fn durable_id(&self) -> String {
        let durable = self.state.installation.active_show().unwrap().unwrap();
        durable.id.0.to_string()
    }

    pub(super) fn previous_id(&self) -> Option<String> {
        self.state
            .installation
            .setting("previous_active_show_id")
            .unwrap()
    }

    pub(super) fn event_watermark(&self) -> u64 {
        let events = self.state.events.audit_events();
        events.iter().map(|event| event.revision).max().unwrap_or(0)
    }

    /// Completion events of all three activation kinds published after `watermark`.
    pub(super) fn completion_events(&self, watermark: u64) -> Vec<Event> {
        self.state
            .events
            .audit_events()
            .into_iter()
            .filter(|event| event.revision > watermark)
            .filter(|event| {
                matches!(
                    event.kind.as_str(),
                    "show_opened" | "show_rolled_back" | "mvr_imported"
                )
            })
            .collect()
    }

    /// Documented previous-ID policy: MVR open-after preserves the stored setting.
    fn expected_previous_after_commit(&self) -> String {
        match self.caller {
            Caller::Mvr => self.alpha.clone(),
            _ => self.live.clone(),
        }
    }

    /// Exact committed destination, durable metadata and single completion event.
    pub(super) fn assert_committed(&self, watermark: u64) -> String {
        let destination = self.current_id();
        match self.caller {
            Caller::Open | Caller::Rollback => assert_eq!(destination, self.alpha),
            _ => assert!(destination != self.alpha && destination != self.live),
        }
        let durable = self.state.installation.active_show().unwrap().unwrap();
        assert_eq!(durable.id.0.to_string(), destination, "{:?}", self.caller);
        assert!(durable.last_loaded_at.is_some());
        assert_eq!(
            self.previous_id(),
            Some(self.expected_previous_after_commit()),
            "{:?} previous-active policy",
            self.caller
        );
        let events = self.completion_events(watermark);
        assert_eq!(events.len(), 1, "{:?}: {:?}", self.caller, events.len());
        let event = &events[0];
        assert_eq!(event.kind, self.caller.completion_kind());
        assert_eq!(event.payload["show"]["id"], destination);
        self.assert_payload_shape(&event.payload, &durable);
        destination
    }

    fn assert_payload_shape(&self, payload: &serde_json::Value, durable: &ShowEntry) {
        let transition = self.caller.transition();
        match self.caller {
            Caller::Open => {
                assert_eq!(payload["previous_show"]["id"], self.live);
                assert_eq!(payload["transition"], transition);
                assert!(payload.get("source").is_none());
                assert!(payload.get("revision_copy").is_none());
            }
            Caller::CleanDefault => {
                assert_eq!(payload["previous_show"]["id"], self.live);
                assert_eq!(payload["source"], "built_in_default");
                assert_eq!(payload["transition"], transition);
            }
            Caller::Rollback => {
                assert!(payload.get("previous_show").is_none(), "{payload}");
                assert_eq!(payload["transition"], transition);
            }
            Caller::RevisionCopy => {
                assert!(payload.get("previous_show").is_none(), "{payload}");
                assert_eq!(payload["transition"], transition);
                let source = &payload["revision_copy"];
                assert_eq!(source["show_id"], self.alpha);
                assert_eq!(source["show_name"], "Caller alpha");
                assert_eq!(source["revision"], 1);
                assert_eq!(source["revision_name"], REVISION_NAME);
                let copy = durable.revision_copy.as_ref().expect("durable provenance");
                assert_eq!(copy.show_id.0.to_string(), self.alpha);
                assert_eq!(copy.revision, 1);
                assert_eq!(copy.revision_name, REVISION_NAME);
            }
            Caller::Mvr => {
                assert!(payload.get("previous_show").is_none(), "{payload}");
                assert!(payload.get("transition").is_none(), "{payload}");
                assert_eq!(payload["fixtures"], 1);
                assert_eq!(payload["unresolved"], 1);
                assert_eq!(payload["scenery"], 0);
            }
        }
    }

    /// Library entries and show files, to detect a created-but-never-activated destination.
    pub(super) fn library_snapshot(&self) -> (Vec<String>, Vec<std::path::PathBuf>) {
        let mut ids: Vec<_> = (self.state.installation.show_library().unwrap().into_iter())
            .map(|show| show.id.0.to_string())
            .collect();
        ids.sort();
        let mut files: Vec<_> = std::fs::read_dir(self.data_dir.join("shows"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "show")
            })
            .collect();
        files.sort();
        (ids, files)
    }

    /// A caller that creates its own destination (clean default, revision copy) must not leave
    /// that unopened copy behind when activation is cancelled or fails. MVR open-after is
    /// excluded: its import is a separate, already-committed operator result.
    pub(super) fn assert_no_orphan_destination(
        &self,
        before: &(Vec<String>, Vec<std::path::PathBuf>),
    ) {
        if self.caller == Caller::Mvr {
            return;
        }
        assert_eq!(
            &self.library_snapshot(),
            before,
            "{:?} left an orphan",
            self.caller
        );
    }

    /// Owned transition overlay while the worker is paused before its commit tail.
    pub(super) fn assert_transition_overlay(&self, base_master: f32, base_blackout: bool) {
        let options = self.state.output.render_options();
        match self.caller.transition() {
            "safe_blackout" => {
                assert!(options.blackout);
                assert_eq!(options.grand_master, base_master);
            }
            "timed_fade" => {
                assert_eq!(options.grand_master, 0.0);
                assert_eq!(options.blackout, base_blackout);
            }
            _ => {
                assert_eq!(options.grand_master, base_master);
                assert_eq!(options.blackout, base_blackout);
            }
        }
    }

    pub(super) fn teardown(self) {
        let Self {
            state,
            app,
            data_dir,
            ..
        } = self;
        drop(app);
        drop(state);
        let _ = std::fs::remove_dir_all(data_dir);
    }
}

/// Show-change serialization is held right now, without waiting.
pub(super) fn show_change_is_held(state: &AppState) -> bool {
    futures_util::FutureExt::now_or_never(state.active_show.acquire_show_change()).is_none()
}

/// Joins the owned workflow: show-change is its last dropped resource, after the overlay lease.
pub(super) async fn join_owned_workflow(state: &AppState) -> tokio::sync::OwnedMutexGuard<()> {
    tokio::time::timeout(
        Duration::from_secs(5),
        state.active_show.acquire_show_change(),
    )
    .await
    .expect("owned activation workflow did not finish its cleanup")
}

/// Joins the whole owned v2 lifecycle action, which holds the library replay gate to its end.
pub(super) async fn join_owned_action(state: &AppState) {
    let gate = tokio::time::timeout(
        Duration::from_secs(5),
        state.replay.acquire_show_library_action(),
    )
    .await
    .expect("owned show-library action did not finish");
    drop(gate);
}

/// Desk-level effects that only a committed activation may clear.
pub(super) struct DeskEffects {
    highlighted: light_core::FixtureId,
    media: light_core::FixtureId,
}

impl DeskEffects {
    pub(super) fn seed(state: &AppState) -> Self {
        let effects = Self {
            highlighted: light_core::FixtureId::new(),
            media: light_core::FixtureId::new(),
        };
        state
            .output
            .set_highlighted_fixtures(vec![effects.highlighted]);
        state.media.record_status(effects.media, None);
        state.output.record_output_health(3, 0);
        effects
    }

    pub(super) fn assert_intact(&self, state: &AppState) {
        assert_eq!(state.output.highlighted_fixtures(), vec![self.highlighted]);
        assert!(state.media.statuses().contains_key(&self.media));
        assert_eq!(state.output.health_snapshot().packets_sent, 3);
    }

    pub(super) fn assert_cleared(&self, state: &AppState) {
        assert!(state.output.highlighted_fixtures().is_empty());
        assert!(!state.media.statuses().contains_key(&self.media));
        assert_eq!(state.output.health_snapshot().packets_sent, 0);
    }
}

fn caller_mvr_profile() -> FixtureProfile {
    let (fixture, _, _) = schema_v2_direct_fixture();
    let mut profile = std::sync::Arc::unwrap_or_clone(fixture.definition.profile_snapshot.unwrap());
    profile.manufacturer = "Caller MVR".into();
    profile.name = "Caller source fixture".into();
    let mode = &mut profile.modes[0];
    mode.name = "Precise".into();
    mode.color_systems.clear();
    mode.control_actions.clear();
    mode.channels.truncate(1);
    let channel = &mut mode.channels[0];
    channel.attribute = light_core::AttributeKey("pan".into());
    channel.fixture_attribute = channel.attribute.clone();
    channel.resolution = ChannelResolution::U16;
    channel.secondary_slots = vec![3];
    channel.functions = vec![ChannelFunction::continuous(
        "Pan",
        channel.attribute.clone(),
        65535,
    )];
    channel.functions[0].behavior = ChannelFunctionBehavior::Continuous {
        physical_min: 540.0,
        physical_max: -540.0,
        unit: Some("degrees".into()),
    };
    mode.splits[0].footprint = 3;
    profile
}

fn caller_mvr_fixture(uuid: u128, spec: &str, address: u16) -> light_mvr::MvrFixture {
    light_mvr::MvrFixture {
        uuid: Uuid::from_u128(uuid),
        name: format!("Caller source {uuid}"),
        fixture_id: None,
        gdtf_spec: spec.into(),
        gdtf_mode: "Precise".into(),
        universe: Some(1),
        address: Some(address),
        matrix: [1., 0., 0., 0., 1., 0., 0., 0., 1., 0., 0., 0.],
        layer: None,
        class: None,
    }
}

/// One matched and one unresolved fixture, so the event must carry both distinct counts.
async fn caller_mvr_preview(app: &Router, token: &str) -> serde_json::Value {
    let archive = light_fixture::gdtf::profile::package_profile(&caller_mvr_profile()).unwrap();
    let document = light_mvr::MvrDocument {
        fixtures: vec![
            caller_mvr_fixture(1, "Caller.gdtf", 1),
            caller_mvr_fixture(2, "Absent.gdtf", 20),
        ],
        files: HashMap::from([("Caller.gdtf".into(), archive)]),
        ..Default::default()
    };
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v2/mvr/imports/preview")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/zip")
                .body(Body::from(light_mvr::write(&document).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = json(response).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

/// Request drop wins Pending -> Cancelled while the worker is paused before its CAS.
async fn assert_cancelled_before_admission(caller: Caller) {
    let scenario = CallerScenario::new(caller).await;
    let state = scenario.state.clone();
    let effects = DeskEffects::seed(&state);
    let control_revision = state
        .output
        .apply_runtime_control(Some(0.55), Some(false))
        .unwrap();
    let snapshot = state.output.snapshot();
    let checkpoint = state.output.dynamic_source_checkpoint().unwrap();
    let alpha_id = light_core::ShowId(Uuid::parse_str(&scenario.alpha).unwrap());
    let alpha_loaded = state
        .installation
        .show(alpha_id)
        .unwrap()
        .unwrap()
        .last_loaded_at;
    let watermark = scenario.event_watermark();
    let library = scenario.library_snapshot();
    let before = ArmedProbe::new(state.active_show.activation_before_admission_probe());
    let completed = ArmedProbe::new(state.active_show.activation_completed_probe());
    let (app, request) = (scenario.app.clone(), scenario.request());
    let opening = tokio::spawn(async move { app.oneshot(request).await.unwrap() });
    before.reached().await;
    scenario.assert_transition_overlay(0.55, false);
    opening.abort();
    assert!(opening.await.unwrap_err().is_cancelled());
    let assert_untouched = |phase: &str| {
        assert!(
            Arc::ptr_eq(&snapshot, &state.output.snapshot()),
            "{caller:?} {phase}"
        );
        assert_eq!(
            state.output.dynamic_source_checkpoint().unwrap(),
            checkpoint
        );
        assert_eq!(scenario.current_id(), scenario.live, "{caller:?} {phase}");
        assert_eq!(scenario.durable_id(), scenario.live, "{caller:?} {phase}");
        assert_eq!(scenario.previous_id(), Some(scenario.alpha.clone()));
        let alpha = state.installation.show(alpha_id).unwrap().unwrap();
        assert_eq!(alpha.last_loaded_at, alpha_loaded, "{caller:?} {phase}");
        assert!(
            scenario.completion_events(watermark).is_empty(),
            "{caller:?} {phase}"
        );
        effects.assert_intact(&state);
        assert_eq!(state.output.control_projection().revision, control_revision);
    };
    assert_untouched("paused");
    before.release();
    // The cancelled worker reaches its terminal return while still owning show-change.
    completed.reached().await;
    assert!(
        show_change_is_held(&state),
        "{caller:?} released before worker cleanup"
    );
    assert_untouched("worker returned");
    completed.release();
    let show_change = join_owned_workflow(&state).await;
    assert_untouched("joined");
    let options = state.output.render_options();
    assert_eq!((options.grand_master, options.blackout), (0.55, false));
    drop(show_change);
    // The owned v2 action (replay gate) outlives the workflow; join it before inspecting
    // caller-level cleanup and before teardown.
    join_owned_action(&state).await;
    assert_untouched("action joined");
    scenario.assert_no_orphan_destination(&library);
    drop((completed, before));
    scenario.teardown();
}

/// Worker wins Pending -> Committing; the dropped request cannot skip the owned commit tail.
async fn assert_cancelled_after_admission(caller: Caller) {
    let scenario = CallerScenario::new(caller).await;
    let state = scenario.state.clone();
    let effects = DeskEffects::seed(&state);
    state
        .output
        .apply_runtime_control(Some(0.55), Some(false))
        .unwrap();
    let snapshot = state.output.snapshot();
    let watermark = scenario.event_watermark();
    let admitted = ArmedProbe::new(state.active_show.activation_after_admission_probe());
    let completed = ArmedProbe::new(state.active_show.activation_completed_probe());
    let (app, request) = (scenario.app.clone(), scenario.request());
    let opening = tokio::spawn(async move { app.oneshot(request).await.unwrap() });
    admitted.reached().await;
    scenario.assert_transition_overlay(0.55, false);
    assert!(Arc::ptr_eq(&snapshot, &state.output.snapshot()));
    assert_eq!(scenario.current_id(), scenario.live);
    opening.abort();
    assert!(opening.await.unwrap_err().is_cancelled());
    assert!(scenario.completion_events(watermark).is_empty());
    effects.assert_intact(&state);
    admitted.release();

    completed.reached().await;
    assert!(!Arc::ptr_eq(&snapshot, &state.output.snapshot()));
    let destination = scenario.assert_committed(watermark);
    effects.assert_cleared(&state);
    // Coherent swap is published, but the owned overlay still separates it from cleanup.
    assert!(
        show_change_is_held(&state),
        "{caller:?} released before cleanup"
    );
    completed.release();
    let show_change = join_owned_workflow(&state).await;
    assert_eq!(scenario.assert_committed(watermark), destination);
    let controls = state.output.control_projection();
    let options = state.output.render_options();
    assert_eq!(
        options.grand_master, controls.grand_master,
        "{caller:?} overlay"
    );
    assert_eq!(options.blackout, controls.blackout, "{caller:?} overlay");
    drop(show_change);
    drop((completed, admitted));
    scenario.teardown();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn open_caller_cancelled_before_admission_changes_nothing() {
    assert_cancelled_before_admission(Caller::Open).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn clean_default_caller_cancelled_before_admission_changes_nothing() {
    assert_cancelled_before_admission(Caller::CleanDefault).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rollback_caller_cancelled_before_admission_changes_nothing() {
    assert_cancelled_before_admission(Caller::Rollback).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn revision_copy_caller_cancelled_before_admission_changes_nothing() {
    assert_cancelled_before_admission(Caller::RevisionCopy).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mvr_open_after_cancelled_before_admission_changes_nothing() {
    assert_cancelled_before_admission(Caller::Mvr).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn open_caller_cancelled_after_admission_completes_once() {
    assert_cancelled_after_admission(Caller::Open).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn clean_default_caller_cancelled_after_admission_completes_once() {
    assert_cancelled_after_admission(Caller::CleanDefault).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rollback_caller_cancelled_after_admission_completes_once() {
    assert_cancelled_after_admission(Caller::Rollback).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn revision_copy_caller_cancelled_after_admission_completes_once() {
    assert_cancelled_after_admission(Caller::RevisionCopy).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mvr_open_after_cancelled_after_admission_completes_once() {
    assert_cancelled_after_admission(Caller::Mvr).await;
}

/// Uncancelled completion through each caller's own response path.
async fn assert_uncancelled_completion(caller: Caller) {
    let scenario = CallerScenario::new(caller).await;
    let watermark = scenario.event_watermark();
    let response = scenario
        .app
        .clone()
        .oneshot(scenario.request())
        .await
        .unwrap();
    let status = response.status();
    let body = json(response).await;
    assert_eq!(status, StatusCode::OK, "{caller:?}: {body}");
    assert_eq!(body["replayed"], false);
    let destination = scenario.assert_committed(watermark);
    let result = &body["result"];
    if caller == Caller::Mvr {
        assert_eq!(result["type"], "mvr_apply");
        assert_eq!(result["result"]["show"]["id"], destination, "{body}");
        assert_eq!(result["result"]["opened"], true);
        assert_eq!(result["result"]["imported_fixtures"], 1);
        assert_eq!(result["result"]["unresolved_fixtures"], 1);
    } else {
        assert_eq!(result["show"]["id"], destination, "{body}");
    }
    drop(join_owned_workflow(&scenario.state).await);
    scenario.teardown();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_caller_completes_with_its_documented_previous_id_and_one_event() {
    for caller in [
        Caller::Open,
        Caller::CleanDefault,
        Caller::Rollback,
        Caller::RevisionCopy,
        Caller::Mvr,
    ] {
        assert_uncancelled_completion(caller).await;
    }
}

/// The last returned-error boundary: the metadata transaction fails after every authority check.
async fn assert_metadata_failure_keeps_live(caller: Caller, key: &str) {
    let scenario = CallerScenario::new(caller).await;
    let state = scenario.state.clone();
    let effects = DeskEffects::seed(&state);
    let control_revision = state
        .output
        .apply_runtime_control(Some(0.61), Some(true))
        .unwrap();
    let snapshot = state.output.snapshot();
    let checkpoint = state.output.dynamic_source_checkpoint().unwrap();
    let watermark = scenario.event_watermark();
    let library = scenario.library_snapshot();
    let database = rusqlite::Connection::open(scenario.data_dir.join("desk.sqlite")).unwrap();
    database
        .execute_batch(&format!(
            "CREATE TRIGGER tl589_reject_insert BEFORE INSERT ON settings WHEN NEW.key='{key}'
             BEGIN SELECT RAISE(ABORT, 'tl589 injected metadata failure'); END;
             CREATE TRIGGER tl589_reject_update BEFORE UPDATE ON settings WHEN NEW.key='{key}'
             BEGIN SELECT RAISE(ABORT, 'tl589 injected metadata failure'); END;"
        ))
        .unwrap();
    let response = tokio::time::timeout(
        Duration::from_secs(5),
        scenario.app.clone().oneshot(scenario.request()),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(!response.status().is_success(), "{caller:?}");
    let error = json(response).await;
    assert!(
        error
            .to_string()
            .contains("tl589 injected metadata failure"),
        "{caller:?}: {error}"
    );
    let show_change = join_owned_workflow(&state).await;
    assert!(
        Arc::ptr_eq(&snapshot, &state.output.snapshot()),
        "{caller:?}"
    );
    assert_eq!(
        state.output.dynamic_source_checkpoint().unwrap(),
        checkpoint
    );
    assert_eq!(scenario.current_id(), scenario.live);
    assert_eq!(scenario.durable_id(), scenario.live);
    assert_eq!(scenario.previous_id(), Some(scenario.alpha.clone()));
    assert!(
        scenario.completion_events(watermark).is_empty(),
        "{caller:?}"
    );
    effects.assert_intact(&state);
    scenario.assert_no_orphan_destination(&library);
    let controls = state.output.control_projection();
    assert_eq!(controls.revision, control_revision);
    let options = state.output.render_options();
    assert_eq!((options.grand_master, options.blackout), (0.61, true));
    drop(show_change);
    database
        .execute_batch("DROP TRIGGER tl589_reject_insert; DROP TRIGGER tl589_reject_update;")
        .unwrap();
    drop(database);
    // Failure released workflow ownership and the replay gate: a legitimate open succeeds.
    let response = scenario
        .app
        .clone()
        .oneshot(open_show_request(&scenario.token, &scenario.alpha))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(scenario.current_id(), scenario.alpha);
    scenario.teardown();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn revision_copy_metadata_failure_keeps_live_runtime_and_identity() {
    assert_metadata_failure_keeps_live(Caller::RevisionCopy, "previous_active_show_id").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn clean_default_metadata_failure_keeps_live_runtime_and_identity() {
    assert_metadata_failure_keeps_live(Caller::CleanDefault, "previous_active_show_id").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mvr_open_after_metadata_failure_keeps_live_runtime_and_identity() {
    // MVR never writes previous-active, so fail its active-ID write instead.
    assert_metadata_failure_keeps_live(Caller::Mvr, "active_show_id").await;
}
