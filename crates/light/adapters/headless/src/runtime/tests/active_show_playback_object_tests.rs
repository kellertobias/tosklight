use super::{playback_topology_route_support::post_topology, *};

#[tokio::test]
async fn active_assignment_update_is_lossless_single_commit_and_stale_failure_safe() {
    let scenario = PlaybackObjectScenario::new("Assignment update").await;
    let store = ShowStore::open(&scenario.entry.path).unwrap();
    let cue_list = cue_list("Assignment target");
    put_raw(
        &store,
        "cue_list",
        &cue_list.id.0.to_string(),
        serde_json::to_value(&cue_list).unwrap(),
    );
    let mut definition = serde_json::to_value(playback(1, cue_list.id)).unwrap();
    definition["future_surface"] = serde_json::json!({"wing":2});
    definition["target"]["future_target"] = serde_json::json!("kept");
    put_raw(
        &store,
        "playback_page",
        "1",
        serde_json::json!({
            "number":1,"name":"Main","assignments":{"1":definition},
            "future_layout":{"columns":10}
        }),
    );
    scenario.open().await;
    let before = scenario.capture();
    let mut updated = playback(99, cue_list.id);
    updated.name = "Updated target".into();
    let response = scenario.put_slot(1, 1, updated, 1, 1).await;
    let status = response.status();
    let response = json(response).await;
    assert_eq!(status, StatusCode::OK, "{response}");
    assert_eq!(response["status"], "changed");
    assert_eq!(projection_revision(&response, "playback_page", "1"), 2);
    assert_eq!(response["event_sequence"], before.event_sequence + 1);
    let document = ShowStore::open(&scenario.entry.path)
        .unwrap()
        .portable_document()
        .unwrap();
    assert_eq!(document.revision().value(), before.show_revision + 1);
    let page = document.object("playback_page", "1").unwrap();
    let stored = &page.body()["assignments"]["1"];
    assert_eq!(stored["name"], "Updated target");
    assert!(stored.get("number").is_none());
    assert_eq!(stored["future_surface"]["wing"], 2);
    assert_eq!(stored["target"]["future_target"], "kept");
    assert_eq!(page.body()["future_layout"]["columns"], 10);
    scenario.assert_one_success(&before, 1);
    let after_success = scenario.capture();
    let stale = scenario
        .put_slot(1, 1, playback(1, cue_list.id), 1, 1)
        .await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    scenario.assert_unchanged(&after_success);
    scenario.cleanup();
}

#[tokio::test]
async fn clearing_one_page_assignment_preserves_independent_peer_page_atomically() {
    let scenario = PlaybackObjectScenario::new("Assignment clear").await;
    let store = ShowStore::open(&scenario.entry.path).unwrap();
    let cue_list = cue_list("Clear target");
    put_raw(
        &store,
        "cue_list",
        &cue_list.id.0.to_string(),
        serde_json::to_value(&cue_list).unwrap(),
    );
    for (number, position) in [(1, 1), (2, 7)] {
        put_raw(
            &store,
            "playback_page",
            &number.to_string(),
            serde_json::json!({
                "number":number,"name":format!("Page {number}"),
                "assignments":{(position.to_string()):playback(position,cue_list.id)},
                "future_layout":{"page":number}
            }),
        );
    }
    scenario.open().await;
    let address = light_playback::PlaybackAddress::new(1, 1).unwrap();
    scenario
        .state
        .output
        .execute_playback(light_engine::EnginePlaybackCommand::Playback {
            address,
            action: light_engine::PlaybackAction::On,
            activation_origin: None,
            control_source: light_playback::PlaybackControlSource {
                surface_id: "test-assignment".into(),
                control_id: 1,
            },
        })
        .unwrap();
    assert_eq!(scenario.state.output.active_playbacks().len(), 1);
    let before = scenario.capture();
    let response = scenario.clear_slot(1, 1, 1, 1).await;
    assert_eq!(response.status(), StatusCode::OK);
    let response = json(response).await;
    assert_eq!(response["status"], "changed");
    assert_eq!(projection_revision(&response, "playback_page", "1"), 2);
    assert_eq!(response["objects"].as_array().unwrap().len(), 1);
    let document = ShowStore::open(&scenario.entry.path)
        .unwrap()
        .portable_document()
        .unwrap();
    assert_eq!(document.revision().value(), before.show_revision + 1);
    assert!(
        document
            .object("cue_list", &cue_list.id.0.to_string())
            .is_some()
    );
    assert_eq!(
        document.object("playback_page", "1").unwrap().body()["assignments"],
        serde_json::json!({})
    );
    let peer = document.object("playback_page", "2").unwrap();
    assert!(peer.body()["assignments"]["7"].is_object());
    assert_eq!(peer.revision(), 1);
    for number in [1, 2] {
        assert_eq!(
            document
                .object("playback_page", &number.to_string())
                .unwrap()
                .body()["future_layout"]["page"],
            number
        );
    }
    scenario.assert_one_success(&before, 1);
    scenario.cleanup();
}

#[tokio::test]
async fn clearing_one_shared_assignment_rebinds_the_running_target_to_its_peer() {
    let scenario = PlaybackObjectScenario::new("Shared assignment clear").await;
    let store = ShowStore::open(&scenario.entry.path).unwrap();
    let cue_list = cue_list("Shared target");
    put_raw(
        &store,
        "cue_list",
        &cue_list.id.0.to_string(),
        serde_json::to_value(&cue_list).unwrap(),
    );
    put_raw(
        &store,
        "playback_page",
        "1",
        serde_json::json!({
            "number":1,"name":"Main","assignments":{"1":playback(1,cue_list.id),"2":playback(2,cue_list.id)}
        }),
    );
    scenario.open().await;
    let address = light_playback::PlaybackAddress::new(1, 1).unwrap();
    scenario
        .state
        .output
        .execute_playback(light_engine::EnginePlaybackCommand::Playback {
            address,
            action: light_engine::PlaybackAction::On,
            activation_origin: None,
            control_source: light_playback::PlaybackControlSource {
                surface_id: "test-assignment".into(),
                control_id: 1,
            },
        })
        .unwrap();
    let before = scenario.capture();
    let response = scenario.clear_slot(1, 1, 1, 1).await;
    assert_eq!(response.status(), StatusCode::OK);
    let peer = light_playback::PlaybackAddress::new(1, 2).unwrap();
    let status = scenario
        .state
        .output
        .playback_runtime_status_at(peer)
        .unwrap();
    assert!(status.playback.enabled);
    assert_eq!(status.playback.playback_address, Some(peer));
    assert_eq!(scenario.state.output.active_playbacks().len(), 1);
    assert!(
        ShowStore::open(&scenario.entry.path)
            .unwrap()
            .portable_document()
            .unwrap()
            .object("playback_page", "1")
            .unwrap()
            .body()["assignments"]["2"]
            .is_object()
    );
    scenario.assert_one_success(&before, 1);
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
            .installation
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
            .oneshot(open_show_request(&self.token, &self.show_id))
            .await
            .unwrap();
        let status = response.status();
        let response = json(response).await;
        assert_eq!(status, StatusCode::OK, "{response}");
    }

    async fn put_slot(
        &self,
        page: u8,
        position: u16,
        playback: light_playback::PlaybackDefinition,
        _expected_playback_revision: u64,
        expected_page_revision: u64,
    ) -> Response {
        post_topology(
            &self.app,
            Some(&self.token),
            &self.show_id,
            Some(self.capture().show_revision),
            serde_json::json!({
                "request_id": Uuid::new_v4().to_string(),
                "action": {
                    "type": "update_playback",
                    "address": {"page": page, "position": position},
                    "definition": playback,
                    "expected_page_revision": expected_page_revision,
                    "expected_page_object_id":
                        (expected_page_revision > 0).then_some(page.to_string())
                }
            }),
            None,
        )
        .await
    }

    async fn clear_slot(
        &self,
        page: u8,
        position: u16,
        _expected_playback_revision: u64,
        expected_page_revision: u64,
    ) -> Response {
        post_topology(
            &self.app,
            Some(&self.token),
            &self.show_id,
            Some(self.capture().show_revision),
            serde_json::json!({
                "request_id": Uuid::new_v4().to_string(),
                "action": {
                    "type": "clear_playback",
                    "address": {"page": page, "position": position},
                    "expected_page_revision": expected_page_revision,
                    "expected_page_object_id":
                        (expected_page_revision > 0).then_some(page.to_string())
                }
            }),
            None,
        )
        .await
    }

    fn capture(&self) -> PlaybackBoundary {
        let document = ShowStore::open(&self.entry.path)
            .unwrap()
            .portable_document()
            .unwrap();
        PlaybackBoundary {
            show_revision: document.revision().value(),
            runtime: self.state.output.snapshot(),
            event_sequence: self.state.events.latest_sequence(),
            backup_count: backup_count(&self.data_dir),
        }
    }

    fn assert_one_success(&self, before: &PlaybackBoundary, changed_objects: usize) {
        // One interval-gated recovery checkpoint per scenario (see api-rules §8).
        assert_eq!(backup_count(&self.data_dir), before.backup_count.max(1));
        assert_eq!(
            self.state.events.latest_sequence(),
            before.event_sequence + 1
        );
        let runtime = self.state.output.snapshot();
        assert_eq!(runtime.revision, before.show_revision + 1);
        assert!(!Arc::ptr_eq(&runtime, &before.runtime));
        let light_application::EventReplay::Events(events) = self.state.events.replay(
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

fn projection_revision(outcome: &serde_json::Value, kind: &str, object_id: &str) -> u64 {
    outcome["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|object| object["kind"] == kind && object["object_id"] == object_id)
        .unwrap()["object_revision"]
        .as_u64()
        .unwrap()
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
        pool_number: None,
        legacy_pool_aliases: Vec::new(),
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
        auto_off_at_zero: false,
        auto_off_flash_release: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_multiplier: 1.0,
        cues: vec![light_playback::Cue::new(cue("1"))],
    }
}

fn playback(number: u16, cue_list_id: light_core::CueListId) -> light_playback::PlaybackDefinition {
    let target = light_playback::PlaybackTarget::CueList { cue_list_id };
    light_playback::PlaybackDefinition {
        name: format!("Playback {number}"),
        buttons: light_playback::PlaybackDefinition::default_buttons(&target),
        target,
        button_count: 3,
        fader: light_playback::PlaybackFaderMode::Master,
        has_fader: true,
        footprint: light_playback::PlaybackFootprint::Normal,
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
