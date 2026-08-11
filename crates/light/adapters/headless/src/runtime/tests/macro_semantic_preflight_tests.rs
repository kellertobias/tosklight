use super::*;

#[tokio::test]
async fn macro_preflight_keeps_one_detached_programmer_and_never_mutates_live_state() {
    let scenario = OperationalScenario::new().await;
    scenario.seed_and_open_show().await;
    let session = authenticate_token(&scenario.state, &scenario.token).unwrap();
    let context = operator_action_context(&session, light_application::ActionSource::Http);
    let assert_live_programmer_untouched = || {
        let live = scenario.state.programming.get(session.id).unwrap();
        assert!(live.selected.is_empty());
        assert!(live.selection_expression.is_none());
        assert!(live.values.is_empty());
        assert!(live.group_values.is_empty());
        assert!(live.dynamic_values.is_empty());
        assert!(live.command_line.is_empty());
        assert!(live.undo.is_empty());
    };
    assert_live_programmer_untouched();

    // The second line is valid only when preflight retains the first line's simulated selection.
    prevalidate_programmer_commands_from(
        &scenario.state,
        &session,
        &["FIXTURE 1", "AT 50"],
        &context,
    )
    .unwrap();
    assert_live_programmer_untouched();

    // A context-invalid later line rejects the entire document before the valid earlier lines can
    // touch the live Programmer.
    let failure = prevalidate_programmer_commands_from(
        &scenario.state,
        &session,
        &["FIXTURE 1", "AT 50", "NOT-A-COMMAND 9999"],
        &context,
    )
    .unwrap_err();
    assert_eq!(failure.0, 2);
    assert!(failure.1.contains("invalid"), "{}", failure.1);
    assert_live_programmer_untouched();

    drop(scenario);
}

#[tokio::test]
async fn macro_restore_preflight_uses_a_concrete_initiating_selection_only_in_detached_state() {
    let scenario = OperationalScenario::new().await;
    scenario.seed_and_open_show().await;
    let session = authenticate_token(&scenario.state, &scenario.token).unwrap();
    let context = operator_action_context(&session, light_application::ActionSource::Macro);
    let fixture_id = scenario.state.output.snapshot().fixtures[0].fixture_id;
    let programmers = scenario.state.programming.programmers();
    programmers.select(session.id, []);

    prevalidate_macro_commands_from(
        &scenario.state,
        &session,
        &["RESTORE SELECTION", "AT 50"],
        &[fixture_id],
        &context,
    )
    .unwrap();

    let live = programmers.selection(session.id).unwrap();
    assert!(live.selected.is_empty());
    assert!(
        live.expression.is_none()
            || live.expression == Some(light_programmer::SelectionExpression::Static)
    );
    drop(scenario);
}

#[tokio::test]
async fn macro_run_restores_its_initiating_selection_before_the_next_statement() {
    let scenario = OperationalScenario::new().await;
    scenario.seed_and_open_show().await;
    let session = authenticate_token(&scenario.state, &scenario.token).unwrap();
    let show_id = scenario.state.active_show.current().unwrap().id;
    let mut snapshot = (*scenario.state.output.snapshot()).clone();
    let mut first = snapshot.fixtures[0].clone();
    first.fixture_number = Some(1);
    let mut second = first.clone();
    second.fixture_id = light_core::FixtureId::new();
    second.fixture_number = Some(2);
    second.universe = Some(2);
    second.address = Some(1);
    second.split_patches.clear();
    let initiating_fixture = second.fixture_id;
    snapshot.fixtures = vec![first, second].into();
    scenario.state.output.replace_snapshot(snapshot).unwrap();
    scenario
        .state
        .programming
        .programmers()
        .select(session.id, [initiating_fixture]);

    let macro_id = Uuid::from_u128(160);
    let created = scenario
        .app
        .clone()
        .oneshot(
            Request::post("/api/v2/macros/actions")
                .header(header::AUTHORIZATION, format!("Bearer {}", scenario.token))
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-tosk-show", show_id.0.to_string())
                .body(Body::from(
                    serde_json::json!({
                        "request_id": "create-restore-selection-macro",
                        "action": {
                            "type": "create",
                            "definition": {
                                "id": macro_id,
                                "number": 160,
                                "name": "Restore selection",
                                "source": "FIXTURE 1; RESTORE SELECTION; AT 50",
                                "presentation": {"color": "#315cab"}
                            }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::OK);
    let created = json(created).await;
    let source_revision = created["object"]["revision"].as_u64().unwrap();
    let copied = scenario
        .app
        .clone()
        .oneshot(
            Request::post("/api/v2/macros/actions")
                .header(header::AUTHORIZATION, format!("Bearer {}", scenario.token))
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-tosk-show", show_id.0.to_string())
                .body(Body::from(
                    serde_json::json!({
                        "request_id": "copy-restore-selection-macro",
                        "action": {
                            "type": "copy",
                            "source_macro_id": macro_id,
                            "expected_revision": source_revision,
                            "pool_number": 161
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(copied.status(), StatusCode::OK);
    let copied = json(copied).await;
    assert_eq!(copied["object"]["body"]["number"], 161);
    assert_eq!(copied["object"]["body"]["name"], "Restore selection Copy");

    let started = scenario
        .app
        .clone()
        .oneshot(
            Request::post(format!("/api/v2/macros/{macro_id}/run"))
                .header(header::AUTHORIZATION, format!("Bearer {}", scenario.token))
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-tosk-show", show_id.0.to_string())
                .header("x-tosk-desk", session.desk.id.to_string())
                .body(Body::from(
                    serde_json::json!({"trigger": {"type": "editor"}}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = started.status();
    assert_eq!(status, StatusCode::OK);
    let started = json(started).await;
    let execution_id = Uuid::parse_str(started["execution_id"].as_str().unwrap()).unwrap();

    for _ in 0..100 {
        if scenario
            .state
            .macros
            .execution(session.desk.id, execution_id)
            .is_some_and(|execution| execution.state.is_terminal())
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let execution = scenario
        .state
        .macros
        .execution(session.desk.id, execution_id)
        .unwrap();
    assert_eq!(
        execution.state,
        light_application::CommandMacroExecutionState::Succeeded,
        "{:?}",
        execution.message
    );
    let programmer = scenario.state.programming.get(session.id).unwrap();
    assert_eq!(programmer.selected, vec![initiating_fixture]);
    assert_eq!(programmer.values.len(), 1);
    assert!(
        programmer
            .values
            .iter()
            .any(|value| value.fixture_id == initiating_fixture)
    );
    drop(scenario);
}
