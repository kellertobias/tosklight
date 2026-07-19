use super::{
    FixtureProfileRevision, PortablePatchRevision, PortableShowObjectKey, PortableShowRevision,
    store::PATCH_REVISION_METADATA_KEY,
};
use crate::{ShowStore, StoreError};
use light_core::FixtureId;
use serde_json::{Value, json};
use std::{fs, path::PathBuf};
use uuid::Uuid;

const PROFILE_A: &str = "10000000-0000-0000-0000-000000000001";
const PROFILE_B: &str = "20000000-0000-0000-0000-000000000002";

fn temporary(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("light-candidate-{name}-{}.sqlite", Uuid::new_v4()))
}

fn create(name: &str) -> (PathBuf, ShowStore) {
    let path = temporary(name);
    let (store, _) = ShowStore::create(&path, "Candidate test").unwrap();
    (path, store)
}

fn profile(id: &str, revision: u64, name: &str) -> FixtureProfileRevision {
    FixtureProfileRevision::from_profile(json!({
        "id": id,
        "revision": revision,
        "name": name,
        "future": {"asset": "retained"}
    }))
    .unwrap()
}

fn fixture_id(id: &str) -> FixtureId {
    FixtureId(Uuid::parse_str(id).unwrap())
}

#[test]
fn missing_legacy_patch_revision_reads_as_zero() {
    let (path, show) = create("legacy-revision");
    show.conn
        .execute(
            "DELETE FROM metadata WHERE key=?1",
            [PATCH_REVISION_METADATA_KEY],
        )
        .unwrap();

    assert_eq!(
        show.portable_patch_revision().unwrap(),
        PortablePatchRevision::new(0)
    );
    let document = show.portable_document().unwrap();
    assert_eq!(document.patch_revision(), PortablePatchRevision::new(0));
    assert!(
        !document
            .metadata()
            .contains_key(PATCH_REVISION_METADATA_KEY)
    );

    drop(show);
    let _ = fs::remove_file(path);
}

#[test]
fn marked_batch_advances_show_and_patch_revisions_once() {
    let (path, show) = create("one-batch-revision");
    let document = show.portable_document().unwrap();
    let mut transaction = document.transaction();
    transaction
        .put("patched_fixture", "one", json!({"fixture_number": 1}))
        .put("patched_fixture", "two", json!({"fixture_number": 2}))
        .mark_patch_changed()
        .mark_patch_changed();
    let candidate = document.candidate(&transaction).unwrap();

    assert_eq!(candidate.revision(), PortableShowRevision::new(1));
    assert_eq!(candidate.patch_revision(), PortablePatchRevision::new(1));
    let predicted_revision = candidate.revision();
    let predicted_patch_revision = candidate.patch_revision();
    let commit = show.apply_portable_transaction(transaction).unwrap();
    assert_eq!(commit.revision(), predicted_revision);
    assert_eq!(commit.patch_revision(), predicted_patch_revision);
    assert_eq!(show.portable_revision().unwrap().value(), 1);
    assert_eq!(show.portable_patch_revision().unwrap().value(), 1);

    let document = show.portable_document().unwrap();
    let mut transaction = document.transaction();
    transaction
        .put("patched_fixture", "three", json!({"fixture_number": 3}))
        .mark_patch_changed();
    let commit = show.apply_portable_transaction(transaction).unwrap();
    assert_eq!(commit.revision().value(), 2);
    assert_eq!(commit.patch_revision().value(), 2);

    drop(show);
    let _ = fs::remove_file(path);
}

#[test]
fn ordinary_show_transaction_keeps_patch_revision_stable() {
    let (path, show) = create("non-patch-revision");
    let document = show.portable_document().unwrap();
    let mut transaction = document.transaction();
    transaction.put("group", "one", json!({"members": []}));
    let candidate = document.candidate(&transaction).unwrap();

    assert_eq!(candidate.revision().value(), 1);
    assert_eq!(candidate.patch_revision().value(), 0);
    let commit = show.apply_portable_transaction(transaction).unwrap();
    assert_eq!(commit.revision().value(), 1);
    assert_eq!(commit.patch_revision().value(), 0);
    assert_eq!(show.portable_patch_revision().unwrap().value(), 0);

    drop(show);
    let _ = fs::remove_file(path);
}

#[test]
fn revision_conflict_rolls_back_patch_marker_and_batch() {
    let (path, show) = create("revision-conflict");
    let document = show.portable_document().unwrap();
    let mut transaction = document.transaction();
    transaction
        .put("patched_fixture", "candidate", json!({"fixture_number": 2}))
        .mark_patch_changed();
    show.put_object(
        "patched_fixture",
        "concurrent",
        &json!({"fixture_number": 1}),
        0,
    )
    .unwrap();

    assert!(matches!(
        show.apply_portable_transaction(transaction),
        Err(StoreError::DocumentRevisionConflict { expected, current })
            if expected.value() == 0 && current.value() == 1
    ));
    assert_eq!(show.portable_patch_revision().unwrap().value(), 0);
    assert!(
        show.objects("patched_fixture")
            .unwrap()
            .iter()
            .all(|object| object.id != "candidate")
    );

    drop(show);
    let _ = fs::remove_file(path);
}

fn seed_candidate_show(show: &ShowStore) {
    show.conn
        .execute(
            "INSERT INTO metadata(key,value) VALUES ('future.editor','opaque')",
            [],
        )
        .unwrap();
    show.put_object(
        "group",
        "a",
        &json!({"name":"Original","future":{"order":[3,1,2]}}),
        0,
    )
    .unwrap();
    show.put_object(
        "future_extension",
        "m",
        &json!({"opaque":{"bytes":[1,2,3]}}),
        0,
    )
    .unwrap();
    show.put_object("future_extension", "z", &json!({"delete":true}), 0)
        .unwrap();
    show.insert_fixture_profile_revision(&profile(PROFILE_B, 1, "Stored"))
        .unwrap();
}

fn stage_candidate(document: &super::PortableShowDocument) -> super::PortableShowTransaction {
    let mut edited = document.object("group", "a").unwrap().body().clone();
    edited["name"] = json!("Candidate");
    let mut transaction = document.transaction();
    transaction
        .put("group", "a", edited)
        .delete("future_extension", "z")
        .put(
            "future_widget",
            "n",
            json!({"format":99,"unknown":{"enabled":true}}),
        )
        .mark_patch_changed();
    transaction
        .put_fixture_profile_revision(profile(PROFILE_A, 2, "Staged"))
        .unwrap();
    transaction
}

#[test]
fn candidate_overlays_objects_profiles_and_revisions_without_mutating_document() {
    let (path, show) = create("overlay");
    seed_candidate_show(&show);
    let document = show.portable_document().unwrap();
    let transaction = stage_candidate(&document);
    let candidate = document.candidate(&transaction).unwrap();

    assert_eq!(candidate.id(), document.id());
    assert_eq!(candidate.name(), document.name());
    assert!(std::ptr::eq(candidate.metadata(), document.metadata()));
    assert_eq!(candidate.metadata()["future.editor"], "opaque");
    assert_eq!(
        candidate.revision().value(),
        document.revision().value() + 1
    );
    assert_eq!(candidate.patch_revision().value(), 1);
    assert_candidate_objects(&document, candidate);
    assert_candidate_profiles(candidate);

    let predicted_show_revision = candidate.revision();
    let predicted_patch_revision = candidate.patch_revision();
    let commit = show.apply_portable_transaction(transaction).unwrap();
    assert_eq!(commit.revision(), predicted_show_revision);
    assert_eq!(commit.patch_revision(), predicted_patch_revision);
    let stored = show.portable_document().unwrap();
    assert_eq!(stored.metadata()["future.editor"], "opaque");
    assert_eq!(
        stored.object("group", "a").unwrap().body()["future"]["order"],
        json!([3, 1, 2])
    );

    drop(show);
    let _ = fs::remove_file(path);
}

fn assert_candidate_objects(
    document: &super::PortableShowDocument,
    candidate: super::PortableShowCandidate<'_>,
) {
    let keys = candidate
        .objects()
        .map(|object| (object.key().kind(), object.key().id()))
        .collect::<Vec<_>>();
    assert_eq!(
        keys,
        vec![
            ("future_extension", "m"),
            ("future_widget", "n"),
            ("group", "a")
        ]
    );
    assert!(candidate.object("future_extension", "z").is_none());
    assert_eq!(
        candidate.object("group", "a").unwrap().body()["name"],
        "Candidate"
    );
    assert_eq!(
        candidate.object("group", "a").unwrap().body()["future"]["order"],
        json!([3, 1, 2])
    );
    assert_eq!(
        document.object("group", "a").unwrap().body()["name"],
        "Original"
    );
    let untouched = candidate.object("future_extension", "m").unwrap();
    assert!(std::ptr::eq(
        untouched.body(),
        document.object("future_extension", "m").unwrap().body()
    ));
    assert_eq!(candidate.object_revision("group", "a"), Some(2));
    assert_eq!(candidate.object_revision("future_widget", "n"), Some(1));
}

fn assert_candidate_profiles(candidate: super::PortableShowCandidate<'_>) {
    let ids = candidate
        .fixture_profile_revisions()
        .map(|profile| profile.id().profile_id())
        .collect::<Vec<_>>();
    assert_eq!(ids, vec![fixture_id(PROFILE_A), fixture_id(PROFILE_B)]);
    let profile = candidate
        .fixture_profile_revision(fixture_id(PROFILE_A), 2)
        .unwrap();
    assert_eq!(profile.profile()["future"]["asset"], "retained");
}

#[test]
fn candidate_rejects_a_profile_revision_conflict_before_persistence() {
    let (path, show) = create("profile-conflict");
    show.insert_fixture_profile_revision(&profile(PROFILE_A, 1, "Stored"))
        .unwrap();
    let document = show.portable_document().unwrap();
    let mut transaction = document.transaction();
    transaction
        .put_fixture_profile_revision(profile(PROFILE_A, 1, "Different"))
        .unwrap();
    transaction
        .put("patched_fixture", "must-roll-back", json!({"number": 1}))
        .mark_patch_changed();

    assert!(matches!(
        document.candidate(&transaction),
        Err(StoreError::FixtureProfileRevisionConflict { .. })
    ));
    assert!(matches!(
        show.apply_portable_transaction(transaction),
        Err(StoreError::FixtureProfileRevisionConflict { .. })
    ));
    assert_eq!(show.portable_patch_revision().unwrap().value(), 0);
    assert!(show.objects("patched_fixture").unwrap().is_empty());

    drop(show);
    let _ = fs::remove_file(path);
}

#[test]
fn candidate_iteration_retains_order_when_a_deleted_base_precedes_a_write() {
    let (path, show) = create("deleted-order");
    show.put_object("kind", "a", &Value::Null, 0).unwrap();
    show.put_object("kind", "z", &Value::Null, 0).unwrap();
    let document = show.portable_document().unwrap();
    let mut transaction = document.transaction();
    transaction
        .delete("kind", "a")
        .put("kind", "b", Value::Null);
    let candidate = document.candidate(&transaction).unwrap();

    let keys = candidate
        .objects()
        .map(|object| object.key().clone())
        .collect::<Vec<PortableShowObjectKey>>();
    assert_eq!(
        keys,
        vec![
            PortableShowObjectKey::new("kind", "b"),
            PortableShowObjectKey::new("kind", "z")
        ]
    );

    drop(show);
    let _ = fs::remove_file(path);
}
