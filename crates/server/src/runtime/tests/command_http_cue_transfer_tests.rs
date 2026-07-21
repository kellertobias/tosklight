struct CueTransferRouteScenario {
    state: AppState,
    app: Router,
    session: Session,
    token: String,
    show_id: String,
    data_dir: PathBuf,
}

#[derive(Clone, Copy)]
struct PendingTransfer {
    choice_id: Uuid,
    show_revision: u64,
    command_revision: u64,
}

impl CueTransferRouteScenario {
    fn new() -> Self {
        let fixture = CueTransferScenario::new();
        let show_id = fixture
            .state
            .active_show
            .read()
            .as_ref()
            .unwrap()
            .id
            .0
            .to_string();
        fixture
            .state
            .sessions
            .write()
            .insert(fixture.session.id, fixture.session.clone());
        Self {
            app: router(fixture.state.clone()),
            state: fixture.state,
            token: fixture.session.token.clone(),
            session: fixture.session,
            show_id,
            data_dir: fixture.data_dir,
        }
    }

    async fn open_copy_choice(&self) -> PendingTransfer {
        let response = self
            .execute("open-transfer-choice", "COPY SET 1 CUE 2 AT SET 2 CUE 2")
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = json(response).await;
        assert_eq!(body["outcome"], "choice_required");
        PendingTransfer {
            choice_id: serde_json::from_value(body["pending_choice"]["choice_id"].clone()).unwrap(),
            show_revision: body["pending_choice"]["show_revision"].as_u64().unwrap(),
            command_revision: body["command_line"]["revision"].as_u64().unwrap(),
        }
    }

    async fn execute(&self, request_id: &str, command: &str) -> Response {
        self.app
            .clone()
            .oneshot(
                Request::post(format!(
                    "/api/v2/desks/{}/command-line/execute",
                    self.session.desk.id
                ))
                .header(header::AUTHORIZATION, format!("Bearer {}", self.token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({"request_id":request_id,"command":command}).to_string(),
                ))
                .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn replace_command_line(&self, text: &str, expected_revision: u64) -> Response {
        self.app
            .clone()
            .oneshot(
                Request::put(format!(
                    "/api/v2/desks/{}/command-line",
                    self.session.desk.id
                ))
                .header(header::AUTHORIZATION, format!("Bearer {}", self.token))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::IF_MATCH, expected_revision.to_string())
                .body(Body::from(serde_json::json!({"text":text}).to_string()))
                .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn transfer(
        &self,
        show_id: &str,
        token: Option<&str>,
        expected_show_revision: Option<u64>,
        body: serde_json::Value,
    ) -> Response {
        let mut request = Request::post(format!("/api/v2/shows/{show_id}/cues/transfer"));
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        if let Some(revision) = expected_show_revision {
            request = request.header(header::IF_MATCH, revision.to_string());
        }
        self.app
            .clone()
            .oneshot(
                request
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    fn request(&self, request_id: &str, pending: PendingTransfer) -> serde_json::Value {
        serde_json::json!({
            "request_id":request_id,
            "choice_id":pending.choice_id,
            "mode":"plain",
            "expected_command_line_revision":pending.command_revision,
        })
    }

    fn compatibility_count(&self) -> usize {
        self.state
            .audit_events
            .lock()
            .iter()
            .filter(|event| event.kind == "show_object_changed")
            .count()
    }
}

#[tokio::test]
async fn legacy_command_transfer_emits_only_its_temporary_per_object_v1_notification() {
    let scenario = CueTransferRouteScenario::new();
    let compatibility = scenario.compatibility_count();
    let response = scenario
        .execute(
            "legacy-transfer",
            "COPY PLAIN SET 1 CUE 2 AT SET 2 CUE 2",
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json(response).await["outcome"], "accepted");
    assert_eq!(scenario.compatibility_count(), compatibility + 1);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[test]
fn non_set_copy_and_move_remain_owned_by_legacy_preset_mutation() {
    let scenario = CueTransferRouteScenario::new();
    let show_path = scenario
        .state
        .active_show
        .read()
        .as_ref()
        .unwrap()
        .path
        .clone();
    let store = ShowStore::open(&show_path).unwrap();
    let preset = serde_json::to_value(light_programmer::Preset {
        name: "Legacy color".into(),
        family: light_programmer::PresetFamily::Color,
        number: 1,
        ..Default::default()
    })
    .unwrap();
    store.put_object("preset", "2.1", &preset, 0).unwrap();

    let dispatch = |request_id: &str, value: &str| {
        dispatch_ws_command(
            &scenario.state,
            &scenario.session,
            WsCommand {
                protocol_version: 1,
                request_id: request_id.into(),
                session_id: scenario.session.id,
                expected_revision: None,
                command: "programmer.execute".into(),
                payload: serde_json::json!({"value":value}),
            },
        )
    };
    let copied = dispatch("legacy-preset-copy", "COPY 2.1 AT 2");
    assert!(copied.ok, "Preset Copy failed: {:?}", copied.error);
    let moved = dispatch("legacy-preset-move", "MOVE 2.2 AT 3");
    assert!(moved.ok, "Preset Move failed: {:?}", moved.error);

    let ids = ShowStore::open(&show_path)
        .unwrap()
        .objects("preset")
        .unwrap()
        .into_iter()
        .map(|object| object.id)
        .collect::<Vec<_>>();
    assert!(ids.iter().any(|id| id == "2.1"));
    assert!(!ids.iter().any(|id| id == "2.2"));
    assert!(ids.iter().any(|id| id == "2.3"));
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn command_line_choice_replay_uses_current_authority_without_resurrecting_choice() {
    let scenario = CueTransferRouteScenario::new();
    let command = "COPY SET 1 CUE 2 AT SET 2 CUE 2";
    let original = scenario.execute("replayable-choice", command).await;
    assert_eq!(original.status(), StatusCode::OK);
    let original = json(original).await;
    assert_eq!(original["outcome"], "choice_required");
    let choice_id = original["pending_choice"]["choice_id"].clone();
    let command_revision = original["command_line"]["revision"].as_u64().unwrap();

    let cancelled = scenario.replace_command_line("", command_revision).await;
    assert_eq!(cancelled.status(), StatusCode::OK);
    let cancelled = json(cancelled).await;
    assert!(cancelled["pending_choice"].is_null());
    let sequence = scenario.state.application_events.latest_sequence();

    let replay = scenario.execute("replayable-choice", command).await;
    assert_eq!(replay.status(), StatusCode::OK);
    let replay = json(replay).await;
    assert_eq!(replay["outcome"], "choice_required");
    assert_eq!(replay["pending_choice"]["choice_id"], choice_id);
    assert_eq!(replay["command_line"], cancelled);
    assert!(replay["command_line"]["pending_choice"].is_null());
    assert!(
        scenario
            .state
            .programmers
            .command_line_state(scenario.session.id)
            .unwrap()
            .pending_choice
            .is_none()
    );
    assert_eq!(scenario.state.application_events.latest_sequence(), sequence);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn cue_transfer_route_returns_one_authoritative_batch_and_replays_without_side_effects() {
    let scenario = CueTransferRouteScenario::new();
    let pending = scenario.open_copy_choice().await;
    let baseline = scenario.state.application_events.latest_sequence();
    let compatibility = scenario.compatibility_count();
    let request = scenario.request("typed-transfer", pending);

    let response = scenario
        .transfer(
            &scenario.show_id,
            Some(&scenario.token),
            Some(pending.show_revision),
            request.clone(),
        )
        .await;
    if response.status() != StatusCode::OK {
        panic!("Cue transfer failed: {}", json(response).await);
    }
    assert_eq!(
        response.headers()[header::ETAG],
        format!("\"{}\"", pending.show_revision + 1)
    );
    let outcome: light_wire::v2::cue_transfer::CueTransferOutcome =
        serde_json::from_value(json(response).await).unwrap();
    let light_wire::v2::cue_transfer::CueTransferOutcome::Changed {
        request_id,
        replayed,
        show_id,
        choice_id,
        summary,
        show_revision,
        projections,
        show_event_sequence,
        command_line,
        interaction_event_sequence,
        persistence_warning,
        ..
    } = &outcome;
    assert_eq!(request_id, "typed-transfer");
    assert!(!replayed);
    assert_eq!(show_id.to_string(), scenario.show_id);
    assert_eq!(*choice_id, pending.choice_id);
    assert_eq!(summary.operation, light_wire::v2::command_line::CueTransferOperation::Copy);
    assert_eq!(summary.mode, light_wire::v2::cue_transfer::CueTransferMode::Plain);
    assert_eq!(*show_revision, pending.show_revision + 1);
    assert_eq!(projections.len(), 1, "plain Copy changes only the destination Cuelist");
    assert_eq!(projections[0].body["future_cuelist_metadata"]["list"], 1);
    assert_eq!(*show_event_sequence, baseline + 1);
    assert_eq!(*interaction_event_sequence, Some(baseline + 2));
    assert!(command_line.pristine);
    assert!(command_line.pending_choice.is_none());
    assert!(persistence_warning.is_none());
    assert_eq!(scenario.state.application_events.latest_sequence(), baseline + 2);
    assert_eq!(scenario.compatibility_count(), compatibility);
    assert_one_cue_transfer_show_event(&scenario.state, baseline);

    let replay = scenario
        .transfer(
            &scenario.show_id,
            Some(&scenario.token),
            Some(pending.show_revision),
            request.clone(),
        )
        .await;
    let replay: light_wire::v2::cue_transfer::CueTransferOutcome =
        serde_json::from_value(json(replay).await).unwrap();
    assert!(matches!(
        replay,
        light_wire::v2::cue_transfer::CueTransferOutcome::Changed { replayed: true, .. }
    ));
    assert_eq!(scenario.state.application_events.latest_sequence(), baseline + 2);
    assert_eq!(scenario.compatibility_count(), compatibility);

    let mut changed_replay = request;
    changed_replay["mode"] = "status".into();
    assert_eq!(
        scenario
            .transfer(
                &scenario.show_id,
                Some(&scenario.token),
                Some(pending.show_revision),
                changed_replay,
            )
            .await
            .status(),
        StatusCode::CONFLICT
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn cue_transfer_route_rejects_forged_scope_and_reports_both_revision_authorities() {
    let scenario = CueTransferRouteScenario::new();
    let pending = scenario.open_copy_choice().await;
    let request = scenario.request("secure-transfer", pending);

    assert_eq!(
        scenario
            .transfer(&scenario.show_id, None, Some(pending.show_revision), request.clone())
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        scenario
            .transfer(&scenario.show_id, Some(&scenario.token), None, request.clone())
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    let mut forged = request.clone();
    forged["user_id"] = serde_json::json!(scenario.session.user.id.0);
    assert_eq!(
        scenario
            .transfer(
                &scenario.show_id,
                Some(&scenario.token),
                Some(pending.show_revision),
                forged,
            )
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );

    let mut stale_command = request.clone();
    stale_command["expected_command_line_revision"] = (pending.command_revision - 1).into();
    let response = scenario
        .transfer(
            &scenario.show_id,
            Some(&scenario.token),
            Some(pending.show_revision),
            stale_command,
        )
        .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert!(response.headers().get(header::ETAG).is_none());
    assert_eq!(
        json(response).await["current_related_revision"],
        pending.command_revision
    );

    let response = scenario
        .transfer(
            &scenario.show_id,
            Some(&scenario.token),
            Some(pending.show_revision - 1),
            request.clone(),
        )
        .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        response.headers()[header::ETAG],
        format!("\"{}\"", pending.show_revision)
    );
    assert_eq!(json(response).await["current_revision"], pending.show_revision);

    assert_eq!(
        scenario
            .transfer(
                &Uuid::new_v4().to_string(),
                Some(&scenario.token),
                Some(pending.show_revision),
                request,
            )
            .await
            .status(),
        StatusCode::CONFLICT
    );
    assert_exact_cue_transfer_authority(&scenario);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

fn assert_one_cue_transfer_show_event(state: &AppState, baseline: u64) {
    let light_application::EventReplay::Events(events) = state
        .application_events
        .replay(baseline, &light_application::EventFilter::default())
    else {
        panic!("expected retained Cue transfer events")
    };
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(
                event.payload,
                light_application::ApplicationEvent::Show(
                    light_application::ShowEvent::ObjectsChanged(_)
                )
            ))
            .count(),
        1
    );
}

fn assert_exact_cue_transfer_authority(scenario: &CueTransferRouteScenario) {
    let ports = command_http::ServerProgrammingCueTransferPorts::new(
        scenario.state.clone(),
        scenario.session.clone(),
        true,
    );
    let valid = light_application::ActionContext::operator(
        scenario.session.desk.id,
        scenario.session.user.id.0,
        scenario.session.id.0,
        light_application::ActionSource::Http,
    );
    for (forged, expected) in [
        (
            light_application::ActionContext { user_id: Some(Uuid::new_v4()), ..valid.clone() },
            light_application::ActionErrorKind::Forbidden,
        ),
        (
            light_application::ActionContext { session_id: Some(Uuid::new_v4()), ..valid.clone() },
            light_application::ActionErrorKind::Unauthorized,
        ),
        (
            light_application::ActionContext { desk_id: Uuid::new_v4(), ..valid },
            light_application::ActionErrorKind::Forbidden,
        ),
    ] {
        let error = light_application::ProgrammingCueTransferPorts::authorize_cue_transfer(
            &ports, &forged,
        )
        .unwrap_err();
        assert_eq!(error.kind, expected);
    }
}
