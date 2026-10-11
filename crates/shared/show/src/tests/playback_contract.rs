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

#[test]
fn retired_group_controls_and_dynamic_scopes_refuse_files_and_history_without_writes() {
    let targetless = json!({"type":"dynamic","assignment":{"target_scope":null,"dynamic":{
        "embedded_fallback":{"definition":{"target_binding":{"type":"targetless"}}}
    }}});
    let scoped = json!({"type":"dynamic","assignment":{"target_scope":{"type":"live_group","group_id":"1"},
        "dynamic":{"embedded_fallback":{"definition":{"target_binding":{"type":"live_group","group_id":"1"}}}}
    }});
    let cases = [
        ("group", json!({"name":"Front","fixtures":[],"master":1})),
        (
            "group",
            json!({"name":"Front","fixtures":[],"playback_fader":1}),
        ),
        (
            "playback_page",
            json!({"number":1,"assignments":{"1":{"target":targetless}}}),
        ),
        (
            "playback_page",
            json!({"number":1,"assignments":{"1":{"target":scoped}}}),
        ),
    ];
    for table in ["objects", "object_history", "object_redo"] {
        for (kind, body) in &cases {
            let path = temporary("retired-assignment-control");
            let (store, _) = ShowStore::create(&path, "Original").unwrap();
            let time_column = if table == "objects" {
                "updated_at"
            } else {
                "created_at"
            };
            store.conn.execute(&format!("INSERT INTO {table}(kind,id,revision,body_json,{time_column}) VALUES(?1,'1',1,?2,'test')"),
                [*kind, &body.to_string()]).unwrap();
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
            assert!(error.contains("before TL-714"), "{error}");
            assert_eq!(fs::read(&path).unwrap(), original);
            let _ = fs::remove_file(path);
        }
    }
}

#[test]
fn bound_dynamic_assignments_and_targetless_pool_templates_remain_valid() {
    let binding = json!({"type":"live_group","group_id":"1"});
    let definition = json!({"target_binding":binding});
    assert!(
        validate_playback_object(
            "dynamic",
            "1",
            &json!({"target_binding":{"type":"targetless"}})
        )
        .is_ok()
    );
    assert!(validate_playback_object("playback_page", "1", &json!({"number":1,"assignments":{"1":{"target":{
        "type":"dynamic","assignment":{"target_scope":null,"dynamic":{"embedded_fallback":{"definition":definition}}}
    }}}})).is_ok());
    let path = temporary("retired-control-writer");
    let (store, _) = ShowStore::create(&path, "Writer").unwrap();
    let document = store.portable_document().unwrap();
    let mut transaction = document.transaction();
    transaction.put("group", "1", json!({"master":0.5}));
    assert!(store.apply_portable_transaction(transaction).is_err());
    assert_eq!(
        store.portable_document().unwrap().revision(),
        document.revision()
    );
    assert!(store.objects("group").unwrap().is_empty());
    drop(store);
    let _ = fs::remove_file(path);
}

mod compact_rig_generation;

#[test]
fn speed_group_legacy_fader_is_refused_before_writable_open_including_history() {
    let body = json!({"number":4,"assignments":{"305":{
        "name":"Speed A","target":{"type":"speed_group","group":"A"},"fader":"speed"
    }}});
    assert!(validate_playback_object("playback_page", "4", &body).is_err());
    for table in ["objects", "object_history", "object_redo"] {
        let path = temporary("retired-speed-group-fader");
        let (store, _) = ShowStore::create(&path, "Original").unwrap();
        let time_column = if table == "objects" {
            "updated_at"
        } else {
            "created_at"
        };
        store.conn.execute(&format!("INSERT INTO {table}(kind,id,revision,body_json,{time_column}) VALUES('playback_page','4',1,?1,'test')"),
            [body.to_string()]).unwrap();
        store
            .conn
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
            .unwrap();
        drop(store);
        let original = fs::read(&path).unwrap();
        let error = ShowStore::open(&path).err().unwrap().to_string();
        assert!(error.contains("learned_percentage"));
        assert!(error.contains("Nothing was changed"));
        assert_eq!(fs::read(&path).unwrap(), original);
        let _ = fs::remove_file(path);
    }
    for fader in ["learned_percentage", "direct_bpm", "centered_relative"] {
        let mut canonical = body.clone();
        canonical["assignments"]["305"]["fader"] = json!(fader);
        validate_playback_object("playback_page", "4", &canonical).unwrap();
    }
}
