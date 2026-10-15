use super::*;

struct TimecodeScopeRig {
    state: AppState,
    app: Router,
    token: String,
    data_dir: PathBuf,
}

impl TimecodeScopeRig {
    async fn new() -> Self {
        let (state, data_dir) = test_state();
        let app = router(state.clone());
        let (token, _) = login(&app, "Timecode scope operator").await;
        Self {
            state,
            app,
            token,
            data_dir,
        }
    }

    async fn create_and_open(&self, name: &str) -> String {
        let show = create_show(&self.app, &self.token, name).await;
        let id = show["id"].as_str().unwrap().to_owned();
        self.open(&id).await;
        id
    }

    async fn open(&self, id: &str) {
        let response = self
            .app
            .clone()
            .oneshot(open_show_request(&self.token, id))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    async fn request(
        &self,
        show: &str,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> axum::response::Response {
        let request = if let Some(body) = body {
            Request::post(path)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap()
        } else {
            Request::get(path).body(Body::empty()).unwrap()
        };
        let mut request = request;
        request.headers_mut().insert(
            header::AUTHORIZATION,
            format!("Bearer {}", self.token).parse().unwrap(),
        );
        request
            .headers_mut()
            .insert("x-tosk-show", show.parse().unwrap());
        self.app.clone().oneshot(request).await.unwrap()
    }

    async fn create_timecode(&self, show: &str, id: Uuid, duration: u64) {
        let response = self.request(show, "/api/v2/timecodes/actions", Some(serde_json::json!({
            "request_id": Uuid::new_v4().to_string(),
            "action": {"type":"create", "definition": {
                "id":id, "number":1, "name":format!("Timecode {duration}"), "duration_frame":duration,
                "transport_offset_frame":0, "auto_start":false, "markers":[], "lanes":[]
            }}
        }))).await;
        let status = response.status();
        assert_eq!(status, StatusCode::OK, "{}", json(response).await);
    }

    fn install_stale_cache(&self, id: Uuid) {
        self.state
            .timecodes
            .install(
                light_playback::TimecodeDefinition {
                    id: light_playback::TimecodeId(id),
                    number: 1,
                    name: "Stale foreign cache".into(),
                    duration: Some(light_playback::TimecodeFrame(440)),
                    transport_offset: light_playback::TimecodeFrame::ZERO,
                    auto_start: false,
                    audio: None,
                    markers: Vec::new(),
                    lanes: Vec::new(),
                },
                None,
            )
            .unwrap();
    }

    async fn transport(
        &self,
        show: &str,
        id: Uuid,
        action: serde_json::Value,
    ) -> axum::response::Response {
        self.request(
            show,
            &format!("/api/v2/timecodes/{id}/transport"),
            Some(serde_json::json!({"timecode_id":id,"action":action})),
        )
        .await
    }
}

impl Drop for TimecodeScopeRig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.data_dir);
    }
}

#[tokio::test]
async fn tl793_runtime_list_contains_only_the_current_show() {
    let rig = TimecodeScopeRig::new().await;
    let a = rig.create_and_open("Scope A").await;
    let aid = Uuid::new_v4();
    rig.create_timecode(&a, aid, 440).await;
    let b = rig.create_and_open("Scope B").await;
    let bid = Uuid::new_v4();
    rig.create_timecode(&b, bid, 880).await;
    let response = rig.request(&b, "/api/v2/timecodes/runtime", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = json(response).await;
    let ids: Vec<_> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["timecode_id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec![bid.to_string()]);
}

#[tokio::test]
async fn tl793_cached_foreign_runtime_read_is_not_authority() {
    let rig = TimecodeScopeRig::new().await;
    let a = rig.create_and_open("Foreign read A").await;
    let id = Uuid::new_v4();
    rig.create_timecode(&a, id, 440).await;
    let b = rig.create_and_open("Foreign read B").await;
    rig.install_stale_cache(id);
    let before = rig.state.timecodes.snapshots();
    let response = rig
        .request(&b, &format!("/api/v2/timecodes/{id}/runtime"), None)
        .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let after = rig.state.timecodes.snapshots();
    assert_eq!(after, before);
}

#[tokio::test]
async fn tl793_cached_foreign_transport_refuses_without_a_live_action() {
    let rig = TimecodeScopeRig::new().await;
    let a = rig.create_and_open("Foreign action A").await;
    let id = Uuid::new_v4();
    rig.create_timecode(&a, id, 440).await;
    let b = rig.create_and_open("Foreign action B").await;
    rig.install_stale_cache(id);
    let before = rig.state.timecodes.snapshots();
    let sequence = rig.state.events.latest_sequence();
    let response = rig
        .transport(&b, id, serde_json::json!({"type":"go"}))
        .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let after = rig.state.timecodes.snapshots();
    assert_eq!(after, before);
    assert_eq!(rig.state.events.latest_sequence(), sequence);
}

#[tokio::test]
async fn tl793_same_uuid_uses_the_definition_of_the_current_show() {
    let rig = TimecodeScopeRig::new().await;
    let id = Uuid::new_v4();
    let a = rig.create_and_open("Same id A").await;
    rig.create_timecode(&a, id, 440).await;
    let b = rig.create_and_open("Same id B").await;
    rig.create_timecode(&b, id, 880).await;
    for (show, duration) in [(&a, 440), (&b, 880), (&a, 440)] {
        rig.open(show).await;
        let response = rig
            .request(show, &format!("/api/v2/timecodes/{id}/runtime"), None)
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(json(response).await["duration_frame"], duration);
    }
}

#[tokio::test]
async fn tl793_repeated_reads_preserve_paused_transport_without_publication_churn() {
    let rig = TimecodeScopeRig::new().await;
    let show = rig.create_and_open("Stable transport").await;
    let id = Uuid::new_v4();
    rig.create_timecode(&show, id, 440).await;
    for action in [
        serde_json::json!({"type":"go"}),
        serde_json::json!({"type":"seek","frame":110}),
        serde_json::json!({"type":"pause"}),
    ] {
        assert_eq!(
            rig.transport(&show, id, action).await.status(),
            StatusCode::OK
        );
    }
    let before = rig
        .state
        .timecodes
        .snapshot(light_playback::TimecodeId(id))
        .unwrap();
    let sequence = rig.state.events.latest_sequence();
    for path in [
        format!("/api/v2/timecodes/{id}/runtime"),
        "/api/v2/timecodes/runtime".into(),
    ] {
        assert_eq!(
            rig.request(&show, &path, None).await.status(),
            StatusCode::OK
        );
    }
    let after = rig
        .state
        .timecodes
        .snapshot(light_playback::TimecodeId(id))
        .unwrap();
    assert_eq!(
        after.transport,
        light_playback::TimecodeTransportState::Paused
    );
    assert_eq!(after.frame, before.frame);
    assert_eq!(after.revision, before.revision);
    assert_eq!(rig.state.events.latest_sequence(), sequence);
}

#[tokio::test]
async fn tl793_show_activation_retires_playing_foreign_transports_before_any_read() {
    let rig = TimecodeScopeRig::new().await;
    let a = rig.create_and_open("Playing A").await;
    let id = Uuid::new_v4();
    rig.create_timecode(&a, id, 440).await;
    assert_eq!(
        rig.transport(&a, id, serde_json::json!({"type":"go"}))
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        rig.state
            .timecodes
            .snapshot(light_playback::TimecodeId(id))
            .unwrap()
            .transport,
        light_playback::TimecodeTransportState::Playing
    );
    let _b = rig.create_and_open("Playing B").await;
    assert!(
        rig.state.timecodes.snapshots().is_empty(),
        "activation must retire foreign transports without waiting for a list read"
    );
    assert!(rig.state.timecodes.tick().is_empty());
    let mut commands = Vec::new();
    rig.state.timecodes.reconcile_cue_lists_with_executor(
        rig.state.output.engine(),
        |command, _| {
            commands.push(command);
            Err("unexpected foreign Timecode action".into())
        },
    );
    assert!(commands.is_empty());
}

#[tokio::test]
async fn tl793_failed_activation_preserves_the_playing_current_show_transport() {
    let rig = TimecodeScopeRig::new().await;
    let a = rig.create_and_open("Failure A").await;
    let id = Uuid::new_v4();
    rig.create_timecode(&a, id, 440).await;
    assert_eq!(
        rig.transport(&a, id, serde_json::json!({"type":"go"}))
            .await
            .status(),
        StatusCode::OK
    );
    let destination = create_show(&rig.app, &rig.token, "Failure B").await;
    let before = rig
        .state
        .timecodes
        .snapshot(light_playback::TimecodeId(id))
        .unwrap();
    let database = rusqlite::Connection::open(rig.data_dir.join("desk.sqlite")).unwrap();
    database.execute_batch("CREATE TRIGGER reject_timecode_scope_activation BEFORE INSERT ON settings WHEN NEW.key='previous_active_show_id' BEGIN SELECT RAISE(ABORT,'injected Timecode activation failure'); END;").unwrap();
    let response = rig
        .app
        .clone()
        .oneshot(open_show_request(
            &rig.token,
            destination["id"].as_str().unwrap(),
        ))
        .await
        .unwrap();
    assert!(!response.status().is_success());
    assert!(
        json(response)
            .await
            .to_string()
            .contains("injected Timecode activation failure")
    );
    assert_eq!(rig.state.active_show.current().unwrap().id.0.to_string(), a);
    assert_eq!(
        rig.state
            .timecodes
            .snapshot(light_playback::TimecodeId(id))
            .unwrap(),
        before
    );
}

#[tokio::test]
async fn tl793_activation_discards_old_clip_ownership_without_releasing_the_destination() {
    let rig = TimecodeScopeRig::new().await;
    rig.create_and_open("Clip owner A").await;
    let fixture = light_core::FixtureId::new();
    let list = operational_cue_list(light_core::CueListId::new(), fixture);
    let id = light_playback::TimecodeId(Uuid::new_v4());
    let definition: light_playback::TimecodeDefinition =
        serde_json::from_value(serde_json::json!({
            "id":id,"number":1,"name":"Prior clip owner","duration":440,"lanes":[{
                "id":Uuid::new_v4(),"name":"Owner","kind":"cue_list","cue_list_id":list.id,
                "clips":[{"id":Uuid::new_v4(),"start_frame":0,"end_frame":440,
                    "start_cue_id":list.cues[0].id,"end_cue_id":list.cues[0].id,
                    "start_behavior":"state","end_behavior":"release",
                    "cue_starts":[],"in_fade_frames":0,"out_fade_frames":0}]
            }]
        }))
        .unwrap();
    rig.state
        .output
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![operational_fixture(fixture)].into(),
            cue_lists: vec![list.clone()].into(),
            ..EngineSnapshot::default()
        })
        .unwrap();
    rig.state.timecodes.install(definition, None).unwrap();
    rig.state
        .timecodes
        .handle(id, light_playback::TimecodeTransportAction::Go)
        .unwrap();
    rig.state
        .timecodes
        .reconcile_cue_lists(rig.state.output.engine());
    assert!(
        rig.state
            .output
            .engine()
            .playback_runtime_status_for_cue_list(list.id)
            .unwrap()
            .playback
            .enabled
    );
    rig.create_and_open("Clip owner B").await;
    // A new show may legitimately restore this same list UUID. No stale owner may issue Release.
    let mut commands = Vec::new();
    rig.state.timecodes.reconcile_cue_lists_with_executor(
        rig.state.output.engine(),
        |command, _| {
            commands.push(command);
            Err("old show must not issue any playback command".into())
        },
    );
    assert!(
        commands.is_empty(),
        "old applied ownership leaked into destination: {commands:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tl793_real_ws_action_queued_behind_activation_cannot_start_same_id_in_destination() {
    use super::show_activation_cancellation_tests::ArmedProbe;
    let rig = TimecodeScopeRig::new().await;
    let id = Uuid::new_v4();
    let a = rig.create_and_open("WS A").await;
    rig.create_timecode(&a, id, 440).await;
    let b = rig.create_and_open("WS B").await;
    rig.create_timecode(&b, id, 880).await;
    rig.open(&a).await;
    let session = rig
        .state
        .sessions
        .sessions()
        .into_iter()
        .find(|session| session.token == rig.token)
        .unwrap();
    let frame = live_action_frame(
        &session,
        "timecode-during-show-change",
        serde_json::from_value(serde_json::json!({
            "type":"timecode","request":{"timecode_id":id,"action":{"type":"go"}}
        }))
        .unwrap(),
    );
    let before = ArmedProbe::new(rig.state.active_show.activation_before_admission_probe());
    let app = rig.app.clone();
    let request = open_show_request(&rig.token, &b);
    let opening = tokio::spawn(async move { app.oneshot(request).await.unwrap() });
    before.reached().await;
    let action = super::super::ws_dispatch::dispatch_live_action_live(&rig.state, &session, frame);
    tokio::pin!(action);
    let blocked = std::future::poll_fn(|cx| {
        std::task::Poll::Ready(std::future::Future::poll(action.as_mut(), cx).is_pending())
    })
    .await;
    assert!(
        blocked,
        "the actual WebSocket action must wait for activation admission"
    );
    before.release();
    assert_eq!(opening.await.unwrap().status(), StatusCode::OK);
    let response = action.await;
    assert!(!response.ok, "an action queued in A must not run against B");
    assert!(response.error.unwrap().contains("active show changed"));
    let response = rig
        .request(&b, &format!("/api/v2/timecodes/{id}/runtime"), None)
        .await;
    let snapshot = json(response).await;
    assert_eq!(snapshot["duration_frame"], 880);
    assert_eq!(snapshot["state"], "stopped");
    assert_eq!(snapshot["frame"], 0);
    let current_frame = live_action_frame(
        &session,
        "timecode-after-show-change",
        serde_json::from_value(serde_json::json!({
            "type":"timecode","request":{"timecode_id":id,"action":{"type":"go"}}
        }))
        .unwrap(),
    );
    let accepted =
        super::super::ws_dispatch::dispatch_live_action_live(&rig.state, &session, current_frame)
            .await;
    assert!(accepted.ok, "{:?}", accepted.error);
    let current = rig
        .state
        .timecodes
        .snapshot(light_playback::TimecodeId(id))
        .unwrap();
    assert_eq!(current.duration, light_playback::TimecodeFrame(880));
    assert_eq!(
        current.transport,
        light_playback::TimecodeTransportState::Playing
    );
}
