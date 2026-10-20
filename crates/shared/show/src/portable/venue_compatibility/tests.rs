use crate::{AtomicObjectWrite, ShowStore};
use serde_json::{Value, json};

fn create() -> (std::path::PathBuf, ShowStore) {
    let scratch = std::env::var_os("LIGHT_TMP_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../.artifacts/tmp")
        });
    std::fs::create_dir_all(&scratch).unwrap();
    let path = scratch.join(format!("venue-compatibility-{}.show", uuid::Uuid::new_v4()));
    let (store, _) = ShowStore::create(&path, "Venue persistence").unwrap();
    (path, store)
}
fn schema(store: &ShowStore) -> i64 {
    store
        .conn
        .query_row("SELECT version FROM schema_info", [], |row| row.get(0))
        .unwrap()
}
fn wall() -> Value {
    json!({"scenery_options":{"architecture":{"kind":"wall","openings":[{
        "id":"2e99aaac-c65e-4fbf-81f3-c3f817131234","offset_millimetres":1000,
        "sill_millimetres":0,"width_millimetres":900,"height_millimetres":2100,
        "opening":{"kind":"door","leaves":"single","swing":"inward","hinge":"left"}
    }],"ceiling_height_millimetres":4000}},"future_extension":{"preserve":true}})
}

#[test]
fn venue_schema_promotion_save_reopen_undo_redo_is_lossless() {
    let (path, store) = create();
    assert_eq!(schema(&store), 10);
    let legacy = json!({"name":"Existing scenery","scenery_options":{"colour_srgb":"#FFFFFF"}});
    store
        .put_object("patched_fixture", "wall", &legacy, 0)
        .unwrap();
    assert_eq!(schema(&store), 10);
    let body = wall();
    let document = store.portable_document().unwrap();
    let mut edit = document.transaction();
    edit.put("patched_fixture", "wall", body.clone());
    store.apply_portable_transaction(edit).unwrap();
    assert_eq!(schema(&store), 11);
    drop(store);
    let store = ShowStore::open(&path).unwrap();
    assert_eq!(schema(&store), 11);
    assert_eq!(store.objects("patched_fixture").unwrap()[0].body, body);
    let mut undo = store.portable_document().unwrap().transaction();
    undo.undo_object(
        store
            .prepare_object_undo("patched_fixture", "wall", 2)
            .unwrap(),
    );
    store.apply_portable_transaction(undo).unwrap();
    assert_eq!(store.objects("patched_fixture").unwrap()[0].body, legacy);
    assert_eq!(schema(&store), 11);
    let mut redo = store.portable_document().unwrap().transaction();
    redo.redo_object(
        store
            .prepare_object_redo("patched_fixture", "wall", 3)
            .unwrap(),
    );
    store.apply_portable_transaction(redo).unwrap();
    assert_eq!(store.objects("patched_fixture").unwrap()[0].body, body);
    store.delete_object("patched_fixture", "wall").unwrap();
    assert_eq!(schema(&store), 11);
    drop(store);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn failed_atomic_venue_write_does_not_promote_schema() {
    let (path, store) = create();
    let body = wall();
    assert!(
        store
            .mutate_objects_atomically(
                &[
                    AtomicObjectWrite {
                        kind: "patched_fixture",
                        id: "wall",
                        body: &body,
                        expected: 0
                    },
                    AtomicObjectWrite {
                        kind: "fixture_note",
                        id: "conflict",
                        body: &serde_json::json!({"text":"conflict"}),
                        expected: 99
                    },
                ],
                &[]
            )
            .is_err()
    );
    assert_eq!(schema(&store), 10);
    assert!(store.objects("patched_fixture").unwrap().is_empty());
    store
        .put_object("patched_fixture", "wall", &body, 0)
        .unwrap();
    assert_eq!(schema(&store), 11);
    drop(store);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn unrelated_extension_metadata_does_not_promote_legacy_show() {
    let (path, store) = create();
    store.put_object("plugin", "extension", &wall(), 0).unwrap();
    store
        .put_object(
            "patched_fixture",
            "legacy",
            &json!({"scenery_options":{"architecture":null}}),
            0,
        )
        .unwrap();
    store
        .put_object(
            "patched_fixture",
            "plugin",
            &serde_json::json!({"future_extension":wall()}),
            0,
        )
        .unwrap();
    assert_eq!(schema(&store), 10);
    drop(store);
    assert_eq!(schema(&ShowStore::open(&path).unwrap()), 10);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn future_schema_refusal_keeps_original_content_and_separate_recovery_is_available() {
    let (path, store) = create();
    store
        .conn
        .execute("UPDATE schema_info SET version=12", [])
        .unwrap();
    drop(store);
    let before = std::fs::read(&path).unwrap();
    let error = ShowStore::open(&path).err().unwrap().to_string();
    assert!(
        error.contains("show schema 12 is newer than supported schema 11"),
        "{error}"
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let (fresh_path, fresh) = create();
    assert_eq!(schema(&fresh), 10);
    drop(fresh);
    std::fs::remove_file(fresh_path).unwrap();
    std::fs::remove_file(path).unwrap();
}

#[test]
fn reader_version_preflight_preserves_sql_errors_for_malformed_or_missing_schema() {
    let (path, store) = create();
    drop(store);
    std::fs::write(&path, b"not a show").unwrap();
    let original = std::fs::read(&path).unwrap();
    assert!(matches!(
        crate::validate_show_reader_version(&path),
        Err(crate::StoreError::Sql(_))
    ));
    assert_eq!(std::fs::read(&path).unwrap(), original);
    std::fs::remove_file(path).unwrap();
    for schema in [
        "CREATE TABLE operator_data(value TEXT)",
        "CREATE TABLE schema_info(version INTEGER NOT NULL)",
        "CREATE TABLE schema_info(version TEXT); INSERT INTO schema_info VALUES ('unreadable')",
    ] {
        let (path, store) = create();
        drop(store);
        std::fs::remove_file(&path).unwrap();
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(schema).unwrap();
        drop(conn);
        let original = std::fs::read(&path).unwrap();
        assert!(
            matches!(
                crate::validate_show_reader_version(&path),
                Err(crate::StoreError::Sql(_))
            ),
            "{schema}"
        );
        // Full file validation keeps its existing actionable missing-schema error.
        assert!(
            matches!(crate::validate_show_file(&path),Err(crate::StoreError::Invalid(message)) if message.contains("schema_info is missing")),
            "{schema}"
        );
        assert_eq!(std::fs::read(&path).unwrap(), original);
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn reader_version_preflight_rejects_only_newer_versions_without_touching_the_show() {
    for version in [10, 11, 12] {
        let (path, store) = create();
        store
            .conn
            .execute("UPDATE schema_info SET version=?1", [version])
            .unwrap();
        drop(store);
        let original = std::fs::read(&path).unwrap();
        let result = crate::validate_show_reader_version(&path);
        if version > 11 {
            assert!(
                matches!(result,Err(crate::StoreError::Invalid(message)) if message.contains("show schema 12 is newer than supported schema 11"))
            );
        } else {
            result.unwrap();
        }
        assert_eq!(std::fs::read(&path).unwrap(), original);
        std::fs::remove_file(path).unwrap();
    }
}
