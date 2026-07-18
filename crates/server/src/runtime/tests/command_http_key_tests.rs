async fn login_to_existing_desk(scenario: &CommandHttpScenario) -> String {
    let response = scenario
        .app
        .clone()
        .oneshot(
            Request::post("/api/v1/sessions")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "username": "Operator",
                        "desk_id": scenario.session.desk.id,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    json(response).await["token"].as_str().unwrap().to_owned()
}

async fn verify_key_replay_scope(scenario: &CommandHttpScenario) {
    let pressed = scenario.press_key(&scenario.token, "7", "digit-7").await;
    assert_eq!(pressed.status(), StatusCode::OK);
    let pressed = json(pressed).await;
    assert_eq!(pressed["outcome"], "accepted");
    assert_eq!(pressed["action"], "edited");
    assert_eq!(pressed["command_line"]["text"], "F7");
    assert_eq!(pressed["command_line"]["revision"], 1);

    let replayed = scenario.press_key(&scenario.token, "7", "digit-7").await;
    assert_eq!(json(replayed).await["command_line"]["revision"], 1);
    let second_token = login_to_existing_desk(scenario).await;
    let second_session = scenario.press_key(&second_token, "7", "digit-7").await;
    let second_session = json(second_session).await;
    assert_eq!(second_session["command_line"]["text"], "F77");
    assert_eq!(second_session["command_line"]["revision"], 2);
}

async fn verify_command_payload_limit(scenario: &CommandHttpScenario) {
    let response = scenario
        .execute("oversized", Some(&"X".repeat(40 * 1024)))
        .await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

async fn verify_atomic_put_boundary(scenario: &CommandHttpScenario) {
    let first = scenario.put("FIXTURE 1", 2);
    let second = scenario.put("FIXTURE 2", 2);
    let (first, second) = tokio::join!(first, second);
    let statuses = [first.status(), second.status()];
    assert_eq!(
        statuses
            .iter()
            .filter(|status| **status == StatusCode::OK)
            .count(),
        1
    );
    assert_eq!(
        statuses
            .iter()
            .filter(|status| **status == StatusCode::CONFLICT)
            .count(),
        1
    );
    assert_eq!(
        scenario
            .state
            .programmers
            .command_line_state(scenario.session.id)
            .unwrap()
            .revision,
        3
    );
}

#[tokio::test]
async fn command_line_v2_keys_are_replay_safe_and_put_writers_use_one_atomic_boundary() {
    let scenario = CommandHttpScenario::new().await;
    verify_key_replay_scope(&scenario).await;
    verify_command_payload_limit(&scenario).await;
    verify_atomic_put_boundary(&scenario).await;
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}
