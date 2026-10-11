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
    // Starting through software and completing through OSC must share the same prefix,
    // just as the pure OSC sequence below does. Selection-context SET remains contextual.
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
async fn set_cuelist_editor_keeps_shifted_assign_and_canonical_playback_configuration() {
    let scenario = CommandHttpScenario::new().await;
    scenario.create_and_open_show("SET editor and ASSIGN isolation").await;
    set_cue_record_value(&scenario);
    assert_eq!(scenario.execute("physical-editor-seed", Some("RECORD CUELIST 15 CUE 1")).await.status(), StatusCode::OK);
    assert_eq!(scenario.execute("explicit-assign", Some("ASSIGN CUELIST 15 AT PBK 2.3")).await.status(), StatusCode::OK);
    let expected = serde_json::json!({"page":2,"position":3});
    // Explicit page configuration stays independent of the selected Cuelist editor.
    let response = json(scenario.execute("explicit-playback-config", Some("SET PBK 2.3")).await).await;
    assert_eq!(response["outcome"], "accepted", "{response}");
    assert!(scenario.state.events.audit_events().iter().any(|event| event.kind == "playback_configuration_requested" && event.payload["address"] == expected));
    scenario.state.installation.set_playback_surface_page(scenario.session.desk.id, "main", 2).unwrap();
    let response = json(scenario.execute("current-playback-config", Some("SET PBK 3")).await).await;
    assert_eq!(response["outcome"], "accepted", "{response}");
    assert!(cuelist_editor_events(&scenario).is_empty(), "canonical SET still opens Playback configuration");
    let response = scenario.execute("independent-editor-after-assign", Some("SET CUELIST 15")).await;
    assert_eq!(response.status(), StatusCode::OK, "{}", json(response).await);
    assert_eq!(cuelist_editor_events(&scenario)[0]["value"], stored_cue_list(&scenario, 15).2.id.0.to_string());
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn pure_osc_set_cue_cue_opens_editor_without_assignment_or_programmer_changes() {
    let scenario = CommandHttpScenario::new().await;
    scenario.create_and_open_show("Pure OSC Cuelist editor").await;
    set_cue_record_value(&scenario);
    let response = scenario.execute("osc-editor-seed104", Some("RECORD CUELIST 104 CUE 1")).await;
    assert_eq!(response.status(), StatusCode::OK, "{}", json(response).await);
    let (_, object, list) = stored_cue_list(&scenario, 104);
    let revision = active_show_revision(&scenario);
    let before = json(scenario.values_snapshot().await).await["projection"].clone();
    let source: SocketAddr = "127.0.0.1:9140".parse().unwrap();
    scenario.state.integrations.register_osc_subscriber("pure-editor".into(), OscSubscriber {
        capability: light_core::SurfaceCapability::Programming,
        path: "desk".into(), target: source, command_source: source,
        session_id: scenario.session.id, last_seen: Instant::now(), shifted: false, shift_held: false,
        update_record_started: None, update_first_release: None, last_highlight_action: None,
    });
    for (key, expected) in [("set", "SET"), ("cue", "SET CUE"), ("cue", "SET CUELIST"),
        ("digit-1", "SET CUELIST 1"), ("digit-0", "SET CUELIST 10"), ("digit-4", "SET CUELIST 104"), ("enter", "")] {
        for pressed in [true, false] {
            handle_programmer_osc(&scenario.state, &format!("/light/desk/programmer/{key}"),
                &[OscArgument::Bool(pressed)], Some("127.0.0.1:9140"));
            assert_eq!(scenario.state.programming.get(scenario.session.id).unwrap().command_line.trim(), expected,
                "actual OSC key {key} pressed={pressed} preserves authoritative editor prefix");
        }
    }
    let events = cuelist_editor_events(&scenario);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["value"], list.id.0.to_string());
    assert_eq!(events[0]["desk_id"], scenario.session.desk.id.to_string());
    assert!(!scenario.state.events.audit_events().iter().any(|event| event.kind == "desk_action" && event.payload["action"] == "set"),
        "empty SET must not dispatch contextual Assign and race the authoritative prefix");
    assert_eq!(active_show_revision(&scenario), revision);
    assert_eq!(json(scenario.values_snapshot().await).await["projection"].clone(), before);
    assert_eq!(stored_cue_list(&scenario, 104).1.body, object.body);
    assert!(scenario.state.output.playback_runtime().is_empty());
    // Shifted SET remains the documented Assign keyword, with release harmless.
    for (key, pressed) in [("shift", true), ("set", true), ("set", false), ("shift", false)] {
        handle_programmer_osc(&scenario.state, &format!("/light/desk/programmer/{key}"),
            &[OscArgument::Bool(pressed)], Some("127.0.0.1:9140"));
    }
    assert_eq!(scenario.state.programming.get(scenario.session.id).unwrap().command_line.trim(), "ASSIGN");
    assert_eq!(cuelist_editor_events(&scenario).len(), 1);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn set_playback_configuration_opens_empty_high_address_without_authoring() {
    let scenario = CommandHttpScenario::new().await;
    scenario.create_and_open_show("Empty high Playback configuration").await;
    let entry = scenario.state.active_show.current().unwrap();
    let before = ShowStore::open(&entry.path).unwrap().portable_document().unwrap();
    let before_pages = serde_json::to_value(&scenario.state.output.snapshot().playback_pages).unwrap();
    assert!(!scenario.state.output.snapshot().playback_pages.iter().any(|page| page.number == 99 && page.assignments.contains_key(&1000)));
    let response = json(scenario.execute("set-empty-high", Some("SET PBK 99.1000")).await).await;
    assert_eq!(response["outcome"], "accepted", "{response}");
    let events = scenario.state.events.audit_events();
    let request = events.iter().find(|event| event.kind == "playback_configuration_requested").unwrap();
    assert_eq!(request.payload["address"], serde_json::json!({"page":99,"position":1000}));
    assert_eq!(request.payload["desk_id"], scenario.session.desk.id.to_string());
    assert_eq!(ShowStore::open(&entry.path).unwrap().portable_document().unwrap(), before);
    execute_programmer_command(&scenario.state, &scenario.session, "SET PBK 99.1000").unwrap();
    let requests = scenario.state.events.audit_events().into_iter().filter(|event| event.kind == "playback_configuration_requested").collect::<Vec<_>>();
    assert_eq!(requests.len(), 2, "HTTP and native command paths both request configuration");
    assert_eq!(requests[1].payload["address"], serde_json::json!({"page":99,"position":1000}));
    assert_eq!(serde_json::to_value(&scenario.state.output.snapshot().playback_pages).unwrap(), before_pages);
    assert_eq!(ShowStore::open(&entry.path).unwrap().portable_document().unwrap(), before);
    assert!(scenario.state.output.playback_runtime().is_empty());
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn set_playback_configuration_uses_selected_surface_for_unqualified_address() {
    let scenario = CommandHttpScenario::new().await;
    scenario.create_and_open_show("Selected surface configuration").await;
    let before_pages = serde_json::to_value(&scenario.state.output.snapshot().playback_pages).unwrap();
    let desk = scenario.session.desk.id;
    let mut surfaces = scenario.state.installation.playback_surfaces(desk).unwrap();
    let mut secondary = surfaces["main"].clone();
    secondary.name = "Selected screen".into();
    secondary.page_source = light_show::PlaybackPageSource::Own { page: 99 };
    surfaces.insert("screen:selected".into(), secondary);
    scenario.state.installation.put_playback_surfaces(desk, &surfaces).unwrap();
    scenario.state.installation.select_playback_surface(desk, "screen:selected").unwrap();
    let response = json(scenario.execute("set-selected-high", Some("SET PBK 1000")).await).await;
    assert_eq!(response["outcome"], "accepted", "{response}");
    let events = scenario.state.events.audit_events();
    let request = events.iter().find(|event| event.kind == "playback_configuration_requested").unwrap();
    assert_eq!(request.payload["address"], serde_json::json!({"page":99,"position":1000}));
    assert_eq!(scenario.state.installation.command_page(desk).unwrap(), 99);
    execute_programmer_command(&scenario.state, &scenario.session, "SET PBK 1000").unwrap();
    let requests = scenario.state.events.audit_events().into_iter().filter(|event| event.kind == "playback_configuration_requested").collect::<Vec<_>>();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[1].payload["address"], serde_json::json!({"page":99,"position":1000}));
    assert_eq!(serde_json::to_value(&scenario.state.output.snapshot().playback_pages).unwrap(), before_pages);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn set_playback_configuration_rejects_invalid_addresses_without_request_or_show_write() {
    let scenario = CommandHttpScenario::new().await;
    scenario.create_and_open_show("Invalid Playback configuration").await;
    let entry = scenario.state.active_show.current().unwrap();
    let before = ShowStore::open(&entry.path).unwrap().portable_document().unwrap();
    for (index, command) in ["SET PBK 99.1001", "SET PBK 100.1", "SET PBK 0.1", "SET PBK 1.0", "SET PBK 99.", "SET PBK 99.1000 EXTRA"].into_iter().enumerate() {
        let response = json(scenario.execute(&format!("set-invalid-{index}"), Some(command)).await).await;
        assert_eq!(response["outcome"], "rejected", "{command}: {response}");
    }
    assert!(!scenario.state.events.audit_events().iter().any(|event| event.kind == "playback_configuration_requested"));
    assert_eq!(ShowStore::open(&entry.path).unwrap().portable_document().unwrap(), before);
    assert!(scenario.state.output.playback_runtime().is_empty());
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}
