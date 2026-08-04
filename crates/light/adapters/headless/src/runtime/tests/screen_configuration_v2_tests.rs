use super::*;

#[tokio::test]
async fn screen_configuration_v2_is_sparse_replay_safe_tolerant_and_retires_v1() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Screen configuration v2").await;
    let show_id = show["id"].as_str().unwrap();
    let opened = app
        .clone()
        .oneshot(open_show_request(&token, show_id))
        .await
        .unwrap();
    assert_eq!(opened.status(), StatusCode::OK);
    let screen_id = Uuid::new_v4();
    let create = serde_json::json!({
        "request_id":"screen-create",
        "future_request_field":true,
        "action":{
            "type":"create",
            "future_action_field":"accepted",
            "configuration":{
                "id":screen_id,
                "name":"Side screen",
                "layout":{"desks":[],"activeDeskId":"main","future_layout":true},
                "show_dock":true,
                "show_playbacks":true,
                "playback_count":8,
                "playback_rows":1,
                "first_playback_slot":1,
                "page_mode":"follow_main",
                "show_page_controls":true,
                "desired_open":false,
                "display_id":null,
                "bounds":null,
                "fullscreen":false,
                "playback_layout":null,
                "future_configuration_field":42
            }
        }
    });
    let event_before = state.events.audit_revision();
    let created = post_screen_action(&app, &token, create.clone()).await;
    assert_eq!(created.status(), StatusCode::OK);
    let created = json(created).await;
    assert_eq!(created["replayed"], false);
    assert_eq!(created["screen"]["name"], "Side screen");
    assert_eq!(created["screen"]["layout"]["future_layout"], true);
    let event_after = state.events.audit_revision();
    assert_eq!(event_after, event_before + 1);

    let replay = post_screen_action(&app, &token, create).await;
    assert_eq!(replay.status(), StatusCode::OK);
    assert_eq!(json(replay).await["replayed"], true);
    assert_eq!(state.events.audit_revision(), event_after);

    let collision = post_screen_action(
        &app,
        &token,
        serde_json::json!({
            "request_id":"screen-create",
            "action":{"type":"delete","screen_id":screen_id}
        }),
    )
    .await;
    assert_eq!(collision.status(), StatusCode::CONFLICT);

    let sparse = post_screen_action(
        &app,
        &token,
        serde_json::json!({
            "request_id":"screen-update",
            "action":{
                "type":"update",
                "screen_id":screen_id,
                "patch":{"name":"Renamed","clear_bounds":false,"clear_display_id":false,
                    "clear_playback_layout":false}
            }
        }),
    )
    .await;
    assert_eq!(sparse.status(), StatusCode::OK);
    let sparse = json(sparse).await;
    assert_eq!(sparse["screen"]["name"], "Renamed");
    assert_eq!(sparse["screen"]["layout"]["future_layout"], true);
    assert_eq!(sparse["screen"]["show_playbacks"], true);

    let followed = post_screen_action(
        &app,
        &token,
        serde_json::json!({
            "request_id":"screen-page-followed",
            "action":{"type":"set_page","screen_id":screen_id,"page":1}
        }),
    )
    .await;
    assert_eq!(followed.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        json(followed).await["error"],
        "screen follows the main page"
    );

    let independent = post_screen_action(
        &app,
        &token,
        serde_json::json!({
            "request_id":"screen-independent",
            "action":{
                "type":"update",
                "screen_id":screen_id,
                "patch":{"page_mode":"independent","clear_bounds":false,"clear_display_id":false,
                    "clear_playback_layout":false}
            }
        }),
    )
    .await;
    assert_eq!(independent.status(), StatusCode::OK);
    let page = post_screen_action(
        &app,
        &token,
        serde_json::json!({
            "request_id":"screen-page",
            "action":{"type":"set_page","screen_id":screen_id,"page":1}
        }),
    )
    .await;
    assert_eq!(page.status(), StatusCode::OK);
    assert_eq!(json(page).await["active_page"], 1);

    let snapshot = app
        .clone()
        .oneshot(
            Request::get("/api/v2/screens")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(snapshot.status(), StatusCode::OK);
    let snapshot = json(snapshot).await;
    assert_eq!(snapshot["screens"][0]["name"], "Renamed");
    assert_eq!(snapshot["active_pages"][screen_id.to_string()], 1);
    assert_eq!(
        snapshot["programmer_control_surface"],
        serde_json::json!({"owner_screen_id":null,"visible_encoders":6})
    );

    let assigned = post_screen_action(
        &app,
        &token,
        serde_json::json!({
            "request_id":"screen-programmer-owner",
            "action":{
                "type":"update_programmer_control_surface",
                "patch":{"owner_screen_id":screen_id,"visible_encoders":4}
            }
        }),
    )
    .await;
    assert_eq!(assigned.status(), StatusCode::OK);
    assert_eq!(
        json(assigned).await["programmer_control_surface"],
        serde_json::json!({"owner_screen_id":screen_id,"visible_encoders":4})
    );

    let invalid_width = post_screen_action(
        &app,
        &token,
        serde_json::json!({
            "request_id":"screen-programmer-invalid-width",
            "action":{
                "type":"update_programmer_control_surface",
                "patch":{"visible_encoders":5}
            }
        }),
    )
    .await;
    assert_eq!(invalid_width.status(), StatusCode::BAD_REQUEST);

    let deleted = post_screen_action(
        &app,
        &token,
        serde_json::json!({
            "request_id":"screen-programmer-owner-delete",
            "action":{"type":"delete","screen_id":screen_id}
        }),
    )
    .await;
    assert_eq!(deleted.status(), StatusCode::OK);
    assert_eq!(
        json(deleted).await["programmer_control_surface"],
        serde_json::json!({"owner_screen_id":null,"visible_encoders":4})
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn screen_configuration_v2_reads_existing_desk_store_rows_without_migration() {
    let (state, data_dir) = test_state();
    let screen_id = Uuid::new_v4();
    state
        .installation
        .put_screen(ScreenConfiguration {
            id: screen_id,
            name: "Existing row".into(),
            layout: serde_json::json!({"desks":[],"activeDeskId":"legacy"}),
            show_dock: false,
            show_playbacks: true,
            playback_count: 6,
            playback_rows: 2,
            first_playback_slot: 5,
            page_mode: "follow_main".into(),
            show_page_controls: false,
            desired_open: true,
            display_id: Some("display-1".into()),
            bounds: Some(serde_json::json!({"x":1,"y":2,"width":800,"height":600})),
            fullscreen: true,
            playback_layout: None,
            content: light_show::ScreenContent::Desktop,
        })
        .unwrap();
    let app = router(state);
    let (token, _) = login(&app, "Operator").await;
    let response = app
        .oneshot(
            Request::get("/api/v2/screens")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = json(response).await;
    assert_eq!(body["screens"][0]["id"], screen_id.to_string());
    assert_eq!(body["screens"][0]["first_playback_slot"], 5);
    assert_eq!(body["screens"][0]["display_id"], "display-1");
    assert_eq!(body["screens"][0]["fullscreen"], true);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn screen_configuration_intent_routes_persist_fixed_content_and_keep_dock_off() {
    let (state, data_dir) = test_state();
    let app = router(state);
    let (token, _) = login(&app, "Operator").await;
    let screen_id = Uuid::new_v4();
    let missing_cue_list_id = Uuid::new_v4();
    let created = post_screen_intent(
        &app,
        &token,
        "/api/v2/screens/create",
        serde_json::json!({
            "request_id":"fixed-screen-create",
            "configuration":{
                "id":screen_id,
                "name":"Fixed fixtures",
                "layout":{"desks":[],"activeDeskId":"preserved"},
                "show_dock":true,
                "show_playbacks":true,
                "playback_count":8,
                "playback_rows":1,
                "first_playback_slot":1,
                "page_mode":"follow_main",
                "show_page_controls":true,
                "desired_open":false,
                "display_id":null,
                "bounds":null,
                "fullscreen":false,
                "playback_layout":null,
                "content":{
                    "type":"fixed_pane",
                    "pane":{
                        "type":"fixture_sheet",
                        "included_heads":"no_sub_heads",
                        "order":"active",
                        "active_only":true,
                        "cue_list_id":missing_cue_list_id,
                        "columns":["id","name","dimmer"],
                        "show_type":false,
                        "show_group_shortcuts":true
                    }
                }
            }
        }),
    )
    .await;
    assert_eq!(created.status(), StatusCode::OK);
    let created = json(created).await;
    assert_eq!(created["screen"]["show_dock"], false);
    assert_eq!(
        created["screen"]["content"]["pane"]["cue_list_id"],
        missing_cue_list_id.to_string()
    );
    assert_eq!(created["screen"]["content"]["pane"]["compact_mode"], "off");
    assert_eq!(
        created["screen"]["content"]["pane"]["columns"],
        serde_json::json!(["id", "name", "intensity"])
    );

    let update_body = serde_json::json!({
        "request_id":"fixed-screen-update",
        "patch":{
            "show_dock":true,
            "content":{
                "type":"fixed_pane",
                "pane":{
                    "type":"stage_3d",
                    "follow_preload":true,
                    "show_floor_grid":false,
                    "show_beam_guides":true,
                    "render_quality":"full",
                    "environment_brightness":0.75
                }
            },
            "clear_bounds":false,
            "clear_display_id":false,
            "clear_playback_layout":false
        }
    });
    let updated = post_screen_intent(
        &app,
        &token,
        &format!("/api/v2/screens/{screen_id}/update"),
        update_body.clone(),
    )
    .await;
    assert_eq!(updated.status(), StatusCode::OK);
    let updated = json(updated).await;
    assert_eq!(updated["screen"]["show_dock"], false);
    assert_eq!(
        updated["screen"]["content"]["pane"]["environment_brightness"],
        0.75
    );

    let replay = post_screen_intent(
        &app,
        &token,
        &format!("/api/v2/screens/{screen_id}/update"),
        update_body,
    )
    .await;
    assert_eq!(replay.status(), StatusCode::OK);
    assert_eq!(json(replay).await["replayed"], true);

    let desktop = post_screen_intent(
        &app,
        &token,
        &format!("/api/v2/screens/{screen_id}/update"),
        serde_json::json!({
            "request_id":"fixed-screen-desktop",
            "patch":{
                "content":{"type":"desktop"},
                "clear_bounds":false,
                "clear_display_id":false,
                "clear_playback_layout":false
            }
        }),
    )
    .await;
    assert_eq!(desktop.status(), StatusCode::OK);
    let desktop = json(desktop).await;
    assert_eq!(desktop["screen"]["content"]["type"], "desktop");
    assert_eq!(desktop["screen"]["show_dock"], false);
    assert_eq!(desktop["screen"]["layout"]["activeDeskId"], "preserved");

    let deleted = post_screen_intent(
        &app,
        &token,
        &format!("/api/v2/screens/{screen_id}/delete"),
        serde_json::json!({"request_id":"fixed-screen-delete"}),
    )
    .await;
    assert_eq!(deleted.status(), StatusCode::OK);
    assert!(json(deleted).await["screen"].is_null());
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn screen_configuration_intent_routes_reject_invalid_fixed_display_settings() {
    let (state, data_dir) = test_state();
    let app = router(state);
    let (token, _) = login(&app, "Operator").await;
    let response = post_screen_intent(
        &app,
        &token,
        "/api/v2/screens/create",
        serde_json::json!({
            "request_id":"invalid-fixed-screen",
            "configuration":{
                "id":Uuid::new_v4(),
                "name":"Invalid Stage",
                "layout":{},
                "show_dock":false,
                "show_playbacks":true,
                "playback_count":8,
                "playback_rows":1,
                "first_playback_slot":1,
                "page_mode":"follow_main",
                "show_page_controls":true,
                "desired_open":false,
                "display_id":null,
                "bounds":null,
                "fullscreen":false,
                "playback_layout":null,
                "content":{
                    "type":"fixed_pane",
                    "pane":{
                        "type":"stage_3d",
                        "follow_preload":false,
                        "show_floor_grid":true,
                        "show_beam_guides":true,
                        "render_quality":"full",
                        "environment_brightness":1.5
                    }
                }
            }
        }),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let _ = std::fs::remove_dir_all(data_dir);
}

async fn post_screen_action(app: &Router, token: &str, body: serde_json::Value) -> Response {
    post_screen_intent(app, token, "/api/v2/screens/actions", body).await
}

async fn post_screen_intent(
    app: &Router,
    token: &str,
    path: &str,
    body: serde_json::Value,
) -> Response {
    app.clone()
        .oneshot(
            Request::post(path)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}
