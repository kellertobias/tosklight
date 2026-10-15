//! Actual HTTP preview/apply acceptance for TL-809; no production helper is substituted.
use super::*;

const NATIVE: u128 = 809_003;

fn primary_archive() -> MvrDocument {
    let profile = pan_profile();
    let mut fixtures = vec![
        source(809_001, "Rig.gdtf", 1),
        source(809_002, "Rig.gdtf", 2),
        source(809_004, "Rig.gdtf", 20),
    ];
    for fixture in &mut fixtures {
        fixture.name = "Repeated name".into();
    }
    MvrDocument {
        fixtures,
        files: HashMap::from([(
            "Rig.gdtf".into(),
            light_fixture::gdtf::profile::package_profile(&profile).unwrap(),
        )]),
        ..Default::default()
    }
}

fn native_archive(primary: bool) -> MvrDocument {
    let mut profile = pan_profile();
    let mode = &mut profile.modes[0];
    let mut tilt = mode.channels[0].clone();
    tilt.id = Uuid::new_v4();
    tilt.split = 2;
    tilt.attribute = light_core::AttributeKey("tilt".into());
    tilt.fixture_attribute = tilt.attribute.clone();
    tilt.functions[0].id = Uuid::new_v4();
    tilt.functions[0].attribute = tilt.attribute.clone();
    mode.channels.push(tilt);
    mode.splits.push(light_fixture::FixtureSplit {
        number: 2,
        footprint: 3,
    });
    let (mut fixture, _, _) = schema_v2_direct_fixture();
    fixture.definition = profile.resolved_definition(profile.modes[0].id).unwrap();
    fixture.universe = primary.then_some(1);
    fixture.address = primary.then_some(1);
    fixture.split_patches = vec![
        light_fixture::SplitPatch {
            split: 1,
            universe: fixture.universe,
            address: fixture.address,
        },
        light_fixture::SplitPatch {
            split: 2,
            universe: Some(1),
            address: Some(100),
        },
    ];
    fixture.multipatch = vec![serde_json::from_value(serde_json::json!({
        "id":Uuid::from_u128(809_500), "name":"Physical copy", "universe":1, "address":200,
        "split_patches":[{"split":1,"universe":1,"address":200},{"split":2,"universe":null,"address":null}]
    })).unwrap()];
    let mut native_source = source(NATIVE, "Rig.gdtf", 1);
    native_source.universe = fixture.universe;
    native_source.address = fixture.address;
    let mut fixtures = vec![
        source(809_001, "Rig.gdtf", 100),
        source(809_002, "Rig.gdtf", 200),
        native_source,
        source(809_004, "Rig.gdtf", 500),
    ];
    for source in &mut fixtures {
        source.name = "Repeated name".into();
    }
    MvrDocument { fixtures, files: HashMap::from([
        ("Rig.gdtf".into(), light_fixture::gdtf::profile::package_profile(&profile).unwrap()),
        (light_application::mvr_export::TOSKLIGHT_MVR_FIXTURE_METADATA_PATH.into(), serde_json::to_vec(&serde_json::json!({"version":1,"fixtures":[{"mvr_uuid":Uuid::from_u128(NATIVE),"fixture":fixture}]})).unwrap()),
    ]), ..Default::default() }
}

#[tokio::test]
async fn tl809_new_preview_marks_only_later_incoming_loser_by_source_uuid_despite_repeated_names() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let document = primary_archive();
    let before = state.active_show.current().map(|entry| entry.id);
    for _ in 0..2 {
        let shown = preview(&app, &token, &document, None).await;
        assert!(
            !shown["address_conflicts"].as_array().unwrap().is_empty(),
            "incoming overlap must be visible in New Show preview: {shown}"
        );
        assert_eq!(
            shown["address_conflict_fixture_ids"],
            serde_json::json!([Uuid::from_u128(809_002)]),
            "name matching must not mark the first or unrelated same-name fixture"
        );
        assert_eq!(
            shown["fixtures"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| row["uuid"].clone())
                .collect::<Vec<_>>(),
            document
                .fixtures
                .iter()
                .map(|fixture| serde_json::json!(fixture.uuid))
                .collect::<Vec<_>>()
        );
    }
    assert_eq!(
        state.active_show.current().map(|entry| entry.id),
        before,
        "preview never switches shows"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn tl809_preview_includes_native_secondary_and_physical_copy_outputs_without_primary() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    for primary in [true, false] {
        let shown = preview(&app, &token, &native_archive(primary), None).await;
        assert!(
            shown["fixtures"]
                .as_array()
                .unwrap()
                .iter()
                .all(|fixture| fixture["matched"] == true),
            "all test inputs must have known exact profiles: {shown}"
        );
        assert_eq!(
            shown["address_conflict_fixture_ids"],
            serde_json::json!([Uuid::from_u128(NATIVE)]),
            "later native fixture loses on retained split/copy, even without primary: {shown}"
        );
        assert!(!shown["address_conflicts"].as_array().unwrap().is_empty());
    }
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn tl809_apply_unpatches_all_outputs_of_later_native_conflict_and_preserves_other_sources() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let document = native_archive(true);
    let shown = preview(&app, &token, &document, None).await;
    let result = apply(
        &app,
        &token,
        &shown,
        serde_json::json!({"type":"new_show","name":"All-output conflict"}),
        serde_json::json!([]),
    )
    .await;
    assert_eq!(result["imported_fixtures"], 4);
    let show = stored_show(&state, &result);
    assert_eq!(show.objects_of_kind("patched_fixture").count(), 4);
    let association = show
        .object("mvr_fixture", &Uuid::from_u128(NATIVE).to_string())
        .unwrap();
    let native_id = association.body()["fixture_id"].as_str().unwrap();
    let record = show.object("patched_fixture", native_id).unwrap();
    let patch = light_fixture::PortablePatchedFixtureRecord::decode(record.body().clone())
        .unwrap()
        .patch()
        .unwrap();
    assert_eq!(patch.name, "Repeated name");
    assert_eq!(patch.split_patches.len(), 2);
    assert_eq!(
        patch.multipatch.len(),
        1,
        "unpatch preserves the physical copy"
    );
    assert_eq!((patch.universe, patch.address), (None, None));
    assert!(
        patch
            .split_patches
            .iter()
            .all(|split| split.universe.is_none() && split.address.is_none()),
        "automatic fallback clears every root split"
    );
    assert!(
        patch.multipatch.iter().all(|copy| copy.universe.is_none()
            && copy.address.is_none()
            && copy
                .split_patches
                .iter()
                .all(|split| split.universe.is_none() && split.address.is_none())),
        "automatic fallback clears every physical copy and its splits"
    );
    let occupied = light_application::mvr_import::occupied_patches(&show).unwrap();
    assert!(
        occupied.iter().all(|row| row.3 != native_id),
        "compiled occupancy must exclude every output of the unpatched native fixture"
    );
    assert_eq!(occupied.len(), 3);
    assert_eq!(
        occupied
            .iter()
            .map(|row| (row.0, row.1, row.2))
            .collect::<std::collections::BTreeSet<_>>(),
        [(1, 100, 3), (1, 200, 3), (1, 500, 3)]
            .into_iter()
            .collect()
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn tl809_destination_reimport_exempts_only_same_source_and_other_sources_see_secondary_occupancy()
 {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let mut document = native_archive(false);
    document
        .fixtures
        .retain(|fixture| fixture.uuid == Uuid::from_u128(NATIVE));
    let shown = preview(&app, &token, &document, None).await;
    let result = apply(
        &app,
        &token,
        &shown,
        serde_json::json!({"type":"new_show","name":"Retained native outputs"}),
        serde_json::json!([]),
    )
    .await;
    let show = stored_show(&state, &result);
    let before = show.revision();
    let same = preview(&app, &token, &document, Some(show.id())).await;
    assert!(
        same["address_conflicts"].as_array().unwrap().is_empty(),
        "all retained outputs belong to the same source"
    );
    assert_eq!(same["address_conflict_fixture_ids"], serde_json::json!([]));
    document.fixtures = vec![source(809_600, "Rig.gdtf", 100)];
    let other = preview(&app, &token, &document, Some(show.id())).await;
    assert_eq!(
        other["address_conflict_fixture_ids"],
        serde_json::json!([Uuid::from_u128(809_600)])
    );
    assert_eq!(
        stored_show(&state, &result).revision(),
        before,
        "preview cannot mutate destination state"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn tl809_chosen_primary_address_cannot_leave_conflicting_secondary_or_copy_outputs() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let document = native_archive(true);
    let shown = preview(&app, &token, &document, None).await;
    let result = apply(&app, &token, &shown,
        serde_json::json!({"type":"new_show","name":"Chosen primary retains conflict"}),
        serde_json::json!([{"fixture_id":Uuid::from_u128(NATIVE),"action":{"type":"address","universe":1,"address":300}}]),
    ).await;
    let show = stored_show(&state, &result);
    assert_eq!(show.objects_of_kind("patched_fixture").count(), 4);
    let association = show
        .object("mvr_fixture", &Uuid::from_u128(NATIVE).to_string())
        .unwrap();
    let native_id = association.body()["fixture_id"].as_str().unwrap();
    let occupied = light_application::mvr_import::occupied_patches(&show).unwrap();
    assert!(
        occupied.iter().all(|row| row.3 != native_id),
        "changing only the primary must not retain conflicting secondary/copy outputs"
    );
    assert_eq!(
        occupied
            .iter()
            .map(|row| (row.0, row.1, row.2))
            .collect::<std::collections::BTreeSet<_>>(),
        [(1, 100, 3), (1, 200, 3), (1, 500, 3)]
            .into_iter()
            .collect()
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn tl809_replace_refuses_expanded_secondary_or_multiowner_removal_atomically() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    for primary in [1, 100] {
        let mut document = native_archive(true);
        document.fixtures[2].address = Some(primary);
        let shown = preview(&app, &token, &document, None).await;
        let active = state.active_show.current().map(|entry| entry.id);
        let inventory = || {
            state
                .installation
                .show_library()
                .unwrap()
                .into_iter()
                .map(|entry| (entry.id.0, entry.path, entry.revision))
                .collect::<std::collections::BTreeSet<_>>()
        };
        let before_library = inventory();
        let before_output = state.output.snapshot();
        let before_runtime = state.output.dynamic_runtime_snapshot();
        let name = format!("Refused expanded Replace {primary}");
        let response = app.clone().oneshot(show_action_request(&token, serde_json::json!({
            "type":"apply_mvr", "token":shown["token"],
            "destination":{"type":"new_show","name":name},
            "resolutions":[{"fixture_id":Uuid::from_u128(NATIVE),"action":{"type":"replace"}}]
        }))).await.unwrap();
        let status = response.status();
        let body = json(response).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert!(body.to_string().contains("secondary/copy"), "{body}");
        assert_eq!(state.active_show.current().map(|entry| entry.id), active);
        let after_library = inventory();
        assert_eq!(
            after_library.len(),
            before_library.len(),
            "refused import cannot register a partial show"
        );
        assert_eq!(
            after_library, before_library,
            "actual show IDs, paths and revisions must remain unchanged"
        );
        assert!(
            std::sync::Arc::ptr_eq(&state.output.snapshot(), &before_output),
            "refused import cannot publish a new output generation"
        );
        assert_eq!(state.output.dynamic_runtime_snapshot(), before_runtime);
    }
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn tl809_default_loser_does_not_reserve_its_suppressed_primary_for_following_sources() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let mut document = native_archive(true);
    document.fixtures.push(source(809_005, "Rig.gdtf", 1));
    let shown = preview(&app, &token, &document, None).await;
    assert_eq!(
        shown["address_conflict_fixture_ids"],
        serde_json::json!([Uuid::from_u128(NATIVE)])
    );
    let result = apply(
        &app,
        &token,
        &shown,
        serde_json::json!({"type":"new_show","name":"Freed primary after loser"}),
        serde_json::json!([]),
    )
    .await;
    let show = stored_show(&state, &result);
    assert_eq!(show.objects_of_kind("patched_fixture").count(), 5);
    let occupied = light_application::mvr_import::occupied_patches(&show).unwrap();
    assert_eq!(
        occupied
            .iter()
            .map(|row| (row.0, row.1, row.2))
            .collect::<std::collections::BTreeSet<_>>(),
        [(1, 1, 3), (1, 100, 3), (1, 200, 3), (1, 500, 3)]
            .into_iter()
            .collect()
    );
    let _ = std::fs::remove_dir_all(data_dir);
}
