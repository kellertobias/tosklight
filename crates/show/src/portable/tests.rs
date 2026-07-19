use super::{
    PortableShowDocument, PortableShowRevision, PortableShowTransaction,
    store::REVISION_METADATA_KEY,
};
use crate::{AtomicObjectDelete, AtomicObjectWrite, ShowStore, StoreError};
use serde_json::{Value, json};
use std::{fs, path::PathBuf};
use uuid::Uuid;

fn temporary(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("light-portable-{name}-{}.sqlite", Uuid::new_v4()))
}

fn create(name: &str) -> (PathBuf, ShowStore) {
    let path = temporary(name);
    let (store, _) = ShowStore::create(&path, "Portable test").unwrap();
    (path, store)
}

fn seed_unknown_objects(show: &ShowStore) -> (Value, Value) {
    show.conn
        .execute(
            "INSERT INTO metadata(key,value) VALUES ('future.editor','v9')",
            [],
        )
        .unwrap();
    let group = json!({"name":"Front","future":{"ordered":[3,1,2]}});
    let extension = json!({"plugin":"future","payload":{"flag":true}});
    show.put_object("group", "1", &group, 0).unwrap();
    show.put_object("future_macro", "alpha", &extension, 0)
        .unwrap();
    (group, extension)
}

#[test]
fn document_loads_unknown_metadata_objects_and_fields() {
    let (path, show) = create("unknown-load");
    let (group, extension) = seed_unknown_objects(&show);
    let document = show.portable_document().unwrap();
    assert_eq!(document.revision(), PortableShowRevision::new(2));
    assert_eq!(document.metadata()["future.editor"], "v9");
    assert_eq!(document.objects().len(), 2);
    assert_eq!(document.object("group", "1").unwrap().body(), &group);
    assert_eq!(
        document.object("future_macro", "alpha").unwrap().body(),
        &extension
    );
    drop(show);
    let _ = fs::remove_file(path);
}

fn edit_fixture(document: &mut PortableShowDocument) -> PortableShowTransaction {
    let mut transaction = document.transaction();
    let fixture = document.object_mut("fixture", "one").unwrap();
    fixture.body_mut()["name"] = json!("Updated");
    fixture.body_mut()["definition"]["owned"] = json!(2);
    transaction.put_object(fixture);
    transaction
}

fn assert_unknown_fixture_data(stored: &Value, original: &Value) {
    assert_eq!(stored["name"], "Updated");
    assert_eq!(stored["definition"]["owned"], 2);
    assert_eq!(
        stored["definition"]["future"],
        original["definition"]["future"]
    );
    assert_eq!(stored["plugin_data"], original["plugin_data"]);
}

#[test]
fn raw_document_edit_preserves_unowned_nested_fields() {
    let (path, show) = create("lossless-edit");
    let body = json!({
        "name":"Original",
        "definition":{"owned":1,"future":{"curve":[0.1,0.4,1.0]}},
        "plugin_data":{"vendor":"Acme","enabled":true}
    });
    show.put_object("fixture", "one", &body, 0).unwrap();
    let mut document = show.portable_document().unwrap();
    let committed = show
        .apply_portable_transaction(edit_fixture(&mut document))
        .unwrap();
    let stored = committed.written_object("fixture", "one").unwrap();
    assert_unknown_fixture_data(stored.body(), &body);
    drop(show);
    let _ = fs::remove_file(path);
}

fn stale_group_transaction(show: &ShowStore) -> PortableShowTransaction {
    show.put_object("group", "1", &json!({"name":"One"}), 0)
        .unwrap();
    show.put_object("group", "2", &json!({"name":"Two"}), 0)
        .unwrap();
    let document = show.portable_document().unwrap();
    let mut transaction = document.transaction();
    transaction.put("group", "1", json!({"name":"Candidate one"}));
    transaction.put("group", "2", json!({"name":"Candidate two"}));
    show.put_object("group", "1", &json!({"name":"Concurrent"}), 1)
        .unwrap();
    transaction
}

#[test]
fn stale_document_revision_rolls_back_every_change() {
    let (path, show) = create("stale-transaction");
    let transaction = stale_group_transaction(&show);
    assert!(matches!(
        show.apply_portable_transaction(transaction),
        Err(StoreError::DocumentRevisionConflict {
            expected,
            current
        }) if expected.value() == 2 && current.value() == 3
    ));
    assert_eq!(show.objects("group").unwrap()[0].body["name"], "Concurrent");
    assert_eq!(show.objects("group").unwrap()[1].body["name"], "Two");
    drop(show);
    let _ = fs::remove_file(path);
}

fn change_group_atomically(show: &ShowStore) {
    let changed = json!({"members":["fixture"]});
    show.mutate_objects_atomically(
        &[AtomicObjectWrite {
            kind: "group",
            id: "one",
            body: &changed,
            expected: 1,
        }],
        &[AtomicObjectDelete {
            kind: "missing",
            id: "none",
            expected: 0,
        }],
    )
    .unwrap();
}

#[test]
fn multi_object_transaction_advances_show_revision_once() {
    let (path, show) = create("single-revision");
    show.put_object("preset", "a", &json!({"value":1}), 0)
        .unwrap();
    show.put_object("preset", "b", &json!({"value":2}), 0)
        .unwrap();
    let document = show.portable_document().unwrap();
    let mut transaction = document.transaction();
    transaction.put("preset", "a", json!({"value":10}));
    transaction.put("future", "c", json!({"value":3,"unknown":true}));
    transaction.delete("preset", "b");

    let committed = show.apply_portable_transaction(transaction).unwrap();
    assert_eq!(committed.revision().value(), 3);
    assert_eq!(committed.written_objects().len(), 2);
    assert_eq!(
        committed.written_object("preset", "a").unwrap().revision(),
        2
    );
    assert_eq!(
        committed.written_object("future", "c").unwrap().revision(),
        1
    );
    assert_eq!(committed.deleted_objects()[0].id(), "b");
    drop(show);
    let _ = fs::remove_file(path);
}

#[test]
fn legacy_mutations_keep_the_document_revision_current() {
    let (path, show) = create("legacy-revisions");
    show.conn
        .execute("DELETE FROM metadata WHERE key=?1", [REVISION_METADATA_KEY])
        .unwrap();
    assert_eq!(show.portable_revision().unwrap().value(), 0);
    show.put_object("group", "one", &json!({"members":[]}), 0)
        .unwrap();
    assert_eq!(show.portable_revision().unwrap().value(), 1);
    change_group_atomically(&show);
    assert_eq!(show.portable_revision().unwrap().value(), 2);
    show.undo_object("group", "one", 2).unwrap();
    assert_eq!(show.portable_revision().unwrap().value(), 3);
    assert!(show.delete_object("group", "one").unwrap());
    assert_eq!(show.portable_revision().unwrap().value(), 4);
    drop(show);
    let _ = fs::remove_file(path);
}

#[test]
fn prepared_undo_is_visible_to_compilation_and_pops_history_atomically() {
    let (path, show) = create("portable-undo");
    let original = json!({
        "name":"Original",
        "future":{"nested":[3, 1, 2]}
    });
    show.put_object("group", "one", &original, 0).unwrap();
    show.put_object(
        "group",
        "one",
        &json!({"name":"Changed","future":{"nested":[9]}}),
        1,
    )
    .unwrap();
    let document = show.portable_document().unwrap();
    let undo = show.prepare_object_undo("group", "one", 2).unwrap();
    assert_eq!(undo.body(), &original);
    let mut transaction = document.transaction();
    transaction.undo_object(undo);
    let candidate = document.candidate(&transaction).unwrap();
    assert_eq!(candidate.object("group", "one").unwrap().body(), &original);
    assert_eq!(candidate.object_revision("group", "one"), Some(3));

    let committed = show.apply_portable_transaction(transaction).unwrap();

    assert_eq!(
        committed.revision().value(),
        document.revision().value() + 1
    );
    let restored = committed.written_object("group", "one").unwrap();
    assert_eq!(restored.revision(), 3);
    assert_eq!(restored.body(), &original);
    assert!(matches!(
        show.prepare_object_undo("group", "one", 3),
        Err(StoreError::Invalid(message)) if message == "object has no undo history"
    ));
    drop(show);
    let _ = fs::remove_file(path);
}

#[test]
fn stale_prepared_undo_preserves_current_body_and_history() {
    let (path, show) = create("stale-portable-undo");
    show.put_object("group", "one", &json!({"name":"Original"}), 0)
        .unwrap();
    show.put_object("group", "one", &json!({"name":"Changed"}), 1)
        .unwrap();
    let document = show.portable_document().unwrap();
    let mut transaction = document.transaction();
    transaction.undo_object(show.prepare_object_undo("group", "one", 2).unwrap());
    show.put_object("group", "one", &json!({"name":"Concurrent"}), 2)
        .unwrap();

    assert!(matches!(
        show.apply_portable_transaction(transaction),
        Err(StoreError::DocumentRevisionConflict { expected, current })
            if expected.value() == 2 && current.value() == 3
    ));
    assert_eq!(show.objects("group").unwrap()[0].body["name"], "Concurrent");
    show.undo_object("group", "one", 3).unwrap();
    assert_eq!(show.objects("group").unwrap()[0].body["name"], "Changed");
    drop(show);
    let _ = fs::remove_file(path);
}

#[test]
fn undo_and_related_write_share_one_portable_transaction() {
    let (path, show) = create("batched-portable-undo");
    show.put_object("group", "one", &json!({"name":"Original"}), 0)
        .unwrap();
    show.put_object("group", "one", &json!({"name":"Changed"}), 1)
        .unwrap();
    let document = show.portable_document().unwrap();
    let mut transaction = document.transaction();
    transaction
        .undo_object(show.prepare_object_undo("group", "one", 2).unwrap())
        .put("future_extension", "linked", json!({"opaque":true}));

    let committed = show.apply_portable_transaction(transaction).unwrap();

    assert_eq!(
        committed.revision().value(),
        document.revision().value() + 1
    );
    assert_eq!(committed.written_objects().len(), 2);
    assert_eq!(
        committed.written_object("group", "one").unwrap().body()["name"],
        "Original"
    );
    assert_eq!(
        committed
            .written_object("future_extension", "linked")
            .unwrap()
            .body()["opaque"],
        true
    );
    drop(show);
    let _ = fs::remove_file(path);
}

fn seed_unknown_backup(show: &ShowStore) -> Value {
    let body = json!({"format":99,"opaque":{"bytes":[1,2,3]}});
    show.put_object("future_extension", "opaque", &body, 0)
        .unwrap();
    show.conn
        .execute(
            "INSERT INTO metadata(key,value) VALUES ('future.backup','retain')",
            [],
        )
        .unwrap();
    body
}

#[test]
fn backup_keeps_unknown_portable_data() {
    let (source, show) = create("backup-source");
    let backup = temporary("backup-destination");
    let body = seed_unknown_backup(&show);
    show.backup_to(&backup).unwrap();

    let copied = ShowStore::open(&backup)
        .unwrap()
        .portable_document()
        .unwrap();
    assert_eq!(copied.metadata()["future.backup"], "retain");
    assert_eq!(
        copied.object("future_extension", "opaque").unwrap().body(),
        &body
    );
    drop(show);
    let _ = fs::remove_file(source);
    let _ = fs::remove_file(backup);
}
