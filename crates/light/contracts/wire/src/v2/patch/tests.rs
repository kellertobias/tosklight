use super::*;

const FIXTURE_ID: Uuid = Uuid::from_u128(1);
const PROFILE_ID: Uuid = Uuid::from_u128(2);
const MODE_ID: Uuid = Uuid::from_u128(3);

#[test]
fn request_contains_references_and_patch_owned_state_only() {
    let request = PatchFixturesRequest {
        request_id: "patch-1".into(),
        fixtures: vec![fixture_input()],
        remove_fixture_ids: Vec::new(),
        placements: Vec::new(),
        vector_spreads: Vec::new(),
    };
    let value = serde_json::to_value(request).expect("serialize patch request");
    let fixture = value["fixtures"][0]
        .as_object()
        .expect("fixture request object");

    assert_eq!(fixture["profile_id"], PROFILE_ID.to_string());
    assert_eq!(fixture["profile_revision"], 9);
    assert_eq!(fixture["mode_id"], MODE_ID.to_string());
    assert!(value.get("show_id").is_none());
    assert!(value.get("expected_show_revision").is_none());
    assert!(value.get("expected_patch_revision").is_none());
    assert!(!fixture.contains_key("definition"));
    assert!(!fixture.contains_key("profile_snapshot"));
    assert!(!fixture.contains_key("catalog"));
    assert!(!fixture.contains_key("logical_heads"));
}

#[test]
fn request_ignores_future_or_non_owned_fixture_properties() {
    let mut value = serde_json::to_value(PatchFixturesRequest {
        request_id: "patch-2".into(),
        fixtures: vec![fixture_input()],
        remove_fixture_ids: Vec::new(),
        placements: Vec::new(),
        vector_spreads: Vec::new(),
    })
    .expect("serialize patch request");
    value["fixtures"][0]["definition"] = serde_json::json!({ "modes": ["catalog"] });

    let decoded = serde_json::from_value::<PatchFixturesRequest>(value)
        .expect("unknown properties are ignored without entering the typed command");
    assert_eq!(decoded.fixtures[0].profile_id, PROFILE_ID);
}

#[test]
fn legacy_patch_input_defaults_output_policies_compatibly() {
    let mut value = serde_json::to_value(fixture_input()).unwrap();
    for field in [
        "group_masters_enabled",
        "grand_master_enabled",
        "invert_pan",
        "invert_tilt",
    ] {
        value.as_object_mut().unwrap().remove(field);
    }
    value
        .as_object_mut()
        .unwrap()
        .remove("installed_appearance");
    let decoded: PatchFixtureInput = serde_json::from_value(value).unwrap();
    assert!(decoded.group_masters_enabled);
    assert!(decoded.grand_master_enabled);
    assert!(!decoded.invert_pan);
    assert!(!decoded.invert_tilt);
    assert_eq!(
        decoded.installed_appearance,
        PatchInstalledFixtureAppearance::default()
    );
}

#[test]
fn installed_appearance_round_trips_stable_catalog_identity_and_fallback() {
    let mut fixture = fixture_input();
    fixture.installed_appearance = PatchInstalledFixtureAppearance {
        light_source: PatchInstalledLightSource::Tungsten,
        color_temperature_kelvin: Some(3_200),
        gel: PatchGelAssignment::BuiltIn {
            catalog_id: "touring-gels".into(),
            entry_id: "deep-red".into(),
            embedded_fallback: PatchGelDefinitionSnapshot {
                number: "R1".into(),
                name: "Deep red".into(),
                display_srgb: "#D92838".into(),
                visualizer_srgb: "#C01020".into(),
            },
        },
        shaper_angles_degrees: [-10.0, 20.0, 0.0, 179.0],
    };

    let value = serde_json::to_value(&fixture).unwrap();
    let decoded: PatchFixtureInput = serde_json::from_value(value).unwrap();
    assert_eq!(decoded.installed_appearance, fixture.installed_appearance);
}

#[test]
fn sparse_fixture_update_keeps_paired_fields_atomic_and_tolerates_future_fields() {
    let value = serde_json::json!({
        "request_id": "fixture-update-1",
        "expected_fixture_revision": 7,
        "expected_patch_revision": 11,
        "expected_show_revision": 19,
        "multipatch_instance_id": null,
        "action": "set_pan_tilt",
        "invert_pan": true,
        "invert_tilt": false,
        "future_field": "ignored"
    });
    let decoded: PatchFixtureUpdateRequest = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(decoded.expected_fixture_revision, 7);
    assert_eq!(decoded.expected_patch_revision, 11);
    assert_eq!(decoded.expected_show_revision, 19);
    assert_eq!(
        decoded.action,
        PatchFixtureUpdateAction::SetPanTilt {
            invert_pan: true,
            invert_tilt: false,
        }
    );

    let mut partial = value;
    partial.as_object_mut().unwrap().remove("invert_tilt");
    assert!(serde_json::from_value::<PatchFixtureUpdateRequest>(partial).is_err());
}

#[test]
fn sparse_fixture_update_distinguishes_clearing_a_module_rotation() {
    let decoded: PatchFixtureUpdateRequest = serde_json::from_value(serde_json::json!({
        "request_id": "fixture-update-2",
        "expected_fixture_revision": 7,
        "expected_patch_revision": 11,
        "expected_show_revision": 19,
        "multipatch_instance_id": Uuid::from_u128(44),
        "action": "set_shaper_module_rotation",
        "degrees": null
    }))
    .unwrap();
    assert_eq!(
        decoded.action,
        PatchFixtureUpdateAction::SetShaperModuleRotation { degrees: None }
    );
}

#[test]
fn request_preserves_explicit_unpatched_and_partial_pairs_for_application_validation() {
    let mut fixture = fixture_input();
    fixture.split_patches[0].universe = None;
    fixture.split_patches[0].address = None;
    let value = serde_json::to_value(PatchFixturesRequest {
        request_id: "patch-empty".into(),
        fixtures: vec![fixture],
        remove_fixture_ids: Vec::new(),
        placements: Vec::new(),
        vector_spreads: Vec::new(),
    })
    .expect("serialize unpatched request");
    let decoded = serde_json::from_value::<PatchFixturesRequest>(value.clone())
        .expect("explicit null address pair is valid");
    assert_eq!(decoded.fixtures[0].split_patches[0].universe, None);
    assert_eq!(decoded.fixtures[0].split_patches[0].address, None);

    let mut partial = value;
    partial["fixtures"][0]["split_patches"][0]["universe"] = serde_json::json!(1);
    let decoded = serde_json::from_value::<PatchFixturesRequest>(partial)
        .expect("shape decoding leaves semantic pair validation to the application");
    assert_eq!(decoded.fixtures[0].split_patches[0].universe, Some(1));
    assert_eq!(decoded.fixtures[0].split_patches[0].address, None);
}

#[test]
fn request_schema_bounds_idempotency_identity_and_batch_collections() {
    let schema = serde_json::to_value(schemars::schema_for!(PatchFixturesRequest))
        .expect("serialize request schema");

    assert_eq!(schema["properties"]["request_id"]["minLength"], 1);
    assert_eq!(schema["properties"]["request_id"]["maxLength"], 128);
    assert!(schema["properties"]["fixtures"].get("minItems").is_none());
    assert_eq!(
        schema["$defs"]["PatchFixtureInput"]["properties"]["split_patches"]["minItems"],
        1
    );
    assert_ne!(
        schema["$defs"]["PatchFixtureInput"]["additionalProperties"],
        false
    );
}

#[test]
fn placement_intent_is_typed_and_tolerates_future_properties() {
    let fixture_id = Uuid::from_u128(44);
    let request = serde_json::json!({
        "request_id": "placement-1",
        "fixtures": [],
        "remove_fixture_ids": [],
        "placements": [{
            "fixture_ids": [fixture_id],
            "splits": [{
                "split": 1,
                "universe": 1,
                "address": 1,
                "mode": {
                    "type": "operator_overrides",
                    "overrides": [{
                        "fixture_id": fixture_id,
                        "universe": 1,
                        "address": 50,
                        "future_override_field": true
                    }],
                    "future_mode_field": true
                },
                "future_split_field": true
            }],
            "future_placement_field": true
        }],
        "future_request_field": true
    });

    let decoded = serde_json::from_value::<PatchFixturesRequest>(request).unwrap();
    assert_eq!(decoded.placements[0].fixture_ids, vec![fixture_id]);
    assert_eq!(
        decoded.placements[0].splits[0].mode,
        PatchSplitPlacementMode::OperatorOverrides {
            overrides: vec![PatchOperatorAddressOverride {
                fixture_id,
                universe: 1,
                address: 50,
            }]
        }
    );
}

#[test]
fn outcome_flattens_the_authoritative_delta_and_replay_identity() {
    let outcome = PatchFixturesOutcome {
        request_id: "patch-3".into(),
        replayed: true,
        changed: true,
        delta: PatchDelta {
            show_id: Uuid::from_u128(4),
            show_revision: 8,
            patch_revision: 4,
            event_sequence: Some(21),
            fixtures: vec![fixture_projection()],
            removed_fixture_ids: Vec::new(),
            profile_revisions: vec![profile_projection()],
        },
    };
    let value = serde_json::to_value(outcome).expect("serialize patch outcome");

    assert_eq!(value["request_id"], "patch-3");
    assert_eq!(value["replayed"], true);
    assert_eq!(value["changed"], true);
    assert_eq!(value["show_revision"], 8);
    assert_eq!(value["patch_revision"], 4);
    assert_eq!(value["event_sequence"], 21);
    assert_eq!(value["fixtures"].as_array().map(Vec::len), Some(1));
    assert_eq!(value["profile_revisions"].as_array().map(Vec::len), Some(1));
}

#[test]
fn snapshot_carries_a_gap_repair_cursor_and_deduplicated_profile_metadata() {
    let snapshot = PatchSnapshot {
        show_id: Uuid::from_u128(4),
        show_revision: 8,
        patch_revision: 4,
        cursor: EventSnapshotCursor { sequence: 21 },
        fixtures: vec![fixture_projection(), fixture_projection()],
        profile_revisions: vec![profile_projection()],
    };
    let value = serde_json::to_value(snapshot).expect("serialize patch snapshot");

    assert_eq!(value["cursor"]["sequence"], 21);
    assert_eq!(value["fixtures"].as_array().map(Vec::len), Some(2));
    assert_eq!(value["profile_revisions"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        value["profile_revisions"][0]["referenced_modes"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
}

fn fixture_input() -> PatchFixtureInput {
    PatchFixtureInput {
        fixture_id: FIXTURE_ID,
        fixture_number: Some(1),
        virtual_fixture_number: None,
        name: "Key light".into(),
        profile_id: PROFILE_ID,
        profile_revision: 9,
        mode_id: MODE_ID,
        split_patches: vec![PatchSplitAssignment {
            split: 1,
            universe: Some(1),
            address: Some(101),
        }],
        layer_id: "default".into(),
        direct_control: None,
        internal_bindings: Default::default(),
        location: PatchFixtureLocation { x: 0, y: 0, z: 0 },
        rotation: PatchFixtureRotation {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        multipatch: Vec::new(),
        group_masters_enabled: true,
        grand_master_enabled: true,
        invert_pan: false,
        invert_tilt: false,
        bracket_angle: 0.0,
        shaper_angle: None,
        installed_appearance: Default::default(),
        move_in_black_enabled: true,
        move_in_black_delay_millis: 0,
        highlight_overrides: Vec::new(),
    }
}

fn fixture_projection() -> PatchFixtureProjection {
    let input = fixture_input();
    PatchFixtureProjection {
        fixture_id: input.fixture_id,
        fixture_revision: 1,
        fixture_number: input.fixture_number,
        virtual_fixture_number: input.virtual_fixture_number,
        name: input.name,
        profile_id: input.profile_id,
        profile_revision: input.profile_revision,
        mode_id: input.mode_id,
        split_patches: input.split_patches,
        layer_id: input.layer_id,
        direct_control: input.direct_control,
        internal_bindings: input.internal_bindings,
        location: input.location,
        rotation: input.rotation,
        logical_heads: Vec::new(),
        multipatch: Vec::new(),
        group_masters_enabled: input.group_masters_enabled,
        grand_master_enabled: input.grand_master_enabled,
        invert_pan: input.invert_pan,
        invert_tilt: input.invert_tilt,
        bracket_angle: 0.0,
        shaper_angle: None,
        installed_appearance: input.installed_appearance,
        move_in_black_enabled: input.move_in_black_enabled,
        move_in_black_delay_millis: input.move_in_black_delay_millis,
        highlight_overrides: Vec::new(),
        freeze_targets: Vec::new(),
    }
}

fn profile_projection() -> PatchProfileRevisionProjection {
    PatchProfileRevisionProjection {
        profile_id: PROFILE_ID,
        profile_revision: 9,
        content_digest: "sha256:abc".into(),
        manufacturer: "Tosk".into(),
        name: "Reference Lamp".into(),
        fixture_type: "LED".into(),
        patch_policy: PatchProfilePolicy::Dmx,
        referenced_modes: vec![PatchModeProjection {
            mode_id: MODE_ID,
            name: "8 channel".into(),
            splits: vec![PatchModeSplitProjection {
                split: 1,
                footprint: 8,
            }],
        }],
        profile_snapshot: serde_json::Value::Null,
    }
}
