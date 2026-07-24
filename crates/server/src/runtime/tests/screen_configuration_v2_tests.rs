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
    let event_before = state.event_revision.load(Ordering::Relaxed);
    let created = post_screen_action(&app, &token, create.clone()).await;
    assert_eq!(created.status(), StatusCode::OK);
    let created = json(created).await;
    assert_eq!(created["replayed"], false);
    assert_eq!(created["screen"]["name"], "Side screen");
    assert_eq!(created["screen"]["layout"]["future_layout"], true);
    let event_after = state.event_revision.load(Ordering::Relaxed);
    assert_eq!(event_after, event_before + 1);

    let replay = post_screen_action(&app, &token, create).await;
    assert_eq!(replay.status(), StatusCode::OK);
    assert_eq!(json(replay).await["replayed"], true);
    assert_eq!(state.event_revision.load(Ordering::Relaxed), event_after);

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

    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn screen_configuration_v2_reads_existing_desk_store_rows_without_migration() {
    let (state, data_dir) = test_state();
    let screen_id = Uuid::new_v4();
    state
        .desk
        .lock()
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

async fn post_screen_action(app: &Router, token: &str, body: serde_json::Value) -> Response {
    app.clone()
        .oneshot(
            Request::post("/api/v2/screens/actions")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}
