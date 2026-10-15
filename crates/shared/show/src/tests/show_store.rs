use super::temporary;
use crate::{AtomicObjectDelete, AtomicObjectWrite, ShowStore, StoreError, validate_show_file};
use std::fs;

#[test]
fn show_objects_enforce_optimistic_revisions() {
    let path = temporary("show");
    let (show, _) = ShowStore::create(&path, "Tour").unwrap();
    assert_eq!(
        show.put_object("preset", "one", &serde_json::json!({"value": 1}), 0)
            .unwrap(),
        1
    );
    assert!(matches!(
        show.put_object("preset", "one", &serde_json::json!({"value": 2}), 0),
        Err(StoreError::RevisionConflict {
            expected: 0,
            current: 1
        })
    ));
    assert_eq!(show.objects("preset").unwrap()[0].revision, 1);
    let _ = fs::remove_file(path);
}

#[test]
fn collection_snapshot_includes_the_matching_portable_revision() {
    let path = temporary("object-collection-snapshot");
    let (show, _) = ShowStore::create(&path, "Snapshot").unwrap();
    show.put_object("group", "front", &serde_json::json!({"fixtures": []}), 0)
        .unwrap();

    let (revision, groups) = show.objects_with_portable_revision("group").unwrap();

    assert_eq!(revision, show.portable_revision().unwrap());
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].id, "front");
    let _ = fs::remove_file(path);
}

#[test]
fn exact_object_snapshot_does_not_decode_unrequested_siblings() {
    let path = temporary("exact-object-snapshot");
    let (show, _) = ShowStore::create(&path, "Exact snapshot").unwrap();
    show.put_object("future", "wanted", &serde_json::json!({"value": 1}), 0)
        .unwrap();
    show.put_object("future", "sibling", &serde_json::json!({"value": 2}), 0)
        .unwrap();
    show.conn
        .execute(
            "UPDATE objects SET body_json=?1 WHERE kind=?2 AND id=?3",
            rusqlite::params!["not-json", "future", "sibling"],
        )
        .unwrap();

    drop(show);
    let show = ShowStore::open(&path).unwrap();
    let (revision, object) = show
        .object_with_portable_revision("future", "wanted")
        .unwrap();

    assert_eq!(revision, show.portable_revision().unwrap());
    assert_eq!(object.unwrap().body, serde_json::json!({"value": 1}));
    assert!(show.objects_with_portable_revision("future").is_err());
    let _ = fs::remove_file(path);
}

#[test]
fn missing_exact_object_snapshot_still_returns_the_current_revision() {
    let path = temporary("missing-object-snapshot");
    let (show, _) = ShowStore::create(&path, "Missing snapshot").unwrap();
    show.put_object("future", "present", &serde_json::json!({}), 0)
        .unwrap();

    let (revision, object) = show
        .object_with_portable_revision("future", "missing")
        .unwrap();

    assert_eq!(revision, show.portable_revision().unwrap());
    assert!(object.is_none());
    let _ = fs::remove_file(path);
}

#[test]
fn related_object_writes_and_deletes_roll_back_together_on_revision_conflict() {
    let path = temporary("atomic-objects");
    let (show, _) = ShowStore::create(&path, "Atomic objects").unwrap();
    let page = serde_json::json!({"number":1,"assignments":{"1":{"name":"Original"}}});
    let playback = serde_json::json!({"number":41,"name":"Original"});
    show.put_object("playback_page", "1", &page, 0).unwrap();
    show.put_object("group", "41", &playback, 0).unwrap();

    let changed_page = serde_json::json!({"number":1,"assignments":{"1":{"name":"Changed"}}});
    let changed_playback = serde_json::json!({"number":41,"name":"Changed"});
    assert!(matches!(
        show.mutate_objects_atomically(
            &[
                AtomicObjectWrite {
                    kind: "playback_page",
                    id: "1",
                    body: &changed_page,
                    expected: 1,
                },
                AtomicObjectWrite {
                    kind: "group",
                    id: "41",
                    body: &changed_playback,
                    expected: 0,
                },
            ],
            &[],
        ),
        Err(StoreError::RevisionConflict {
            expected: 0,
            current: 1
        })
    ));
    let stored_page = show.objects("playback_page").unwrap().remove(0);
    let stored_playback = show.objects("group").unwrap().remove(0);
    assert_eq!((stored_page.revision, stored_page.body), (1, page.clone()));
    assert_eq!(
        (stored_playback.revision, stored_playback.body),
        (1, playback.clone())
    );

    assert_eq!(
        show.mutate_objects_atomically(
            &[
                AtomicObjectWrite {
                    kind: "playback_page",
                    id: "1",
                    body: &changed_page,
                    expected: 1,
                },
                AtomicObjectWrite {
                    kind: "group",
                    id: "41",
                    body: &changed_playback,
                    expected: 1,
                },
            ],
            &[],
        )
        .unwrap(),
        vec![2, 2]
    );

    let cleared_page = serde_json::json!({"number":1,"assignments":{}});
    assert!(matches!(
        show.mutate_objects_atomically(
            &[AtomicObjectWrite {
                kind: "playback_page",
                id: "1",
                body: &cleared_page,
                expected: 2,
            }],
            &[AtomicObjectDelete {
                kind: "group",
                id: "41",
                expected: 1,
            }],
        ),
        Err(StoreError::RevisionConflict {
            expected: 1,
            current: 2
        })
    ));
    assert_eq!(
        show.objects("playback_page").unwrap().remove(0).body,
        changed_page
    );
    assert_eq!(show.objects("group").unwrap().len(), 1);

    show.mutate_objects_atomically(
        &[AtomicObjectWrite {
            kind: "playback_page",
            id: "1",
            body: &cleared_page,
            expected: 2,
        }],
        &[AtomicObjectDelete {
            kind: "group",
            id: "41",
            expected: 2,
        }],
    )
    .unwrap();
    assert_eq!(
        show.objects("playback_page").unwrap().remove(0).body,
        cleared_page
    );
    assert!(show.objects("group").unwrap().is_empty());
    let _ = fs::remove_file(path);
}

#[test]
fn group_membership_edits_are_undoable_with_revision_protection() {
    let path = temporary("group-undo");
    let (show, _) = ShowStore::create(&path, "Template").unwrap();
    assert_eq!(
        show.put_object("group", "front", &serde_json::json!({"fixtures":[]}), 0)
            .unwrap(),
        1
    );
    assert_eq!(
        show.put_object(
            "group",
            "front",
            &serde_json::json!({"fixtures":["fixture-a"]}),
            1
        )
        .unwrap(),
        2
    );
    assert_eq!(show.undo_object("group", "front", 2).unwrap(), 3);
    let group = &show.objects("group").unwrap()[0];
    assert_eq!(group.body["fixtures"], serde_json::json!([]));
    assert!(matches!(
        show.undo_object("group", "front", 2),
        Err(StoreError::RevisionConflict { current: 3, .. })
    ));
    let _ = fs::remove_file(path);
}

#[test]
fn backup_is_a_standalone_valid_show() {
    let source = temporary("source");
    let backup = temporary("backup");
    let (show, id) = ShowStore::create(&source, "Portable").unwrap();
    show.put_object("group", "front", &serde_json::json!([1, 2, 3]), 0)
        .unwrap();
    show.backup_to(&backup).unwrap();
    assert_eq!(
        validate_show_file(&backup).unwrap(),
        (id, "Portable".into())
    );
    let reopened = ShowStore::open(&backup).unwrap();
    assert_eq!(reopened.objects("group").unwrap().len(), 1);
    let _ = fs::remove_file(source);
    let _ = fs::remove_file(backup);
}

#[test]
fn concurrent_readers_do_not_rerun_show_migrations() {
    let path = temporary("concurrent");
    let (show, _) = ShowStore::create(&path, "Concurrent").unwrap();
    show.put_object("group", "front", &serde_json::json!({"fixtures":[]}), 0)
        .unwrap();
    drop(show);
    let handles = (0..8)
        .map(|_| {
            let path = path.clone();
            std::thread::spawn(move || ShowStore::open(path).unwrap().objects("group").unwrap())
        })
        .collect::<Vec<_>>();
    for handle in handles {
        assert_eq!(handle.join().unwrap().len(), 1);
    }
    let _ = fs::remove_file(path);
}

#[test]
fn observed_open_preserves_current_and_migrated_documents_and_reports_ordered_phases() {
    use crate::ShowStoreOpenPhase::*;
    for version in [9, 10] {
        let path = temporary(&format!("observed-open-{version}"));
        let ordinary_path = temporary(&format!("ordinary-open-{version}"));
        let (show, original_id) = ShowStore::create(&path, "Observed").unwrap();
        show.put_object("group", "front", &serde_json::json!({"fixtures": []}), 0)
            .unwrap();
        show.conn
            .execute("UPDATE schema_info SET version=?1", [version])
            .unwrap();
        show.conn
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
            .unwrap();
        drop(show);
        fs::copy(&path, &ordinary_path).unwrap();
        let before = fs::read(&path).unwrap();
        let mut phases = Vec::new();
        let observed = ShowStore::open_observed(&path, |phase| phases.push(phase)).unwrap();
        let ordinary = ShowStore::open(&ordinary_path).unwrap();
        assert_eq!(
            observed.portable_document().unwrap(),
            ordinary.portable_document().unwrap()
        );
        assert_eq!(observed.id().unwrap(), original_id);
        assert_eq!(observed.name().unwrap(), "Observed");
        assert_eq!(
            phases,
            vec![
                PlaybackContract,
                ConnectionOpen,
                ConnectionConfigure,
                SchemaVersion,
                Migration,
                Integrity,
                SchemaValidation,
                Identity
            ]
        );
        assert_eq!(
            observed
                .conn
                .query_row("SELECT version FROM schema_info", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            10
        );
        drop((observed, ordinary));
        if version == 10 {
            assert_eq!(fs::read(&path).unwrap(), before);
            assert_eq!(fs::read(&ordinary_path).unwrap(), before);
        }
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(ordinary_path);
    }
}

#[test]
fn observed_open_retired_history_refuses_before_callbacks_or_writable_changes() {
    for table in ["objects", "object_history", "object_redo"] {
        let path = temporary(&format!("observed-retired-{table}"));
        let (show, _) = ShowStore::create(&path, "Refusal").unwrap();
        let time_column = if table == "objects" {
            "updated_at"
        } else {
            "created_at"
        };
        show.conn.execute(&format!("INSERT INTO {table}(kind,id,revision,body_json,{time_column}) VALUES('playback_page','1',1,?1,'test')"),
            [serde_json::json!({"number":1,"slots":{"1":7}}).to_string()]).unwrap();
        show.conn
            .execute("UPDATE schema_info SET version=9", [])
            .unwrap();
        show.conn
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
            .unwrap();
        drop(show);
        let before = fs::read(&path).unwrap();
        let ordinary_error = ShowStore::open(&path).err().unwrap().to_string();
        let mut phases = Vec::new();
        let observed_error = ShowStore::open_observed(&path, |phase| phases.push(phase))
            .err()
            .unwrap()
            .to_string();
        assert_eq!(observed_error, ordinary_error);
        assert!(observed_error.contains("before TL-714"));
        assert!(phases.is_empty());
        assert_eq!(fs::read(&path).unwrap(), before);
        let _ = fs::remove_file(path);
    }
}

#[test]
fn observed_open_malformed_file_preserves_error_and_emits_no_success() {
    let path = temporary("observed-malformed");
    fs::write(&path, b"not a SQLite show").unwrap();
    let before = fs::read(&path).unwrap();
    let ordinary_error = ShowStore::open(&path).err().unwrap().to_string();
    let mut phases = Vec::new();
    let observed_error = ShowStore::open_observed(&path, |phase| phases.push(phase))
        .err()
        .unwrap()
        .to_string();
    assert_eq!(observed_error, ordinary_error);
    assert!(phases.is_empty());
    assert_eq!(fs::read(&path).unwrap(), before);
    let _ = fs::remove_file(path);
}

#[test]
fn observed_open_stops_at_newer_schema_or_missing_identity_failure() {
    use crate::ShowStoreOpenPhase::*;
    for newer_schema in [false, true] {
        let path = temporary(&format!("observed-stage-failure-{newer_schema}"));
        let (show, _) = ShowStore::create(&path, "Stage boundary").unwrap();
        if newer_schema {
            show.conn
                .execute("UPDATE schema_info SET version=999", [])
                .unwrap();
        } else {
            show.conn
                .execute("DELETE FROM metadata WHERE key='show_id'", [])
                .unwrap();
        }
        show.conn
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
            .unwrap();
        drop(show);
        let ordinary_error = ShowStore::open(&path).err().unwrap().to_string();
        let mut phases = Vec::new();
        let observed_error = ShowStore::open_observed(&path, |phase| phases.push(phase))
            .err()
            .unwrap()
            .to_string();
        assert_eq!(observed_error, ordinary_error);
        let mut expected = vec![
            PlaybackContract,
            ConnectionOpen,
            ConnectionConfigure,
            SchemaVersion,
            Migration,
            Integrity,
        ];
        if !newer_schema {
            expected.push(SchemaValidation);
        }
        assert_eq!(phases, expected);
        assert!(observed_error.contains(if newer_schema {
            "newer than supported"
        } else {
            "missing show_id"
        }));
        let _ = fs::remove_file(path);
    }
}
