use super::*;

async fn open_show(app: &Router, token: &str, show_id: &str) {
    let response = app
        .clone()
        .oneshot(open_show_request(token, show_id))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

async fn seed_stage_layout(
    state: &AppState,
    token: &str,
    show_id: &str,
    body: &serde_json::Value,
) -> u64 {
    let response = seed_show_object(
        state,
        token,
        show_id,
        "stage_layout",
        "main",
        0,
        body.clone(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    json(response).await["revision"].as_u64().unwrap()
}

async fn read_stage_layout(app: &Router, token: &str, show_id: &str) -> serde_json::Value {
    let response = app
        .clone()
        .oneshot(v2_show_object_get(
            token,
            show_id,
            "stage_layout",
            Some("main"),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    json(response).await["object"].clone()
}

async fn post_stage_layout_action(app: &Router, token: &str, body: &serde_json::Value) -> Response {
    post_stage_layout_action_with_show(app, token, None, body).await
}

async fn post_stage_layout_action_with_show(
    app: &Router,
    token: &str,
    show_id: Option<&str>,
    body: &serde_json::Value,
) -> Response {
    let mut request = Request::post("/api/v2/stage-layout/actions")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(show_id) = show_id {
        request = request.header("x-tosk-show", show_id);
    }
    app.clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
}

fn move_request(
    request_id: &str,
    fixture_ids: &[Uuid],
    axis: &str,
    delta: f64,
) -> serde_json::Value {
    serde_json::json!({
        "request_id": request_id,
        "action": {
            "type": "move_selection",
            "fixture_ids": fixture_ids,
            "axis": axis,
            "delta": delta,
        },
    })
}

#[tokio::test]
async fn move_selection_applies_one_uniform_delta_server_side_in_selection_order() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Stage move").await;
    let show_id = show["id"].as_str().unwrap();
    open_show(&app, &token, show_id).await;

    let (a, b, c, legacy, absent) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    let seeded = seed_stage_layout(
        &state,
        &token,
        show_id,
        &serde_json::json!({
            "version": 2,
            "positions": { legacy.to_string(): {"x": 50.0, "y": 50.0, "rotation": 90.0} },
            "positions3d": {
                a.to_string(): {"x": 1.0, "y": 2.0, "z": 3.0, "rotationX": 0.0, "rotationY": 0.0, "rotationZ": 0.0},
                b.to_string(): {"x": -2.0, "y": 0.5, "z": 5.0, "rotationX": 10.0, "rotationY": 0.0, "rotationZ": 0.0, "future_field": "kept"},
                c.to_string(): {"x": 9.0, "y": 9.0, "z": 9.0, "rotationX": 0.0, "rotationY": 0.0, "rotationZ": 0.0},
            },
            "camera3d": {"position": [0.0, 1.0, 2.0], "target": [0.0, 0.0, 0.0]},
            "future_layout_field": true,
        }),
    )
    .await;
    let application_cursor = state.application_events.latest_sequence();

    let response = post_stage_layout_action(
        &app,
        &token,
        &move_request("move-1", &[a, legacy, b, absent], "x", 1.5),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let outcome = json(response).await;
    assert_eq!(outcome["changed"], true);
    assert_eq!(outcome["replayed"], false);
    assert_eq!(
        outcome["moved_fixture_ids"],
        serde_json::json!([a, legacy, b])
    );
    let revision = outcome["revision"].as_u64().unwrap();
    assert!(revision > seeded);
    assert_eq!(
        state.application_events.latest_sequence(),
        application_cursor + 1,
        "one stage-layout commit publishes one semantic application event"
    );

    let layout = read_stage_layout(&app, &token, show_id).await;
    let positions3d = &layout["body"]["positions3d"];
    assert_eq!(positions3d[a.to_string()]["x"], 2.5);
    assert_eq!(positions3d[a.to_string()]["y"], 2.0);
    assert_eq!(positions3d[b.to_string()]["x"], -0.5);
    assert_eq!(positions3d[b.to_string()]["future_field"], "kept");
    assert_eq!(positions3d[c.to_string()]["x"], 9.0);
    // The legacy 2D entry is migrated with the stage views' percent-to-meter formula before
    // the delta applies; the 2D entry itself stays untouched.
    assert_eq!(positions3d[legacy.to_string()]["x"], 1.5);
    assert_eq!(positions3d[legacy.to_string()]["y"], 4.0);
    assert_eq!(positions3d[legacy.to_string()]["z"], 5.0);
    assert_eq!(positions3d[legacy.to_string()]["rotationZ"], 90.0);
    assert!(positions3d[absent.to_string()].is_null());
    assert_eq!(layout["body"]["positions"][legacy.to_string()]["x"], 50.0);
    assert_eq!(layout["body"]["future_layout_field"], true);
    assert_eq!(layout["body"]["camera3d"]["position"][2], 2.0);

    let light_application::EventReplay::Events(events) = state.application_events.replay(
        application_cursor,
        &light_application::EventFilter::default()
            .with_capability(light_application::EventCapability::Show),
    ) else {
        panic!("stage event cursor must remain replayable");
    };
    let observed_change = events.iter().any(|event| {
        let light_application::ApplicationEvent::Show(
            light_application::ShowEvent::ObjectsChanged(change),
        ) = &event.payload
        else {
            return false;
        };
        change.changes.iter().any(|object| {
            object.kind == light_application::ActiveShowObjectKind::StageLayout
                && object.object_id == "main"
                && object.object_revision == revision
        })
    });
    assert!(
        observed_change,
        "stage move must emit one typed Stage Layout object change"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn move_selection_replays_on_request_id_and_rejects_reuse_for_a_different_action() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Stage replay").await;
    let show_id = show["id"].as_str().unwrap();
    open_show(&app, &token, show_id).await;
    let fixture = Uuid::new_v4();
    seed_stage_layout(
        &state,
        &token,
        show_id,
        &serde_json::json!({
            "version": 2,
            "positions": {},
            "positions3d": { fixture.to_string(): {"x": 1.0, "y": 0.0, "z": 0.0, "rotationX": 0.0, "rotationY": 0.0, "rotationZ": 0.0} },
        }),
    )
    .await;

    let request = move_request("replay-1", &[fixture], "y", 2.0);
    let first = json(post_stage_layout_action(&app, &token, &request).await).await;
    assert_eq!(first["changed"], true);
    let committed_cursor = state.application_events.latest_sequence();
    let replayed = post_stage_layout_action(&app, &token, &request).await;
    assert_eq!(replayed.status(), StatusCode::OK);
    let replayed = json(replayed).await;
    assert_eq!(replayed["replayed"], true);
    assert_eq!(replayed["revision"], first["revision"]);
    assert_eq!(
        state.application_events.latest_sequence(),
        committed_cursor,
        "a replay must not publish another semantic event"
    );
    let layout = read_stage_layout(&app, &token, show_id).await;
    assert_eq!(
        layout["body"]["positions3d"][fixture.to_string()]["y"],
        2.0,
        "a replay must not re-apply the delta"
    );

    let collision = post_stage_layout_action(
        &app,
        &token,
        &move_request("replay-1", &[fixture], "y", 3.0),
    )
    .await;
    assert_eq!(collision.status(), StatusCode::CONFLICT);
    let collision = json(collision).await;
    assert_eq!(collision["retryable"], false);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn move_selection_validates_requests_and_tolerates_unknown_fields() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let unauthenticated = post_stage_layout_action(
        &app,
        "not-a-token",
        &move_request("auth", &[Uuid::new_v4()], "x", 1.0),
    )
    .await;
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    let (token, _) = login(&app, "Operator").await;
    let inactive = post_stage_layout_action(
        &app,
        &token,
        &move_request("no-show", &[Uuid::new_v4()], "x", 1.0),
    )
    .await;
    assert_eq!(inactive.status(), StatusCode::CONFLICT);

    let show = create_show(&app, &token, "Stage validation").await;
    let show_id = show["id"].as_str().unwrap();
    open_show(&app, &token, show_id).await;
    let fixture = Uuid::new_v4();
    seed_stage_layout(
        &state,
        &token,
        show_id,
        &serde_json::json!({
            "version": 2,
            "positions": {},
            "positions3d": { fixture.to_string(): {"x": 1.0, "y": 0.0, "z": 0.0, "rotationX": 0.0, "rotationY": 0.0, "rotationZ": 0.0} },
        }),
    )
    .await;

    let other_show_id = Uuid::new_v4().to_string();
    let mismatched_show = post_stage_layout_action_with_show(
        &app,
        &token,
        Some(&other_show_id),
        &move_request("wrong-show", &[fixture], "x", 1.0),
    )
    .await;
    assert_eq!(mismatched_show.status(), StatusCode::CONFLICT);

    let malformed_show = post_stage_layout_action_with_show(
        &app,
        &token,
        Some("not-a-uuid"),
        &move_request("malformed-show", &[fixture], "x", 1.0),
    )
    .await;
    assert_eq!(malformed_show.status(), StatusCode::BAD_REQUEST);

    let empty_selection = post_stage_layout_action(
        &app,
        &token,
        &move_request("empty-selection", &[], "x", 1.0),
    )
    .await;
    assert_eq!(empty_selection.status(), StatusCode::BAD_REQUEST);

    let blank_request_id =
        post_stage_layout_action(&app, &token, &move_request("", &[fixture], "x", 1.0)).await;
    assert_eq!(blank_request_id.status(), StatusCode::BAD_REQUEST);

    // Unknown request fields are accepted, never rejected (api-rules §5).
    let mut tolerant = move_request("tolerant", &[fixture], "z", -1.0);
    tolerant["desk_hint"] = serde_json::json!("ignored");
    tolerant["action"]["velocity"] = serde_json::json!(3);
    let accepted = post_stage_layout_action(&app, &token, &tolerant).await;
    assert_eq!(accepted.status(), StatusCode::OK);
    assert_eq!(json(accepted).await["changed"], true);
    let committed_cursor = state.application_events.latest_sequence();

    // A zero delta and a selection without any stored position both change nothing.
    let zero = json(
        post_stage_layout_action(&app, &token, &move_request("zero", &[fixture], "x", 0.0)).await,
    )
    .await;
    assert_eq!(zero["changed"], false);
    assert_eq!(zero["moved_fixture_ids"], serde_json::json!([]));
    let unknown_only = json(
        post_stage_layout_action(
            &app,
            &token,
            &move_request("unknown-only", &[Uuid::new_v4()], "x", 1.0),
        )
        .await,
    )
    .await;
    assert_eq!(unknown_only["changed"], false);
    assert_eq!(
        state.application_events.latest_sequence(),
        committed_cursor,
        "no-change stage actions must not publish semantic events"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn move_selection_defaults_patched_fixtures_without_any_stored_position() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let seeded_path = data_dir.join("seeded-default.show");
    default_show::initialise(&seeded_path).unwrap();
    let upload = app
        .clone()
        .oneshot(show_action_request(
            &token,
            serde_json::json!({
                "type": "create",
                "name": "Seeded stage",
                "data_base64": STANDARD.encode(std::fs::read(&seeded_path).unwrap()),
                "overwrite": false,
            }),
        ))
        .await
        .unwrap();
    assert_eq!(upload.status(), StatusCode::OK);
    let show_id = show_action_result(json(upload).await, "show")["id"]
        .as_str()
        .unwrap()
        .to_owned();
    open_show(&app, &token, &show_id).await;

    let fixtures = app
        .clone()
        .oneshot(v2_show_object_get(
            &token,
            &show_id,
            "patched_fixture",
            None,
        ))
        .await
        .unwrap();
    let fixtures = json(fixtures).await;
    let patched: Vec<Uuid> = fixtures["objects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|object| Uuid::parse_str(object["id"].as_str().unwrap()).unwrap())
        .collect();
    assert!(patched.len() >= 2, "the seeded default show is patched");

    let current = read_stage_layout(&app, &token, &show_id).await;
    let response = seed_show_object(
        &state,
        &token,
        &show_id,
        "stage_layout",
        "main",
        current["revision"].as_u64().unwrap(),
        serde_json::json!({"version": 2, "positions": {}, "positions3d": {}}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    // Selection order [second, first] — each defaults to its authoritative patch-order grid
    // slot (the position every stage surface already displays) before the delta applies.
    let outcome = json(
        post_stage_layout_action(
            &app,
            &token,
            &move_request("default-grid", &[patched[1], patched[0]], "x", 2.0),
        )
        .await,
    )
    .await;
    assert_eq!(outcome["changed"], true);
    assert_eq!(
        outcome["moved_fixture_ids"],
        serde_json::json!([patched[1], patched[0]])
    );
    let layout = read_stage_layout(&app, &token, &show_id).await;
    let positions3d = &layout["body"]["positions3d"];
    assert_eq!(positions3d[patched[0].to_string()]["x"], -3.25);
    assert_eq!(positions3d[patched[1].to_string()]["x"], -1.75);
    assert_eq!(positions3d[patched[0].to_string()]["y"], 1.0);
    assert_eq!(positions3d[patched[1].to_string()]["z"], 5.0);
    assert_eq!(
        positions3d.as_object().unwrap().len(),
        2,
        "only the selected fixtures gain persisted entries"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}
