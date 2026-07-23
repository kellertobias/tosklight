#[tokio::test]
async fn recovery_checkpoints_follow_the_autosave_interval() {
    let clock = Arc::new(ManualClock::new(chrono::Utc::now()));
    let (state, data_dir) = test_state_with_clock(clock.clone());
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let show = create_show(&app, &token, "Autosave show").await;
    let show_id = show["id"].as_str().unwrap().to_owned();
    let opened = app
        .clone()
        .oneshot(
            Request::post(format!("/api/v1/shows/{show_id}/open"))
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from(r#"{"transition":"hold_current"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(opened.status(), StatusCode::OK);
    assert_eq!(
        state.configuration.read().autosave_interval_seconds,
        30,
        "operator-facing default is 30 s"
    );

    // Rapid mutations share one recovery checkpoint: only the first takes the full-file copy.
    for number in 1..=3 {
        assert_eq!(
            put_show_object(
                &app,
                &token,
                &show_id,
                "group",
                &number.to_string(),
                serde_json::json!({"id":number.to_string(),"name":format!("Group {number}"),"fixtures":[]}),
            )
            .await
            .status(),
            StatusCode::OK
        );
    }
    assert_eq!(autosave_checkpoint_count(&data_dir), 1);

    // Once the configured autosave interval elapses, the next mutation checkpoints again.
    clock.advance_millis(30_000);
    assert_eq!(
        put_show_object(
            &app,
            &token,
            &show_id,
            "group",
            "4",
            serde_json::json!({"id":"4","name":"Group 4","fixtures":[]}),
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(autosave_checkpoint_count(&data_dir), 2);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn autosave_interval_is_validated_operator_configuration() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    for (value, expected) in [
        (4_u64, StatusCode::BAD_REQUEST),
        (3_601, StatusCode::BAD_REQUEST),
        (5, StatusCode::OK),
        (600, StatusCode::OK),
    ] {
        let mut configuration =
            serde_json::to_value(state.configuration.read().clone()).unwrap();
        configuration["autosave_interval_seconds"] = serde_json::json!(value);
        let response = app
            .clone()
            .oneshot(
                Request::put("/api/v1/configuration")
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::from(configuration.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), expected, "autosave interval {value}");
    }
    assert_eq!(state.configuration.read().autosave_interval_seconds, 600);
    let _ = std::fs::remove_dir_all(data_dir);
}

fn autosave_checkpoint_count(data_dir: &std::path::Path) -> usize {
    std::fs::read_dir(data_dir.join("backups"))
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|entry| {
                    entry
                        .file_name()
                        .to_str()
                        .is_some_and(|name| name.contains("-show-object-"))
                })
                .count()
        })
        .unwrap_or(0)
}
