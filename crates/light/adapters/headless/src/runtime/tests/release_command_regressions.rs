use super::*;

async fn seed(
    scenario: &CommandHttpScenario,
    name: &str,
    objects: &[(&str, &str, serde_json::Value)],
) -> String {
    let id = scenario.create_and_open_show(name).await;
    let entry = scenario.state.active_show.current().unwrap();
    let store = ShowStore::open(&entry.path).unwrap();
    for (kind, key, body) in objects {
        store.put_object(kind, key, body, 0).unwrap();
    }
    scenario
        .state
        .output
        .replace_snapshot(load_engine_snapshot(&entry).unwrap())
        .unwrap();
    id
}

fn subset(tokens: &str) -> Result<light_programmer::SelectionRule, String> {
    parse_subset_rule(
        &tokens
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>(),
    )
}

#[test]
fn release_documented_subset_defaults_and_offset_contract() {
    use light_programmer::SelectionRule;
    for (command, n, offset) in [
        ("DIV 2 OFFSET 1", 2, 1),
        ("DIV OFFSET", 2, 1),
        ("DIV OFFSET 1", 2, 1),
        ("DIV 2 OFFSET", 2, 1),
        ("DIV", 2, 0),
        ("DIV 3 OFFSET 1", 3, 1),
        ("DIV 2 + 1", 2, 1),
    ] {
        assert_eq!(
            subset(command).unwrap(),
            SelectionRule::EveryNth { n, offset },
            "{command}"
        );
    }
    assert_eq!(subset("DIV DIV").unwrap(), SelectionRule::Even);
    assert_eq!(subset("OFFSET").unwrap(), SelectionRule::Even);
    for command in [
        "DIV 0",
        "DIV 2 OFFSET 2",
        "DIV 2 OFFSET 3",
        "DIV 2 OFFSET -1",
        "DIV 2 OFFSET 1 9",
        "DIV 2 +",
        "DIV nonsense",
    ] {
        assert!(subset(command).is_err(), "must reject {command}");
    }
}

#[tokio::test]
async fn release_documented_subsets_preserve_order_and_live_derived_membership() {
    let scenario = CommandHttpScenario::new().await;
    let ids: Vec<_> = (0..5).map(|_| light_core::FixtureId::new()).collect();
    let source = light_programmer::GroupDefinition {
        id: "100".into(),
        name: "Ordered".into(),
        fixtures: ids.clone(),
        ..Default::default()
    };
    let show_id = seed(
        &scenario,
        "Derived subsets",
        &[("group", "100", serde_json::to_value(source).unwrap())],
    )
    .await;
    for (request, command, expected) in [
        ("odd", "GROUP 100 DIV 2", vec![ids[0], ids[2], ids[4]]),
        ("even", "GROUP 100 DIV 2 OFFSET 1", vec![ids[1], ids[3]]),
        ("defaults", "GROUP 100 DIV OFFSET", vec![ids[1], ids[3]]),
    ] {
        let response = scenario.execute(request, Some(command)).await;
        let status = response.status();
        let body = json(response).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["outcome"], "accepted", "{body}");
        assert_eq!(
            scenario
                .state
                .programming
                .get(scenario.session.id)
                .unwrap()
                .selected,
            expected
        );
    }
    let response = scenario
        .execute("store-even", Some("RECORD GROUP 101"))
        .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{}",
        json(response).await
    );
    let entry = scenario
        .state
        .installation
        .show(light_core::ShowId(Uuid::parse_str(&show_id).unwrap()))
        .unwrap()
        .unwrap();
    let store = ShowStore::open(&entry.path).unwrap();
    let stored = store
        .objects("group")
        .unwrap()
        .into_iter()
        .find(|g| g.id == "101")
        .unwrap();
    let derived: light_programmer::GroupDefinition = serde_json::from_value(stored.body).unwrap();
    let Some(light_programmer::GroupFixtureSource::References { references }) = &derived.source
    else {
        panic!("derived group must retain live references")
    };
    assert_eq!(references[0].group_id, "100");
    assert_eq!(
        references[0].rule,
        light_programmer::SelectionRule::EveryNth { n: 2, offset: 1 }
    );
    let reversed: Vec<_> = ids.iter().copied().rev().collect();
    let source = light_programmer::GroupDefinition {
        id: "100".into(),
        name: "Ordered".into(),
        fixtures: reversed.clone(),
        ..Default::default()
    };
    store
        .put_object(
            "group",
            "100",
            &serde_json::to_value(source).unwrap(),
            store.object("group", "100").unwrap().unwrap().revision,
        )
        .unwrap();
    scenario
        .state
        .output
        .replace_snapshot(load_engine_snapshot(&entry).unwrap())
        .unwrap();
    let response = scenario.execute("select-derived", Some("GROUP 101")).await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{}",
        json(response).await
    );
    assert_eq!(
        scenario
            .state
            .programming
            .get(scenario.session.id)
            .unwrap()
            .selected,
        vec![reversed[1], reversed[3]]
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn release_dynamic_assignment_routes_empty_current_explicit_and_virtual_targets() {
    let clock = Arc::new(ManualClock::new(fixed_test_time()));
    let scenario = CommandHttpScenario::with_clock(clock.clone()).await;
    let fixture = light_core::FixtureId::new();
    let unselected = light_core::FixtureId::new();
    let mut lamp = operational_fixture(fixture);
    lamp.fixture_number = Some(1);
    let mut other = operational_fixture(unselected);
    other.fixture_number = Some(2);
    other.address = Some(2);
    let dynamic_id = Uuid::new_v4();
    let mut definition = command_test_dynamic(dynamic_id, 100);
    definition.target_binding = light_dynamics::DynamicTargetBinding::FrozenTargets {
        targets: vec![fixture],
    };
    let show_id = seed(
        &scenario,
        "Direct Dynamic assignment",
        &[
            (
                "patched_fixture",
                &fixture.0.to_string(),
                serde_json::to_value(lamp).unwrap(),
            ),
            (
                "patched_fixture",
                &unselected.0.to_string(),
                serde_json::to_value(other).unwrap(),
            ),
            (
                "dynamic",
                &dynamic_id.to_string(),
                serde_json::to_value(definition).unwrap(),
            ),
        ],
    )
    .await;
    assert_eq!(scenario.state.output.snapshot().fixtures.len(), 2);
    let show = light_core::ShowId(Uuid::parse_str(&show_id).unwrap());
    scenario
        .state
        .installation
        .set_desk_page(scenario.session.desk.id, show, 3)
        .unwrap();
    scenario
        .state
        .programming
        .select(scenario.session.id, [unselected]);
    for (request, command, page, slot, virtual_number) in [
        ("current", "ASSIGN DYNAMIC 100 AT PBK 8", 3, Some(8), None),
        (
            "explicit",
            "ASSIGN DYNAMIC 100 AT PBK 2.6",
            2,
            Some(6),
            None,
        ),
        (
            "virtual",
            "ASSIGN DYNAMIC 100 AT VPBK 1001",
            1,
            None,
            Some(1001),
        ),
    ] {
        let response = scenario.execute(request, Some(command)).await;
        let status = response.status();
        let body = json(response).await;
        assert_eq!(status, StatusCode::OK, "{command}: {body}");
        assert_eq!(body["outcome"], "accepted", "{command}: {body}");
        let snapshot = scenario.state.output.snapshot();
        let page_def = snapshot
            .playback_pages
            .iter()
            .find(|p| p.number == page)
            .unwrap();
        let playback = if let Some(slot) = slot {
            snapshot
                .playbacks
                .iter()
                .find(|p| p.number == page_def.slots[&slot])
                .unwrap()
        } else {
            &page_def.virtual_playbacks[&virtual_number.unwrap()]
        };
        let light_playback::PlaybackTarget::Dynamic { assignment } = &playback.target else {
            panic!("assignment target must be Dynamic")
        };
        assert_eq!(assignment.dynamic.dynamic_id, Some(dynamic_id));
        assert_eq!(assignment.revision, 1);
        assert!(assignment.target_scope.is_none());
        assert!(
            snapshot.cue_lists.is_empty(),
            "no synthetic Cuelist wrapper"
        );
        assert!(
            scenario
                .state
                .programming
                .get(scenario.session.id)
                .unwrap()
                .dynamic_values
                .is_empty()
        );
    }
    for (id, address, surface) in [
        (
            "current-on",
            serde_json::json!({"kind":"current_page","slot":8}),
            "physical",
        ),
        (
            "explicit-on",
            serde_json::json!({"kind":"explicit_page","page":2,"slot":6}),
            "physical",
        ),
        (
            "virtual-on",
            serde_json::json!({"kind":"virtual","page":1,"playback_number":1001}),
            "virtual",
        ),
    ] {
        let on = id;
        let response = playback_action(&scenario, id, address.clone(), surface, "on").await;
        let status = response.status();
        let body = json(response).await;
        assert_eq!(status, StatusCode::OK, "{on}: {body}");
        scenario
            .state
            .programming
            .programmers()
            .clear_values(scenario.session.id);
        clock.advance_millis(250);
        let batches = scenario.state.output.dynamic_contributions_for_test();
        let values: Vec<_> = batches
            .iter()
            .flat_map(|b| b.samples())
            .map(|s| s.value())
            .collect();
        assert!(
            !values.is_empty(),
            "{on} must contribute actual Dynamic output: {:?}",
            scenario.state.output.dynamic_runtime_snapshot()
        );
        assert!(
            values.iter().all(|v| v.fixture_id == fixture),
            "stored target binding must win over programmer selection"
        );
        assert!(
            values.iter().any(
                |v| matches!(v.value,light_core::AttributeValue::Normalized(value) if value>0.0)
            )
        );
        let rendered = scenario
            .state
            .output
            .render_with_playback_events(
                &scenario.state.active_show.output_projection(),
                &scenario.state.playback.render_capability(),
                scenario.state.output.render_options(),
            )
            .unwrap();
        assert!(
            rendered.rendered.universes[&1][0] > 0,
            "assigned Dynamic reaches encoded DMX"
        );
        assert_eq!(
            rendered.rendered.universes[&1][1], 0,
            "unrelated selected fixture stays dark"
        );
        let response =
            playback_action(&scenario, &format!("{id}-off"), address, surface, "off").await;
        let status = response.status();
        let body = json(response).await;
        assert_eq!(status, StatusCode::OK, "{id} off: {body}");
        assert!(
            scenario
                .state
                .output
                .dynamic_contributions_for_test()
                .is_empty(),
            "OFF must remove assigned owner"
        );
        let rendered = scenario
            .state
            .output
            .render_with_playback_events(
                &scenario.state.active_show.output_projection(),
                &scenario.state.playback.render_capability(),
                scenario.state.output.render_options(),
            )
            .unwrap();
        assert_eq!(
            rendered.rendered.universes[&1][0], 0,
            "OFF removes encoded Dynamic output"
        );
    }
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

async fn playback_action(
    scenario: &CommandHttpScenario,
    id: &str,
    address: serde_json::Value,
    surface: &str,
    action: &str,
) -> Response {
    scenario.app.clone().oneshot(Request::post("/api/v2/playback-actions")
        .header(header::AUTHORIZATION,format!("Bearer {}",scenario.token))
        .header("x-tosk-desk",scenario.session.desk.id.to_string())
        .header(header::CONTENT_TYPE,"application/json")
        .body(Body::from(serde_json::json!({"request_id":id,"address":address,"surface":surface,"action":{"type":action,"pressed":true}}).to_string())).unwrap()).await.unwrap()
}
