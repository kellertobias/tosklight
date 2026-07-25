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
    let page = serde_json::json!({"number":1,"slots":{"1":41}});
    let playback = serde_json::json!({"number":41,"name":"Original"});
    show.put_object("playback_page", "1", &page, 0).unwrap();
    show.put_object("playback", "41", &playback, 0).unwrap();

    let changed_page = serde_json::json!({"number":1,"slots":{"1":42}});
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
                    kind: "playback",
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
    let stored_playback = show.objects("playback").unwrap().remove(0);
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
                    kind: "playback",
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

    let cleared_page = serde_json::json!({"number":1,"slots":{}});
    assert!(matches!(
        show.mutate_objects_atomically(
            &[AtomicObjectWrite {
                kind: "playback_page",
                id: "1",
                body: &cleared_page,
                expected: 2,
            }],
            &[AtomicObjectDelete {
                kind: "playback",
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
    assert_eq!(show.objects("playback").unwrap().len(), 1);

    show.mutate_objects_atomically(
        &[AtomicObjectWrite {
            kind: "playback_page",
            id: "1",
            body: &cleared_page,
            expected: 2,
        }],
        &[AtomicObjectDelete {
            kind: "playback",
            id: "41",
            expected: 2,
        }],
    )
    .unwrap();
    assert_eq!(
        show.objects("playback_page").unwrap().remove(0).body,
        cleared_page
    );
    assert!(show.objects("playback").unwrap().is_empty());
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
