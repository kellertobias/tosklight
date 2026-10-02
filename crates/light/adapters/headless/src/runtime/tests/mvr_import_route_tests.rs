use super::*;
use light_fixture::{ChannelFunction, ChannelFunctionBehavior, ChannelResolution, FixtureProfile};
use light_mvr::{MvrDocument, MvrFixture};

fn pan_profile() -> FixtureProfile {
    let (fixture, _, _) = schema_v2_direct_fixture();
    let mut profile = *fixture.definition.profile_snapshot.unwrap();
    profile.manufacturer = "MVR Precision".into();
    profile.name = "Source fixture".into();
    let mode = &mut profile.modes[0];
    mode.name = "Precise".into();
    mode.color_systems.clear();
    mode.control_actions.clear();
    mode.channels.truncate(1);
    let channel = &mut mode.channels[0];
    channel.attribute = light_core::AttributeKey("pan".into());
    channel.fixture_attribute = channel.attribute.clone();
    channel.resolution = ChannelResolution::U16;
    channel.secondary_slots = vec![3];
    channel.default_raw = 32769;
    channel.highlight_raw = 65534;
    channel.functions = vec![ChannelFunction::continuous(
        "Pan",
        channel.attribute.clone(),
        65535,
    )];
    channel.functions[0].behavior = ChannelFunctionBehavior::Continuous {
        physical_min: 540.0,
        physical_max: -540.0,
        unit: Some("degrees".into()),
    };
    mode.splits[0].footprint = 3;
    profile
}

fn source(uuid: u128, spec: &str, address: u16) -> MvrFixture {
    MvrFixture {
        uuid: Uuid::from_u128(uuid),
        name: format!("Source {uuid}"),
        fixture_id: None,
        gdtf_spec: spec.into(),
        gdtf_mode: "Precise".into(),
        universe: Some(1),
        address: Some(address),
        matrix: [1., 0., 0., 0., 1., 0., 0., 0., 1., 0., 0., 0.],
        layer: None,
        class: None,
    }
}

async fn preview(
    app: &Router,
    token: &str,
    document: &MvrDocument,
    show: Option<light_core::ShowId>,
) -> serde_json::Value {
    let path = show.map_or_else(
        || "/api/v2/mvr/imports/preview".into(),
        |id| format!("/api/v2/mvr/imports/preview?show_id={}", id.0),
    );
    let response = app
        .clone()
        .oneshot(
            Request::post(path)
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/zip")
                .body(Body::from(light_mvr::write(document).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = json(response).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

async fn apply(
    app: &Router,
    token: &str,
    preview: &serde_json::Value,
    destination: serde_json::Value,
    resolutions: serde_json::Value,
) -> serde_json::Value {
    let response = app.clone().oneshot(show_action_request(token, serde_json::json!({
        "type":"apply_mvr", "token": preview["token"], "destination":destination, "resolutions":resolutions,
    }))).await.unwrap();
    let status = response.status();
    let body = json(response).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["result"]["result"].clone()
}

fn stored_show(state: &AppState, result: &serde_json::Value) -> light_show::PortableShowDocument {
    let id = light_core::ShowId(Uuid::parse_str(result["show"]["id"].as_str().unwrap()).unwrap());
    let entry = state.installation.show(id).unwrap().unwrap();
    ActiveShowRepository::open(entry.path)
        .unwrap()
        .portable_document()
        .unwrap()
}

#[tokio::test]
async fn mvr_import_keeps_exact_archive_bindings_precision_and_distinct_revisions() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let profile = pan_profile();
    let first = light_fixture::gdtf::profile::package_profile(&profile).unwrap();
    let mut second_profile = profile.clone();
    second_profile.modes[0].channels[0].functions[0].behavior =
        ChannelFunctionBehavior::Continuous {
            physical_min: 270.,
            physical_max: -270.,
            unit: Some("degrees".into()),
        };
    let second = light_fixture::gdtf::profile::package_profile(&second_profile).unwrap();
    let document = MvrDocument {
        fixtures: vec![
            source(1, "Odd-Name.GDTF", 1),
            source(2, "Other.GDTF", 20),
            source(3, "Alias.GDTF", 40),
        ],
        files: HashMap::from([
            ("Odd-Name.GDTF".into(), first.clone()),
            ("Other.GDTF".into(), second.clone()),
            ("Alias.GDTF".into(), first.clone()),
        ]),
        ..Default::default()
    };
    let before = state.installation.fixture_profiles().unwrap().len();
    let shown = preview(&app, &token, &document, None).await;
    assert!(
        shown["fixtures"]
            .as_array()
            .unwrap()
            .iter()
            .all(|fixture| fixture["matched"] == true)
    );
    assert_eq!(
        state.installation.fixture_profiles().unwrap().len(),
        before,
        "preview cannot publish profiles"
    );
    // A competing editor save after preview must not change the staged meaning.
    let mut unrelated_revision = profile.clone();
    unrelated_revision.notes = "Edited after preview".into();
    state
        .installation
        .save_fixture_profile(unrelated_revision, 0)
        .unwrap();
    let result = apply(
        &app,
        &token,
        &shown,
        serde_json::json!({"type":"new_show","name":"Exact Source"}),
        serde_json::json!([]),
    )
    .await;
    assert_eq!(result["imported_fixtures"], 3);
    assert_eq!(result["unresolved_fixtures"], 0);
    let show = stored_show(&state, &result);
    assert_eq!(show.fixture_profile_revisions().len(), 2);
    let mut archives = Vec::new();
    for revision in show.fixture_profile_revisions() {
        assert!(revision.id().revision() > 1);
        let profile: FixtureProfile = serde_json::from_value(revision.profile().clone()).unwrap();
        let channel = &profile.modes[0].channels[0];
        assert_eq!(channel.resolution, ChannelResolution::U16);
        assert_eq!(channel.secondary_slots, vec![3]);
        assert_eq!(channel.default_raw, 32769);
        let retained = profile.source_gdtf.as_ref().unwrap();
        assert!(retained.matches_profile(&profile).unwrap());
        archives.push(retained.decoded_archive().unwrap());
        let published = state
            .installation
            .fixture_profile(profile.id, profile.revision)
            .unwrap()
            .unwrap();
        assert_eq!(
            serde_json::to_value(published).unwrap(),
            *revision.profile()
        );
    }
    assert!(archives.contains(&first) && archives.contains(&second));
    assert_eq!(
        state
            .installation
            .fixture_profile(profile.id, 1)
            .unwrap()
            .unwrap()
            .notes,
        "Edited after preview"
    );
    let fixtures = light_application::mvr_export::compile_export_fixtures(
        show.objects_of_kind("patched_fixture")
            .map(|o| (o.key().id().to_owned(), o.body().clone())),
        |reference| {
            show.fixture_profile_revision(reference.profile_id, reference.profile_revision)
                .map(|profile| {
                    light_fixture::ResolvedFixtureProfileRevision::new(
                        profile.id().profile_id(),
                        profile.id().revision(),
                        profile.digest().as_str(),
                        profile.profile().clone(),
                    )
                })
        },
    )
    .unwrap();
    assert_eq!(fixtures.len(), 3);
    // The portable show retains source even when the library is removed.
    for revision in state
        .installation
        .fixture_profile_revisions(profile.id)
        .unwrap()
    {
        state
            .installation
            .delete_fixture_profile(profile.id, revision.revision)
            .unwrap();
    }
    let (_, exported, _) = build_mvr_export(&state, show.id().0).unwrap();
    assert!(exported.files.values().any(|bytes| bytes == &first));
    assert!(exported.files.values().any(|bytes| bytes == &second));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn mvr_import_reports_invalid_ambiguous_missing_mode_and_unknown_sources_without_fallback() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let profile = pan_profile();
    state
        .installation
        .save_fixture_profile(profile.clone(), 0)
        .unwrap();
    let valid = light_fixture::gdtf::profile::package_profile(&profile).unwrap();
    let mut unknown = profile.clone();
    unknown.modes[0].channels[0].attribute =
        light_core::AttributeKey("custom.unknown_import".into());
    unknown.modes[0].channels[0].fixture_attribute = unknown.modes[0].channels[0].attribute.clone();
    unknown.modes[0].channels[0].functions[0].attribute =
        unknown.modes[0].channels[0].attribute.clone();
    let mut wrong_mode = source(12, "Valid.gdtf", 30);
    wrong_mode.gdtf_mode = "Absent".into();
    let document = MvrDocument {
        fixtures: vec![
            source(10, "Source fixture.gdtf", 1),
            source(11, "Ambiguous.gdtf", 20),
            wrong_mode,
            source(13, "Unknown.gdtf", 40),
        ],
        files: HashMap::from([
            ("Source fixture.gdtf".into(), b"malformed".to_vec()),
            ("one/Ambiguous.gdtf".into(), valid.clone()),
            ("two/Ambiguous.gdtf".into(), valid.clone()),
            ("Valid.gdtf".into(), valid),
            (
                "Unknown.gdtf".into(),
                light_fixture::gdtf::profile::package_profile(&unknown).unwrap(),
            ),
        ]),
        ..Default::default()
    };
    let shown = preview(&app, &token, &document, None).await;
    assert!(
        shown["fixtures"]
            .as_array()
            .unwrap()
            .iter()
            .all(|fixture| fixture["matched"] == false),
        "{shown}"
    );
    let warnings = shown["warnings"].to_string();
    assert!(
        warnings.contains("ambiguous")
            && warnings.contains("Absent")
            && warnings.contains("unmapped"),
        "{warnings}"
    );
    let result = apply(
        &app,
        &token,
        &shown,
        serde_json::json!({"type":"new_show","name":"Unresolved"}),
        serde_json::json!([]),
    )
    .await;
    assert_eq!(result["imported_fixtures"], 0);
    assert_eq!(result["unresolved_fixtures"], 4);
    assert!(result["warnings"].to_string().contains("unmapped"));
    let show = stored_show(&state, &result);
    assert_eq!(show.objects_of_kind("unresolved_mvr_fixture").count(), 4);
    assert_eq!(
        show.objects_of_kind(light_application::mvr_import::MVR_SOURCE_ARCHIVE_KIND)
            .count(),
        3,
        "same archive bytes are retained once even for ambiguous member names"
    );
    assert_eq!(
        show.object("unresolved_mvr_fixture", &Uuid::from_u128(11).to_string())
            .unwrap()
            .body()["retained_sources"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn mvr_import_skip_does_not_publish_any_profile() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let profile = pan_profile();
    let document = MvrDocument {
        fixtures: vec![source(20, "Skip.gdtf", 1)],
        files: HashMap::from([(
            "Skip.gdtf".into(),
            light_fixture::gdtf::profile::package_profile(&profile).unwrap(),
        )]),
        ..Default::default()
    };
    let shown = preview(&app, &token, &document, None).await;
    let result = apply(
        &app,
        &token,
        &shown,
        serde_json::json!({"type":"new_show","name":"Skip All"}),
        serde_json::json!([{"fixture_id":Uuid::from_u128(20),"action":{"type":"skip"}}]),
    )
    .await;
    assert_eq!(result["imported_fixtures"], 0);
    assert!(
        state
            .installation
            .fixture_profile_revisions(profile.id)
            .unwrap()
            .is_empty()
    );
    assert!(
        stored_show(&state, &result)
            .fixture_profile_revisions()
            .is_empty()
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn mvr_import_previews_lean_reference_occupancy_and_preserves_active_show_addresses() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let profile = pan_profile();
    let mut document = MvrDocument {
        fixtures: vec![source(30, "Rig.gdtf", 1)],
        files: HashMap::from([(
            "Rig.gdtf".into(),
            light_fixture::gdtf::profile::package_profile(&profile).unwrap(),
        )]),
        ..Default::default()
    };
    let shown = preview(&app, &token, &document, None).await;
    let result = apply(
        &app,
        &token,
        &shown,
        serde_json::json!({"type":"new_show","name":"Occupied"}),
        serde_json::json!([]),
    )
    .await;
    let show = stored_show(&state, &result);
    let object = show.objects_of_kind("patched_fixture").next().unwrap();
    assert!(
        object.body().get("definition").is_none(),
        "new import stores a lean reference plus the complete retained profile"
    );
    assert_eq!(
        show.id().0.to_string(),
        result["show"]["id"].as_str().unwrap()
    );
    let opened = app
        .clone()
        .oneshot(open_show_request(&token, show.id().0))
        .await
        .unwrap();
    assert_eq!(opened.status(), StatusCode::OK, "{}", json(opened).await);
    let reimport = preview(&app, &token, &document, Some(show.id())).await;
    assert!(
        reimport["address_conflicts"].as_array().unwrap().is_empty(),
        "an existing source mapping identifies the same fixture, not an address conflict"
    );
    document.fixtures = vec![source(31, "Rig.gdtf", 2)];
    let shown = preview(&app, &token, &document, Some(show.id())).await;
    assert_eq!(shown["address_conflicts"].as_array().unwrap().len(), 1);
    let result = apply(
        &app,
        &token,
        &shown,
        serde_json::json!({"type":"existing_show","show_id":show.id().0}),
        serde_json::json!([]),
    )
    .await;
    assert_eq!(result["imported_fixtures"], 1);
    assert!(
        result["warnings"]
            .to_string()
            .contains("imported unpatched")
    );
    let show = stored_show(&state, &result);
    assert_eq!(show.objects_of_kind("patched_fixture").count(), 2);
    assert_eq!(
        light_application::mvr_import::occupied_patches(&show)
            .unwrap()
            .len(),
        1
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn mvr_import_uses_only_verified_explicit_attribute_mapping_for_the_same_source() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let mut source_profile = pan_profile();
    let channel = &mut source_profile.modes[0].channels[0];
    channel.attribute = light_core::AttributeKey("custom.vendor_position".into());
    channel.fixture_attribute = channel.attribute.clone();
    channel.functions[0].attribute = channel.attribute.clone();
    let archive = light_fixture::gdtf::profile::package_profile(&source_profile).unwrap();
    let mut mapped = light_fixture::gdtf::read::import_profile(&archive)
        .unwrap()
        .profile;
    let channel = &mut mapped.modes[0].channels[0];
    channel.attribute = light_core::AttributeKey("pan".into());
    channel.fixture_attribute = channel.attribute.clone();
    channel.functions[0].attribute = channel.attribute.clone();
    let mapped = state
        .installation
        .save_fixture_profile_with_gdtf(mapped, 0, &archive)
        .unwrap();
    let document = MvrDocument {
        fixtures: vec![source(40, "Mapped.gdtf", 1)],
        files: HashMap::from([("Mapped.gdtf".into(), archive.clone())]),
        ..Default::default()
    };
    let shown = preview(&app, &token, &document, None).await;
    assert_eq!(shown["fixtures"][0]["matched"], true, "{shown}");
    assert!(
        shown["warnings"]
            .to_string()
            .contains("verified source mapping")
    );
    let result = apply(
        &app,
        &token,
        &shown,
        serde_json::json!({"type":"new_show","name":"Mapped Source"}),
        serde_json::json!([]),
    )
    .await;
    assert_eq!(result["imported_fixtures"], 1);
    assert_eq!(
        state
            .installation
            .fixture_profile_revisions(mapped.id)
            .unwrap()
            .len(),
        1,
        "the exact verified mapping reuses the existing revision"
    );
    let profile: FixtureProfile = serde_json::from_value(
        stored_show(&state, &result).fixture_profile_revisions()[0]
            .profile()
            .clone(),
    )
    .unwrap();
    assert_eq!(profile.modes[0].channels[0].attribute.0.as_ref(), "pan");
    assert_eq!(
        profile
            .source_gdtf
            .as_ref()
            .unwrap()
            .decoded_archive()
            .unwrap(),
        archive
    );
    let mut stale = mapped.clone();
    stale.notes = "Unverified later edit".into();
    state.installation.save_fixture_profile(stale, 1).unwrap();
    let shown = preview(&app, &token, &document, None).await;
    assert_eq!(
        shown["fixtures"][0]["matched"], false,
        "stale source evidence must not justify a mapping"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn mvr_import_multiple_breaks_keep_secondary_addresses_explicitly_unpatched() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
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
    let document = MvrDocument {
        fixtures: vec![source(50, "Breaks.gdtf", 1)],
        files: HashMap::from([(
            "Breaks.gdtf".into(),
            light_fixture::gdtf::profile::package_profile(&profile).unwrap(),
        )]),
        ..Default::default()
    };
    let shown = preview(&app, &token, &document, None).await;
    assert_eq!(shown["fixtures"][0]["matched"], true, "{shown}");
    assert!(
        shown["warnings"]
            .to_string()
            .contains("additional DMX breaks")
    );
    let result = apply(
        &app,
        &token,
        &shown,
        serde_json::json!({"type":"new_show","name":"Separate Breaks"}),
        serde_json::json!([]),
    )
    .await;
    assert_eq!(result["imported_fixtures"], 1);
    let show = stored_show(&state, &result);
    let body = show
        .objects_of_kind("patched_fixture")
        .next()
        .unwrap()
        .body();
    let patch = light_fixture::PortablePatchedFixtureRecord::decode(body.clone())
        .unwrap()
        .patch()
        .unwrap();
    assert_eq!(patch.split_patches.len(), 2);
    assert_eq!(patch.split_patches[0].address, Some(1));
    assert_eq!(patch.split_patches[1].address, None);
    assert_eq!(
        light_application::mvr_import::occupied_patches(&show)
            .unwrap()
            .len(),
        1
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[tokio::test]
async fn mvr_native_metadata_precedes_standard_source_and_conflicts_before_creating_a_show() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let mut profile = pan_profile();
    profile.revision = 7;
    profile.notes = "Native authored metadata".into();
    let (mut fixture, _, _) = schema_v2_direct_fixture();
    fixture.definition = profile.resolved_definition(profile.modes[0].id).unwrap();
    fixture.universe = Some(1);
    fixture.address = Some(1);
    fixture.split_patches.clear();
    fixture.invert_pan = true;
    let uuid = Uuid::from_u128(60);
    let document = MvrDocument { fixtures: vec![source(60, "Native.gdtf", 1)], files: HashMap::from([
        ("Native.gdtf".into(), b"deliberately unusable standard fallback".to_vec()),
        (light_application::mvr_export::TOSKLIGHT_MVR_FIXTURE_METADATA_PATH.into(), serde_json::to_vec(&serde_json::json!({"version":1,"fixtures":[{"mvr_uuid":uuid,"fixture":fixture}]})).unwrap()),
    ]), ..Default::default() };
    let shown = preview(&app, &token, &document, None).await;
    assert_eq!(shown["fixtures"][0]["matched"], true, "{shown}");
    let result = apply(
        &app,
        &token,
        &shown,
        serde_json::json!({"type":"new_show","name":"Native Exact"}),
        serde_json::json!([]),
    )
    .await;
    let show = stored_show(&state, &result);
    assert_eq!(
        show.fixture_profile_revisions()[0].profile()["notes"],
        "Native authored metadata"
    );
    assert_eq!(
        show.objects_of_kind("patched_fixture")
            .next()
            .unwrap()
            .body()["invert_pan"],
        true
    );
    let mut collision = profile.clone();
    collision.notes = "Unrelated immutable revision".into();
    state
        .installation
        .publish_fixture_profile_revision(&collision)
        .unwrap();
    let shown = preview(&app, &token, &document, None).await;
    let response = app.clone().oneshot(show_action_request(&token, serde_json::json!({"type":"apply_mvr","token":shown["token"],"destination":{"type":"new_show","name":"Must Not Exist"},"resolutions":[]}))).await.unwrap();
    assert_eq!(
        response.status(),
        StatusCode::CONFLICT,
        "{}",
        json(response).await
    );
    assert!(!data_dir.join("shows/Must Not Exist.show").exists());
    assert_eq!(
        state
            .installation
            .fixture_profile(profile.id, 7)
            .unwrap()
            .unwrap()
            .notes,
        "Unrelated immutable revision"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}
