use super::super::{compile_show_candidate, stage_candidate_migrations};
use super::support::{
    document_with_objects, portable_fixture, portable_fixture_with_policy, profile_head_id,
};
use light_core::FixtureId;
use light_fixture::{PatchPolicy, PortablePatchedFixtureRecord};
use serde_json::json;
use uuid::Uuid;

#[test]
fn inline_and_lean_records_compile_equivalently_with_stable_heads() {
    let (profile, fixture, reference) = portable_fixture();
    let stable_profile_head = profile_head_id(&profile);
    let retained_fixture_head = fixture.logical_heads[0].fixture_id;
    let mut legacy_body = serde_json::to_value(&fixture).unwrap();
    legacy_body["definition"]["profile_snapshot"] = profile.profile().clone();
    legacy_body["future_fixture"] = json!({"kept": true});

    let (legacy_store, legacy_document) = document_with_objects(&[(
        "patched_fixture",
        &fixture.fixture_id.0.to_string(),
        legacy_body,
    )]);
    let mut legacy_transaction = legacy_document.transaction();
    stage_candidate_migrations(&legacy_document, &mut legacy_transaction).unwrap();
    let legacy_candidate = legacy_document.candidate(&legacy_transaction).unwrap();
    assert_eq!(
        legacy_candidate.patch_revision().value(),
        legacy_document.patch_revision().value() + 1
    );
    let migrated_body = legacy_candidate
        .object("patched_fixture", &fixture.fixture_id.0.to_string())
        .unwrap()
        .body();
    assert!(migrated_body.get("definition").is_none());
    assert_eq!(migrated_body["future_fixture"], json!({"kept": true}));
    assert_eq!(migrated_body["logical_heads"].as_array().unwrap().len(), 1);
    assert_eq!(
        migrated_body["logical_heads"][0]["profile_head_id"],
        stable_profile_head.to_string()
    );
    assert_eq!(
        migrated_body["logical_heads"][0]["fixture_id"],
        retained_fixture_head.0.to_string()
    );
    assert_eq!(migrated_body["split_patches"].as_array().unwrap().len(), 1);
    assert_eq!(
        legacy_candidate
            .fixture_profile_revision(reference.profile_id, reference.profile_revision)
            .unwrap()
            .profile()["future_profile"],
        json!({"kept": [2, 1]})
    );
    let legacy_fixture = compile_show_candidate(legacy_candidate).unwrap().fixtures;
    let first_commit = legacy_store
        .apply_portable_transaction(legacy_transaction)
        .unwrap();
    let migrated_document = legacy_store.portable_document().unwrap();
    let mut idempotent = migrated_document.transaction();
    stage_candidate_migrations(&migrated_document, &mut idempotent).unwrap();
    assert!(idempotent.is_empty());
    assert_eq!(
        migrated_document
            .candidate(&idempotent)
            .unwrap()
            .patch_revision(),
        first_commit.patch_revision()
    );

    let mut stable_fixture = fixture.clone();
    stable_fixture.logical_heads[0].profile_head_id = Some(stable_profile_head);
    stable_fixture.logical_heads[0].head_index = 0;
    let mut lean_body = PortablePatchedFixtureRecord::from_runtime_fixture(&stable_fixture)
        .unwrap()
        .into_body();
    lean_body["future_fixture"] = json!({"kept": true});
    let (current_store, _) = document_with_objects(&[(
        "patched_fixture",
        &fixture.fixture_id.0.to_string(),
        lean_body,
    )]);
    current_store
        .insert_fixture_profile_revision(&profile)
        .unwrap();
    let current_document = current_store.portable_document().unwrap();
    let mut current_transaction = current_document.transaction();
    stage_candidate_migrations(&current_document, &mut current_transaction).unwrap();
    let current_candidate = current_document.candidate(&current_transaction).unwrap();
    let current_head = &current_candidate
        .object("patched_fixture", &fixture.fixture_id.0.to_string())
        .unwrap()
        .body()["logical_heads"][0];
    assert_eq!(
        current_head["profile_head_id"],
        stable_profile_head.to_string()
    );
    assert_eq!(current_head["head_index"], 1);
    assert_eq!(
        current_head["fixture_id"],
        retained_fixture_head.0.to_string()
    );
    let current_fixture = compile_show_candidate(current_candidate).unwrap().fixtures;

    assert_eq!(
        serde_json::to_value(legacy_fixture).unwrap(),
        serde_json::to_value(current_fixture).unwrap()
    );
}

#[test]
fn schema_one_fixture_is_materialized_and_compiled_in_one_candidate() {
    let (_, mut fixture, _) = portable_fixture();
    fixture.definition.schema_version = 1;
    fixture.definition.profile_id = None;
    fixture.definition.mode_id = None;
    fixture.definition.profile_snapshot = None;
    let mut legacy_body = serde_json::to_value(&fixture).unwrap();
    legacy_body["definition"]["future_schema_one"] = json!({"kept": true});

    let (_, document) = document_with_objects(&[(
        "patched_fixture",
        &fixture.fixture_id.0.to_string(),
        legacy_body,
    )]);
    let mut transaction = document.transaction();
    stage_candidate_migrations(&document, &mut transaction).unwrap();
    let candidate = document.candidate(&transaction).unwrap();
    let migrated = candidate
        .object("patched_fixture", &fixture.fixture_id.0.to_string())
        .unwrap()
        .body();

    assert!(migrated.get("definition").is_none());
    assert_eq!(candidate.fixture_profile_revisions().count(), 1);
    assert!(
        migrated["_light_legacy_definition_fields"]
            .as_array()
            .unwrap()
            .iter()
            .any(|field| field["json_pointer"] == "/future_schema_one"
                && field["value"] == json!({"kept": true}))
    );
    let compiled = compile_show_candidate(candidate).unwrap();
    assert_eq!(compiled.fixtures.len(), 1);
    assert_eq!(compiled.fixtures[0].definition.schema_version, 2);
}

#[test]
fn legacy_non_uuid_object_key_remains_loadable() {
    let (profile, fixture, _) = portable_fixture();
    let mut legacy_body = serde_json::to_value(&fixture).unwrap();
    legacy_body["definition"]["profile_snapshot"] = profile.profile().clone();
    let (_, document) = document_with_objects(&[("patched_fixture", "dimmer", legacy_body)]);
    let mut transaction = document.transaction();

    stage_candidate_migrations(&document, &mut transaction).unwrap();
    let candidate = document.candidate(&transaction).unwrap();

    assert!(candidate.object("patched_fixture", "dimmer").is_some());
    let compiled = compile_show_candidate(candidate).unwrap();
    assert_eq!(compiled.fixtures[0].fixture_id, fixture.fixture_id);
}

#[test]
fn missing_fixture_numbers_and_visual_numbers_follow_legacy_inference() {
    let (dmx_profile, mut front, _) = portable_fixture_with_policy(PatchPolicy::Dmx, 20_000);
    front.name = "Front Fresnel 5".into();
    front.fixture_number = None;
    front.virtual_fixture_number = None;
    let mut overflow = front.clone();
    overflow.fixture_id = FixtureId(Uuid::from_u128(20_020));
    overflow.logical_heads[0].fixture_id = FixtureId(Uuid::from_u128(20_021));
    overflow.logical_heads[1].fixture_id = FixtureId(Uuid::from_u128(20_022));
    overflow.name = "Back Profile 4294967295".into();
    overflow.address = Some(2);
    let (visual_profile, mut visual, _) =
        portable_fixture_with_policy(PatchPolicy::VisualOnly, 30_000);
    visual.name = "Scenery".into();
    visual.fixture_number = None;
    visual.virtual_fixture_number = None;
    visual.universe = None;
    visual.address = None;

    let front_body = PortablePatchedFixtureRecord::from_runtime_fixture(&front)
        .unwrap()
        .into_body();
    let overflow_body = PortablePatchedFixtureRecord::from_runtime_fixture(&overflow)
        .unwrap()
        .into_body();
    let visual_body = PortablePatchedFixtureRecord::from_runtime_fixture(&visual)
        .unwrap()
        .into_body();
    let (store, _) = document_with_objects(&[
        (
            "patched_fixture",
            &front.fixture_id.0.to_string(),
            front_body,
        ),
        (
            "patched_fixture",
            &overflow.fixture_id.0.to_string(),
            overflow_body,
        ),
        (
            "patched_fixture",
            &visual.fixture_id.0.to_string(),
            visual_body,
        ),
    ]);
    store.insert_fixture_profile_revision(&dmx_profile).unwrap();
    store
        .insert_fixture_profile_revision(&visual_profile)
        .unwrap();
    let document = store.portable_document().unwrap();
    let mut transaction = document.transaction();
    stage_candidate_migrations(&document, &mut transaction).unwrap();
    let candidate = document.candidate(&transaction).unwrap();

    assert_eq!(
        fixture_body(candidate, front.fixture_id)["fixture_number"],
        5
    );
    assert_eq!(
        fixture_body(candidate, overflow.fixture_id)["fixture_number"],
        1
    );
    let visual = fixture_body(candidate, visual.fixture_id);
    assert!(visual["fixture_number"].is_null());
    assert_eq!(visual["virtual_fixture_number"], 1);
}

fn fixture_body<'a>(
    candidate: light_show::PortableShowCandidate<'a>,
    fixture_id: FixtureId,
) -> &'a serde_json::Value {
    candidate
        .object("patched_fixture", &fixture_id.0.to_string())
        .unwrap()
        .body()
}
