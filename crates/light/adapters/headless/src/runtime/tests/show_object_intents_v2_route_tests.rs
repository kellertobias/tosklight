use super::*;

async fn post_show_object_intent(
    app: &Router,
    token: &str,
    show_id: &str,
    path: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(
            Request::post(path)
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header("x-tosk-show", show_id)
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    (status, json(response).await)
}

async fn create_seeded_show(
    state: &AppState,
    app: &Router,
    token: &str,
    name: &str,
    seeds: &[(&str, &str, serde_json::Value)],
) -> String {
    let show = create_show(app, token, name).await;
    let show_id = show["id"].as_str().unwrap().to_owned();
    let entry = state
        .desk
        .lock()
        .show(light_core::ShowId(Uuid::parse_str(&show_id).unwrap()))
        .unwrap()
        .unwrap();
    let store = ShowStore::open(&entry.path).unwrap();
    for (kind, id, body) in seeds {
        store.put_object(kind, id, body, 0).unwrap();
    }
    let opened = app
        .clone()
        .oneshot(open_show_request(token, &show_id))
        .await
        .unwrap();
    assert_eq!(opened.status(), StatusCode::OK);
    show_id
}

#[tokio::test]
async fn layout_and_patch_intents_preserve_unknown_fields_and_replay_once() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let user_id = state
        .sessions
        .read()
        .values()
        .next()
        .unwrap()
        .user
        .id
        .0
        .to_string();
    let show_id = create_seeded_show(
        &state,
        &app,
        &token,
        "Typed layout and patch",
        &[
            (
                "user_layout",
                &user_id,
                serde_json::json!({
                    "desks": [],
                    "activeDeskId": "old",
                    "future_layout_field": {"kept": true}
                }),
            ),
            (
                "patch_layer",
                "front",
                serde_json::json!({
                    "id": "front",
                    "name": "Old",
                    "order": 1,
                    "future_patch_field": "kept"
                }),
            ),
        ],
    )
    .await;

    let layout = serde_json::json!({
        "request_id": "save-layout-1",
        "action": {
            "type": "update",
            "expected_revision": 1,
            "patch": {
                "desks": [{"id": "main"}],
                "active_desk_id": "main"
            }
        }
    });
    let before_events = state.facade_events.latest_sequence();
    let (status, saved_layout) = post_show_object_intent(
        &app,
        &token,
        &show_id,
        &format!("/api/v2/user-layouts/{user_id}/update"),
        layout.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved_layout["object"]["revision"], 2);
    assert_eq!(
        saved_layout["object"]["body"]["future_layout_field"]["kept"],
        true
    );
    assert_eq!(saved_layout["object"]["body"]["activeDeskId"], "main");

    let (status, replayed) = post_show_object_intent(
        &app,
        &token,
        &show_id,
        &format!("/api/v2/user-layouts/{user_id}/update"),
        layout,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replayed["replayed"], true);
    assert_eq!(replayed["object"], saved_layout["object"]);
    assert_eq!(state.facade_events.latest_sequence(), before_events + 1);

    let (status, saved_layer) = post_show_object_intent(
        &app,
        &token,
        &show_id,
        "/api/v2/patch/layers/front/update",
        serde_json::json!({
            "request_id": "save-layer-1",
            "action": {
                "type": "save",
                "expected_revision": 1,
                "layer": {"name": "Front truss", "order": 2}
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved_layer["object"]["revision"], 2);
    assert_eq!(saved_layer["object"]["body"]["name"], "Front truss");
    assert_eq!(saved_layer["object"]["body"]["future_patch_field"], "kept");

    let (status, stale) = post_show_object_intent(
        &app,
        &token,
        &show_id,
        "/api/v2/patch/layers/front/update",
        serde_json::json!({
            "request_id": "stale-layer",
            "action": {
                "type": "save",
                "expected_revision": 1,
                "layer": {"name": "Stale", "order": 3}
            }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(stale["error"].as_str().unwrap().contains("revision"));

    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn dynamic_intent_appends_to_the_existing_cue_without_replacing_it() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let cue_list = test_cue_list();
    let cue_list_id = cue_list.id.0.to_string();
    let show_id = create_seeded_show(
        &state,
        &app,
        &token,
        "Typed dynamics",
        &[(
            "cue_list",
            &cue_list_id,
            serde_json::to_value(&cue_list).unwrap(),
        )],
    )
    .await;
    let fixture_id = Uuid::new_v4();
    let action = serde_json::json!({
        "request_id": "record-dynamic-1",
        "action": {
            "type": "append",
            "expected_revision": 1,
            "speed": 60,
            "width": 25,
            "direction": "reverse",
            "fixture_ids": [fixture_id],
            "group_ids": []
        }
    });

    let (status, stored) = post_show_object_intent(
        &app,
        &token,
        &show_id,
        &format!("/api/v2/cue-lists/{cue_list_id}/dynamics/record"),
        action.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(stored["object"]["revision"], 2);
    assert_eq!(stored["object"]["body"]["name"], "Main");
    let phaser = &stored["object"]["body"]["cues"][0]["phasers"][0];
    assert_eq!(phaser["fixture_ids"][0], fixture_id.to_string());
    assert_eq!(phaser["phaser"]["cycles_per_minute"], 60.0);
    assert_eq!(phaser["phaser"]["phase_start_degrees"], 360.0);
    assert_eq!(phaser["phaser"]["phase_end_degrees"], 0.0);
    assert_eq!(phaser["phaser"]["width"], 0.25);

    let (status, replayed) = post_show_object_intent(
        &app,
        &token,
        &show_id,
        &format!("/api/v2/cue-lists/{cue_list_id}/dynamics/record"),
        action,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replayed["replayed"], true);
    assert_eq!(
        replayed["object"]["body"]["cues"][0]["phasers"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn preload_record_intent_stores_the_pending_scene_and_replays_once() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, session_id) = login(&app, "Operator").await;
    let show_id = create_seeded_show(&state, &app, &token, "Typed preload", &[]).await;
    let session_id = light_core::SessionId(Uuid::parse_str(&session_id).unwrap());
    assert!(state.programmers.set_preload_group(
        session_id,
        "front".into(),
        light_core::AttributeKey::intensity(),
        light_core::AttributeValue::Normalized(0.6),
    ));
    let action = serde_json::json!({
        "request_id": "record-preload-1",
        "action": {
            "type": "preset",
            "target_id": "1.1",
            "expected_revision": 0,
            "name": "Front at sixty",
            "mode": "merge",
            "family": "intensity"
        }
    });

    let (status, stored) = post_show_object_intent(
        &app,
        &token,
        &show_id,
        "/api/v2/preload/record",
        action.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(stored["object"]["kind"], "preset");
    assert_eq!(stored["object"]["id"], "1.1");
    assert_eq!(stored["object"]["revision"], 1);
    assert_eq!(stored["object"]["body"]["name"], "Front at sixty");
    let intensity = &stored["object"]["body"]["group_values"]["front"]["intensity"];
    assert_eq!(intensity["kind"], "normalized");
    assert!(
        (intensity["value"].as_f64().unwrap() - 0.6).abs() < 0.000_001,
        "stored preload intensity should retain the programmer value"
    );

    let (status, replayed) =
        post_show_object_intent(&app, &token, &show_id, "/api/v2/preload/record", action).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replayed["replayed"], true);
    assert_eq!(replayed["object"], stored["object"]);

    let _ = std::fs::remove_dir_all(data_dir);
}

fn test_cue_list() -> light_playback::CueList {
    light_playback::CueList {
        id: light_core::CueListId::new(),
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
        cues: vec![light_playback::Cue::new(1.0)],
    }
}
