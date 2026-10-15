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

fn original_serialized_contract(kind: &str, raw: &str) -> Result<(), String> {
    match serde_json::from_str(raw) {
        Ok(body) => {
            validate_playback_object(kind, "test", &body).map_err(|error| error.to_string())
        }
        Err(error)
            if matches!(
                kind,
                "playback"
                    | "playback_page"
                    | "playback_solo_regions"
                    | "virtual_playback_exclusion_zones"
            ) =>
        {
            Err(error.to_string())
        }
        Err(_) => Ok(()),
    }
}

#[test]
fn lexical_contract_guard_matches_original_for_escaped_nested_duplicate_and_malformed_json() {
    use crate::playback_contract::validate_serialized_playback_object;
    let mut bodies = vec![
        r#"{}"#.to_owned(), r#"[]"#.to_owned(), "null".to_owned(), "1e999".to_owned(),
        r#"{"name":"Harmless Spot","position":{"x":1,"y":2}}"#.to_owned(),
        r#"{"note":"physical virtual playback dynamic words only"}"#.to_owned(),
        r#""playback_number""#.to_owned(),
        r#"{"note":"quoted \"physical\" and \u0076irtual"}"#.to_owned(),
        r#"{"deep":[{"play\u0062ack_number":7}]}"#.to_owned(),
        r#"{"deep":{"\u006bind":"phys\u0069cal"}}"#.to_owned(),
        r#"{"deep":{"t\u0079pe":"dyn\u0061mic","assignment":{"target_sc\u006fpe":{"type":"live_group","group_id":"1"}}}}"#.to_owned(),
        r#"{"deep":{"type":"dynamic","assignment":{"dynamic":{"embedded_fallback":{"definition":{"target_binding":{"type":"targetless"}}}}}}}"#.to_owned(),
        r#"{"kind":"physical","kind":"valid"}"#.to_owned(),
        r#"{"kind":"valid","kind":"physical"}"#.to_owned(),
        r#"{"x":{"kind":"physical"},"x":{}}"#.to_owned(),
        r#"{"k\u0069nd":"physical","kind":"valid"}"#.to_owned(),
        r#"{"deep":[{"kind":"virtual"}]}"#.to_owned(),
        r#"{"master":1,"playback_fader":1}"#.to_owned(),
        r#"{"number":1,"assignments":{"1":{"target":{"type":"speed_group"},"fader":"speed"}}}"#.to_owned(),
        r#"{"kind":"physical""#.to_owned(), r#"{"ordinary": "#.to_owned(),
        r#"{"number":1,"assignments":{}}"#.to_owned(),
        r#"{"note":"\uD83D\uDE00"}"#.to_owned(),
    ];
    for key in [
        "playback_number",
        "playback_identity",
        "virtual_playbacks",
        "virtual_playback_number",
        "virtualPlaybackRows",
        "virtualPlaybackColumns",
        "virtualPlaybackPageMode",
        "virtualPlaybackPinnedPage",
    ] {
        bodies.push(json!({"nested":[{(key): 1}]}).to_string());
        assert!(original_serialized_contract("future_extension", bodies.last().unwrap()).is_err());
    }
    for kind in [
        "patched_fixture",
        "future_extension",
        "macro",
        "group",
        "playback",
        "playback_page",
        "playback_solo_regions",
        "virtual_playback_exclusion_zones",
    ] {
        for raw in &bodies {
            let actual = validate_serialized_playback_object(kind, "test", raw, |text| {
                serde_json::from_str(text)
            })
            .map_err(|error| error.to_string());
            assert_eq!(
                actual,
                original_serialized_contract(kind, raw),
                "kind={kind} raw={raw}"
            );
        }
    }
}

#[test]
fn lexical_contract_guard_removes_unrelated_history_parsing_but_keeps_candidates_on_original_parser()
 {
    use crate::playback_contract::validate_serialized_playback_object;
    use std::cell::Cell;
    let calls = Cell::new(0);
    let raw = r#"{"fixture_number":1,"name":"Spot","profile_revision":7,"location":{"x":0,"y":0,"z":3000}}"#;
    for _ in 0..512 {
        // The retained original implementation parses every row; the result must agree.
        assert_eq!(original_serialized_contract("patched_fixture", raw), Ok(()));
        validate_serialized_playback_object("patched_fixture", "test", raw, |text| {
            calls.set(calls.get() + 1);
            serde_json::from_str(text)
        })
        .unwrap();
    }
    assert_eq!(calls.get(), 0);
    for (kind, raw) in [
        ("patched_fixture", r#"{"note":"dynamic"}"#),
        ("patched_fixture", r#"{"name":"Sp\u006ft"}"#),
        ("group", "{}"),
        ("playback", "{}"),
        ("playback_page", "{}"),
        ("playback_solo_regions", "{}"),
        ("virtual_playback_exclusion_zones", "{}"),
    ] {
        let before = calls.get();
        let actual = validate_serialized_playback_object(kind, "test", raw, |text| {
            calls.set(calls.get() + 1);
            serde_json::from_str(text)
        })
        .map_err(|error| error.to_string());
        assert_eq!(calls.get(), before + 1);
        assert_eq!(actual, original_serialized_contract(kind, raw));
    }
}

#[test]
fn lexical_guard_still_refuses_escaped_retired_content_in_every_table_before_schema_writes() {
    let cases = [
        (
            "patched_fixture",
            r#"{"deep":[{"play\u0062ack_number":7}]}"#,
        ),
        (
            "future_extension",
            r#"{"deep":{"\u006bind":"phys\u0069cal"}}"#,
        ),
        (
            "macro",
            r#"{"deep":{"t\u0079pe":"dyn\u0061mic","assignment":{"target_sc\u006fpe":{"type":"live_group","group_id":"1"}}}}"#,
        ),
        ("group", r#"{"master":1}"#),
        ("playback_page", r#"{"number":1,"slots":{"1":7}}"#),
        ("playback", r#"{"ordinary": "#),
        ("playback_page", r#"{"ordinary": "#),
        ("playback_solo_regions", r#"{"ordinary": "#),
        ("virtual_playback_exclusion_zones", r#"{"ordinary": "#),
    ];
    for version in [9, 10] {
        for table in ["objects", "object_history", "object_redo"] {
            for (kind, raw) in cases {
                let path = temporary(&format!("lexical-retired-{version}-{table}-{kind}"));
                let (store, _) = ShowStore::create(&path, "Original").unwrap();
                assert_eq!(
                    store
                        .metadata_value(crate::PLAYBACK_CONTRACT_METADATA_KEY)
                        .unwrap()
                        .as_deref(),
                    Some("1")
                );
                let time_column = if table == "objects" {
                    "updated_at"
                } else {
                    "created_at"
                };
                store.conn.execute(&format!("INSERT INTO {table}(kind,id,revision,body_json,{time_column}) VALUES(?1,'test',1,?2,'test')"), [kind, raw]).unwrap();
                store
                    .conn
                    .execute("UPDATE schema_info SET version=?1", [version])
                    .unwrap();
                store
                    .conn
                    .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
                    .unwrap();
                drop(store);
                let before = fs::read(&path).unwrap();
                let expected = original_serialized_contract(kind, raw).unwrap_err();
                let observed = ShowStore::open(&path).err().unwrap().to_string();
                assert_eq!(observed, expected);
                assert_eq!(fs::read(&path).unwrap(), before);
                let _ = fs::remove_file(path);
            }
        }
    }
}

#[test]
fn lexical_guard_preserves_duplicate_last_wins_and_unrelated_malformed_history_policy() {
    for table in ["objects", "object_history", "object_redo"] {
        for raw in [
            r#"{"kind":"physical","kind":"valid"}"#,
            r#"{"x":{"kind":"physical"},"x":{}}"#,
            r#"{"kind":"physical""#,
            r#"{"ordinary": "#,
        ] {
            let path = temporary(&format!("lexical-accepted-{table}"));
            let (store, _) = ShowStore::create(&path, "Unrelated reader").unwrap();
            let time_column = if table == "objects" {
                "updated_at"
            } else {
                "created_at"
            };
            store.conn.execute(&format!("INSERT INTO {table}(kind,id,revision,body_json,{time_column}) VALUES('future_extension','test',1,?1,'test')"), [raw]).unwrap();
            store
                .conn
                .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
                .unwrap();
            drop(store);
            assert!(original_serialized_contract("future_extension", raw).is_ok());
            assert!(validate_show_playback_contract(&path).is_ok());
            assert!(ShowStore::open(&path).is_ok());
            let _ = fs::remove_file(path);
        }
    }
}
