use super::*;

#[test]
fn rejects_patch_overlap_and_boundary_overflow() {
    let def = definition(10);
    let first = PatchedFixture {
        fixture_id: FixtureId::new(),
        fixture_number: None,
        virtual_fixture_number: None,
        name: "First".into(),
        definition: def.clone(),
        universe: Some(1),
        address: Some(1),
        split_patches: vec![],
        layer_id: default_patch_layer(),
        direct_control: None,
        location: Default::default(),
        rotation: Default::default(),
        logical_heads: vec![],
        multipatch: vec![],
        move_in_black_enabled: true,
        move_in_black_delay_millis: 0,
        highlight_overrides: BTreeMap::new(),
    };
    let overlap = PatchedFixture {
        fixture_id: FixtureId::new(),
        fixture_number: None,
        virtual_fixture_number: None,
        name: "Overlap".into(),
        definition: def.clone(),
        universe: Some(1),
        address: Some(10),
        split_patches: vec![],
        layer_id: default_patch_layer(),
        direct_control: None,
        location: Default::default(),
        rotation: Default::default(),
        logical_heads: vec![],
        multipatch: vec![],
        move_in_black_enabled: true,
        move_in_black_delay_millis: 0,
        highlight_overrides: BTreeMap::new(),
    };
    assert!(validate_patch(&[first.clone(), overlap]).is_err());
    let overflow = PatchedFixture {
        fixture_id: FixtureId::new(),
        fixture_number: None,
        virtual_fixture_number: None,
        name: "Overflow".into(),
        definition: def,
        universe: Some(1),
        address: Some(504),
        split_patches: vec![],
        layer_id: default_patch_layer(),
        direct_control: None,
        location: Default::default(),
        rotation: Default::default(),
        logical_heads: vec![],
        multipatch: vec![],
        move_in_black_enabled: true,
        move_in_black_delay_millis: 0,
        highlight_overrides: BTreeMap::new(),
    };
    assert!(validate_patch(&[overflow]).is_err());
    assert!(validate_patch(&[first]).is_ok());
}
#[test]
fn multipatch_reserves_real_addresses_and_allows_visualizer_only_instances() {
    let mut fixture = PatchedFixture {
        fixture_id: FixtureId::new(),
        fixture_number: None,
        virtual_fixture_number: None,
        name: "Multi".into(),
        definition: definition(3),
        universe: Some(1),
        address: Some(1),
        split_patches: vec![],
        layer_id: default_patch_layer(),
        direct_control: None,
        location: Default::default(),
        rotation: Default::default(),
        logical_heads: vec![],
        multipatch: vec![
            MultiPatchInstance {
                id: Uuid::new_v4(),
                name: "Output".into(),
                universe: Some(1),
                address: Some(10),
                split_patches: vec![],
                location: Default::default(),
                rotation: Default::default(),
            },
            MultiPatchInstance {
                id: Uuid::new_v4(),
                name: "Visual".into(),
                universe: None,
                address: None,
                split_patches: vec![],
                location: Default::default(),
                rotation: Default::default(),
            },
        ],
        move_in_black_enabled: true,
        move_in_black_delay_millis: 0,
        highlight_overrides: BTreeMap::new(),
    };
    assert!(validate_patch(std::slice::from_ref(&fixture)).is_ok());
    fixture.multipatch[1].universe = Some(1);
    assert!(validate_patch(std::slice::from_ref(&fixture)).is_err());
    fixture.multipatch[1].address = Some(2);
    assert!(validate_patch(&[fixture]).is_err());
}

#[test]
fn stable_fixture_identities_are_unique_across_the_complete_patch() {
    let fixture = schema_v2_two_split_fixture();
    let mut duplicate_fixture = fixture.clone();
    duplicate_fixture.fixture_number = Some(2);
    assert_identity_error(&[fixture.clone(), duplicate_fixture], "stable fixture");

    let mut head_uses_parent = fixture;
    head_uses_parent.logical_heads[0].fixture_id = head_uses_parent.fixture_id;
    assert_identity_error(&[head_uses_parent], "stable fixture");
}

#[test]
fn logical_head_identities_and_selected_mode_topology_are_exact() {
    let mut fixture = schema_v2_two_split_fixture();
    let duplicate = fixture.logical_heads[0].clone();
    fixture.logical_heads.push(duplicate);
    assert_identity_error(&[fixture], "logical head index 1");

    let mut missing = schema_v2_two_split_fixture();
    missing.logical_heads.clear();
    assert_identity_error(&[missing], "selected mode");

    let mut duplicate_ids = fixture_with_two_child_heads();
    duplicate_ids.logical_heads[1].fixture_id = duplicate_ids.logical_heads[0].fixture_id;
    assert_identity_error(&[duplicate_ids], "stable fixture");
}

#[test]
fn multipatch_identities_are_unique_across_all_stable_entities() {
    let mut duplicate_multipatch = schema_v2_two_split_fixture();
    duplicate_multipatch
        .multipatch
        .push(duplicate_multipatch.multipatch[0].clone());
    assert_identity_error(&[duplicate_multipatch], "multipatch identity");

    let mut overlapping_kinds = schema_v2_two_split_fixture();
    overlapping_kinds.multipatch[0].id = overlapping_kinds.logical_heads[0].fixture_id.0;
    assert_identity_error(&[overlapping_kinds], "multipatch identity");
}

fn fixture_with_two_child_heads() -> PatchedFixture {
    let mut fixture = schema_v2_two_split_fixture();
    let mut profile = fixture.definition.profile_snapshot.take().unwrap();
    profile.modes[0].heads.push(FixtureHead {
        id: Uuid::new_v4(),
        name: "Third".into(),
        master_shared: false,
    });
    fixture.definition = profile.resolved_definition(profile.modes[0].id).unwrap();
    reconcile_logical_heads(&mut fixture);
    fixture
}

fn assert_identity_error(fixtures: &[PatchedFixture], expected: &str) {
    assert!(
        validate_patch(fixtures)
            .unwrap_err()
            .to_string()
            .contains(expected)
    );
}

#[test]
fn schema_v2_multi_split_requires_exact_optional_assignments_for_every_instance() {
    let fixture = schema_v2_two_split_fixture();
    validate_patch(std::slice::from_ref(&fixture)).unwrap();

    let mut missing_parent = fixture.clone();
    missing_parent.split_patches.pop();
    assert!(
        validate_patch(&[missing_parent])
            .unwrap_err()
            .to_string()
            .contains("missing split 2")
    );

    let mut duplicate = fixture.clone();
    duplicate.split_patches[1].split = 1;
    assert!(
        validate_patch(&[duplicate])
            .unwrap_err()
            .to_string()
            .contains("more than once")
    );

    let mut unknown = fixture.clone();
    unknown.split_patches[1].split = 99;
    assert!(
        validate_patch(&[unknown])
            .unwrap_err()
            .to_string()
            .contains("unknown split 99")
    );

    let mut partial = fixture.clone();
    partial.split_patches[1].universe = Some(2);
    assert!(
        validate_patch(&[partial])
            .unwrap_err()
            .to_string()
            .contains("both universe and address or neither")
    );

    let mut missing_multipatch = fixture;
    missing_multipatch.multipatch[0].split_patches.clear();
    assert!(
        validate_patch(&[missing_multipatch])
            .unwrap_err()
            .to_string()
            .contains("must assign every split")
    );
}
#[test]
fn media_server_layers_inherit_parent_direct_control_endpoint() {
    let endpoint = DirectControlEndpoint {
        protocol: DirectControlProtocol::Citp,
        ip_address: "192.0.2.20".parse().unwrap(),
        port: 4811,
    };
    let mut media_definition = definition(1);
    media_definition.direct_control_protocols = vec![DirectControlProtocol::Citp];
    media_definition.heads[0].index = 1;
    media_definition.heads[0].shared = false;
    let parent = PatchedFixture {
        fixture_id: FixtureId::new(),
        fixture_number: None,
        virtual_fixture_number: None,
        name: "Media".into(),
        definition: media_definition,
        universe: Some(1),
        address: Some(1),
        split_patches: vec![],
        layer_id: default_patch_layer(),
        direct_control: Some(endpoint.clone()),
        location: Default::default(),
        rotation: Default::default(),
        logical_heads: vec![PatchedHead {
            head_index: 1,
            fixture_id: FixtureId::new(),
        }],
        multipatch: vec![],
        move_in_black_enabled: true,
        move_in_black_delay_millis: 0,
        highlight_overrides: BTreeMap::new(),
    };
    validate_patch(std::slice::from_ref(&parent)).unwrap();
    assert_eq!(parent.direct_control, Some(endpoint));
    assert_eq!(parent.logical_heads.len(), 1);
    let mut unsupported = parent.clone();
    unsupported.definition.direct_control_protocols.clear();
    assert!(
        validate_patch(&[unsupported])
            .unwrap_err()
            .to_string()
            .contains("does not support")
    );
}
#[test]
fn logical_head_reconciliation_preserves_matching_ids_and_repairs_shape() {
    let kept = FixtureId::new();
    let stale = FixtureId::new();
    let mut fixture = PatchedFixture {
        fixture_id: FixtureId::new(),
        fixture_number: Some(100),
        virtual_fixture_number: None,
        name: "Multi".into(),
        definition: definition(2),
        universe: Some(1),
        address: Some(1),
        split_patches: vec![],
        layer_id: default_patch_layer(),
        direct_control: None,
        location: Default::default(),
        rotation: Default::default(),
        logical_heads: vec![
            PatchedHead {
                head_index: 0,
                fixture_id: kept,
            },
            PatchedHead {
                head_index: 99,
                fixture_id: stale,
            },
        ],
        multipatch: vec![],
        move_in_black_enabled: true,
        move_in_black_delay_millis: 0,
        highlight_overrides: BTreeMap::new(),
    };
    fixture.definition.heads = vec![
        LogicalHead {
            index: 10,
            name: "Master".into(),
            shared: true,
            parameters: vec![],
        },
        LogicalHead {
            index: 0,
            name: "Cell 1".into(),
            shared: false,
            parameters: vec![],
        },
        LogicalHead {
            index: 4,
            name: "Cell 2".into(),
            shared: false,
            parameters: vec![],
        },
    ];
    assert!(reconcile_logical_heads(&mut fixture));
    assert_eq!(fixture.logical_heads.len(), 2);
    assert_eq!(fixture.logical_heads[0].head_index, 0);
    assert_eq!(fixture.logical_heads[0].fixture_id, kept);
    assert_eq!(fixture.logical_heads[1].head_index, 4);
    assert_ne!(fixture.logical_heads[1].fixture_id, stale);
    assert!(!reconcile_logical_heads(&mut fixture));
}

#[test]
fn legacy_patch_defaults_move_in_black_on_and_round_trips_explicit_settings() {
    let legacy = serde_json::json!({
        "fixture_id": FixtureId::new(),
        "definition": definition(2)
    });
    let mut fixture: PatchedFixture = serde_json::from_value(legacy).unwrap();
    assert!(fixture.move_in_black_enabled);
    assert_eq!(fixture.move_in_black_delay_millis, 0);

    fixture.move_in_black_enabled = false;
    fixture.move_in_black_delay_millis = 1_250;
    let restored: PatchedFixture =
        serde_json::from_value(serde_json::to_value(fixture).unwrap()).unwrap();
    assert!(!restored.move_in_black_enabled);
    assert_eq!(restored.move_in_black_delay_millis, 1_250);
}

#[test]
fn legacy_patch_inherits_highlight_look_and_round_trips_instance_overrides() {
    let legacy = serde_json::json!({
        "fixture_id": FixtureId::new(),
        "definition": definition(2)
    });
    let mut fixture: PatchedFixture = serde_json::from_value(legacy).unwrap();
    assert!(fixture.highlight_overrides.is_empty());

    let channel_id = Uuid::new_v4();
    fixture.highlight_overrides.insert(channel_id, 173);
    let restored: PatchedFixture =
        serde_json::from_value(serde_json::to_value(fixture).unwrap()).unwrap();
    assert_eq!(restored.highlight_overrides.get(&channel_id), Some(&173));
}

#[test]
fn visual_only_fixtures_allow_addressless_multipatches_and_reject_dmx_addresses() {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Venue".into();
    profile.name = "Truss".into();
    profile.patch_policy = PatchPolicy::VisualOnly;
    profile.modes[0].splits[0].footprint = 0;
    let definition = profile.resolved_definition(profile.modes[0].id).unwrap();
    let mut fixture: PatchedFixture = serde_json::from_value(serde_json::json!({
        "fixture_id": FixtureId::new(),
        "definition": definition,
        "split_patches": [{"split": 1, "universe": null, "address": null}],
        "multipatch": [{
            "id": Uuid::new_v4(),
            "name": "Second span",
            "universe": null,
            "address": null,
            "split_patches": [{"split": 1, "universe": null, "address": null}],
            "location": {"x": 1000, "y": 0, "z": 0},
            "rotation": {"x": 0, "y": 0, "z": 0}
        }]
    }))
    .unwrap();
    assert!(validate_patch(std::slice::from_ref(&fixture)).is_err());
    fixture.virtual_fixture_number = Some(1);
    validate_patch(std::slice::from_ref(&fixture)).unwrap();
    let mut duplicate = fixture.clone();
    duplicate.fixture_id = FixtureId::new();
    assert!(validate_patch(&[fixture.clone(), duplicate]).is_err());
    fixture.fixture_number = Some(1);
    assert!(validate_patch(std::slice::from_ref(&fixture)).is_err());
    fixture.fixture_number = None;
    fixture.universe = Some(1);
    fixture.address = Some(1);
    assert!(validate_patch(&[fixture]).is_err());
}
