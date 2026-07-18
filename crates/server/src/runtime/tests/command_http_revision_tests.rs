async fn verify_revisioned_command_line(scenario: &CommandHttpScenario) {
    let initial = scenario.get().await;
    assert_eq!(initial.status(), StatusCode::OK);
    assert_eq!(initial.headers()[header::ETAG], "\"0\"");
    let initial = json(initial).await;
    assert_eq!(initial["text"], "FIXTURE");
    assert_eq!(initial["target"], "FIXTURE");
    assert_eq!(initial["pristine"], true);
    assert_eq!(initial["revision"], 0);
    assert!(initial["pending_choice"].is_null());

    let no_op = scenario.put("FIXTURE", 0).await;
    assert_eq!(no_op.status(), StatusCode::OK);
    assert_eq!(no_op.headers()[header::ETAG], "\"0\"");
    assert_eq!(json(no_op).await["text"], "FIXTURE");

    let replaced = scenario.put("FIXTURE 1", 0).await;
    assert_eq!(replaced.status(), StatusCode::OK);
    assert_eq!(replaced.headers()[header::ETAG], "\"1\"");
    assert_eq!(json(replaced).await["text"], "FIXTURE 1");

    let stale = scenario.put("FIXTURE 99", 0).await;
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    assert_eq!(
        scenario
            .state
            .programmers
            .command_line_state(scenario.session.id)
            .unwrap()
            .visible_text(),
        "FIXTURE 1"
    );
}

async fn verify_idempotent_execution(
    scenario: &CommandHttpScenario,
    fixture_id: light_core::FixtureId,
) {
    let executed = scenario.execute("execute-fixture-1", None).await;
    assert_eq!(executed.status(), StatusCode::OK);
    let executed_etag = executed.headers()[header::ETAG].clone();
    let executed = json(executed).await;
    assert_eq!(executed["outcome"], "accepted");
    assert_eq!(executed["action"], "executed");
    assert_eq!(executed["applied"], 1);
    assert_eq!(executed["command_line"]["text"], "FIXTURE");
    assert_eq!(
        scenario
            .state
            .programmers
            .get(scenario.session.id)
            .unwrap()
            .selected,
        vec![fixture_id]
    );

    let replayed = scenario.execute("execute-fixture-1", None).await;
    assert_eq!(replayed.status(), StatusCode::OK);
    assert_eq!(replayed.headers()[header::ETAG], executed_etag);
    assert_eq!(json(replayed).await, executed);
    assert_eq!(scenario.history_len(), 1);

    let reused = scenario
        .execute("execute-fixture-1", Some("GROUP 1 AT BOGUS"))
        .await;
    assert_eq!(reused.status(), StatusCode::CONFLICT);
}

async fn verify_choice_and_rejection_replay(scenario: &CommandHttpScenario) {
    let choice = command_http::execute_existing_command(
        &scenario.state,
        &scenario.session,
        "COPY SET 1 CUE 1 AT SET 2 CUE 2",
        "test",
        Some("compatibility-choice"),
        command_http::ExistingCommandPolicy::Compatibility,
    );
    assert!(matches!(
        choice,
        command_http::ExistingCommandOutcome::ChoiceRequired { .. }
    ));
    let compatibility = scenario
        .execute(
            "pending-choice",
            Some("COPY SET 1 CUE 1 AT SET 2 CUE 2"),
        )
        .await;
    assert_eq!(compatibility.status(), StatusCode::OK);
    let compatibility = json(compatibility).await;
    assert_eq!(compatibility["outcome"], "rejected");
    assert!(
        compatibility["error"]
            .as_str()
            .unwrap()
            .contains("not yet available through the atomic")
    );
    assert_eq!(
        compatibility["command_line"]["text"],
        "COPY SET 1 CUE 1 AT SET 2 CUE 2"
    );
    verify_rejected_command_is_atomic(scenario).await;
}

async fn verify_rejected_command_is_atomic(scenario: &CommandHttpScenario) {
    let selection_before = scenario
        .state
        .programmers
        .selection(scenario.session.id)
        .unwrap();
    let programmer_before = scenario.state.programmers.get(scenario.session.id).unwrap();
    let rejected = scenario
        .execute("missing-fixture", Some("GROUP 1 AT BOGUS"))
        .await;
    assert_eq!(rejected.status(), StatusCode::OK);
    let rejected = json(rejected).await;
    assert_eq!(rejected["outcome"], "rejected");
    assert_eq!(rejected["command_line"]["text"], "GROUP 1 AT BOGUS");
    assert_eq!(
        scenario
            .state
            .programmers
            .selection(scenario.session.id)
            .unwrap(),
        selection_before
    );
    let programmer_after = scenario.state.programmers.get(scenario.session.id).unwrap();
    assert_eq!(
        serde_json::to_value(programmer_after.values).unwrap(),
        serde_json::to_value(programmer_before.values).unwrap()
    );
    assert_eq!(
        serde_json::to_value(programmer_after.group_values).unwrap(),
        serde_json::to_value(programmer_before.group_values).unwrap()
    );
    assert_eq!(scenario.history_len(), 3);
    let replayed = scenario
        .execute("missing-fixture", Some("GROUP 1 AT BOGUS"))
        .await;
    assert_eq!(json(replayed).await, rejected);
    assert_eq!(scenario.history_len(), 3);
}

#[tokio::test]
async fn command_line_v2_is_revisioned_desk_scoped_and_idempotent() {
    let scenario = CommandHttpScenario::new().await;
    let fixture_id = scenario.install_direct_fixture();
    verify_revisioned_command_line(&scenario).await;
    verify_idempotent_execution(&scenario, fixture_id).await;
    verify_choice_and_rejection_replay(&scenario).await;
    let wrong_desk = scenario
        .app
        .clone()
        .oneshot(
            Request::get(format!(
                "/api/v2/desks/{}/command-line",
                Uuid::new_v4()
            ))
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", scenario.token),
            )
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(wrong_desk.status(), StatusCode::FORBIDDEN);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}
