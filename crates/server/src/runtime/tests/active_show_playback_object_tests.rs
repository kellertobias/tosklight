use super::*;

#[tokio::test]
async fn active_slot_upsert_is_lossless_single_commit_and_stale_failure_safe() {
    let scenario = PlaybackObjectScenario::new("Slot upsert").await;
    let store = ShowStore::open(&scenario.entry.path).unwrap();
    let cue_list = cue_list("Slot target");
    put_raw(
        &store,
        "cue_list",
        &cue_list.id.0.to_string(),
        serde_json::to_value(&cue_list).unwrap(),
    );
    let mut raw_playback = serde_json::to_value(playback(1, cue_list.id)).unwrap();
    raw_playback["future_surface"] = serde_json::json!({"wing": 2});
    raw_playback["target"]["future_target"] = serde_json::json!("kept");
    put_raw(&store, "playback", "1", raw_playback);
    let mut page = serde_json::json!({
        "number": 1,
        "name": "Main",
        "slots": {"1": 1},
        "future_layout": {"columns": 10}
    });
    put_raw(&store, "playback_page", "1", page.take());
    scenario.open().await;

    let before = scenario.capture();
    let response = scenario
        .put_slot(1, 1, playback(99, cue_list.id), 1, 1)
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let response = json(response).await;
    assert_eq!(response["playback_revision"], 2);
    assert_eq!(response["page_revision"], 2);
    assert_eq!(response["event_sequence"], before.event_sequence + 1);

    let document = ShowStore::open(&scenario.entry.path)
        .unwrap()
        .portable_document()
        .unwrap();
    assert_eq!(document.revision().value(), before.show_revision + 1);
    let stored_playback = document.object("playback", "1").unwrap();
    assert_eq!(stored_playback.body()["number"], 1);
    assert_eq!(stored_playback.body()["future_surface"]["wing"], 2);
    assert_eq!(stored_playback.body()["target"]["future_target"], "kept");
    assert_eq!(
        document.object("playback_page", "1").unwrap().body()["future_layout"]["columns"],
        10
    );
    scenario.assert_one_success(&before, 2);

    let after_success = scenario.capture();
    let stale = scenario
        .put_slot(1, 1, playback(1, cue_list.id), 1, 1)
        .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    scenario.assert_unchanged(&after_success);
    scenario.cleanup();
}

#[tokio::test]
async fn clearing_a_slot_removes_the_pool_playback_from_every_page_atomically() {
    let scenario = PlaybackObjectScenario::new("Slot clear").await;
    let store = ShowStore::open(&scenario.entry.path).unwrap();
    let cue_list = cue_list("Clear target");
    put_raw(
        &store,
        "cue_list",
        &cue_list.id.0.to_string(),
        serde_json::to_value(&cue_list).unwrap(),
    );
    put_raw(
        &store,
        "playback",
        "1",
        serde_json::to_value(playback(1, cue_list.id)).unwrap(),
    );
    for (number, slot) in [(1, 1), (2, 7)] {
        put_raw(
            &store,
            "playback_page",
            &number.to_string(),
            serde_json::json!({
                "number": number,
                "name": format!("Page {number}"),
                "slots": {(slot.to_string()): 1},
                "future_layout": {"page": number}
            }),
        );
    }
    scenario.open().await;

    let before = scenario.capture();
    let response = scenario.clear_slot(1, 1, 1, 1).await;
    assert_eq!(response.status(), StatusCode::OK);
    let response = json(response).await;
    assert_eq!(response["page_revisions"], serde_json::json!([2, 2]));
    assert_eq!(response["event_sequence"], before.event_sequence + 1);

    let document = ShowStore::open(&scenario.entry.path)
        .unwrap()
        .portable_document()
        .unwrap();
    assert_eq!(document.revision().value(), before.show_revision + 1);
    assert!(document.object("playback", "1").is_none());
    for number in [1, 2] {
        let page = document
            .object("playback_page", &number.to_string())
            .unwrap();
        assert_eq!(page.body()["slots"], serde_json::json!({}));
        assert_eq!(page.body()["future_layout"]["page"], number);
    }
    scenario.assert_one_success(&before, 3);
    scenario.cleanup();
}

struct PlaybackObjectScenario {
    state: AppState,
    app: Router,
    token: String,
    show_id: String,
    entry: ShowEntry,
    data_dir: PathBuf,
}

impl PlaybackObjectScenario {
    async fn new(name: &str) -> Self {
        let (state, data_dir) = test_state();
        let app = router(state.clone());
        let (token, _) = login(&app, "Operator").await;
        let show = create_show(&app, &token, name).await;
        let show_id = show["id"].as_str().unwrap().to_owned();
        let entry = state
            .desk
            .lock()
            .show(light_core::ShowId(Uuid::parse_str(&show_id).unwrap()))
            .unwrap()
            .unwrap();
        Self {
            state,
            app,
            token,
            show_id,
            entry,
            data_dir,
        }
    }

    async fn open(&self) {
        let response = self
            .app
            .clone()
            .oneshot(
                Request::post(format!("/api/v1/shows/{}/open", self.show_id))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {}", self.token))
                    .body(Body::from(r#"{"transition":"hold_current"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    async fn put_slot(
        &self,
        page: u8,
        slot: u8,
        playback: light_playback::PlaybackDefinition,
        expected_playback_revision: u64,
        expected_page_revision: u64,
    ) -> Response {
        self.app
            .clone()
            .oneshot(
                Request::put(format!("/api/v1/playback-pages/{page}/slots/{slot}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {}", self.token))
                    .body(Body::from(
                        serde_json::json!({
                            "playback": playback,
                            "expected_playback_revision": expected_playback_revision,
                            "expected_page_revision": expected_page_revision
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn clear_slot(
        &self,
        page: u8,
        slot: u8,
        expected_playback_revision: u64,
        expected_page_revision: u64,
    ) -> Response {
        self.app
            .clone()
            .oneshot(
                Request::delete(format!("/api/v1/playback-pages/{page}/slots/{slot}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {}", self.token))
                    .body(Body::from(
                        serde_json::json!({
                            "expected_playback_revision": expected_playback_revision,
                            "expected_page_revision": expected_page_revision
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    fn capture(&self) -> PlaybackBoundary {
        let document = ShowStore::open(&self.entry.path)
            .unwrap()
            .portable_document()
            .unwrap();
        PlaybackBoundary {
            show_revision: document.revision().value(),
            runtime: self.state.engine.snapshot(),
            event_sequence: self.state.application_events.latest_sequence(),
            backup_count: backup_count(&self.data_dir),
        }
    }

    fn assert_one_success(&self, before: &PlaybackBoundary, changed_objects: usize) {
        assert_eq!(backup_count(&self.data_dir), before.backup_count + 1);
        assert_eq!(
            self.state.application_events.latest_sequence(),
            before.event_sequence + 1
        );
        let runtime = self.state.engine.snapshot();
        assert_eq!(runtime.revision, before.show_revision + 1);
        assert!(!Arc::ptr_eq(&runtime, &before.runtime));
        let light_application::EventReplay::Events(events) = self.state.application_events.replay(
            before.event_sequence,
            &light_application::EventFilter::default(),
        ) else {
            panic!("expected one typed playback object event");
        };
        assert_eq!(events.len(), 1);
        let light_application::ApplicationEvent::Show(
            light_application::ShowEvent::ObjectsChanged(change),
        ) = &events[0].payload
        else {
            panic!("expected typed active-show object event");
        };
        assert_eq!(change.changes.len(), changed_objects);
        assert_eq!(
            events[0].source,
            light_application::EventSource::Action(light_application::ActionSource::Http)
        );
        assert!(events[0].correlation_id.is_some());
    }

    fn assert_unchanged(&self, before: &PlaybackBoundary) {
        let current = self.capture();
        assert_eq!(current.show_revision, before.show_revision);
        assert_eq!(current.event_sequence, before.event_sequence);
        assert_eq!(current.backup_count, before.backup_count);
        assert!(Arc::ptr_eq(&current.runtime, &before.runtime));
    }

    fn cleanup(&self) {
        let _ = std::fs::remove_dir_all(&self.data_dir);
    }
}

struct PlaybackBoundary {
    show_revision: u64,
    runtime: Arc<EngineSnapshot>,
    event_sequence: u64,
    backup_count: usize,
}

fn put_raw(store: &ShowStore, kind: &str, id: &str, body: serde_json::Value) {
    store.put_object(kind, id, &body, 0).unwrap();
}

fn backup_count(data_dir: &std::path::Path) -> usize {
    std::fs::read_dir(data_dir.join("backups"))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().contains("show-object"))
        .count()
}

fn cue_list(name: &str) -> light_playback::CueList {
    light_playback::CueList {
        id: light_core::CueListId::new(),
        name: name.into(),
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
        cues: vec![light_playback::Cue::new(1.0)],
    }
}

fn playback(number: u16, cue_list_id: light_core::CueListId) -> light_playback::PlaybackDefinition {
    let target = light_playback::PlaybackTarget::CueList { cue_list_id };
    light_playback::PlaybackDefinition {
        number,
        name: format!("Playback {number}"),
        buttons: light_playback::PlaybackDefinition::default_buttons(&target),
        target,
        button_count: 3,
        fader: light_playback::PlaybackFaderMode::Master,
        has_fader: true,
        go_activates: true,
        auto_off: true,
        xfade_millis: 0,
        color: "#20c997".into(),
        flash_release: light_playback::FlashReleaseMode::default(),
        protect_from_swap: false,
        presentation_icon: None,
        presentation_image: None,
    }
}
