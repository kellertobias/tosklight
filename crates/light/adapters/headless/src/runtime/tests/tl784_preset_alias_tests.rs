// Production Record + command/tile recall + authenticated Update Targets (TL-784).

async fn tl784_targets(scenario: &CommandHttpScenario, show_id: &str) -> serde_json::Value {
    let response = scenario.app.clone().oneshot(
        Request::post("/api/v2/programming-update/targets")
            .header(header::AUTHORIZATION, format!("Bearer {}", scenario.token))
            .header("x-tosk-show", show_id)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::json!({"request_id":"tl784-targets","filter":"eligible_for_update_existing"}).to_string())).unwrap(),
    ).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    json(response).await
}

async fn tl784_record_recall_targets(tile: bool) {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario
        .create_and_open_show("TL784 actual mixed recording")
        .await;
    let mut fixture = super::template_dimmer("TL784 physical dimmer", 1);
    fixture.fixture_number = Some(1);
    assert_eq!(
        scenario
            .put_active_object(
                &show_id,
                "patched_fixture",
                &fixture.fixture_id.0.to_string(),
                0,
                serde_json::to_value(&fixture).unwrap(),
            )
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        scenario
            .execute("tl784-intensity", Some("FIXTURE 1 AT 50"))
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        scenario
            .execute("tl784-record", Some("RECORD 0.150"))
            .await
            .status(),
        StatusCode::OK
    );
    let show = scenario.state.active_show.current().clone().unwrap();
    let store = ShowStore::open(&show.path).unwrap();
    let recorded = store
        .objects("preset")
        .unwrap()
        .into_iter()
        .find(|object| object.id == "150")
        .unwrap();
    let instance = recorded.body["instance_id"].clone();
    assert!(!instance.is_null());
    assert_eq!(
        scenario
            .execute("tl784-selection", Some("FIXTURE 1"))
            .await
            .status(),
        StatusCode::OK
    );
    if tile {
        let registry = scenario.state.programming.programmers();
        let selection = registry.selection(scenario.session.id).unwrap();
        let request = serde_json::json!({
            "address":{"family":"mixed","number":150},
            "expected_preset_revision":recorded.revision,
            "expected_show_revision":store.portable_revision().unwrap().value(),
            "expected_programmer_revision":registry.normal_values_revision(),
            "expected_capture_mode_revision":registry.capture_mode_revision(),
            "expected_selection_revision":selection.revision,
        });
        assert_eq!(
            scenario
                .preset_recall_action(&show_id, Some(&scenario.token), request)
                .await
                .status(),
            StatusCode::OK
        );
    } else {
        assert_eq!(
            scenario
                .execute("tl784-recall", Some("FIXTURE 1 AT 0.150"))
                .await
                .status(),
            StatusCode::OK
        );
    }
    assert_eq!(
        scenario
            .state
            .programming
            .get(scenario.session.id)
            .unwrap()
            .active_context
            .as_deref(),
        Some("preset:0.150")
    );
    assert_eq!(
        scenario
            .execute("tl784-edit", Some("FIXTURE 1 AT 80"))
            .await
            .status(),
        StatusCode::OK
    );
    let alias_preview = scenario
        .app
        .clone()
        .oneshot(
            Request::post("/api/v2/programming-update/preview")
                .header(header::AUTHORIZATION, format!("Bearer {}", scenario.token))
                .header("x-tosk-show", &show_id)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::json!({
                        "request_id":"tl784-explicit-alias-preview",
                        "target":{"type":"preset","object_id":"0.150"},
                        "mode":{"target_type":"existing_content","mode":"update_existing"}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let alias_status = alias_preview.status();
    let alias_body = json(alias_preview).await;
    let unchanged_after_alias = store
        .objects("preset")
        .unwrap()
        .into_iter()
        .find(|object| object.id == "150")
        .unwrap();
    assert_eq!(unchanged_after_alias.body, recorded.body);
    assert_eq!(
        alias_status,
        StatusCode::NOT_FOUND,
        "explicit API IDs must remain exact: {alias_body}"
    );
    let targets = tl784_targets(&scenario, &show_id).await;
    let entries = targets["targets"].as_array().unwrap();
    assert_eq!(entries.len(), 1, "{targets}");
    assert_eq!(entries[0]["object"]["object_id"], "150");
    assert_eq!(entries[0]["object"]["object_revision"], recorded.revision);
    assert_eq!(entries[0]["request_target"]["object_id"], "150");
    let after = store
        .objects("preset")
        .unwrap()
        .into_iter()
        .find(|object| object.id == "150")
        .unwrap();
    assert_eq!(
        after.body, recorded.body,
        "opening/cancelling Targets must not write the preset"
    );
    assert_eq!(after.body["instance_id"], instance);
    let confirm = |request_id: &str, revision: u64| {
        serde_json::json!({
            "request_id":request_id,
            "action":{
                "type":"confirm_preview", "target":entries[0]["request_target"],
                "mode":entries[0]["existing_preview"]["mode"],
                "expected_object_revision":revision,
                "expected_programmer_revision":entries[0]["programmer_revision"],
            }
        })
    };
    let post = |body: serde_json::Value| {
        scenario.app.clone().oneshot(
            Request::post("/api/v2/programming-update/actions")
                .header(header::AUTHORIZATION, format!("Bearer {}", scenario.token))
                .header("x-tosk-show", &show_id)
                .header(
                    header::IF_MATCH,
                    targets["show_revision"].as_u64().unwrap().to_string(),
                )
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
    };
    let direct_alias = post(serde_json::json!({
        "request_id":"tl784-explicit-alias-action",
        "action":{
            "type":"apply_direct", "target":{"type":"preset","object_id":"0.150"},
            "mode":{"target_type":"existing_content","mode":"update_existing"}
        }
    }))
    .await
    .unwrap();
    assert_eq!(direct_alias.status(), StatusCode::NOT_FOUND);
    let unchanged_after_action = store
        .objects("preset")
        .unwrap()
        .into_iter()
        .find(|object| object.id == "150")
        .unwrap();
    assert_eq!(unchanged_after_action.body, recorded.body);
    let stale = post(confirm("tl784-stale", recorded.revision + 1))
        .await
        .unwrap();
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    let unchanged = store
        .objects("preset")
        .unwrap()
        .into_iter()
        .find(|object| object.id == "150")
        .unwrap();
    assert_eq!(unchanged.body, recorded.body);
    let applied = post(confirm("tl784-confirm", recorded.revision))
        .await
        .unwrap();
    assert_eq!(applied.status(), StatusCode::OK);
    let applied = json(applied).await;
    assert_eq!(applied["projection"]["object_id"], "150");
    let committed = store
        .objects("preset")
        .unwrap()
        .into_iter()
        .find(|object| object.id == "150")
        .unwrap();
    assert_eq!(committed.body["instance_id"], instance);
    assert_eq!(committed.revision, recorded.revision + 1);
    let value = committed.body["values"][fixture.fixture_id.0.to_string()]["intensity"]["value"]
        .as_f64()
        .unwrap();
    assert!((value - 0.8).abs() < 1e-7);
    assert!(
        store
            .objects("preset")
            .unwrap()
            .iter()
            .all(|object| object.id != "0.150")
    );
    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn tl784_command_recall_offers_actual_recorded_mixed_object() {
    tl784_record_recall_targets(false).await;
}

#[tokio::test]
async fn tl784_tile_recall_offers_actual_recorded_mixed_object() {
    tl784_record_recall_targets(true).await;
}

async fn tl784_typed_update_alias(mode: &str, includes_new: bool) {
    let scenario = CommandHttpScenario::new().await;
    let show_id = scenario
        .create_and_open_show("TL784 trusted typed alias")
        .await;
    let mut fixtures = Vec::new();
    for number in [1, 2] {
        let mut fixture = super::template_dimmer("TL784 typed dimmer", number);
        fixture.fixture_number = Some(u32::from(number));
        assert_eq!(
            scenario
                .put_active_object(
                    &show_id,
                    "patched_fixture",
                    &fixture.fixture_id.0.to_string(),
                    0,
                    serde_json::to_value(&fixture).unwrap()
                )
                .await
                .status(),
            StatusCode::OK
        );
        fixtures.push(fixture);
    }
    assert_eq!(
        scenario
            .execute("typed-seed", Some("FIXTURE 1 AT 50"))
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        scenario
            .execute("typed-record", Some("RECORD 0.150"))
            .await
            .status(),
        StatusCode::OK
    );
    let show = scenario.state.active_show.current().clone().unwrap();
    let store = ShowStore::open(&show.path).unwrap();
    let before = store
        .objects("preset")
        .unwrap()
        .into_iter()
        .find(|o| o.id == "150")
        .unwrap();
    assert_eq!(
        scenario
            .execute("typed-edit", Some("FIXTURE 1 THRU 2 AT 80"))
            .await
            .status(),
        StatusCode::OK
    );
    let command = format!("UPDATE {mode} ALL PRESET 150");
    let response = scenario.execute("typed-update", Some(&command)).await;
    let status = response.status();
    let body = json(response).await;
    assert_eq!(status, StatusCode::OK, "actual command {command}: {body}");
    let objects = store.objects("preset").unwrap();
    assert!(objects.iter().all(|o| o.id != "0.150"));
    let after = objects.iter().find(|o| o.id == "150").unwrap();
    assert_eq!(
        after.revision,
        before.revision + 1,
        "actual command {command}: {body}"
    );
    assert_eq!(after.body["instance_id"], before.body["instance_id"]);
    assert_eq!(after.body["name"], before.body["name"]);
    let value = after.body["values"][fixtures[0].fixture_id.0.to_string()]["intensity"]["value"]
        .as_f64()
        .unwrap();
    assert!((value - 0.8).abs() < 1e-7);
    assert_eq!(
        after.body["values"]
            .get(fixtures[1].fixture_id.0.to_string())
            .is_some(),
        includes_new
    );
    // Alias resolution freezes portable authority before the existing Update commit boundary.
    let request = super::super::show_command_update::update_request(
        &scenario.state,
        &scenario.session,
        &["ALL".into(), "PRESET".into(), "150".into()],
        &scenario.state.output.snapshot(),
    )
    .unwrap();
    assert_eq!(request.target.object_id.as_deref(), Some("150"));
    assert_eq!(
        request.expected_show_revision,
        Some(store.portable_revision().unwrap().value())
    );
    store.put_object("preset", "0.150", &after.body, 0).unwrap();
    let preset_state = || {
        store
            .objects("preset")
            .unwrap()
            .into_iter()
            .map(|object| (object.id, object.revision, object.body))
            .collect::<Vec<_>>()
    };
    let duplicated = preset_state();
    let stale = perform_update(&scenario.state, &scenario.session, &request);
    assert!(
        stale.is_err(),
        "frozen authority must reject intervening alias creation"
    );
    assert_eq!(preset_state(), duplicated);
    let _ = scenario.execute("typed-ambiguous", Some(&command)).await;
    assert_eq!(
        preset_state(),
        duplicated,
        "ambiguous command must never choose a source"
    );
    let _ = scenario
        .execute("typed-missing", Some("UPDATE ALL PRESET 151"))
        .await;
    assert_eq!(
        preset_state(),
        duplicated,
        "missing address must not create an object"
    );

    let _ = std::fs::remove_dir_all(scenario.data_dir);
}

#[tokio::test]
async fn tl784_trusted_typed_smart_resolves_bare_mixed_object() {
    tl784_typed_update_alias("SMART", false).await;
}

#[tokio::test]
async fn tl784_trusted_typed_merge_resolves_bare_mixed_object() {
    tl784_typed_update_alias("MERGE", true).await;
}

#[tokio::test]
async fn tl784_trusted_typed_plain_all_preset_resolves_bare_mixed_object() {
    tl784_typed_update_alias("", false).await;
}
