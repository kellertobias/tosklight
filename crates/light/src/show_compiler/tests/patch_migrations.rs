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
fn schema_two_inline_fixture_is_upgraded_and_materialized_in_one_candidate() {
    let (_, fixture, reference) = portable_fixture();
    let mut legacy_body = serde_json::to_value(&fixture).unwrap();
    legacy_body["definition"]["schema_version"] = json!(2);
    legacy_body["definition"]["profile_snapshot"]["schema_version"] = json!(2);

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
    assert_eq!(
        candidate
            .fixture_profile_revision(reference.profile_id, reference.profile_revision)
            .unwrap()
            .profile()["schema_version"],
        2
    );
    let compiled = compile_show_candidate(candidate).unwrap();
    assert_eq!(compiled.fixtures.len(), 1);
    assert_eq!(
        compiled.fixtures[0].definition.schema_version,
        light_fixture::FIXTURE_PROFILE_SCHEMA_VERSION
    );
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

#[test]
fn mixed_patch_policies_keep_regular_and_virtual_number_namespaces_distinct() {
    let (dmx_profile, mut dmx, _) = portable_fixture_with_policy(PatchPolicy::Dmx, 40_000);
    dmx.fixture_number = None;
    dmx.virtual_fixture_number = None;
    let (visual_profile, mut visual, _) =
        portable_fixture_with_policy(PatchPolicy::VisualOnly, 41_000);
    visual.fixture_number = Some(44);
    visual.virtual_fixture_number = Some(44);
    visual.universe = None;
    visual.address = None;
    let (internal_profile, mut internal, _) =
        portable_fixture_with_policy(PatchPolicy::Internal, 42_000);
    internal.fixture_number = None;
    internal.virtual_fixture_number = Some(44);
    internal.universe = None;
    internal.address = None;

    let records = [
        (&dmx, &dmx_profile),
        (&visual, &visual_profile),
        (&internal, &internal_profile),
    ];
    let objects = records
        .iter()
        .map(|(fixture, _)| {
            (
                "patched_fixture",
                fixture.fixture_id.0.to_string(),
                PortablePatchedFixtureRecord::from_runtime_fixture(fixture)
                    .unwrap()
                    .into_body(),
            )
        })
        .collect::<Vec<_>>();
    let object_refs = objects
        .iter()
        .map(|(object_type, id, body)| (*object_type, id.as_str(), body.clone()))
        .collect::<Vec<_>>();
    let (store, _) = document_with_objects(&object_refs);
    for (_, profile) in records {
        store.insert_fixture_profile_revision(profile).unwrap();
    }
    let document = store.portable_document().unwrap();
    let mut transaction = document.transaction();

    stage_candidate_migrations(&document, &mut transaction).unwrap();
    let candidate = document.candidate(&transaction).unwrap();

    let dmx = fixture_body(candidate, dmx.fixture_id);
    assert!(dmx["fixture_number"].as_u64().is_some());
    assert!(dmx["virtual_fixture_number"].is_null());
    let visual = fixture_body(candidate, visual.fixture_id);
    assert!(visual["fixture_number"].is_null());
    assert_eq!(visual["virtual_fixture_number"], 44);
    let internal = fixture_body(candidate, internal.fixture_id);
    assert!(internal["fixture_number"].as_u64().is_some());
    assert!(internal["virtual_fixture_number"].is_null());
    assert_ne!(dmx["fixture_number"], internal["fixture_number"]);

    let compiled = compile_show_candidate(candidate).unwrap();
    assert_eq!(compiled.fixtures.len(), 3);
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
