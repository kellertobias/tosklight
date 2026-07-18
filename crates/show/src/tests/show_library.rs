use super::temporary;
use crate::{DeskStore, PersistedSession, RevisionCopySource, ShowStore};
use chrono::Utc;
use light_core::{SessionId, ShowId};
use rusqlite::Connection;
use std::fs;

#[test]
fn desk_sessions_survive_reopen() {
    let path = temporary("desk");
    let (user, session) = {
        let desk = DeskStore::open(&path).unwrap();
        let user = desk.users().unwrap().remove(0);
        let session = PersistedSession {
            id: SessionId::new(),
            user_id: user.id,
            token: "token".into(),
            programmer_json: "{}".into(),
            connected: false,
            updated_at: Utc::now().to_rfc3339(),
        };
        desk.save_session(&session).unwrap();
        (user, session)
    };
    let desk = DeskStore::open(&path).unwrap();
    let loaded = desk.persisted_sessions().unwrap();
    assert_eq!(loaded[0].id, session.id);
    assert_eq!(loaded[0].user_id, user.id);
    let _ = fs::remove_file(path);
}

#[test]
fn named_show_revisions_are_numbered_and_survive_reopen() {
    let path = temporary("named-revisions");
    let show_path = temporary("named-revision-show");
    let revision_path = temporary("named-revision-snapshot");
    let show_id = {
        let mut desk = DeskStore::open(&path).unwrap();
        let entry = desk
            .upsert_show("Tour", show_path.to_str().unwrap(), false)
            .unwrap();
        let first = desk
            .add_show_revision(
                entry.id,
                "Before experiments",
                revision_path.to_str().unwrap(),
            )
            .unwrap();
        let second = desk
            .add_show_revision(entry.id, "Approved", revision_path.to_str().unwrap())
            .unwrap();
        assert_eq!(first.revision, 1);
        assert_eq!(second.revision, 2);
        entry.id
    };
    let desk = DeskStore::open(&path).unwrap();
    let revisions = desk.show_revisions(show_id).unwrap();
    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[0].revision, 2);
    assert_eq!(revisions[0].name, "Approved");
    assert_eq!(
        desk.show_revision(show_id, 1).unwrap().unwrap().name,
        "Before experiments"
    );
    drop(desk);
    let _ = fs::remove_file(path);
}

#[test]
fn revision_copy_provenance_survives_reopen_and_source_deletion() {
    let desk_path = temporary("revision-copy-desk");
    let source_path = temporary("revision-copy-source");
    let copy_path = temporary("revision-copy-file");
    let expected = {
        let desk = DeskStore::open(&desk_path).unwrap();
        let source = desk
            .upsert_show("Tour", source_path.to_str().unwrap(), false)
            .unwrap();
        let provenance = RevisionCopySource {
            show_id: source.id,
            show_name: source.name.clone(),
            revision: 4,
            revision_name: "Before focus rewrite".into(),
            copied_at: "2026-07-17T10:30:00Z".into(),
        };
        let copy = desk
            .upsert_show_with_revision_copy(
                "Tour-rev-4-2026-07-17",
                copy_path.to_str().unwrap(),
                false,
                Some(&provenance),
            )
            .unwrap();
        assert_eq!(copy.revision_copy.as_ref(), Some(&provenance));
        assert!(desk.remove_show(source.id).unwrap());
        provenance
    };
    let desk = DeskStore::open(&desk_path).unwrap();
    let copy = desk.library().unwrap().remove(0);
    assert_eq!(copy.revision_copy, Some(expected));
    drop(desk);
    let _ = fs::remove_file(desk_path);
}

#[test]
fn revision_copy_metadata_is_portable_with_the_show_file() {
    let path = temporary("revision-copy-metadata");
    let (store, copy_id) = ShowStore::create(&path, "Copy").unwrap();
    let source = RevisionCopySource {
        show_id: ShowId::new(),
        show_name: "Original".into(),
        revision: 2,
        revision_name: "Approved plot".into(),
        copied_at: "2026-07-17T11:00:00Z".into(),
    };
    store.set_identity(copy_id, "Copy", Some(&source)).unwrap();
    drop(store);
    let reopened = ShowStore::open(&path).unwrap();
    assert_eq!(reopened.revision_copy_source().unwrap(), Some(source));
    drop(reopened);
    let _ = fs::remove_file(path);
}

#[test]
fn desk_schema_six_migrates_existing_shows_without_copy_provenance() {
    let path = temporary("legacy-desk-revision-copy");
    let show_id = ShowId::new();
    {
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE schema_info(version INTEGER NOT NULL);
                 INSERT INTO schema_info(version) VALUES(6);
                 CREATE TABLE show_library(id TEXT PRIMARY KEY,name TEXT NOT NULL UNIQUE COLLATE NOCASE,path TEXT NOT NULL,revision INTEGER NOT NULL DEFAULT 1,updated_at TEXT NOT NULL);",
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO show_library(id,name,path,revision,updated_at) VALUES(?1,'Legacy','legacy.show',3,'2025-01-01T00:00:00Z')",
                [show_id.0.to_string()],
            )
            .unwrap();
    }
    let desk = DeskStore::open(&path).unwrap();
    let legacy = desk.show(show_id).unwrap().unwrap();
    assert_eq!(legacy.name, "Legacy");
    assert!(legacy.revision_copy.is_none());
    let version: i64 = desk
        .conn
        .query_row("SELECT version FROM schema_info", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, crate::desk::DESK_SCHEMA_VERSION);
    drop(desk);
    let _ = fs::remove_file(path);
}

#[test]
fn desk_always_retains_an_enabled_login_user() {
    let path = temporary("users");
    let desk = DeskStore::open(&path).unwrap();
    let operator = desk.users().unwrap().remove(0);
    assert!(desk.update_user(operator.id, "Operator", false).is_err());
    assert!(desk.delete_user(operator.id).is_err());
    let second = desk.add_user("Programmer").unwrap();
    desk.update_user(operator.id, "Operator", false).unwrap();
    assert!(!desk.user(operator.id).unwrap().unwrap().enabled);
    assert!(desk.delete_user(operator.id).unwrap());
    assert_eq!(desk.users().unwrap(), vec![second]);
    let _ = fs::remove_file(path);
}
