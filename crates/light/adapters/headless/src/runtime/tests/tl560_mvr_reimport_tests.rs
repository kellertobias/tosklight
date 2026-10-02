//! TL-560 matrix row "MVR import": MVR/GDTF import is patch and geometry only.
//!
//! A rig is imported from MVR into a new show through the real preview/apply routes and the
//! show is opened. Semantic programming that references the imported fixtures and a desk-owned
//! Point (which MVR does not carry) is stored in the show: a Position Preset with Angles on one
//! fixture and a Point Target on the other, a Color Preset on a live Group of both, and the live
//! Group itself. The same MVR fixture UUIDs are then re-imported into that show with a changed
//! GDTF (another Pan range) and moved placement. The re-import must update patch/geometry of the
//! same fixture identities, leave the Point alone, keep every programming object byte-identical
//! at its revision, never bind a stored reference to another fixture, and leave a show that the
//! show-open compiler accepts with the same references.
//!
//! The programming objects are written directly as typed bodies (the writers are covered by the
//! other TL-560 cells); the pan-only GDTF has no physical Position model, so this cell proves
//! storage and binding, not fitted output.
use super::*;
use crate::runtime::output_scheduler::physical_adapters::color::tests::{magenta, program};
use crate::runtime::output_scheduler::position_test_support::point;
use light_core::programming::{PositionIntent, ProgrammingOwner, TargetReference};
use light_fixture::{ChannelFunction, ChannelFunctionBehavior, ChannelResolution, FixtureProfile};
use light_mvr::{MvrDocument, MvrFixture};
use light_programmer::{GroupDefinition, Preset, PresetAddress, PresetFamily};

const GROUP: &str = "tl560-mvr-front";

fn pan_profile(range: f32) -> FixtureProfile {
    let (fixture, _, _) = schema_v2_direct_fixture();
    let mut profile = *fixture.definition.profile_snapshot.unwrap();
    profile.manufacturer = "TL-560 MVR".into();
    profile.name = "Re-imported mover".into();
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
    channel.functions = vec![ChannelFunction::continuous(
        "Pan",
        channel.attribute.clone(),
        65535,
    )];
    channel.functions[0].behavior = ChannelFunctionBehavior::Continuous {
        physical_min: range,
        physical_max: -range,
        unit: Some("degrees".into()),
    };
    mode.splits[0].footprint = 3;
    profile
}

fn source(uuid: u128, address: u16, x: f64) -> MvrFixture {
    MvrFixture {
        uuid: Uuid::from_u128(uuid),
        name: format!("TL-560 {uuid}"),
        fixture_id: None,
        gdtf_spec: "Mover.gdtf".into(),
        gdtf_mode: "Precise".into(),
        universe: Some(1),
        address: Some(address),
        matrix: [1., 0., 0., 0., 1., 0., 0., 0., 1., x, 0., 0.],
        layer: None,
        class: None,
    }
}

fn document(range: f32, x: f64) -> MvrDocument {
    MvrDocument {
        fixtures: vec![source(0x560_1, 1, x), source(0x560_2, 20, x + 500.)],
        files: HashMap::from([(
            "Mover.gdtf".into(),
            light_fixture::gdtf::profile::package_profile(&pan_profile(range)).unwrap(),
        )]),
        ..Default::default()
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
) -> serde_json::Value {
    let response = app
        .clone()
        .oneshot(show_action_request(
            token,
            serde_json::json!({
                "type":"apply_mvr", "token": preview["token"], "destination":destination,
                "resolutions":[],
            }),
        ))
        .await
        .unwrap();
    let status = response.status();
    let body = json(response).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["result"]["result"].clone()
}

fn document_of(state: &AppState, show: light_core::ShowId) -> light_show::PortableShowDocument {
    let entry = state.installation.show(show).unwrap().unwrap();
    ActiveShowRepository::open(entry.path)
        .unwrap()
        .portable_document()
        .unwrap()
}

/// `(object id, revision, body)` of every object of `kind`, sorted by id.
fn objects(
    document: &light_show::PortableShowDocument,
    kind: &str,
) -> Vec<(String, u64, serde_json::Value)> {
    let mut objects: Vec<_> = document
        .objects_of_kind(kind)
        .map(|o| (o.key().id().to_owned(), o.revision(), o.body().clone()))
        .collect();
    objects.sort_by(|a, b| a.0.cmp(&b.0));
    objects
}

/// MVR source UUID → bound fixture identity.
fn bindings(document: &light_show::PortableShowDocument) -> HashMap<Uuid, light_core::FixtureId> {
    light_application::mvr_import::mvr_destination_fixture_ids(document, &HashMap::new())
}

#[tokio::test]
async fn mvr_reimport_updates_patch_and_geometry_only_and_never_rebinds_semantic_programming() {
    let (state, data_dir) = test_state();
    let app = router(state.clone());
    let (token, _) = login(&app, "Operator").await;
    let shown = preview(&app, &token, &document(540., 0.), None).await;
    let result = apply(
        &app,
        &token,
        &shown,
        serde_json::json!({"type":"new_show","name":"TL-560 MVR"}),
    )
    .await;
    assert_eq!(result["imported_fixtures"], 2);
    let show_id =
        light_core::ShowId(Uuid::parse_str(result["show"]["id"].as_str().unwrap()).unwrap());
    let imported = document_of(&state, show_id);
    let bound = bindings(&imported);
    let (first, second) = (
        bound[&Uuid::from_u128(0x560_1)],
        bound[&Uuid::from_u128(0x560_2)],
    );

    // Desk-owned programming: a Point, a live Group and semantic Presets referencing them.
    let entry = state.installation.show(show_id).unwrap().unwrap();
    let aim = light_core::FixtureId::new();
    {
        let store = ShowStore::open(&entry.path).unwrap();
        let mut point = point(
            aim,
            light_fixture::FixtureLocation {
                x: 0,
                y: 4000,
                z: 0,
            },
        );
        // A numbered Point: every show candidate assigns a number to an unnumbered fixture,
        // which is general show normalization, not MVR behaviour.
        point.fixture_number = Some(900);
        let profile = point.definition.profile_snapshot.as_deref().unwrap();
        store
            .insert_fixture_profile_revision(
                &light_show::FixtureProfileRevision::from_profile(
                    serde_json::to_value(profile).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
        let body = light_fixture::PortablePatchedFixtureRecord::from_runtime_fixture(&point)
            .unwrap()
            .into_body();
        store
            .put_object("patched_fixture", &aim.0.to_string(), &body, 0)
            .unwrap();
        let group = GroupDefinition {
            id: GROUP.into(),
            name: "Front".into(),
            fixtures: vec![first, second],
            ..Default::default()
        };
        store
            .put_object("group", GROUP, &serde_json::to_value(group).unwrap(), 0)
            .unwrap();
        let position = ProgrammingOwner::Position.key();
        let aim_preset = Preset {
            name: "TL-560 aim".into(),
            family: PresetFamily::Position,
            number: 1,
            values: HashMap::from([
                (
                    first,
                    HashMap::from([(
                        position.clone(),
                        light_core::AttributeValue::Position(Arc::new(PositionIntent::angles(
                            -35.5, 72.25,
                        ))),
                    )]),
                ),
                (
                    second,
                    HashMap::from([(
                        position,
                        light_core::AttributeValue::Position(Arc::new(PositionIntent::target(
                            TargetReference::Point { point_id: aim.0 },
                            [0.5, -1.25, 2.0],
                        ))),
                    )]),
                ),
            ]),
            group_values: HashMap::new(),
            aim_at_fixture_number: None,
            universal_values: HashMap::new(),
        };
        let color_preset = Preset {
            name: "TL-560 front".into(),
            family: PresetFamily::Color,
            number: 1,
            values: HashMap::new(),
            group_values: HashMap::from([(
                GROUP.to_owned(),
                HashMap::from([(ProgrammingOwner::Color.key(), program(&magenta()))]),
            )]),
            aim_at_fixture_number: None,
            universal_values: HashMap::new(),
        };
        for preset in [aim_preset, color_preset] {
            let key = PresetAddress::new(preset.family, 1).unwrap().storage_key();
            store
                .put_object("preset", &key, &serde_json::to_value(preset).unwrap(), 0)
                .unwrap();
        }
    }
    let opened = app
        .clone()
        .oneshot(open_show_request(&token, show_id.0))
        .await
        .unwrap();
    assert_eq!(opened.status(), StatusCode::OK, "{}", json(opened).await);
    let before = document_of(&state, show_id);
    let programming = |document: &light_show::PortableShowDocument| {
        (objects(document, "preset"), objects(document, "group"))
    };
    let programmed = programming(&before);
    let fixtures_before = objects(&before, "patched_fixture");
    let point_before = fixtures_before
        .iter()
        .find(|(id, _, _)| *id == aim.0.to_string())
        .cloned()
        .unwrap();

    // Re-import the same sources with another GDTF Pan range and moved placement.
    let shown = preview(&app, &token, &document(270., 1500.), Some(show_id)).await;
    assert!(
        shown["address_conflicts"].as_array().unwrap().is_empty(),
        "the existing source mapping identifies the same fixtures: {shown}"
    );
    let result = apply(
        &app,
        &token,
        &shown,
        serde_json::json!({"type":"existing_show","show_id":show_id.0}),
    )
    .await;
    let after = document_of(&state, show_id);

    // Patch/geometry only: same fixture identities, updated patch, untouched Point.
    assert_eq!(bindings(&after), bound, "no source is rebound: {result}");
    let fixtures_after = objects(&after, "patched_fixture");
    assert_eq!(
        fixtures_after
            .iter()
            .map(|(id, _, _)| id)
            .collect::<Vec<_>>(),
        fixtures_before
            .iter()
            .map(|(id, _, _)| id)
            .collect::<Vec<_>>(),
        "no fixture is created, deleted or re-identified"
    );
    for id in [first, second] {
        let key = id.0.to_string();
        let old = fixtures_before.iter().find(|(i, _, _)| *i == key).unwrap();
        let new = fixtures_after.iter().find(|(i, _, _)| *i == key).unwrap();
        assert_ne!(old.2, new.2, "{key}: the re-import updates patch/geometry");
    }
    assert_eq!(
        fixtures_after
            .iter()
            .find(|(id, _, _)| *id == aim.0.to_string())
            .unwrap(),
        &point_before,
        "the desk-owned Point is not part of MVR and is untouched"
    );
    assert_eq!(
        programming(&after),
        programmed,
        "every Preset and Group is byte-identical at its revision"
    );

    // The re-imported show compiles through the show-open reader with the same references.
    let (_, snapshot) = light_application::prepare_show_candidate(&after, after.transaction())
        .unwrap()
        .into_parts();
    let ids: Vec<_> = snapshot.fixtures.iter().map(|f| f.fixture_id).collect();
    for id in [first, second, aim] {
        assert!(ids.contains(&id), "{id:?} is still a show fixture");
    }
    let group = snapshot.groups.iter().find(|g| g.id == GROUP).unwrap();
    assert_eq!(group.fixtures, vec![first, second]);
    let mover = snapshot
        .fixtures
        .iter()
        .find(|f| f.fixture_id == first)
        .unwrap();
    let profile = mover.definition.profile_snapshot.as_deref().unwrap();
    assert!(
        matches!(
            profile.modes[0].channels[0].functions[0].behavior,
            ChannelFunctionBehavior::Continuous { physical_min, physical_max, .. }
                if physical_min == 270. && physical_max == -270.
        ),
        "the new GDTF is the patched profile"
    );
    let _ = std::fs::remove_dir_all(data_dir);
}
