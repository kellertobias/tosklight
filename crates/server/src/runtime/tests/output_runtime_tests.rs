use super::*;

#[tokio::test]
async fn legacy_master_update_publishes_one_typed_change_and_v2_repairs_it() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = authenticate_token(&state, &token).unwrap();
    let show = create_show(&app, &token, "Output runtime show").await;
    open_show_for_output_test(&app, &token, &show).await;

    let denied = app
        .clone()
        .oneshot(
            Request::get(format!(
                "/api/v2/desks/{}/output-runtime/global-master",
                session.desk.id
            ))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);

    let cursor = state.application_events.latest_sequence();
    let persistence_attempts = output_persistence_attempts(&state);
    let response = put_master(
        &app,
        &token,
        serde_json::json!({"grand_master":0.4,"blackout":true}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = json(response).await;
    assert_eq!(body["blackout"], true);
    assert!((body["grand_master"].as_f64().unwrap() - 0.4).abs() < 0.000_001);
    assert_eq!(state.output_control.lock().revision, 1);
    assert_eq!(
        output_persistence_attempts(&state),
        persistence_attempts + 1
    );

    let light_application::EventReplay::Events(events) = state.application_events.replay(
        cursor,
        &light_application::EventFilter::default()
            .with_object(light_application::EventObject::global_output()),
    ) else {
        panic!("global-output change should be retained");
    };
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].desk_id, None);
    assert_eq!(
        events[0].source,
        light_application::EventSource::Action(light_application::ActionSource::Http)
    );
    assert!(events[0].correlation_id.is_some());
    let light_application::ApplicationEvent::Output(
        light_application::OutputEvent::RuntimeChanged(change),
    ) = &events[0].payload
    else {
        panic!("expected output-runtime change");
    };
    assert_eq!(change.projection.revision, 1);

    let no_change = put_master(
        &app,
        &token,
        serde_json::json!({"grand_master":0.4,"blackout":true}),
    )
    .await;
    assert_eq!(no_change.status(), StatusCode::OK);
    assert_eq!(state.application_events.latest_sequence(), cursor + 1);
    assert_eq!(
        output_persistence_attempts(&state),
        persistence_attempts + 1
    );

    let snapshot = app
        .clone()
        .oneshot(
            Request::get(format!(
                "/api/v2/desks/{}/output-runtime/global-master",
                session.desk.id
            ))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(snapshot.status(), StatusCode::OK);
    let snapshot = json(snapshot).await;
    assert_eq!(snapshot["cursor"]["sequence"], cursor + 1);
    assert_eq!(snapshot["projection"]["identity"], "global_master");
    assert_eq!(snapshot["projection"]["revision"], 1);
    assert!((snapshot["projection"]["grand_master"].as_f64().unwrap() - 0.4).abs() < 0.000_001);
    assert_eq!(snapshot["projection"]["blackout"], true);
    assert_eq!(snapshot["projection"]["scope"]["show_id"], show["id"]);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn websocket_master_retry_is_idempotent_at_the_typed_boundary() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = authenticate_token(&state, &token).unwrap();
    let command = || WsCommand {
        protocol_version: 1,
        request_id: "global-master-1".into(),
        session_id: session.id,
        expected_revision: None,
        command: "master.set".into(),
        payload: serde_json::json!({"grand_master":0.25}),
    };

    let cursor = state.application_events.latest_sequence();
    let first = dispatch_ws_command(&state, &session, command());
    let replay = dispatch_ws_command(&state, &session, command());
    assert!(first.ok, "{:?}", first.error);
    assert!(replay.ok, "{:?}", replay.error);
    assert_eq!(first.payload, replay.payload);
    assert_eq!(state.output_control.lock().options.grand_master, 0.25);
    assert_eq!(state.application_events.latest_sequence(), cursor + 1);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_output_action_is_atomic_revisioned_idempotent_and_strict() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = authenticate_token(&state, &token).unwrap();
    let show = create_show(&app, &token, "Revisioned output").await;
    open_show_for_output_test(&app, &token, &show).await;
    let initial = output_snapshot(&app, &token, session.desk.id).await;
    let cursor = state.application_events.latest_sequence();
    let attempts = output_persistence_attempts(&state);
    let request = output_request("output-combined", &initial, Some(0.45), Some(true));

    let changed = post_output(&app, &token, session.desk.id, request.clone()).await;
    assert_eq!(changed.status(), StatusCode::OK);
    let changed = json(changed).await;
    assert_eq!(changed["request_id"], "output-combined");
    assert!(Uuid::parse_str(changed["correlation_id"].as_str().unwrap()).is_ok());
    assert_eq!(changed["status"], "changed");
    assert_eq!(changed["projection"]["revision"], 1);
    assert_eq!(changed["projection"]["grand_master"], 0.45);
    assert_eq!(changed["projection"]["blackout"], true);
    assert_eq!(changed["replayed"], false);
    assert_eq!(changed["durability"], "durable");
    assert!(changed.get("warning").is_none());
    assert_eq!(output_persistence_attempts(&state), attempts + 1);
    let events = output_events(&state, cursor);
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].sequence,
        changed["event_sequence"].as_u64().unwrap()
    );
    assert_eq!(
        events[0].correlation_id.unwrap().to_string(),
        changed["correlation_id"].as_str().unwrap()
    );

    let replay = post_output(&app, &token, session.desk.id, request.clone()).await;
    assert_eq!(replay.status(), StatusCode::OK);
    let replay = json(replay).await;
    assert_eq!(replay["status"], "changed");
    assert_eq!(replay["event_sequence"], changed["event_sequence"]);
    assert_eq!(replay["correlation_id"], changed["correlation_id"]);
    assert_eq!(replay["replayed"], true);
    assert_eq!(output_persistence_attempts(&state), attempts + 1);
    assert_eq!(output_events(&state, cursor).len(), 1);

    let mut collision_request = request.clone();
    collision_request["grand_master"] = serde_json::json!(0.6);
    let collision = post_output(&app, &token, session.desk.id, collision_request).await;
    assert_eq!(collision.status(), StatusCode::CONFLICT);
    let collision = json(collision).await;
    assert_eq!(collision["kind"], "conflict");
    assert_eq!(collision["current_revision"], 1);

    let current = output_snapshot(&app, &token, session.desk.id).await;
    let no_change = post_output(
        &app,
        &token,
        session.desk.id,
        output_request("output-no-change", &current, Some(0.45), Some(true)),
    )
    .await;
    assert_eq!(no_change.status(), StatusCode::OK);
    let no_change = json(no_change).await;
    assert_eq!(no_change["status"], "no_change");
    assert!(no_change.get("event_sequence").is_none());
    assert_eq!(no_change["projection"]["revision"], 1);
    assert_eq!(output_persistence_attempts(&state), attempts + 1);
    assert_eq!(output_events(&state, cursor).len(), 1);

    let conflict = post_output(
        &app,
        &token,
        session.desk.id,
        output_request("output-stale", &initial, Some(0.2), None),
    )
    .await;
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
    let conflict = json(conflict).await;
    assert_eq!(conflict["kind"], "conflict");
    assert_eq!(conflict["current_revision"], 1);

    let empty = post_output(
        &app,
        &token,
        session.desk.id,
        output_request("output-empty", &current, None, None),
    )
    .await;
    assert_eq!(empty.status(), StatusCode::BAD_REQUEST);
    assert_eq!(json(empty).await["kind"], "invalid");
    let mut unknown_request = output_request("output-unknown", &current, Some(0.2), None);
    unknown_request["unexpected"] = serde_json::json!(true);
    let unknown = post_output(&app, &token, session.desk.id, unknown_request).await;
    assert_eq!(unknown.status(), StatusCode::BAD_REQUEST);
    assert_eq!(json(unknown).await["kind"], "invalid");
    assert_eq!(output_persistence_attempts(&state), attempts + 1);
    assert_eq!(output_events(&state, cursor).len(), 1);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn v2_output_is_shared_across_desks_but_enforces_exact_desk_and_lock() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (front_token, _) = login(&app, "Operator").await;
    let front = authenticate_token(&state, &front_token).unwrap();
    let show = create_show(&app, &front_token, "Shared output").await;
    open_show_for_output_test(&app, &front_token, &show).await;
    let wing = state.desk.lock().add_desk("Wing", "wing").unwrap();
    let wing_token = login_for_desk(&app, "Operator", wing.id).await;
    let initial = output_snapshot(&app, &wing_token, wing.id).await;

    let changed = post_output(
        &app,
        &wing_token,
        wing.id,
        output_request("wing-output", &initial, Some(0.7), None),
    )
    .await;
    assert_eq!(changed.status(), StatusCode::OK);
    let front_projection = output_snapshot(&app, &front_token, front.desk.id).await;
    assert_eq!(front_projection["projection"]["revision"], 1);
    assert_eq!(front_projection["projection"]["grand_master"], 0.7);

    let foreign_action = post_output(
        &app,
        &front_token,
        wing.id,
        output_request("foreign-output", &front_projection, None, Some(true)),
    )
    .await;
    assert_eq!(foreign_action.status(), StatusCode::FORBIDDEN);
    assert_eq!(json(foreign_action).await["kind"], "forbidden");
    let foreign_snapshot = get_output(&app, &front_token, wing.id).await;
    assert_eq!(foreign_snapshot.status(), StatusCode::FORBIDDEN);

    write_desk_lock(
        &state,
        wing.id,
        &DeskLockConfiguration {
            locked: true,
            ..DeskLockConfiguration::default()
        },
    )
    .unwrap();
    let cursor = state.application_events.latest_sequence();
    let attempts = output_persistence_attempts(&state);
    let locked = post_output(
        &app,
        &wing_token,
        wing.id,
        output_request("locked-output", &front_projection, None, Some(true)),
    )
    .await;
    assert_eq!(locked.status(), StatusCode::CONFLICT);
    let locked = json(locked).await;
    assert_eq!(locked["kind"], "conflict");
    assert_eq!(locked["current_revision"], 1);
    assert_eq!(output_events(&state, cursor).len(), 0);
    assert_eq!(output_persistence_attempts(&state), attempts);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn output_revision_ignores_engine_and_show_revisions_and_replaces_with_show_scope() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = authenticate_token(&state, &token).unwrap();
    let show_a = create_show(&app, &token, "Output A").await;
    let show_b = create_show(&app, &token, "Output B").await;
    open_show_for_output_test(&app, &token, &show_a).await;
    let a_initial = output_snapshot(&app, &token, session.desk.id).await;

    let mut engine = (*state.engine.snapshot()).clone();
    engine.revision = 9_001;
    state.engine.replace_snapshot(engine).unwrap();
    state.active_show.write().as_mut().unwrap().revision += 17;
    let a_changed = post_output(
        &app,
        &token,
        session.desk.id,
        output_request("show-a-output", &a_initial, Some(0.35), None),
    )
    .await;
    assert_eq!(a_changed.status(), StatusCode::OK);
    assert_eq!(json(a_changed).await["projection"]["revision"], 1);

    open_show_for_output_test(&app, &token, &show_b).await;
    let b_initial = output_snapshot(&app, &token, session.desk.id).await;
    assert_eq!(b_initial["projection"]["revision"], 0);
    assert_eq!(b_initial["projection"]["grand_master"], 1.0);
    let old_scope = post_output(
        &app,
        &token,
        session.desk.id,
        output_request("stale-show-a", &a_initial, Some(0.2), None),
    )
    .await;
    assert_eq!(old_scope.status(), StatusCode::CONFLICT);
    let old_scope = json(old_scope).await;
    assert_eq!(old_scope["current_revision"], 0);

    let b_changed = post_output(
        &app,
        &token,
        session.desk.id,
        output_request("show-b-output", &b_initial, None, Some(true)),
    )
    .await;
    assert_eq!(b_changed.status(), StatusCode::OK);
    open_show_for_output_test(&app, &token, &show_a).await;
    let a_restored = output_snapshot(&app, &token, session.desk.id).await;
    assert_eq!(a_restored["projection"]["scope"]["show_id"], show_a["id"]);
    assert_eq!(a_restored["projection"]["revision"], 1);
    assert_eq!(a_restored["projection"]["grand_master"], 0.35);
    assert_eq!(a_restored["projection"]["blackout"], false);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn output_persistence_failure_is_visible_and_replay_does_not_retry_it() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = authenticate_token(&state, &token).unwrap();
    let show = create_show(&app, &token, "Pending output").await;
    open_show_for_output_test(&app, &token, &show).await;
    let initial = output_snapshot(&app, &token, session.desk.id).await;
    state
        .output_runtime_persistence_failure
        .store(true, Ordering::Relaxed);
    let cursor = state.application_events.latest_sequence();
    let attempts = output_persistence_attempts(&state);
    let request = output_request("pending-output", &initial, Some(0.5), Some(true));

    let changed = post_output(&app, &token, session.desk.id, request.clone()).await;
    assert_eq!(changed.status(), StatusCode::OK);
    let changed = json(changed).await;
    assert_eq!(changed["status"], "changed");
    assert_eq!(changed["projection"]["revision"], 1);
    assert_eq!(changed["durability"], "persistence_pending");
    assert!(
        changed["warning"]
            .as_str()
            .unwrap()
            .contains("persistence is pending")
    );
    assert_eq!(output_persistence_attempts(&state), attempts + 1);
    assert_eq!(output_events(&state, cursor).len(), 1);

    let replay = post_output(&app, &token, session.desk.id, request).await;
    assert_eq!(replay.status(), StatusCode::OK);
    let replay = json(replay).await;
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["durability"], "persistence_pending");
    assert_eq!(replay["warning"], changed["warning"]);
    assert_eq!(output_persistence_attempts(&state), attempts + 1);
    assert_eq!(output_events(&state, cursor).len(), 1);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn legacy_persisted_output_without_revision_activates_at_revision_zero() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let session = authenticate_token(&state, &token).unwrap();
    let show = create_show(&app, &token, "Legacy output").await;
    let show_id = light_core::ShowId(Uuid::parse_str(show["id"].as_str().unwrap()).unwrap());
    state
        .desk
        .lock()
        .set_setting(
            &output_runtime_setting(show_id),
            r#"{"grand_master":0.65,"blackout":true,"dynamics_paused_at":null,"group_masters":{}}"#,
        )
        .unwrap();

    open_show_for_output_test(&app, &token, &show).await;
    let projection = output_snapshot(&app, &token, session.desk.id).await;
    assert_eq!(projection["projection"]["revision"], 0);
    assert_eq!(projection["projection"]["grand_master"], 0.65);
    assert_eq!(projection["projection"]["blackout"], true);
    let _ = std::fs::remove_dir_all(data_dir);
}

fn output_request(
    request_id: &str,
    snapshot: &serde_json::Value,
    grand_master: Option<f32>,
    blackout: Option<bool>,
) -> serde_json::Value {
    serde_json::json!({
        "request_id":request_id,
        "expected_show_id":snapshot["projection"]["scope"]["show_id"],
        "expected_revision":snapshot["projection"]["revision"],
        "grand_master":grand_master,
        "blackout":blackout,
    })
}

async fn get_output(app: &Router, token: &str, desk_id: Uuid) -> Response {
    app.clone()
        .oneshot(
            Request::get(format!(
                "/api/v2/desks/{desk_id}/output-runtime/global-master"
            ))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap()
}

async fn output_snapshot(app: &Router, token: &str, desk_id: Uuid) -> serde_json::Value {
    let response = get_output(app, token, desk_id).await;
    assert_eq!(response.status(), StatusCode::OK);
    json(response).await
}

async fn post_output(
    app: &Router,
    token: &str,
    desk_id: Uuid,
    payload: serde_json::Value,
) -> Response {
    app.clone()
        .oneshot(
            Request::post(format!(
                "/api/v2/desks/{desk_id}/output-runtime/global-master"
            ))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(payload.to_string()))
            .unwrap(),
        )
        .await
        .unwrap()
}

async fn login_for_desk(app: &Router, username: &str, desk_id: Uuid) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v1/sessions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({"username":username,"desk_id":desk_id}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    json(response).await["token"].as_str().unwrap().to_owned()
}

fn output_events(state: &AppState, cursor: u64) -> Vec<Arc<light_application::EventEnvelope>> {
    let light_application::EventReplay::Events(events) = state.application_events.replay(
        cursor,
        &light_application::EventFilter::default()
            .with_object(light_application::EventObject::global_output()),
    ) else {
        panic!("output event history should be retained");
    };
    events
}

fn output_persistence_attempts(state: &AppState) -> u64 {
    state
        .output_runtime_persistence_attempts
        .load(Ordering::Relaxed)
}

async fn put_master(app: &Router, token: &str, payload: serde_json::Value) -> Response {
    app.clone()
        .oneshot(
            Request::put("/api/v1/master")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn open_show_for_output_test(app: &Router, token: &str, show: &serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::post(format!(
                "/api/v1/shows/{}/open",
                show["id"].as_str().unwrap()
            ))
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"transition":"hold_current"}"#))
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
