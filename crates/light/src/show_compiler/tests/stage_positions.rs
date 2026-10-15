use super::super::compile_show_candidate;
use super::support::document_with_objects;
use light_core::FixtureId;
use light_dynamics::SpatialPosition;
use serde_json::json;
use uuid::Uuid;

#[test]
fn dynamic_stage_positions_retain_xyz_for_fixture_and_logical_head_identities() {
    let fixture_id = Uuid::new_v4();
    let logical_head_id = Uuid::new_v4();
    let legacy_2d_id = Uuid::new_v4();
    let objects = vec![(
        "stage_layout",
        "main",
        json!({
            "version": 2,
            "positions3d": {
                fixture_id.to_string(): {"x": 1.25, "y": 2.5, "z": 3.75},
                logical_head_id.to_string(): {"x": -4.0, "y": -5.0, "z": -6.0}
            },
            "positions": {
                legacy_2d_id.to_string(): {"x": 7.0, "y": 8.0}
            }
        }),
    )];
    let (_, document) = document_with_objects(&objects);

    let snapshot =
        compile_show_candidate(document.candidate(&document.transaction()).unwrap()).unwrap();

    assert_eq!(
        snapshot.dynamic_stage_positions.get(&FixtureId(fixture_id)),
        Some(&SpatialPosition {
            x: 1.25,
            y: 2.5,
            z: 3.75,
        })
    );
    assert_eq!(
        snapshot
            .dynamic_stage_positions
            .get(&FixtureId(logical_head_id)),
        Some(&SpatialPosition {
            x: -4.0,
            y: -5.0,
            z: -6.0,
        })
    );
    assert_eq!(
        snapshot
            .dynamic_stage_positions
            .get(&FixtureId(legacy_2d_id)),
        Some(&SpatialPosition {
            x: 7.0,
            y: 0.0,
            z: 8.0,
        })
    );
}

fn patch_position_document(
    xs: [i32; 4],
) -> (
    light_show::ShowStore,
    light_show::PortableShowDocument,
    Vec<FixtureId>,
) {
    use super::support::portable_fixture_with_policy;
    use light_fixture::{FixtureLocation, PatchPolicy, PortablePatchedFixtureRecord};
    let (store, _) = light_show::ShowStore::create(":memory:", "Physical spatial inputs").unwrap();
    let mut ids = Vec::new();
    for (index, x) in xs.into_iter().enumerate() {
        let (profile, mut fixture, _) =
            portable_fixture_with_policy(PatchPolicy::Dmx, 90_000 + index as u128 * 100);
        fixture.fixture_number = Some(index as u32 + 1);
        fixture.location = FixtureLocation { x, y: 0, z: 3000 };
        // An unpatched fixture remains a programming/spatial target.
        fixture.universe = None;
        fixture.address = None;
        ids.push(fixture.fixture_id);
        store.insert_fixture_profile_revision(&profile).unwrap();
        let body = PortablePatchedFixtureRecord::from_runtime_fixture(&fixture)
            .unwrap()
            .into_body();
        store
            .put_object(
                "patched_fixture",
                &fixture.fixture_id.0.to_string(),
                &body,
                0,
            )
            .unwrap();
    }
    store.put_object("group", "1", &json!({
        "id":"1", "name":"Spatial", "fixtures":ids,
        "mapping": {"projection":{"anchor":{"x":0,"y":0,"z":0},"preset":"top","rotation_degrees":0,"view_direction":{"x":0,"y":0,"z":-1}},"shape":{"type":"grid","angle_degrees":0,"direction":"ascending"}},
        "programming":{}
    }), 0).unwrap();
    let document = store.portable_document().unwrap();
    let transaction = super::super::prepare_show_candidate(&document, document.transaction())
        .unwrap()
        .into_parts()
        .0;
    if !transaction.is_empty() {
        store.apply_portable_transaction(transaction).unwrap();
    }
    let document = store.portable_document().unwrap();
    (store, document, ids)
}

fn physical_group_ranks(snapshot: &light_engine::EngineSnapshot, ids: &[FixtureId]) -> Vec<usize> {
    let groups = snapshot
        .groups
        .iter()
        .cloned()
        .map(|group| (group.id.clone(), group))
        .collect();
    let positions = snapshot
        .dynamic_stage_positions
        .iter()
        .map(|(id, position)| {
            (
                *id,
                light_dynamics::Position3d {
                    x: f64::from(position.x),
                    y: f64::from(position.y),
                    z: f64::from(position.z),
                },
            )
        })
        .collect();
    let group = light_programmer::resolve_group_spatial("1", &groups, &positions).unwrap();
    assert!(group.ranked_selection.warnings.is_empty());
    ids.iter()
        .map(|id| group.ranked_selection.rank_by_fixture[id])
        .collect()
}

#[test]
fn patch_positions_supply_cold_group_ranks_without_stage_layout_even_when_unpatched() {
    let (_store, document, ids) = patch_position_document([-1000, -1000, 1000, 3000]);
    assert!(document.objects_of_kind("stage_layout").next().is_none());
    let snapshot =
        compile_show_candidate(document.candidate(&document.transaction()).unwrap()).unwrap();
    assert_eq!(
        snapshot.dynamic_stage_positions.get(&ids[0]),
        Some(&SpatialPosition {
            x: -1.,
            y: 0.,
            z: 3.
        })
    );
    assert_eq!(physical_group_ranks(&snapshot, &ids), vec![0, 0, 1, 2]);
}

#[test]
fn patch_location_edit_rebuilds_spatial_inputs_and_matches_full_compile() {
    let (_store, document, ids) = patch_position_document([-3000, -1000, 1000, 3000]);
    let previous =
        compile_show_candidate(document.candidate(&document.transaction()).unwrap()).unwrap();
    let key = ids[0].0.to_string();
    let mut fixture = document
        .object("patched_fixture", &key)
        .unwrap()
        .body()
        .clone();
    fixture["location"]["x"] = json!(-1000);
    let mut transaction = document.transaction();
    transaction.put("patched_fixture", &key, fixture);
    let next = super::super::prepare_normalized_show_candidate_incremental(
        &document,
        transaction.clone(),
        &previous,
    )
    .unwrap()
    .into_parts()
    .1;
    assert_eq!(
        next.dynamic_stage_positions.get(&ids[0]),
        Some(&SpatialPosition {
            x: -1.,
            y: 0.,
            z: 3.
        })
    );
    assert!(!std::sync::Arc::ptr_eq(
        &next.dynamic_stage_positions,
        &previous.dynamic_stage_positions
    ));
    let full = compile_show_candidate(document.candidate(&transaction).unwrap()).unwrap();
    assert_eq!(next.dynamic_stage_positions, full.dynamic_stage_positions);
    assert_eq!(physical_group_ranks(&next, &ids), vec![0, 0, 1, 2]);
    assert!(std::sync::Arc::ptr_eq(&next.routes, &previous.routes));
}

#[test]
fn patch_owner_overrides_legacy_stage_position_including_origin_and_head_alias() {
    use super::super::spatial_positions::derive_for_patch;
    let (_, mut fixture, _) = super::support::portable_fixture();
    fixture.location = Default::default();
    fixture.position_master = Some(Uuid::new_v4());
    let root = fixture.fixture_id;
    let head = fixture.logical_heads[0].fixture_id;
    let legacy_only = FixtureId(Uuid::new_v4());
    let (_, document) = document_with_objects(&[(
        "stage_layout",
        "legacy",
        json!({
            "version": 2, "positions3d": {
                root.0.to_string(): {"x": 99, "y": 88, "z": 77},
                head.0.to_string(): {"x": 66, "y": 55, "z": 44},
                legacy_only.0.to_string(): {"x": 3, "y": 2, "z": 1}
            }
        }),
    )]);
    let tx = document.transaction();
    let positions = derive_for_patch(document.candidate(&tx).unwrap(), &[fixture]).unwrap();
    // A saved Point reference does not add its origin to an absolute resting mount.
    assert_eq!(
        positions[&root],
        SpatialPosition {
            x: 0.,
            y: 0.,
            z: 0.
        }
    );
    assert_eq!(positions[&head], positions[&root]);
    assert_eq!(
        positions[&legacy_only],
        SpatialPosition {
            x: 3.,
            y: 2.,
            z: 1.
        }
    );
}

#[test]
fn logical_head_geometry_uses_physical_metres_and_rotation_independent_of_visual_scale() {
    use super::super::spatial_positions::derive_for_patch;
    let (_, mut fixture, _) = super::support::portable_fixture();
    let mut profile = fixture
        .definition
        .profile_snapshot
        .as_ref()
        .unwrap()
        .as_ref()
        .clone();
    let head_id = profile.modes[0].heads[1].id;
    let node = Uuid::new_v4();
    profile.modes[0].geometry = serde_json::from_value(json!({
        "nodes": [{"id":node,"name":"Cell","parent_id":null,
          "transform":{"translation":{"x":1000,"y":2000,"z":3000},"rotation_degrees":{"x":0,"y":0,"z":0},"scale":{"x":0,"y":0,"z":0}}}],
        "emitters": [{"id":Uuid::new_v4(),"name":"Cell","node_id":node,"head_id":head_id,
          "beam_angle_degrees":10,"field_angle_degrees":20,"layout":{"type":"point"}}]
    }))
    .unwrap();
    fixture.definition.profile_snapshot = Some(std::sync::Arc::new(profile));
    fixture.logical_heads.truncate(1);
    fixture.logical_heads[0].profile_head_id = Some(head_id);
    fixture.location = light_fixture::FixtureLocation {
        x: 1000,
        y: 2000,
        z: 3000,
    };
    fixture.rotation.z = 90.;
    fixture.model_scale = Some(2.);
    fixture.position_master = Some(Uuid::new_v4());
    let head = fixture.logical_heads[0].fixture_id;
    let (_, document) = document_with_objects(&[]);
    let tx = document.transaction();
    let positions = derive_for_patch(document.candidate(&tx).unwrap(), &[fixture.clone()]).unwrap();
    fixture.model_scale = None;
    let unscaled = derive_for_patch(document.candidate(&tx).unwrap(), &[fixture]).unwrap();
    assert_eq!(positions, unscaled);
    // profile(1,2,3)m -> desk(1,-3,2), desk Z90 -> (3,1,2), + mount.
    assert_eq!(
        positions[&head],
        SpatialPosition {
            x: 4.,
            y: 3.,
            z: 5.
        }
    );
}

#[test]
fn valid_point_master_preserves_absolute_resting_patch_positions_on_cold_compile() {
    use light_fixture::{
        ChannelResolution, FixtureChannel, FixtureProfile, PatchPolicy,
        PortablePatchedFixtureRecord,
    };
    let (store, _, ids) = patch_position_document([-1000, -1000, 1000, 3000]);
    let (_, mut point, _) = super::support::portable_fixture_with_policy(PatchPolicy::Dmx, 100_000);
    let mut profile: FixtureProfile = point
        .definition
        .profile_snapshot
        .as_ref()
        .unwrap()
        .as_ref()
        .clone();
    let mode = &mut profile.modes[0];
    mode.name = "Point".into();
    mode.splits[0].footprint = 2;
    mode.channels.push(FixtureChannel {
        id: Uuid::from_u128(100_050),
        head_id: mode.heads[0].id,
        split: 1,
        fixture_attribute: light_core::AttributeKey("point.position.x".into()),
        attribute: light_core::AttributeKey("point.position.x".into()),
        canonical_transform: Default::default(),
        resolution: ChannelResolution::U16,
        secondary_slots: vec![2],
        default_raw: 32_768,
        highlight_raw: 32_768,
        physical_min: Some(-100.),
        physical_max: Some(100.),
        unit: Some("m".into()),
        invert: false,
        snap: false,
        reacts_to_virtual_intensity: false,
        virtual_intensity_inverted: false,
        behavior: Default::default(),
        functions: Vec::new(),
    });
    let mode_id = mode.id;
    point.definition = profile.resolved_definition(mode_id).unwrap();
    point.fixture_number = Some(5);
    point.location = light_fixture::FixtureLocation {
        x: 100_000,
        y: 200_000,
        z: 300_000,
    };
    point.rotation.z = 45.;
    let stored =
        light_show::FixtureProfileRevision::from_profile(serde_json::to_value(profile).unwrap())
            .unwrap();
    store.insert_fixture_profile_revision(&stored).unwrap();
    store
        .put_object(
            "patched_fixture",
            &point.fixture_id.0.to_string(),
            &PortablePatchedFixtureRecord::from_runtime_fixture(&point)
                .unwrap()
                .into_body(),
            0,
        )
        .unwrap();
    let document = store.portable_document().unwrap();
    let mut slave = document
        .object("patched_fixture", &ids[0].0.to_string())
        .unwrap()
        .body()
        .clone();
    slave["position_master"] = json!(point.fixture_id.0);
    let mut tx = document.transaction();
    tx.put("patched_fixture", ids[0].0.to_string(), slave);
    let snapshot = compile_show_candidate(document.candidate(&tx).unwrap()).unwrap();
    let compiled_point = snapshot
        .fixtures
        .iter()
        .find(|f| f.fixture_id == point.fixture_id)
        .unwrap();
    assert!(compiled_point.definition.heads.iter().any(|head| {
        head.parameters
            .iter()
            .any(|parameter| parameter.attribute.0.as_ref() == "point.position.x")
    }));
    assert_eq!(
        snapshot.dynamic_stage_positions[&point.fixture_id],
        SpatialPosition {
            x: 100.,
            y: 200.,
            z: 300.
        }
    );
    assert_eq!(
        snapshot.dynamic_stage_positions[&ids[0]],
        SpatialPosition {
            x: -1.,
            y: 0.,
            z: 3.
        }
    );
    assert_eq!(physical_group_ranks(&snapshot, &ids), vec![0, 0, 1, 2]);
}
