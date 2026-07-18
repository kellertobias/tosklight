#[tokio::test]
async fn supplied_command_executes_without_publishing_intermediate_command_text() {
    let scenario = CommandHttpScenario::new().await;
    let fixture_id = scenario.install_direct_fixture();
    let events_before = scenario
        .state
        .audit_events
        .lock()
        .iter()
        .filter(|event| event.kind == "command_line_changed")
        .count();
    let response = scenario
        .execute("atomic-supplied-line", Some("GROUP 1 AT 50"))
        .await;

    assert_eq!(response.status(), StatusCode::OK);
    let response = json(response).await;
    assert_eq!(response["outcome"], "accepted");
    assert_eq!(response["applied"], 1);
    assert_eq!(response["command_line"]["text"], "FIXTURE");
    assert_eq!(response["command_line"]["revision"], 0);
    assert_eq!(
        scenario
            .state
            .audit_events
            .lock()
            .iter()
            .filter(|event| event.kind == "command_line_changed")
            .count(),
        events_before
    );
    assert_eq!(
        scenario
            .state
            .audit_events
            .lock()
            .iter()
            .filter(|event| {
                matches!(
                    event.kind.as_str(),
                    "command_applied" | "programmer_changed"
                ) && event.payload["request_id"] == "atomic-supplied-line"
            })
            .count(),
        2
    );
    let programmer = scenario
        .state
        .programmers
        .get(scenario.session.id)
        .unwrap();
    assert_eq!(programmer.selected, vec![fixture_id]);
    assert_eq!(
        programmer.group_values["1"][&light_core::AttributeKey::intensity()].value,
        light_core::AttributeValue::Normalized(0.5)
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn sensitive_command_text_is_returned_to_its_writer_but_redacted_from_global_events() {
    let scenario = CommandHttpScenario::new().await;
    let mut events = scenario.state.events.subscribe();
    let response = scenario
        .put("FIXTURE 1 TOKEN super-secret-value", 0)
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        json(response).await["text"],
        "FIXTURE 1 TOKEN super-secret-value"
    );

    let event = events.recv().await.unwrap();
    assert_eq!(event.kind, "command_line_changed");
    assert_eq!(event.payload["text"], "[REDACTED SENSITIVE COMMAND]");
    assert_eq!(event.payload["redacted"], true);
    assert!(!event.payload.to_string().contains("super-secret-value"));
    let audit_event = scenario
        .state
        .audit_events
        .lock()
        .back()
        .cloned()
        .unwrap();
    assert_eq!(audit_event.revision, event.revision);
    assert_eq!(audit_event.payload, event.payload);
    assert_eq!(
        json(scenario.get().await).await["text"],
        "FIXTURE 1 TOKEN super-secret-value"
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}
