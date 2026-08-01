use super::*;

/// A renderer that has never been configured still has something to follow, and the desk can
/// select every named view it offers.
#[tokio::test]
async fn the_default_target_answers_before_anything_has_been_configured() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;

    let snapshot = json(get_views(&app, &token).await).await;
    assert_eq!(snapshot["views"].as_array().unwrap().len(), 1);
    let view = &snapshot["views"][0];
    assert_eq!(view["target"], "main");
    assert_eq!(view["mode"], "full_3d");
    assert_eq!(view["quality"], "high");
    assert_eq!(view["revision"], 0);
    assert!(view.get("camera").is_none() || view["camera"].is_null());
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn every_named_view_can_be_selected_and_is_published_once() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;

    let modes = [
        "top_down",
        "left_to_right",
        "right_to_left",
        "front_to_back",
        "back_to_front",
        "lines_3d",
        "simple_3d",
        "full_3d",
    ];
    for (index, mode) in modes.iter().enumerate() {
        let outcome = json(
            update_view(
                &app,
                &token,
                "main",
                &format!("select-{mode}"),
                serde_json::json!({"mode": mode}),
            )
            .await,
        )
        .await;
        assert_eq!(outcome["view"]["mode"], *mode, "{mode} was not applied");
        assert_eq!(outcome["changed"], true, "{mode} reported no change");
        assert_eq!(outcome["view"]["revision"], index as u64 + 1);
    }

    // Selecting the view that is already displayed changes nothing and publishes nothing.
    let cursor = state.events.latest_sequence();
    let repeated = json(
        update_view(
            &app,
            &token,
            "main",
            "select-full_3d-again",
            serde_json::json!({"mode": "full_3d"}),
        )
        .await,
    )
    .await;
    assert_eq!(repeated["changed"], false);
    assert_eq!(repeated["view"]["revision"], modes.len() as u64);
    assert_eq!(state.events.latest_sequence(), cursor);
    let _ = std::fs::remove_dir_all(data_dir);
}

/// A patch carries only what changed: choosing a quality must not throw away the view.
#[tokio::test]
async fn a_patch_leaves_every_field_it_does_not_name_alone() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;

    update_view(
        &app,
        &token,
        "main",
        "set-view",
        serde_json::json!({"mode": "top_down", "ambient": 0.2}),
    )
    .await;
    let outcome = json(
        update_view(
            &app,
            &token,
            "main",
            "set-quality",
            serde_json::json!({"quality": "ultra"}),
        )
        .await,
    )
    .await;
    assert_eq!(outcome["view"]["mode"], "top_down");
    assert_eq!(outcome["view"]["quality"], "ultra");
    assert_eq!(outcome["view"]["ambient"], 0.2);
    assert_eq!(outcome["view"]["exposure"], 1.0);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn one_desk_can_address_one_renderer_without_moving_the_others() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;

    update_view(
        &app,
        &token,
        "main",
        "main-plan",
        serde_json::json!({"mode": "top_down"}),
    )
    .await;
    update_view(
        &app,
        &token,
        "front-of-house",
        "foh-beams",
        serde_json::json!({"mode": "simple_3d"}),
    )
    .await;

    let snapshot = json(get_views(&app, &token).await).await;
    let views = snapshot["views"].as_array().unwrap();
    assert_eq!(views.len(), 2);
    let mode_of = |target: &str| {
        views
            .iter()
            .find(|view| view["target"] == target)
            .unwrap_or_else(|| panic!("{target} is missing"))["mode"]
            .clone()
    };
    assert_eq!(mode_of("main"), "top_down");
    assert_eq!(mode_of("front-of-house"), "simple_3d");
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn a_resent_edit_returns_the_first_outcome_instead_of_moving_the_camera_twice() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;

    let first = json(
        update_view(
            &app,
            &token,
            "main",
            "same-request",
            serde_json::json!({"mode": "lines_3d"}),
        )
        .await,
    )
    .await;
    assert_eq!(first["replayed"], false);
    assert_eq!(first["view"]["revision"], 1);

    let replay = json(
        update_view(
            &app,
            &token,
            "main",
            "same-request",
            serde_json::json!({"mode": "lines_3d"}),
        )
        .await,
    )
    .await;
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["view"]["revision"], 1);

    let reused = update_view(
        &app,
        &token,
        "main",
        "same-request",
        serde_json::json!({"mode": "top_down"}),
    )
    .await;
    assert_eq!(reused.status(), StatusCode::CONFLICT);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn an_impossible_view_is_named_rather_than_stored() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;

    let bad_exposure = update_view(
        &app,
        &token,
        "main",
        "bad-exposure",
        serde_json::json!({"exposure": 99.0}),
    )
    .await;
    assert_eq!(bad_exposure.status(), StatusCode::BAD_REQUEST);
    assert!(
        json(bad_exposure).await["error"]
            .as_str()
            .unwrap()
            .contains("exposure")
    );

    let bad_target = update_view(
        &app,
        &token,
        "../etc",
        "bad-target",
        serde_json::json!({"mode": "top_down"}),
    )
    .await;
    assert!(
        bad_target.status() == StatusCode::BAD_REQUEST
            || bad_target.status() == StatusCode::NOT_FOUND
    );

    // Nothing was written by either refusal.
    let snapshot = json(get_views(&app, &token).await).await;
    assert_eq!(snapshot["views"].as_array().unwrap().len(), 1);
    assert_eq!(snapshot["views"][0]["revision"], 0);
    let _ = std::fs::remove_dir_all(data_dir);
}

/// The view is desk-level presentation state, so it survives a restart of the server and never
/// travels inside the show file.
#[tokio::test]
async fn the_view_is_kept_with_the_installation_rather_than_the_show() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    update_view(
        &app,
        &token,
        "main",
        "keep-it",
        serde_json::json!({"mode": "back_to_front", "quality": "draft"}),
    )
    .await;

    let stored = state
        .installation
        .configuration()
        .visualizer_views
        .get("main")
        .copied()
        .expect("the view is stored with the installation");
    assert_eq!(
        serde_json::to_value(stored).unwrap()["mode"],
        "back_to_front"
    );

    let show = create_show(&app, &token, "Visualizer view").await;
    let show_id = show["id"].as_str().unwrap();
    let objects = json(
        app.clone()
            .oneshot(
                Request::get("/api/v2/objects/visualizer_view")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header("x-tosk-show", show_id)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(
        objects["objects"].as_array().map(Vec::len).unwrap_or(0),
        0,
        "the show file must not carry the desk's visualizer view"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

/// Saving desk settings must not take every renderer's camera with it.
#[tokio::test]
async fn saving_the_desk_settings_leaves_the_view_alone() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    update_view(
        &app,
        &token,
        "main",
        "set-plan",
        serde_json::json!({"mode": "top_down"}),
    )
    .await;

    let saved = app
        .clone()
        .oneshot(
            Request::post("/api/v2/configuration/update")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "request_id": "save-desk-settings",
                        "patch": {"frame_rate_hz": 42},
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(saved.status(), StatusCode::OK);

    let snapshot = json(get_views(&app, &token).await).await;
    assert_eq!(snapshot["views"][0]["mode"], "top_down");
    assert_eq!(snapshot["views"][0]["revision"], 1);
    let _ = std::fs::remove_dir_all(data_dir);
}

async fn get_views(app: &Router, token: &str) -> Response {
    app.clone()
        .oneshot(
            Request::get("/api/v2/visualizer-views")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn update_view(
    app: &Router,
    token: &str,
    target: &str,
    request_id: &str,
    patch: serde_json::Value,
) -> Response {
    app.clone()
        .oneshot(
            Request::post(format!("/api/v2/visualizer-views/{target}/update"))
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({"request_id": request_id, "patch": patch}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}
