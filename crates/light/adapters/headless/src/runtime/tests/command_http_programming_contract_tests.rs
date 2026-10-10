#[tokio::test]
async fn unavailable_programming_contract_rejects_http_and_ws_writes_atomically_in_both_lanes() {
    let scenario = CommandHttpScenario::from_state(test_state_with_programming_contract(
        ProgrammerRegistry::default(), None, 0,
    )).await;
    let fixture = scenario.install_direct_fixture();
    let value = light_core::AttributeValue::Position(Arc::new(
        light_core::programming::PositionIntent::angles(720.0, -30.0),
    ));
    let value = serde_json::to_value(value).unwrap();

    for preload in [false, true] {
        if preload {
            assert_eq!(scenario.press_key(&scenario.token, "PRE", "contract-enter-preload").await.status(), StatusCode::OK);
        }
        let before = serde_json::to_value(scenario.state.programming.get(scenario.session.id).unwrap()).unwrap();
        let sequence = scenario.state.events.latest_sequence();
        let request = serde_json::json!({
            "request_id": format!("contract-http-{preload}"),
            "expected_revision": 0,
            "expected_capture_mode_revision": u64::from(preload),
            "action": {
                "type": "batch",
                "mutations": [
                    {"type":"set_fixture", "fixture_id":fixture.0, "attribute":"intensity", "value":{"kind":"normalized", "value":0.5}, "timing":{"fade":false}},
                    {"type":"set_fixture", "fixture_id":fixture.0, "attribute":"position", "value":value, "timing":{"fade":false}}
                ]
            }
        });
        let response = if preload {
            scenario.preload_values_action(request.clone()).await
        } else {
            scenario.values_action(request.clone()).await
        };
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert!(json(response).await.to_string().contains("programming contract"));

        let mut request = request;
        let request_id = format!("contract-ws-{preload}");
        request["request_id"] = request_id.clone().into();
        let response = dispatch_live_action(
            &scenario.state, &scenario.session,
            live_action_frame(&scenario.session, &request_id, serde_json::from_value(serde_json::json!({
                "type": if preload {"programmer_preload_values"} else {"programming_values"},
                "request": request,
            })).unwrap()),
        );
        assert!(!response.ok, "{response:?}");
        assert!(serde_json::to_string(&response.error).unwrap().contains("programming contract"));
        assert_eq!(scenario.state.events.latest_sequence(), sequence);
        assert_eq!(serde_json::to_value(scenario.state.programming.get(scenario.session.id).unwrap()).unwrap(), before);
    }
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn unavailable_programming_contract_rejects_command_presets_before_selection_or_fixat_changes() {
    let scenario = CommandHttpScenario::from_state(test_state_with_programming_contract(
        ProgrammerRegistry::default(), None, 0,
    )).await;
    let fixture = scenario.install_direct_fixture();
    let path = scenario.data_dir.join("shows/contract-presets.show");
    let show_id = default_show::initialise_legacy_test_show(&path).unwrap();
    let entry = ShowEntry {
        id:show_id, name:"Contract presets".into(), path:path.display().to_string(),
        is_base_show:false, revision:0, updated_at:String::new(), created_at:None,
        last_loaded_at:None, revision_copy:None,
    };
    scenario.state.active_show.replace_current(Some(entry));
    let preset = light_programmer::Preset {

fixture_replacement_projections: Default::default(),
group_replacement_projections: Default::default(),
instance_id: None,
        name:"Turns".into(), family:light_programmer::PresetFamily::Position, number:7,
        values:HashMap::from([(fixture, HashMap::from([(
            light_core::AttributeKey("position".into()),
            light_core::AttributeValue::Position(Arc::new(light_core::programming::PositionIntent::angles(720.0, -30.0))),
        )]))]), group_values:HashMap::new(), universal_values:Default::default(), aim_at_fixture_number:None,
    };
    ActiveShowRepository::open(&path).unwrap().put_object("preset", "3.7", &serde_json::to_value(&preset).unwrap(), 0).unwrap();
    let context = operator_action_context(&scenario.session, light_application::ActionSource::Http);
    let number = scenario.state.output.snapshot().fixtures[0].fixture_number.unwrap();
    for command in [format!("FIXTURE {number} AT 3.7"), "GROUP 1 AT 3.7".into(), "DEGRP 1 AT 3.7".into(), format!("FIXTURE {number} FIXAT POSITION PRESET 7"), format!("FIXTURE {number} ATTRIBUTE pan FIXAT POSITION PRESET 7")] {
        let before = serde_json::to_value(scenario.state.programming.get(scenario.session.id).unwrap()).unwrap();
        let error = execute_programmer_command_from(&scenario.state, &scenario.session, &command, &context).unwrap_err();
        assert!(error.contains("programming contract"), "{command}: {error}");
        assert_eq!(serde_json::to_value(scenario.state.programming.get(scenario.session.id).unwrap()).unwrap(), before, "{command}");
    }
    scenario.state.programming.select(scenario.session.id, [fixture]);
    let before = serde_json::to_value(scenario.state.programming.get(scenario.session.id).unwrap()).unwrap();
    assert!(execute_programmer_command_from(&scenario.state, &scenario.session, "AT 3.7", &context).unwrap_err().contains("programming contract"));
    assert_eq!(serde_json::to_value(scenario.state.programming.get(scenario.session.id).unwrap()).unwrap(), before);
    let ports = super::super::dynamics_adapter::ServerDynamicsPorts {state:&scenario.state, session:&scenario.session};
    let result = scenario.state.dynamics.fix_at_batch(&context, light_application::DynamicFixAtBatchCommand {
        values: vec![light_application::DynamicFixAtValue {fixture_id:fixture, attribute:light_core::AttributeKey("position".into()), value:preset.values[&fixture].values().next().unwrap().clone(), programming_mask:None}],
        timing:Default::default(),
    }, &ports);
    assert!(result.unwrap_err().message.contains("programming contract"));
    assert_eq!(serde_json::to_value(scenario.state.programming.get(scenario.session.id).unwrap()).unwrap(), before);
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}
