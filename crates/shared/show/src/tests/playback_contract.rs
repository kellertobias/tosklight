use super::temporary;
use crate::{
    ShowStore, validate_playback_object, validate_playback_runtime, validate_show_playback_contract,
};
use serde_json::json;
use std::fs;

#[test]
fn old_playback_show_is_refused_before_migration_without_changing_original_bytes() {
    let path = temporary("old-playback-format");
    let (store, _) = ShowStore::create(&path, "Old Playback show").unwrap();
    store.conn.execute("INSERT INTO objects(kind,id,revision,body_json,updated_at) VALUES('playback','7',1,?1,'test')",
        [json!({"number":7,"target":{"type":"group","group_id":"1"}}).to_string()]).unwrap();
    store
        .conn
        .execute("UPDATE schema_info SET version=9", [])
        .unwrap();
    store
        .conn
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
        .unwrap();
    drop(store);
    let original = fs::read(&path).unwrap();
    let error = ShowStore::open(&path).err().unwrap().to_string();
    assert!(error.contains("before TL-714"));
    assert!(error.contains("separate empty show"));
    assert_eq!(fs::read(&path).unwrap(), original);
    let _ = fs::remove_file(path);
}

#[test]
fn old_nested_maps_and_redundant_definition_numbers_cannot_be_written_or_restored() {
    for body in [
        json!({"number":1,"slots":{},"virtual_playbacks":{}}),
        json!({"number":1,"assignments":{"1":{"number":7}}}),
        json!({"number":1,"assignments":{},"virtual_playbacks":{}}),
    ] {
        assert!(validate_playback_object("playback_page", "1", &body).is_err());
    }
    assert!(
        validate_playback_object("virtual_playback_exclusion_zones", "show", &json!({})).is_err()
    );
    assert!(
        validate_playback_object(
            "playback_page",
            "99",
            &json!({"number":99,"assignments":{"1000":{"name":"Last"}}})
        )
        .is_ok()
    );
}

#[test]
fn legacy_runtime_identities_are_rejected_and_canonical_addresses_are_accepted() {
    for body in [
        json!([{"playback_number":null}]),
        json!({"dynamic_playbacks":[{"playback_identity":{"kind":"virtual","page":4,"number":1904}}]}),
    ] {
        assert!(validate_playback_runtime(&body).is_err());
    }
    assert!(
        validate_playback_runtime(
            &json!({"dynamic_playbacks":[{"address":{"page":99,"position":1000}}]})
        )
        .is_ok()
    );
}

#[test]
fn fixture_only_repository_shows_have_no_retired_playback_content() {
    for name in ["compact-rig", "default-stage"] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../../tests/fixtures/{name}.show"));
        let original = fs::read(&path).unwrap();
        validate_show_playback_contract(&path).unwrap();
        assert_eq!(fs::read(path).unwrap(), original);
    }
}

#[test]
fn unified_transaction_stamps_contract_and_cannot_reintroduce_retired_bodies() {
    let path = temporary("unified-playback-writer");
    let (store, _) = ShowStore::create(&path, "Unified").unwrap();
    let document = store.portable_document().unwrap();
    let mut transaction = document.transaction();
    transaction.put("playback_page", "1", json!({"number":1,"assignments":{}}));
    store.apply_portable_transaction(transaction).unwrap();
    assert_eq!(
        store
            .metadata_value(crate::PLAYBACK_CONTRACT_METADATA_KEY)
            .unwrap()
            .as_deref(),
        Some("1")
    );
    let document = store.portable_document().unwrap();
    let revision = document.revision();
    let mut bad = document.transaction();
    bad.put("playback", "7", json!({"number":7}));
    assert!(store.apply_portable_transaction(bad).is_err());
    assert_eq!(store.portable_document().unwrap().revision(), revision);
    drop(store);
    let _ = fs::remove_file(path);
}

#[test]
fn retired_undo_and_redo_bodies_refuse_reopen_before_any_write() {
    for table in ["object_history", "object_redo"] {
        let path = temporary(&format!("retired-{table}"));
        let (store, _) = ShowStore::create(&path, "History").unwrap();
        store.conn.execute(&format!("INSERT INTO {table}(kind,id,revision,body_json,created_at) VALUES('playback_page','1',1,?1,'test')"),[json!({"number":1,"slots":{"1":7}}).to_string()]).unwrap();
        store
            .conn
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
            .unwrap();
        drop(store);
        let original = fs::read(&path).unwrap();
        assert!(ShowStore::open(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), original);
        let _ = fs::remove_file(path);
    }
}
