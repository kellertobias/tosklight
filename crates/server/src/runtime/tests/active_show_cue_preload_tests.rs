use super::*;

#[tokio::test]
async fn active_preload_cue_is_one_lossless_contextual_show_transaction() {
    let scenario = CuePreloadScenario::new("Active Preload Cue", true).await;
    scenario.activate_preload();
    let before = scenario.boundary();

    let stale = scenario.store_preload(0).await;

    assert_eq!(stale.status(), StatusCode::CONFLICT);
    scenario.assert_unchanged(&before);
    assert!(scenario.has_active_preload());

    let response = scenario.store_preload(1).await;

    assert_eq!(response.status(), StatusCode::OK);
    let response = json(response).await;
    assert_eq!(response["revision"], 2);
    assert_eq!(response["event_sequence"], before.event_sequence + 1);
    scenario.assert_one_active_commit(&before);
    assert!(!scenario.has_active_preload());

    let document = scenario.document();
    let stored = document
        .object("cue_list", &scenario.cue_list_id())
        .unwrap();
    assert_eq!(stored.body()["id"], scenario.cue_list_id());
    assert_eq!(stored.body()["cues"][0]["name"], "From active Preload");
    assert_eq!(
        stored.body()["cues"][0]["future_cue_metadata"],
        serde_json::json!({"owner":"newer-desk"})
    );
    assert_eq!(
        stored.body()["cues"][0]["group_changes"][0]["future_change_metadata"],
        serde_json::json!({"curve":"soft"})
    );
    let cue_list: light_playback::CueList = serde_json::from_value(stored.body().clone()).unwrap();
    assert!(cue_list.cues[0].group_changes.iter().any(|change| {
        change.group_id == "1"
            && change.attribute == light_core::AttributeKey::intensity()
            && change
                .value
                .as_ref()
                .and_then(light_core::AttributeValue::normalized)
                == Some(0.6)
    }));

    let events = scenario.events_after(before.event_sequence);
    assert_eq!(events.len(), 1);
    let event = &events[0];
    assert_eq!(
        event.source,
        light_application::EventSource::Action(light_application::ActionSource::Http)
    );
    let correlation_id = event.correlation_id.expect("operator event correlation");
    assert!(scenario.backup_names().iter().any(|name| {
        name.contains("show-object") && name.contains(&correlation_id.to_string())
    }));
    assert!(matches!(
        &event.payload,
        light_application::ApplicationEvent::Show(
            light_application::ShowEvent::ObjectsChanged(change)
        ) if change.changes.len() == 1
            && change.changes[0].kind == light_application::ActiveShowObjectKind::CueList
            && change.changes[0].object_id == scenario.cue_list_id()
            && change.changes[0].body.as_ref() == Some(stored.body())
    ));
}

#[tokio::test]
async fn inactive_preload_cue_keeps_the_compatibility_path_and_raw_extensions() {
    let scenario = CuePreloadScenario::new("Inactive Preload Cue", false).await;
    scenario.activate_preload();
    let before = scenario.boundary();

    let response = scenario.store_preload(1).await;

    assert_eq!(response.status(), StatusCode::OK);
    let response = json(response).await;
    assert_eq!(response["revision"], 2);
    assert!(response["event_sequence"].is_null());
    assert_eq!(
        scenario.document().revision().value(),
        before.show_revision + 1
    );
    assert_eq!(
        scenario.state.application_events.latest_sequence(),
        before.event_sequence
    );
    assert!(std::sync::Arc::ptr_eq(
        &scenario.state.engine.snapshot(),
        &before.runtime
    ));
    assert!(!scenario.has_active_preload());
    let body = scenario
        .document()
        .object("cue_list", &scenario.cue_list_id())
        .unwrap()
        .body()
        .clone();
    assert_eq!(body["cues"][0]["name"], "From active Preload");
    assert_eq!(
        body["cues"][0]["future_cue_metadata"]["owner"],
        "newer-desk"
    );
    assert_eq!(
        body["cues"][0]["group_changes"][0]["future_change_metadata"]["curve"],
        "soft"
    );
}

#[tokio::test]
async fn storing_active_preload_publishes_the_capture_mode_release() {
    let scenario = CuePreloadScenario::new("Capture-mode Preload Cue", true).await;
    scenario.activate_preload();
    assert_eq!(scenario.enter_preload().await.status(), StatusCode::OK);
    let before = scenario.boundary();

    let response = scenario.store_preload(1).await;

    assert_eq!(response.status(), StatusCode::OK);
    let response = json(response).await;
    assert_eq!(response["event_sequence"], before.event_sequence + 1);
    assert!(!scenario.has_active_preload());
    let events = scenario.events_after(before.event_sequence);
    assert_eq!(events.len(), 2);
    assert!(matches!(
        events[0].payload,
        light_application::ApplicationEvent::Show(light_application::ShowEvent::ObjectsChanged(_))
    ));
    let capture = &events[1];
    assert_eq!(capture.desk_id, None);
    assert_eq!(
        capture.delivery,
        light_application::DeliveryPolicy::Replaceable
    );
    let light_application::ApplicationEvent::Programming(
        light_application::ProgrammingEvent::CaptureModeChanged(change),
    ) = &capture.payload
    else {
        panic!("active Preload Store must publish the capture-mode release")
    };
    assert_eq!(change.projection.revision, 2);
    assert!(!change.projection.blind);
}

#[tokio::test]
async fn active_preload_store_holds_one_activation_guard_through_release() {
    let scenario = CuePreloadScenario::new("Atomic Preload Store release", true).await;
    scenario.activate_preload();
    scenario.state.preload_store_release_lifecycle.arm();
    let request = scenario.store_preload(1);
    let pause = Arc::clone(&scenario.state.preload_store_release_lifecycle);
    let wait = tokio::task::spawn_blocking(move || pause.wait_until_started());
    let verify_guard = async {
        wait.await.unwrap();
        assert!(
            scenario
                .state
                .activation_lock
                .clone()
                .try_lock_owned()
                .is_err(),
            "the persisted target and active Preload release must share one activation guard"
        );
        scenario.state.preload_store_release_lifecycle.release();
    };
    let ((), response) = tokio::join!(verify_guard, request);
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!scenario.has_active_preload());
}

struct CuePreloadScenario {
    state: AppState,
    app: Router,
    token: String,
    session_id: light_core::SessionId,
    entry: ShowEntry,
    data_dir: PathBuf,
    cue_list_id: light_core::CueListId,
}

impl CuePreloadScenario {
    async fn new(name: &str, active: bool) -> Self {
        let (state, data_dir) = test_state();
        let app = router(state.clone());
        let (token, session_id) = login(&app, "Operator").await;
        let show = create_show(&app, &token, name).await;
        let show_id = light_core::ShowId(Uuid::parse_str(show["id"].as_str().unwrap()).unwrap());
        let entry = state.desk.lock().show(show_id).unwrap().unwrap();
        let cue_list_id = light_core::CueListId::new();
        seed_cue_preload_show(&entry, cue_list_id);
        if active {
            open_show(&app, &token, show_id).await;
        }
        Self {
            state,
            app,
            token,
            session_id: light_core::SessionId(Uuid::parse_str(&session_id).unwrap()),
            entry,
            data_dir,
            cue_list_id,
        }
    }

    fn activate_preload(&self) {
        assert!(self.state.programmers.set_preload_group(
            self.session_id,
            "1".into(),
            light_core::AttributeKey::intensity(),
            light_core::AttributeValue::Normalized(0.6),
        ));
        assert!(self.state.programmers.activate_preload(self.session_id));
        assert!(self.has_active_preload());
    }

    async fn store_preload(&self, expected: u64) -> Response {
        self.app
            .clone()
            .oneshot(
                Request::post(format!("/api/v1/shows/{}/preload/store", self.entry.id.0))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {}", self.token))
                    .header(header::IF_MATCH, expected.to_string())
                    .body(Body::from(
                        serde_json::json!({
                            "target":"cue",
                            "target_id":self.cue_list_id(),
                            "cue_number":1.0,
                            "name":"From active Preload"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn enter_preload(&self) -> Response {
        let session = self.state.sessions.read()[&self.session_id].clone();
        self.app
            .clone()
            .oneshot(
                Request::post(format!(
                    "/api/v2/desks/{}/command-line/keys",
                    session.desk.id
                ))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {}", self.token))
                .body(Body::from(
                    serde_json::json!({
                        "key":"PRE",
                        "phase":"press",
                        "request_id":"preload-store-capture-mode"
                    })
                    .to_string(),
                ))
                .unwrap(),
            )
            .await
            .unwrap()
    }

    fn cue_list_id(&self) -> String {
        self.cue_list_id.0.to_string()
    }

    fn document(&self) -> light_show::PortableShowDocument {
        ShowStore::open(&self.entry.path)
            .unwrap()
            .portable_document()
            .unwrap()
    }

    fn has_active_preload(&self) -> bool {
        let programmer = self.state.programmers.get(self.session_id).unwrap();
        !programmer.preload_active.is_empty() || !programmer.preload_group_active.is_empty()
    }

    fn boundary(&self) -> CuePreloadBoundary {
        CuePreloadBoundary {
            show_revision: self.document().revision().value(),
            backup_count: self.backup_names().len(),
            runtime: self.state.engine.snapshot(),
            event_sequence: self.state.application_events.latest_sequence(),
        }
    }

    fn assert_unchanged(&self, before: &CuePreloadBoundary) {
        assert_eq!(self.document().revision().value(), before.show_revision);
        assert_eq!(self.backup_names().len(), before.backup_count);
        assert!(std::sync::Arc::ptr_eq(
            &self.state.engine.snapshot(),
            &before.runtime
        ));
        assert_eq!(
            self.state.application_events.latest_sequence(),
            before.event_sequence
        );
    }

    fn assert_one_active_commit(&self, before: &CuePreloadBoundary) {
        let document = self.document();
        assert_eq!(document.revision().value(), before.show_revision + 1);
        assert_eq!(self.backup_names().len(), before.backup_count + 1);
        let runtime = self.state.engine.snapshot();
        assert_eq!(runtime.revision, document.revision().value());
        assert!(!std::sync::Arc::ptr_eq(&runtime, &before.runtime));
        assert_eq!(
            self.state.application_events.latest_sequence(),
            before.event_sequence + 1
        );
    }

    fn backup_names(&self) -> Vec<String> {
        std::fs::read_dir(self.data_dir.join("backups"))
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect()
    }

    fn events_after(&self, sequence: u64) -> Vec<std::sync::Arc<light_application::EventEnvelope>> {
        let light_application::EventReplay::Events(events) = self
            .state
            .application_events
            .replay(sequence, &light_application::EventFilter::default())
        else {
            panic!("expected retained CueList event");
        };
        events
    }
}

impl Drop for CuePreloadScenario {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.data_dir);
    }
}

struct CuePreloadBoundary {
    show_revision: u64,
    backup_count: usize,
    runtime: std::sync::Arc<EngineSnapshot>,
    event_sequence: u64,
}

fn seed_cue_preload_show(entry: &ShowEntry, cue_list_id: light_core::CueListId) {
    let store = ShowStore::open(&entry.path).unwrap();
    store
        .put_object(
            "group",
            "1",
            &serde_json::json!({"id":"1","name":"Empty","fixtures":[]}),
            0,
        )
        .unwrap();
    let mut cue = light_playback::Cue::new(1.0);
    cue.id = Uuid::from_u128(0xc001);
    cue.group_changes.push(light_playback::GroupCueChange {
        group_id: "1".into(),
        attribute: light_core::AttributeKey::intensity(),
        value: Some(light_core::AttributeValue::Normalized(0.1)),
        automatic_restore: false,
        fade_millis: None,
        delay_millis: None,
    });
    let cue_list = light_playback::CueList {
        id: cue_list_id,
        name: "Main".into(),
        priority: 0,
        mode: light_playback::CueListMode::Sequence,
        looped: false,
        chaser_step_millis: 1_000,
        speed_group: None,
        intensity_priority_mode: light_playback::IntensityPriorityMode::Htp,
        wrap_mode: Some(light_playback::WrapMode::Off),
        restart_mode: light_playback::RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_multiplier: 1.0,
        cues: vec![cue],
    };
    let mut body = serde_json::to_value(cue_list).unwrap();
    body["cues"][0]["future_cue_metadata"] = serde_json::json!({"owner":"newer-desk"});
    body["cues"][0]["group_changes"][0]["future_change_metadata"] =
        serde_json::json!({"curve":"soft"});
    store
        .put_object("cue_list", &cue_list_id.0.to_string(), &body, 0)
        .unwrap();
}

async fn open_show(app: &Router, token: &str, show_id: light_core::ShowId) {
    let response = app
        .clone()
        .oneshot(
            Request::post(format!("/api/v1/shows/{}/open", show_id.0))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(r#"{"transition":"hold_current"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
