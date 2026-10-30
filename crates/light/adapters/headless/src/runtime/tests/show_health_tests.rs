#[test]
fn show_health_detects_overlaps_missing_positions_and_references_without_inventing_empty_group_errors()
 {
    use semantic_contract_startup_support::ContractDesk;
    let desk = ContractDesk::new("health-scan");
    let show = desk.create_show("Health scan");
    let mut first = schema_v2_direct_fixture().0;
    let mut profile = first
        .definition
        .profile_snapshot
        .as_deref()
        .unwrap()
        .clone();
    profile.modes[0].splits[0].number = 2;
    for channel in &mut profile.modes[0].channels {
        channel.split = 2;
    }
    first.definition = profile.resolved_definition(profile.modes[0].id).unwrap();
    first.fixture_number = Some(1);
    first.location = Default::default();
    first.stage_position_set = true;
    let mut second = first.clone();
    second.fixture_id = light_core::FixtureId::new();
    second.fixture_number = Some(2);
    let mut third = first.clone();
    third.fixture_id = light_core::FixtureId::new();
    third.fixture_number = Some(3);
    third.universe = None;
    third.address = None;
    third.split_patches.clear();
    for fixture in [&first, &second, &third] {
        show.patch(fixture);
    }
    let store = show.store();
    let first_id = first.fixture_id.0;
    let mut extended = store.object("patched_fixture", &first_id.to_string()).unwrap().unwrap().body;
    extended["patch"] = serde_json::json!({"future_extension":true});
    show.put("patched_fixture", &first_id.to_string(), extended);
    let key = second.fixture_id.0.to_string();
    let mut missing_position = store.object("patched_fixture", &key).unwrap().unwrap().body;
    missing_position.as_object_mut().unwrap().remove("location");
    store.delete_object("patched_fixture", &key).unwrap();
    show.put("patched_fixture", "legacy-second", missing_position);
    let missing = Uuid::new_v4();
    show.put("group","bad",serde_json::json!({"id":"bad","name":"Broken","source":{"type":"explicit","fixture_ids":[first.fixture_id.0,missing]},"fixtures":[]}));
    show.put(
        "group",
        "empty",
        serde_json::json!({"id":"empty","name":"Empty","fixtures":[]}),
    );
    let document = store.portable_document().unwrap();
    let findings = super::show_health::analyze(
        document.id().0,
        &document,
        &EngineSnapshot {
            fixtures: vec![first, second, third].into(),
            ..Default::default()
        },
    )
    .findings;
    assert!(
        findings
            .iter()
            .any(|finding| finding.id.contains("overlap:"))
    );
    assert!(!findings.iter().any(|finding| finding.fixture_id == Some(first_id) && finding.id.contains("position-")), "explicit origin is placed; unrelated future fields do not replace flat metadata");
    assert!(
        findings
            .iter()
            .any(|finding| finding.id.ends_with("position-missing")
                && finding.object_id == "legacy-second")
    );
    assert!(
        !findings
            .iter()
            .any(|finding| finding.id.contains(":split:")),
        "legacy single split 2 is valid"
    );
    assert!(
        findings
            .iter()
            .any(|finding| finding.object_id == "bad" && finding.remove_missing_group_fixtures)
    );
    assert!(!findings.iter().any(|finding| finding.object_id == "empty"));
    assert!(
        findings
            .iter()
            .filter(|finding| finding.id.contains("unpatched:")
)
            .all(
                |finding| finding.severity == light_wire::v2::show_health::ShowHealthSeverity::Info
            )
    );
}

#[tokio::test]
async fn show_health_group_cleanup_is_one_replay_safe_undoable_intent_and_keeps_unpatched_members()
{
    use semantic_contract_startup_support::{ContractDesk, SEMANTIC_CONTRACT};
    let desk = ContractDesk::new("health-fix");
    let show = desk.create_show("Health cleanup");
    let mut fixture = schema_v2_direct_fixture().0;
    fixture.universe = None;
    fixture.address = None;
    fixture.split_patches.clear();
    show.patch(&fixture);
    let missing = Uuid::new_v4();
    let body = serde_json::json!({"id":"health","name":"Health Group","fixtures":[fixture.fixture_id.0,missing],"source":{"type":"explicit","fixture_ids":[fixture.fixture_id.0,missing]},"future":{"retain":[2,1]}});
    show.put("group", "health", body.clone());
    desk.activate(&show);
    let served = desk.serve(desk.load_at(SEMANTIC_CONTRACT)).await;
    let app = router(served.state.clone());
    let (token, _) = login(&app, "Operator").await;
    let get_health = || async {
        let response = app
            .clone()
            .oneshot(
                Request::get("/api/v2/show-health")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        json(response).await
    };
    let health = get_health().await;
    let original = show
        .store()
        .object("group", "health")
        .unwrap()
        .unwrap()
        .body;
    assert!(
        health["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["remove_missing_group_fixtures"] == true)
    );
    let revision = show
        .store()
        .object("group", "health")
        .unwrap()
        .unwrap()
        .revision;
    let request = serde_json::json!({"request_id":"health-clean","group_id":"health","operation":{"type":"remove_missing_fixtures"},"expected_object_revision":revision,"expected_show_revision":health["show_revision"]});
    let update = |request: serde_json::Value| {
        let app = app.clone();
        let token = token.clone();
        async move {
            app.clone()
                .oneshot(
                    Request::post("/api/v2/groups/health/update")
                        .header(header::AUTHORIZATION, format!("Bearer {token}"))
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(request.to_string()))
                        .unwrap(),
                )
                .await
                .unwrap()
        }
    };
    let baseline = served.state.events.latest_sequence();
    let group_events = || {
        let filter = light_application::EventFilter::default().with_object(
            light_application::EventObject::show_storage_object(show.entry.id, "group", "health"),
        );
        match served.state.events.replay(baseline, &filter) {
            light_application::EventReplay::Events(events) => events.len(),
            _ => panic!("Group event cursor must remain replayable"),
        }
    };
    let response = update(request.clone()).await;
    assert_eq!(response.status(), StatusCode::OK);
    let changed = json(response).await;
    let after = show.store().object("group", "health").unwrap().unwrap();
    assert_eq!(
        after.body["source"]["fixture_ids"],
        serde_json::json!([fixture.fixture_id.0])
    );
    assert_eq!(
        after.body["fixtures"],
        serde_json::json!([fixture.fixture_id.0])
    );
    assert_eq!(after.body["future"], body["future"]);
    assert_eq!(group_events(), 1);
    assert_eq!(update(request).await.status(), StatusCode::OK);
    assert_eq!(group_events(), 1);
    let undo = serde_json::json!({"request_id":"health-undo","group_id":"health","operation":{"type":"undo"},"expected_object_revision":after.revision,"expected_show_revision":changed["show_revision"]});
    assert_eq!(update(undo).await.status(), StatusCode::OK);
    assert_eq!(
        show.store()
            .object("group", "health")
            .unwrap()
            .unwrap()
            .body,
        original
    );
    assert!(
        get_health().await["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["remove_missing_group_fixtures"] == true)
    );
    served.close().await;
}

#[test]
fn show_health_stage_placement_migrates_legacy_coordinates_once_on_real_startup() {
    use semantic_contract_startup_support::{ContractDesk, SEMANTIC_CONTRACT};
    let desk = ContractDesk::new("stage-placement");
    let show = desk.create_show("Placement compatibility");
    let mut legacy = schema_v2_direct_fixture().0;
    legacy.location = Default::default();
    legacy.universe = None;
    legacy.address = None;
    legacy.split_patches.clear();
    legacy.multipatch.push(light_fixture::MultiPatchInstance {
        id: Uuid::new_v4(),
        ..Default::default()
    });
    show.patch(&legacy);
    let id = legacy.fixture_id.0.to_string();
    let mut body = show.store().object("patched_fixture", &id).unwrap().unwrap().body;
    body.as_object_mut().unwrap().remove("stage_position_set");
    body["multipatch"][0].as_object_mut().unwrap().remove("stage_position_set");
    body["multipatch"][0]["location"] = serde_json::json!({"x":0.0,"y":1.2,"z":0.0});
    body["future_placement"] = serde_json::json!({"keep": true});
    show.put("patched_fixture", &id, body);
    let mut untouched = legacy.clone();
    untouched.fixture_id = light_core::FixtureId::new();
    untouched.fixture_number = Some(2);
    untouched.stage_position_set = false;
    untouched.multipatch.clear();
    show.patch(&untouched);
    let mut absent = untouched.clone();
    absent.fixture_id = light_core::FixtureId::new();
    absent.fixture_number = Some(3);
    show.patch(&absent);
    let absent_id = absent.fixture_id.0.to_string();
    let mut body = show.store().object("patched_fixture", &absent_id).unwrap().unwrap().body;
    body.as_object_mut().unwrap().remove("location");
    body.as_object_mut().unwrap().remove("stage_position_set");
    show.put("patched_fixture", &absent_id, body);
    desk.activate(&show);
    let startup = desk.load_at(SEMANTIC_CONTRACT);
    assert!(startup.active_show_error.is_none(), "{:?}", startup.active_show_error);
    let document = show.store().portable_document().unwrap();
    let migrated = document.object("patched_fixture", &id).unwrap().body();
    assert_eq!(migrated["stage_position_set"], false, "legacy origin keeps its fallback placement until explicitly confirmed");
    assert_eq!(migrated["multipatch"][0]["stage_position_set"], true);
    assert_eq!(migrated["future_placement"], serde_json::json!({"keep": true}));
    assert_eq!(document.object("patched_fixture", &untouched.fixture_id.0.to_string()).unwrap().body()["stage_position_set"], false);
    assert_eq!(document.object("patched_fixture", &absent_id).unwrap().body()["stage_position_set"], false);
    let revision = document.revision();
    drop(startup);
    let reopened = desk.load_at(SEMANTIC_CONTRACT);
    assert!(reopened.active_show_error.is_none());
    assert_eq!(show.store().portable_document().unwrap().revision(), revision, "migration is idempotent");
    drop(reopened);
    let mut invalid = show.store().object("patched_fixture", &id).unwrap().unwrap().body;
    invalid["stage_position_set"] = serde_json::json!("invalid");
    show.put("patched_fixture", &id, invalid);
    settle_sqlite_wal_before_file_baseline(&show.path);
    let original = show_file_digest(&show.path);
    let recovered = desk.load_at(SEMANTIC_CONTRACT);
    assert!(recovered.active_show_error.is_some(), "malformed marker must be actionable recovery, never a silent overwrite");
    assert_eq!(show_file_digest(&show.path), original);
}

#[test]
fn show_health_stage_layout_placements_count_for_exact_physical_instances() {
    use semantic_contract_startup_support::ContractDesk;
    let desk = ContractDesk::new("health-layout-placement");
    let show = desk.create_show("Layout placement");
    let mut fixture = schema_v2_direct_fixture().0;
    fixture.stage_position_set = false;
    let placed_copy = Uuid::new_v4();
    let unplaced_copy = Uuid::new_v4();
    fixture.multipatch = [placed_copy, unplaced_copy].into_iter().map(|id| {
        light_fixture::MultiPatchInstance { id, ..Default::default() }
    }).collect();
    show.patch(&fixture);
    show.put("stage_layout", "main", serde_json::json!({
        "positions3d": {
            fixture.fixture_id.0.to_string(): {"x":0,"y":0,"z":0},
            placed_copy.to_string(): {"x":0,"y":0,"z":0}
        }
    }));
    let document = show.store().portable_document().unwrap();
    let findings = super::show_health::analyze(document.id().0, &document, &EngineSnapshot {
        fixtures: vec![fixture].into(), ..Default::default()
    }).findings;
    let placement_findings: Vec<_> = findings.iter().filter(|finding| finding.id.contains("position-")).collect();
    assert_eq!(placement_findings.len(), 1);
    assert_eq!(placement_findings[0].multipatch_instance_id, Some(unplaced_copy));
}
