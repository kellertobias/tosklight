use super::temporary;
use crate::{DeskStore, PersistedSession, RevisionCopySource, ShowStore};
use chrono::Utc;
use light_core::{SessionId, ShowId};
use rusqlite::Connection;
use std::fs;

#[test]
fn desk_sessions_survive_reopen() {
    let path = temporary("desk");
    let session = {
        let desk = DeskStore::open(&path).unwrap();
        let session = PersistedSession {
            id: SessionId::new(),
            token: "token".into(),
            programmer_json: "{}".into(),
            connected: false,
            updated_at: Utc::now().to_rfc3339(),
        };
        desk.save_session(&session).unwrap();
        session
    };
    let desk = DeskStore::open(&path).unwrap();
    let loaded = desk.persisted_sessions().unwrap();
    assert_eq!(loaded[0].id, session.id);
    assert_eq!(loaded[0].token, session.token);
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
fn show_creation_and_explicit_load_times_survive_reopen_without_changing_last_save() {
    let path = temporary("show-history");
    let show_id = {
        let desk = DeskStore::open(&path).unwrap();
        let created = desk.upsert_show("Tour", "tour.show", false).unwrap();
        assert!(created.created_at.is_some());
        assert_eq!(created.last_loaded_at, None);
        let loaded = desk.mark_show_loaded(created.id).unwrap();
        assert!(loaded.last_loaded_at.is_some());
        assert_eq!(loaded.created_at, created.created_at);
        assert_eq!(loaded.updated_at, created.updated_at);
        created.id
    };
    let desk = DeskStore::open(&path).unwrap();
    let reopened = desk.show(show_id).unwrap().unwrap();
    assert!(reopened.created_at.is_some());
    assert!(reopened.last_loaded_at.is_some());
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
    assert!(!legacy.is_base_show);
    assert!(legacy.revision_copy.is_none());
    assert_eq!(legacy.created_at, None);
    assert_eq!(legacy.last_loaded_at, None);
    let version: i64 = desk
        .conn
        .query_row("SELECT version FROM schema_info", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, crate::desk::DESK_SCHEMA_VERSION);
    drop(desk);
    let _ = fs::remove_file(path);
}

#[test]
fn base_show_designation_survives_reopen_rename_and_overwrite_without_marking_copies() {
    let path = temporary("base-show-library");
    let id = {
        let mut desk = DeskStore::open(&path).unwrap();
        let entry = desk.upsert_show("Base", "base.show", false).unwrap();
        assert!(!entry.is_base_show);
        desk.add_show_revision(entry.id, "Approved", "approved.show")
            .unwrap();
        let marked = desk.set_show_base(entry.id, true).unwrap();
        assert!(marked.is_base_show);
        assert_eq!(marked.revision, entry.revision + 1);
        let repeated = desk.set_show_base(entry.id, true).unwrap();
        assert_eq!(repeated.revision, marked.revision);
        assert_eq!(repeated.updated_at, marked.updated_at);
        assert!(
            desk.rename_show(entry.id, "Renamed base", "renamed.show")
                .unwrap()
                .is_base_show
        );
        assert!(
            desk.upsert_show("Renamed base", "renamed.show", true)
                .unwrap()
                .is_base_show
        );
        assert!(
            !desk
                .upsert_show("Working copy", "copy.show", false)
                .unwrap()
                .is_base_show
        );
        assert_eq!(desk.show_revisions(entry.id).unwrap()[0].name, "Approved");
        assert!(desk.set_show_base(ShowId::new(), true).is_err());
        entry.id
    };
    let desk = DeskStore::open(&path).unwrap();
    assert!(desk.show(id).unwrap().unwrap().is_base_show);
    assert!(!desk.set_show_base(id, false).unwrap().is_base_show);
    assert_eq!(desk.show_revisions(id).unwrap().len(), 1);
    drop(desk);
    assert!(
        !DeskStore::open(&path)
            .unwrap()
            .show(id)
            .unwrap()
            .unwrap()
            .is_base_show
    );
    let _ = fs::remove_file(path);
}

#[test]
fn schema_fifteen_base_migration_preserves_library_identity_history_and_active_show() {
    let path = temporary("base-show-legacy-fifteen");
    let id = ShowId::new();
    {
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("CREATE TABLE schema_info(version INTEGER NOT NULL);
            INSERT INTO schema_info VALUES(15);
            CREATE TABLE show_library(id TEXT PRIMARY KEY,name TEXT NOT NULL UNIQUE COLLATE NOCASE,path TEXT NOT NULL,revision INTEGER NOT NULL DEFAULT 1,updated_at TEXT NOT NULL,created_at TEXT,last_loaded_at TEXT);
            CREATE TABLE settings(key TEXT PRIMARY KEY,value TEXT NOT NULL);
            CREATE TABLE show_revisions(show_id TEXT NOT NULL,revision INTEGER NOT NULL,name TEXT NOT NULL,path TEXT NOT NULL,created_at TEXT NOT NULL,PRIMARY KEY(show_id,revision));").unwrap();
        conn.execute("INSERT INTO show_library(id,name,path,revision,updated_at,created_at,last_loaded_at) VALUES(?1,'Legacy base candidate','legacy.show',7,'saved','created','loaded')", [id.0.to_string()]).unwrap();
        conn.execute(
            "INSERT INTO show_revisions(show_id,revision,name,path,created_at) VALUES(?1,3,'Approved legacy','approved.show','saved')",
            [id.0.to_string()],
        ).unwrap();
        conn.execute(
            "INSERT INTO settings(key,value) VALUES('active_show_id',?1)",
            [id.0.to_string()],
        )
        .unwrap();
    }
    let desk = DeskStore::open(&path).unwrap();
    let entry = desk.show(id).unwrap().unwrap();
    assert!(!entry.is_base_show);
    assert_eq!(entry.revision, 7);
    assert_eq!(entry.path, "legacy.show");
    assert_eq!(entry.updated_at, "saved");
    assert_eq!(entry.created_at.as_deref(), Some("created"));
    assert_eq!(entry.last_loaded_at.as_deref(), Some("loaded"));
    let active: String = desk
        .conn
        .query_row(
            "SELECT value FROM settings WHERE key='active_show_id'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(active, id.0.to_string());
    let revision = desk.show_revision(id, 3).unwrap().unwrap();
    assert_eq!(revision.name, "Approved legacy");
    assert_eq!(revision.path, "approved.show");
    assert!(desk.set_show_base(id, true).unwrap().is_base_show);
    drop(desk);
    assert!(
        DeskStore::open(&path)
            .unwrap()
            .show(id)
            .unwrap()
            .unwrap()
            .is_base_show
    );
    let _ = fs::remove_file(path);
}

#[test]
fn legacy_show_entry_json_defaults_to_an_ordinary_show() {
    let entry: crate::ShowEntry = serde_json::from_value(serde_json::json!({
        "id":ShowId::new(),"name":"Legacy","path":"legacy.show","revision":1,"updated_at":"saved"
    }))
    .unwrap();
    assert!(!entry.is_base_show);
}

fn activation_metadata_snapshot(
    desk: &DeskStore,
) -> (Option<String>, Option<String>, serde_json::Value) {
    (
        desk.setting("active_show_id").unwrap(),
        desk.setting("previous_active_show_id").unwrap(),
        serde_json::to_value(desk.library().unwrap()).unwrap(),
    )
}

#[test]
fn record_show_activation_survives_reopen_and_preserves_content_metadata() {
    let path = temporary("activation-metadata");
    let desk = DeskStore::open(&path).unwrap();
    let previous = desk
        .upsert_show("Previous", "previous.show", false)
        .unwrap();
    let source = RevisionCopySource {
        show_id: previous.id,
        show_name: previous.name.clone(),
        revision: 4,
        revision_name: "Approved".into(),
        copied_at: "2026-09-30T10:00:00Z".into(),
    };
    let destination = desk
        .upsert_show_with_revision_copy("Destination", "destination.show", false, Some(&source))
        .unwrap();
    let destination = desk.set_show_base(destination.id, true).unwrap();
    desk.set_active_show(Some(previous.id)).unwrap();
    let loaded = desk
        .record_show_activation(destination.id, Some(previous.id))
        .unwrap();
    assert!(loaded.last_loaded_at.is_some());
    let mut unchanged_content = loaded.clone();
    unchanged_content.last_loaded_at = destination.last_loaded_at.clone();
    assert_eq!(
        serde_json::to_value(unchanged_content).unwrap(),
        serde_json::to_value(&destination).unwrap()
    );
    drop(desk);

    let desk = DeskStore::open(&path).unwrap();
    assert_eq!(
        serde_json::to_value(desk.active_show().unwrap().unwrap()).unwrap(),
        serde_json::to_value(&loaded).unwrap()
    );
    assert_eq!(
        desk.setting("previous_active_show_id").unwrap(),
        Some(previous.id.0.to_string())
    );
    // MVR's None must leave previous metadata intact, rather than deleting it.
    let previous_loaded = desk.record_show_activation(previous.id, None).unwrap();
    assert!(previous_loaded.last_loaded_at.is_some());
    assert_eq!(previous_loaded.updated_at, previous.updated_at);
    assert_eq!(previous_loaded.revision, previous.revision);
    drop(desk);
    let desk = DeskStore::open(&path).unwrap();
    assert_eq!(desk.active_show().unwrap().unwrap().id, previous.id);
    assert_eq!(
        desk.setting("previous_active_show_id").unwrap(),
        Some(previous.id.0.to_string())
    );
    assert_eq!(
        desk.show(destination.id).unwrap().unwrap().last_loaded_at,
        loaded.last_loaded_at
    );
    drop(desk);
    let _ = fs::remove_file(path);
}

#[test]
fn record_show_activation_missing_destination_changes_no_metadata() {
    let path = temporary("activation-missing");
    let desk = DeskStore::open(&path).unwrap();
    let previous = desk
        .upsert_show("Previous", "previous.show", false)
        .unwrap();
    desk.set_active_show(Some(previous.id)).unwrap();
    desk.set_setting("previous_active_show_id", "original previous value")
        .unwrap();
    let before = activation_metadata_snapshot(&desk);
    assert!(
        desk.record_show_activation(ShowId::new(), Some(previous.id))
            .is_err()
    );
    assert_eq!(activation_metadata_snapshot(&desk), before);
    drop(desk);
    let desk = DeskStore::open(&path).unwrap();
    assert_eq!(activation_metadata_snapshot(&desk), before);
    drop(desk);
    let _ = fs::remove_file(path);
}

#[test]
fn record_show_activation_sql_failure_rolls_back_prior_timestamp_and_active_id_writes() {
    let path = temporary("activation-sql-rollback");
    let desk = DeskStore::open(&path).unwrap();
    let previous = desk
        .upsert_show("Previous", "previous.show", false)
        .unwrap();
    let destination = desk
        .upsert_show("Destination", "destination.show", false)
        .unwrap();
    desk.set_active_show(Some(previous.id)).unwrap();
    desk.set_setting("previous_active_show_id", "original previous value")
        .unwrap();
    // The helper has already updated last_loaded_at and active_show_id when this third
    // statement fails. RAISE(ABORT) rolls back only its statement; the helper must roll back
    // the enclosing transaction to undo the earlier two writes as well.
    desk.conn
        .execute_batch(
            "CREATE TRIGGER reject_activation_previous BEFORE INSERT ON settings
         WHEN NEW.key='previous_active_show_id'
         BEGIN SELECT RAISE(ABORT, 'injected activation metadata failure'); END;",
        )
        .unwrap();
    let before = activation_metadata_snapshot(&desk);
    let error = desk
        .record_show_activation(destination.id, Some(previous.id))
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("injected activation metadata failure"),
        "{error}"
    );
    assert_eq!(activation_metadata_snapshot(&desk), before);
    drop(desk);
    let desk = DeskStore::open(&path).unwrap();
    assert_eq!(activation_metadata_snapshot(&desk), before);
    desk.conn
        .execute_batch("DROP TRIGGER reject_activation_previous")
        .unwrap();
    let loaded = desk
        .record_show_activation(destination.id, Some(previous.id))
        .unwrap();
    assert!(loaded.last_loaded_at.is_some());
    assert_eq!(desk.active_show().unwrap().unwrap().id, destination.id);
    drop(desk);
    let _ = fs::remove_file(path);
}
