#[tokio::test]
async fn pool_presentation_is_typed_replay_safe_and_desk_persistent() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let request = serde_json::json!({
        "request_id":"pool-presentation-1",
        "patch":{
            "pool_presentation":{
                "palette":{
                    "group":"#d8ad55",
                    "macro_color":"#8f3541",
                    "dynamic":"#3bbdce",
                    "cuelist":"#93cc55",
                    "sequence":"#93cc55",
                    "preset":{
                        "mixed":"#89939e",
                        "intensity":"#89939e",
                        "color":"#2244cc",
                        "position":"#89939e",
                        "beam":"#89939e"
                    }
                },
                "modes":{"show:show-1:pane:presets":"individual"},
                "items":{
                    "show:show-1:preset:2.1":{
                        "title":"Ocean",
                        "icon":"◇",
                        "color":"#2255aa"
                    }
                },
                "future_field":"accepted"
            }
        }
    });
    let send = || {
        Request::post("/api/v2/configuration/update")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::from(request.to_string()))
            .unwrap()
    };
    let first = app.clone().oneshot(send()).await.unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    assert!(!json(first).await["replayed"].as_bool().unwrap());

    let replay = app.clone().oneshot(send()).await.unwrap();
    assert_eq!(replay.status(), StatusCode::OK);
    assert!(json(replay).await["replayed"].as_bool().unwrap());

    let persisted = state
        .installation.setting("server_configuration")
        .unwrap()
        .unwrap();
    let persisted: serde_json::Value = serde_json::from_str(&persisted).unwrap();
    assert_eq!(
        persisted["pool_presentation"]["items"]["show:show-1:preset:2.1"]["color"],
        "#2255aa"
    );
    assert!(
        persisted.get("active_show_id").is_none(),
        "pool presentation must remain outside portable show content"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn pool_presentation_rejects_invalid_colors_without_mutating_other_configuration() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let original_frame_rate = state.installation.configuration().frame_rate_hz;
    let mut pool = serde_json::to_value(PoolPresentationConfiguration::default()).unwrap();
    pool["palette"]["group"] = "orange".into();
    let response = app
        .oneshot(
            Request::post("/api/v2/configuration/update")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(
                    serde_json::json!({
                        "request_id":"pool-presentation-invalid",
                        "patch":{"pool_presentation":pool}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(state.installation.configuration().frame_rate_hz, original_frame_rate);
    assert_eq!(
        state.installation.configuration().pool_presentation,
        PoolPresentationConfiguration::default()
    );
    let _ = std::fs::remove_dir_all(data_dir);
}
