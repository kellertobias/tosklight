use super::support::{
    PROFILE_A, PROFILE_PORTABLE, PROFILE_Z, create, fixture_id, profile, profile_value, temporary,
};
use crate::{FixtureProfileRevision, FixtureProfileRevisionInsertStatus, ShowStore, StoreError};
use serde_json::json;
use std::fs;

#[test]
fn old_show_migration_adds_profile_table_without_repurposing_embedded_fixtures() {
    let (path, show) = create("migration");
    show.put_object("future_fixture", "raw", &json!({"unknown":true}), 0)
        .unwrap();
    show.conn
        .execute(
            "INSERT INTO embedded_fixtures(id,revision,definition_json) VALUES ('legacy',7,'{\"keep\":true}')",
            [],
        )
        .unwrap();
    show.conn
        .execute_batch("DROP TABLE fixture_profile_revisions; UPDATE schema_info SET version=3;")
        .unwrap();
    drop(show);

    let migrated = ShowStore::open(&path).unwrap();
    let embedded: String = migrated
        .conn
        .query_row(
            "SELECT definition_json FROM embedded_fixtures WHERE id='legacy' AND revision=7",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let version: i64 = migrated
        .conn
        .query_row("SELECT version FROM schema_info", [], |row| row.get(0))
        .unwrap();
    assert_eq!((version, embedded.as_str()), (4, "{\"keep\":true}"));
    assert!(
        migrated
            .list_fixture_profile_revisions()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        migrated.objects("future_fixture").unwrap()[0].body["unknown"],
        true
    );
    drop(migrated);
    let _ = fs::remove_file(path);
}

#[test]
fn resolving_a_profile_verifies_its_full_content_digest() {
    let (path, show) = create("digest-verification");
    show.insert_fixture_profile_revision(&profile(PROFILE_A, 1, "Original"))
        .unwrap();
    show.conn
        .execute(
            "UPDATE fixture_profile_revisions SET profile_json=?1 WHERE profile_id=?2 AND revision=1",
            (profile_value(PROFILE_A, 1, "Tampered").to_string(), PROFILE_A),
        )
        .unwrap();
    assert!(matches!(
        show.resolve_fixture_profile_revision(fixture_id(PROFILE_A), 1),
        Err(StoreError::Invalid(message)) if message.contains("invalid content digest")
    ));
    drop(show);
    let _ = fs::remove_file(path);
}

#[test]
fn canonical_byte_equivalent_profiles_deduplicate_to_one_identity() {
    let (path, show) = create("deduplicate");
    let first: serde_json::Value = serde_json::from_str(
        r#"{"id":"00000000-0000-0000-0000-000000000001","revision":1,"name":"Lamp","modes":[],"unknown":{"b":2,"a":1}}"#,
    )
    .unwrap();
    let second: serde_json::Value = serde_json::from_str(
        r#"{ "unknown": { "a": 1, "b": 2 }, "modes": [], "name": "Lamp", "revision": 1, "id": "00000000-0000-0000-0000-000000000001" }"#,
    )
    .unwrap();
    let first = FixtureProfileRevision::from_profile(first).unwrap();
    let second = FixtureProfileRevision::from_profile(second).unwrap();
    assert_eq!(first.digest(), second.digest());

    let inserted = show.insert_fixture_profile_revision(&first).unwrap();
    let deduplicated = show.insert_fixture_profile_revision(&second).unwrap();
    assert_eq!(
        inserted.status(),
        FixtureProfileRevisionInsertStatus::Inserted
    );
    assert_eq!(
        deduplicated.status(),
        FixtureProfileRevisionInsertStatus::AlreadyPresent
    );
    assert_eq!(inserted.show_revision().value(), 1);
    assert_eq!(deduplicated.show_revision().value(), 1);
    assert_eq!(show.list_fixture_profile_revisions().unwrap().len(), 1);
    drop(show);
    let _ = fs::remove_file(path);
}

#[test]
fn portable_transaction_commits_one_profile_revision_and_targeted_delta() {
    let (path, show) = create("transaction-delta");
    let candidate = profile(PROFILE_A, 3, "Mover");
    let document = show.portable_document().unwrap();
    let mut transaction = document.transaction();
    transaction
        .put_fixture_profile_revision(candidate.clone())
        .unwrap()
        .put_fixture_profile_revision(candidate.clone())
        .unwrap();

    let commit = show.apply_portable_transaction(transaction).unwrap();
    assert_eq!(commit.revision().value(), 1);
    assert_eq!(
        commit.fixture_profile_revisions(),
        std::slice::from_ref(&candidate)
    );
    let all = show.list_fixture_profile_revisions().unwrap();
    let matching = show
        .list_fixture_profile_revisions_for(fixture_id(PROFILE_A))
        .unwrap();
    assert_eq!(all.as_slice(), std::slice::from_ref(&candidate));
    assert_eq!(matching.as_slice(), std::slice::from_ref(&candidate));
    assert_eq!(
        show.resolve_fixture_profile_revision(fixture_id(PROFILE_A), 3)
            .unwrap(),
        Some(candidate)
    );
    drop(show);
    let _ = fs::remove_file(path);
}

#[test]
fn profile_identity_conflict_rolls_back_profiles_and_objects() {
    let (path, show) = create("conflict-rollback");
    show.insert_fixture_profile_revision(&profile(PROFILE_Z, 1, "Stored"))
        .unwrap();
    let document = show.portable_document().unwrap();
    let mut transaction = document.transaction();
    transaction
        .put_fixture_profile_revision(profile(PROFILE_A, 1, "New"))
        .unwrap();
    transaction
        .put_fixture_profile_revision(profile(PROFILE_Z, 1, "Different"))
        .unwrap();
    transaction.put("fixture", "new", json!({"name":"must roll back"}));

    assert!(matches!(
        show.apply_portable_transaction(transaction),
        Err(StoreError::FixtureProfileRevisionConflict {
            profile_id,
            revision: 1,
            ..
        }) if profile_id == PROFILE_Z
    ));
    assert!(
        show.resolve_fixture_profile_revision(fixture_id(PROFILE_A), 1)
            .unwrap()
            .is_none()
    );
    assert!(show.objects("fixture").unwrap().is_empty());
    assert_eq!(show.portable_revision().unwrap().value(), 1);
    drop(show);
    let _ = fs::remove_file(path);
}

#[test]
fn backup_preserves_profile_json_digest_and_inline_assets() {
    let (source, show) = create("backup-source");
    let backup = temporary("backup-copy");
    let candidate =
        FixtureProfileRevision::from_profile(profile_value(PROFILE_PORTABLE, 9, "Portable"))
            .unwrap();
    show.insert_fixture_profile_revision(&candidate).unwrap();
    show.backup_to(&backup).unwrap();

    let copied = ShowStore::open(&backup).unwrap();
    let resolved = copied
        .resolve_fixture_profile_revision(fixture_id(PROFILE_PORTABLE), 9)
        .unwrap()
        .unwrap();
    assert_eq!(resolved, candidate);
    assert_eq!(
        resolved.profile()["future_asset_manifest"]["checksum"],
        "retain-exactly"
    );
    assert_eq!(
        copied
            .portable_document()
            .unwrap()
            .fixture_profile_revisions()
            .len(),
        1
    );
    drop(show);
    drop(copied);
    let _ = fs::remove_file(source);
    let _ = fs::remove_file(backup);
}
