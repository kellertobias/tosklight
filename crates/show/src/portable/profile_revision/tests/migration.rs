use super::support::{PROFILE_A, create, fixture_id, temporary};
use crate::{ShowStore, StoreError};
use rusqlite::{Connection, params};
use std::{fs, path::Path};

#[test]
fn schema_three_upgrade_deduplicates_nested_profiles_without_rewriting_objects() {
    let first = first_fixture_body();
    let nested = nested_fixture_body();
    let (path, show) = create("automatic-migration");
    insert_raw_object(&show.conn, "fixture", "one", &first);
    insert_raw_object(&show.conn, "fixture_bundle", "two", &nested);
    downgrade_to_schema_three(&show.conn);
    drop(show);

    let migrated = ShowStore::open(&path).unwrap();
    let profiles = migrated.list_fixture_profile_revisions().unwrap();
    assert_eq!(schema_version(&migrated.conn), 4);
    assert_eq!(migrated.portable_revision().unwrap().value(), 0);
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].profile()["future_asset"]["bytes"], "AAECAwQ=");
    assert_eq!(raw_body(&migrated.conn, "fixture", "one"), first);
    assert_eq!(raw_body(&migrated.conn, "fixture_bundle", "two"), nested);
    drop(migrated);
    remove_show(&path);
}

#[test]
fn conflicting_inline_profiles_roll_back_the_complete_schema_upgrade() {
    let first = first_fixture_body();
    let conflicting = nested_fixture_body().replace("Legacy profile", "Conflicting profile");
    let (path, show) = create("migration-conflict");
    insert_raw_object(&show.conn, "fixture", "a", &first);
    insert_raw_object(&show.conn, "fixture", "b", &conflicting);
    downgrade_to_schema_three(&show.conn);
    drop(show);

    assert_profile_conflict(&path);
    assert_rollback_state(&path, &first, &conflicting);
    remove_show(&path);
}

#[test]
fn reopening_a_migrated_schema_four_show_is_idempotent() {
    let body = first_fixture_body();
    let (path, show) = create("migration-reopen");
    insert_raw_object(&show.conn, "fixture", "one", &body);
    downgrade_to_schema_three(&show.conn);
    drop(show);

    let first_open = ShowStore::open(&path).unwrap();
    let before = first_open.list_fixture_profile_revisions().unwrap();
    assert_eq!(before.len(), 1);
    drop(first_open);
    let reopened = ShowStore::open(&path).unwrap();
    assert_eq!(reopened.list_fixture_profile_revisions().unwrap(), before);
    assert_eq!(schema_version(&reopened.conn), 4);
    assert_eq!(raw_body(&reopened.conn, "fixture", "one"), body);
    drop(reopened);
    remove_show(&path);
}

#[test]
fn backup_after_migration_keeps_profiles_assets_and_inline_fallbacks() {
    let body = first_fixture_body();
    let (path, show) = create("migration-backup-source");
    let backup = temporary("migration-backup-copy");
    insert_raw_object(&show.conn, "fixture", "one", &body);
    downgrade_to_schema_three(&show.conn);
    drop(show);

    let migrated = ShowStore::open(&path).unwrap();
    let expected = migrated.list_fixture_profile_revisions().unwrap();
    migrated.backup_to(&backup).unwrap();
    let copied = ShowStore::open(&backup).unwrap();
    assert_portable_copy(&copied, &expected, &body);
    drop(migrated);
    drop(copied);
    remove_show(&path);
    remove_show(&backup);
}

fn assert_profile_conflict(path: &Path) {
    assert!(matches!(
        ShowStore::open(path),
        Err(StoreError::FixtureProfileRevisionConflict {
            profile_id,
            revision: 7,
            ..
        }) if profile_id == PROFILE_A
    ));
}

fn assert_rollback_state(path: &Path, first: &str, conflicting: &str) {
    let unchanged = Connection::open(path).unwrap();
    assert_eq!(schema_version(&unchanged), 3);
    assert!(!table_exists(&unchanged, "fixture_profile_revisions"));
    assert_eq!(raw_body(&unchanged, "fixture", "a"), first);
    assert_eq!(raw_body(&unchanged, "fixture", "b"), conflicting);
}

fn assert_portable_copy(
    copied: &ShowStore,
    expected: &[crate::FixtureProfileRevision],
    body: &str,
) {
    assert_eq!(copied.list_fixture_profile_revisions().unwrap(), expected);
    assert_eq!(
        copied
            .resolve_fixture_profile_revision(fixture_id(PROFILE_A), 7)
            .unwrap(),
        expected.first().cloned()
    );
    assert_eq!(raw_body(&copied.conn, "fixture", "one"), body);
    assert_eq!(
        expected[0].profile()["future_asset"]["uri"],
        "data:application/octet-stream;base64,AAECAwQ="
    );
}

fn insert_raw_object(conn: &Connection, kind: &str, id: &str, body: &str) {
    conn.execute(
        "INSERT INTO objects(kind,id,body_json,revision,updated_at) VALUES (?1,?2,?3,1,'legacy')",
        params![kind, id, body],
    )
    .unwrap();
}

fn downgrade_to_schema_three(conn: &Connection) {
    conn.execute_batch("DROP TABLE fixture_profile_revisions; UPDATE schema_info SET version=3;")
        .unwrap();
}

fn schema_version(conn: &Connection) -> i64 {
    conn.query_row("SELECT version FROM schema_info", [], |row| row.get(0))
        .unwrap()
}

fn raw_body(conn: &Connection, kind: &str, id: &str) -> String {
    conn.query_row(
        "SELECT body_json FROM objects WHERE kind=?1 AND id=?2",
        [kind, id],
        |row| row.get(0),
    )
    .unwrap()
}

fn table_exists(conn: &Connection, table: &str) -> bool {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
        [table],
        |row| row.get(0),
    )
    .unwrap()
}

fn remove_show(path: &Path) {
    let _ = fs::remove_file(path);
}

fn first_fixture_body() -> String {
    FIRST_FIXTURE_BODY.replace("__PROFILE_ID__", PROFILE_A)
}

fn nested_fixture_body() -> String {
    NESTED_FIXTURE_BODY.replace("__PROFILE_ID__", PROFILE_A)
}

const FIRST_FIXTURE_BODY: &str = r#"{
  "future_outer": {"order": [3, 2, 1], "retain": true},
  "definition": {
    "future_definition": "untouched",
    "profile_snapshot": {
      "revision": 7,
      "id": "__PROFILE_ID__",
      "name": "Legacy profile",
      "schema_version": 2,
      "modes": [],
      "future_asset": {
        "bytes": "AAECAwQ=",
        "uri": "data:application/octet-stream;base64,AAECAwQ="
      }
    }
  }
}"#;

const NESTED_FIXTURE_BODY: &str = r#"{"future_outer":{"retain":true},"fixtures":[{"definition":{"profile_snapshot":{"future_asset":{"uri":"data:application/octet-stream;base64,AAECAwQ=","bytes":"AAECAwQ="},"modes":[],"schema_version":2,"name":"Legacy profile","id":"__PROFILE_ID__","revision":7}},"future_nested":"untouched"}]}"#;
