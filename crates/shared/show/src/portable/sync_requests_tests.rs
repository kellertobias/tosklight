//! Exactly-once sync request identities share the edit's SQLite transaction.
use super::{SYNC_APPLIED_REQUEST_RETENTION, SyncRequestRecord};
use crate::ShowStore;
use rusqlite::Connection;
use serde_json::json;
use std::{fs, path::PathBuf};
use uuid::Uuid;

fn temporary(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("light-sync-{name}-{}.show", Uuid::new_v4()))
}

fn record(association_id: Uuid, request_id: &str) -> SyncRequestRecord {
    SyncRequestRecord {
        association_id,
        request_id: request_id.into(),
        signature: "sha256:abc".into(),
        outcome: json!({"status": "accepted"}),
    }
}

#[test]
fn a_request_identity_is_recorded_with_the_edit_it_carried_and_survives_reopening() {
    let path = temporary("recorded");
    let (show, _) = ShowStore::create(&path, "Sync").unwrap();
    let association = Uuid::new_v4();
    let document = show.portable_document().unwrap();
    let mut transaction = document.transaction();
    transaction
        .put("cad_annotation", "note-1", json!({"text": "Front truss"}))
        .set_metadata("architect.venue", Some("Hall A".into()))
        .record_sync_request(record(association, "request-1"));
    let commit = show.apply_portable_transaction(transaction).unwrap();
    assert_eq!(commit.sync_request(), Some((association, "request-1")));
    assert_eq!(
        commit.metadata_changes(),
        &[("architect.venue".to_owned(), Some("Hall A".to_owned()))]
    );
    drop(show);
    let reopened = ShowStore::open(&path).unwrap();
    let applied = reopened
        .sync_applied_request(association, "request-1")
        .unwrap()
        .expect("the identity committed with its edit");
    assert_eq!(applied.show_revision, commit.revision().value());
    assert_eq!(applied.outcome, json!({"status": "accepted"}));
    assert_eq!(
        reopened
            .metadata_value("architect.venue")
            .unwrap()
            .as_deref(),
        Some("Hall A")
    );
    assert!(
        reopened
            .sync_applied_request(Uuid::new_v4(), "request-1")
            .unwrap()
            .is_none(),
        "an identity is scoped to its association"
    );
    drop(reopened);
    let _ = fs::remove_file(path);
}

#[test]
fn a_transaction_that_changes_nothing_records_no_identity_and_no_revision() {
    let path = temporary("unchanged");
    let (show, _) = ShowStore::create(&path, "Sync").unwrap();
    let association = Uuid::new_v4();
    let document = show.portable_document().unwrap();
    let mut transaction = document.transaction();
    transaction.record_sync_request(record(association, "noop"));
    let commit = show.apply_portable_transaction(transaction).unwrap();
    assert_eq!(commit.revision(), document.revision());
    assert_eq!(commit.sync_request(), None);
    assert!(
        show.sync_applied_request(association, "noop")
            .unwrap()
            .is_none()
    );
    drop(show);
    let _ = fs::remove_file(path);
}

#[test]
fn a_stale_transaction_records_neither_its_edit_nor_its_identity() {
    let path = temporary("stale");
    let (show, _) = ShowStore::create(&path, "Sync").unwrap();
    let association = Uuid::new_v4();
    let document = show.portable_document().unwrap();
    show.put_object("fixture_note", "a", &json!({"text": "moved on"}), 0)
        .unwrap();
    let mut transaction = document.transaction();
    transaction
        .put("fixture_note", "b", json!({"text": "late"}))
        .record_sync_request(record(association, "stale"));
    assert!(show.apply_portable_transaction(transaction).is_err());
    assert!(
        show.sync_applied_request(association, "stale")
            .unwrap()
            .is_none()
    );
    drop(show);
    let _ = fs::remove_file(path);
}

#[test]
fn a_legacy_show_without_the_table_opens_unchanged_and_gains_it_with_its_first_sync_write() {
    let path = temporary("legacy");
    let (show, _) = ShowStore::create(&path, "Legacy").unwrap();
    drop(show);
    Connection::open(&path)
        .unwrap()
        .execute_batch("DROP TABLE sync_applied_requests;")
        .unwrap();
    let reopened = ShowStore::open(&path).unwrap();
    let association = Uuid::new_v4();
    assert!(
        reopened
            .sync_applied_request(association, "first")
            .unwrap()
            .is_none()
    );
    let document = reopened.portable_document().unwrap();
    let mut transaction = document.transaction();
    transaction
        .put("fixture_note", "n", json!({"text": "first sync"}))
        .record_sync_request(record(association, "first"));
    reopened.apply_portable_transaction(transaction).unwrap();
    assert!(
        reopened
            .sync_applied_request(association, "first")
            .unwrap()
            .is_some()
    );
    drop(reopened);
    let _ = fs::remove_file(path);
}

#[test]
fn retention_keeps_only_the_newest_identities() {
    let path = temporary("retention");
    let (show, _) = ShowStore::create(&path, "Retention").unwrap();
    let association = Uuid::new_v4();
    drop(show);
    {
        let mut conn = Connection::open(&path).unwrap();
        let tx = conn.transaction().unwrap();
        for index in 0..SYNC_APPLIED_REQUEST_RETENTION {
            tx.execute(
                "INSERT INTO sync_applied_requests VALUES(?1,?2,'s','{}',1,'2026-01-01T00:00:00Z')",
                (association.to_string(), format!("old-{index}")),
            )
            .unwrap();
        }
        tx.commit().unwrap();
    }
    let show = ShowStore::open(&path).unwrap();
    let document = show.portable_document().unwrap();
    let mut transaction = document.transaction();
    transaction
        .put("fixture_note", "n", json!({"text": "newest"}))
        .record_sync_request(record(association, "newest"));
    show.apply_portable_transaction(transaction).unwrap();
    assert!(
        show.sync_applied_request(association, "old-0")
            .unwrap()
            .is_none(),
        "the oldest identity is pruned"
    );
    assert!(
        show.sync_applied_request(association, "old-1")
            .unwrap()
            .is_some()
    );
    assert!(
        show.sync_applied_request(association, "newest")
            .unwrap()
            .is_some()
    );
    drop(show);
    let _ = fs::remove_file(path);
}

#[test]
fn a_replacement_file_adopts_the_desk_identities_and_the_desk_row_wins() {
    let desk_path = temporary("adopt-desk");
    let architect_path = temporary("adopt-architect");
    let (desk, _) = ShowStore::create(&desk_path, "Desk").unwrap();
    let (architect, _) = ShowStore::create(&architect_path, "Architect").unwrap();
    let association = Uuid::new_v4();
    for (store, text) in [(&desk, "desk"), (&architect, "architect")] {
        let document = store.portable_document().unwrap();
        let mut transaction = document.transaction();
        let mut shared = record(association, "shared");
        shared.outcome = json!({"from": text});
        transaction
            .put("fixture_note", "n", json!({"text": text}))
            .record_sync_request(shared);
        store.apply_portable_transaction(transaction).unwrap();
    }
    let document = desk.portable_document().unwrap();
    let mut transaction = document.transaction();
    transaction
        .put("fixture_note", "m", json!({"text": "desk only"}))
        .record_sync_request(record(association, "desk-only"));
    desk.apply_portable_transaction(transaction).unwrap();

    assert_eq!(architect.adopt_sync_applied_requests(&desk).unwrap(), 2);
    assert!(
        architect
            .sync_applied_request(association, "desk-only")
            .unwrap()
            .is_some()
    );
    assert_eq!(
        architect
            .sync_applied_request(association, "shared")
            .unwrap()
            .unwrap()
            .outcome,
        json!({"from": "desk"})
    );
    drop((desk, architect));
    let _ = fs::remove_file(desk_path);
    let _ = fs::remove_file(architect_path);
}
