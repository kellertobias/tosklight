fn cuelist_editor_events(scenario: &CommandHttpScenario) -> Vec<serde_json::Value> {
    scenario.state.events.audit_events().iter()
        .filter(|event| event.kind == "desk_action" && event.payload["action"] == "open-object-editor" && event.payload["control"] == "cuelist")
        .map(|event| event.payload.clone()).collect()
}

#[tokio::test]
async fn set_cuelist_editor_http_and_osc_keypad_open_unassigned_uuid_without_programming() {
    let scenario = CommandHttpScenario::new().await;
    scenario.create_and_open_show("Independent Cuelist editor").await;
    set_cue_record_value(&scenario);
    let response = scenario.execute("editor-seed-15", Some("RECORD CUELIST 15 CUE 1")).await;
    assert_eq!(response.status(), StatusCode::OK, "{}", json(response).await);
    let (assignment, object, list) = stored_cue_list(&scenario, 15);
    assert!(assignment.is_none());
    let revision = active_show_revision(&scenario);
    let before = json(scenario.values_snapshot().await).await["projection"].clone();
    let response = scenario.execute("open-cuelist15", Some("SET CUELIST 15")).await;
    assert_eq!(response.status(), StatusCode::OK, "{}", json(response).await);
    assert_eq!(cuelist_editor_events(&scenario).len(), 1);
    assert_eq!(cuelist_editor_events(&scenario)[0]["value"], list.id.0.to_string());
    assert_eq!(cuelist_editor_events(&scenario)[0]["desk_id"], scenario.session.desk.id.to_string());

    for (index, key) in ["SET", "CUE", "CUE", "1", "5", "ENT"].into_iter().enumerate() {
        let response = scenario.press_key(&scenario.token, key, &format!("editor-key-{index}")).await;
        assert_eq!(response.status(), StatusCode::OK, "key {key}: {}", json(response).await);
    }
    assert_eq!(cuelist_editor_events(&scenario).len(), 2, "literal SET/CUE/CUE keypad opens same UUID");
    // Bare attached SET is an existing contextual window action; keep that convention.
    // Start SET through the shared software keypad, then complete the authoritative line
    // from the attached OSC keys, as the same desk supports mixed control surfaces.
    assert_eq!(scenario.press_key(&scenario.token, "SET", "editor-osc-prefix").await.status(), StatusCode::OK);
    assert_eq!(scenario.state.programming.get(scenario.session.id).unwrap().command_line.trim(), "SET");
    let source: SocketAddr = "127.0.0.1:9135".parse().unwrap();
    scenario.state.integrations.register_osc_subscriber("cuelist-editor".into(), OscSubscriber {
        capability: light_core::SurfaceCapability::Programming,
        path: "editor".into(), target: source, command_source: source,
        session_id: scenario.session.id, last_seen: Instant::now(), shifted: false, shift_held: false,
        update_record_started: None, update_first_release: None, last_highlight_action: None,
    });
    for key in ["cue", "cue", "digit-1", "digit-5", "enter"] {
        handle_programmer_osc(&scenario.state, &format!("/light/editor/programmer/{key}"), &[OscArgument::Bool(true)], Some("127.0.0.1:9135"));
    }
    assert_eq!(cuelist_editor_events(&scenario).len(), 3, "actual OSC CUE/CUE keypad completes shared SET line");
    assert_eq!(cuelist_editor_events(&scenario)[2]["value"], list.id.0.to_string());
    assert_eq!(active_show_revision(&scenario), revision);
    assert_eq!(json(scenario.values_snapshot().await).await["projection"].clone(), before);
    assert_eq!(stored_cue_list(&scenario, 15).1.body, object.body);
    assert!(scenario.state.output.playback_runtime().is_empty());
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn set_cuelist_editor_rejects_invalid_missing_or_assignment_syntax_without_effect() {
    let scenario = CommandHttpScenario::new().await;
    scenario.create_and_open_show("Cuelist editor validation").await;
    set_cue_record_value(&scenario);
    let revision = active_show_revision(&scenario);
    let before = json(scenario.values_snapshot().await).await["projection"].clone();
    for (i, command) in ["SET CUELIST 0", "SET CUELIST 65536", "SET CUELIST 15", "SET CUELIST 15 AT PBK 1", "SET CUELIST"].into_iter().enumerate() {
        let response = scenario.execute(&format!("bad-cuelist-editor-{i}"), Some(command)).await;
        assert_eq!(json(response).await["outcome"], "rejected", "{command}");
    }
    assert!(cuelist_editor_events(&scenario).is_empty());
    assert_eq!(active_show_revision(&scenario), revision);
    assert_eq!(json(scenario.values_snapshot().await).await["projection"].clone(), before);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn set_cuelist_editor_keeps_shifted_assign_and_existing_physical_set_addressing() {
    let scenario = CommandHttpScenario::new().await;
    scenario.create_and_open_show("SET editor and ASSIGN isolation").await;
    set_cue_record_value(&scenario);
    assert_eq!(scenario.execute("physical-editor-seed", Some("RECORD CUELIST 15 CUE 1")).await.status(), StatusCode::OK);
    assert_eq!(scenario.execute("explicit-assign", Some("ASSIGN CUELIST 15 AT PBK 2.3")).await.status(), StatusCode::OK);
    let expected = scenario.state.output.snapshot().playback_pages.iter().find(|page| page.number == 2).unwrap().slots[&3];
    let context = light_application::ActionContext::operator(scenario.session.desk.id, scenario.session.id.0, light_application::ActionSource::Http);
    // The existing compatibility SET path still addresses physical Playbacks; its HTTP
    // atomic policy remains unchanged and is separately exercised by command policy tests.
    assert_eq!(set_commands::execute_set_command(&scenario.state, &scenario.session, &["2".into(), ".".into(), "3".into()], &context).unwrap(), 0);
    assert!(scenario.state.events.audit_events().iter().any(|event| event.kind == "playback_configuration_requested" && event.payload["playback"] == expected));
    assert_eq!(set_commands::execute_set_command(&scenario.state, &scenario.session, &[expected.to_string()], &context).unwrap(), 0);
    assert!(cuelist_editor_events(&scenario).is_empty(), "physical SET still opens physical configuration");
    let response = scenario.execute("independent-editor-after-assign", Some("SET CUELIST 15")).await;
    assert_eq!(response.status(), StatusCode::OK, "{}", json(response).await);
    assert_eq!(cuelist_editor_events(&scenario)[0]["value"], stored_cue_list(&scenario, 15).2.id.0.to_string());
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}
