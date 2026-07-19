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
fn request_rejects_mass_assigned_definition_data() {
    let mut value = serde_json::to_value(PatchFixturesRequest {
        request_id: "patch-2".into(),
        fixtures: vec![fixture_input()],
        remove_fixture_ids: Vec::new(),
    })
    .expect("serialize patch request");
    value["fixtures"][0]["definition"] = serde_json::json!({ "modes": ["catalog"] });

    let error = serde_json::from_value::<PatchFixturesRequest>(value)
        .expect_err("definition must not cross the patch command boundary");
    assert!(error.to_string().contains("unknown field `definition`"));
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
    assert_eq!(
        schema["$defs"]["PatchFixtureInput"]["additionalProperties"],
        false
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
        location: PatchFixtureLocation { x: 0, y: 0, z: 0 },
        rotation: PatchFixtureRotation {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        multipatch: Vec::new(),
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
        location: input.location,
        rotation: input.rotation,
        logical_heads: Vec::new(),
        multipatch: Vec::new(),
        move_in_black_enabled: input.move_in_black_enabled,
        move_in_black_delay_millis: input.move_in_black_delay_millis,
        highlight_overrides: Vec::new(),
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
    }
}
