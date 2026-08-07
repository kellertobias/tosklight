use super::temporary;
use crate::{CueThumbnail, ShowStore, StoreError};
use chrono::{TimeZone, Utc};
use std::fs;

fn thumbnail(cue_id: &str, hash: &str) -> CueThumbnail {
    CueThumbnail {
        cue_id: cue_id.into(),
        image: vec![0x52, 0x49, 0x46, 0x46, 0x01, 0x02, 0x03, 0x04],
        state_hash: hash.into(),
        width: 240,
        height: 135,
        updated_at: Utc.with_ymd_and_hms(2026, 8, 7, 12, 0, 0).unwrap(),
    }
}

#[test]
fn stored_previews_round_trip_and_replace_by_cue() {
    let path = temporary("cue-thumbnail");
    let (show, _) = ShowStore::create(&path, "Tour").unwrap();

    show.put_cue_thumbnails(&[thumbnail("cue-a", "hash-1"), thumbnail("cue-b", "hash-1")])
        .unwrap();
    let stored = show.cue_thumbnail("cue-a").unwrap().unwrap();
    assert_eq!(stored.state_hash, "hash-1");
    assert_eq!(stored.width, 240);
    assert_eq!(stored.image, thumbnail("cue-a", "hash-1").image);

    let mut redrawn = thumbnail("cue-a", "hash-2");
    redrawn.image = vec![0x99, 0x98];
    show.put_cue_thumbnails(&[redrawn]).unwrap();

    let index = show.cue_thumbnail_index().unwrap();
    assert_eq!(index.len(), 2, "redrawing a cue replaces rather than adds");
    assert_eq!(index[0].cue_id, "cue-a");
    assert_eq!(index[0].state_hash, "hash-2");
    assert_eq!(
        show.cue_thumbnail("cue-a").unwrap().unwrap().image,
        [0x99, 0x98]
    );

    let _ = fs::remove_file(path);
}

#[test]
fn a_cue_without_a_stored_preview_reads_as_absent_rather_than_failing() {
    let path = temporary("cue-thumbnail-absent");
    let (show, _) = ShowStore::create(&path, "Tour").unwrap();

    assert!(show.cue_thumbnail("never-drawn").unwrap().is_none());
    assert!(show.cue_thumbnail_index().unwrap().is_empty());

    let _ = fs::remove_file(path);
}

#[test]
fn pruning_drops_previews_for_cues_the_show_no_longer_holds() {
    let path = temporary("cue-thumbnail-prune");
    let (show, _) = ShowStore::create(&path, "Tour").unwrap();
    show.put_cue_thumbnails(&[
        thumbnail("kept", "hash"),
        thumbnail("deleted", "hash"),
        thumbnail("also-deleted", "hash"),
    ])
    .unwrap();

    let removed = show.prune_cue_thumbnails(&["kept".to_string()]).unwrap();

    assert_eq!(removed, 2);
    let index = show.cue_thumbnail_index().unwrap();
    assert_eq!(index.len(), 1);
    assert_eq!(index[0].cue_id, "kept");

    let _ = fs::remove_file(path);
}

#[test]
fn oversized_or_empty_previews_are_rejected_without_writing_the_batch() {
    let path = temporary("cue-thumbnail-limits");
    let (show, _) = ShowStore::create(&path, "Tour").unwrap();

    let mut empty = thumbnail("cue-a", "hash");
    empty.image = Vec::new();
    assert!(matches!(
        show.put_cue_thumbnails(&[empty]),
        Err(StoreError::Invalid(_))
    ));

    let mut huge = thumbnail("cue-a", "hash");
    huge.image = vec![0; 512 * 1024 + 1];
    assert!(matches!(
        show.put_cue_thumbnails(&[huge]),
        Err(StoreError::Invalid(_))
    ));

    let mut unhashed = thumbnail("cue-a", "");
    unhashed.state_hash = String::new();
    assert!(matches!(
        show.put_cue_thumbnails(&[unhashed]),
        Err(StoreError::Invalid(_))
    ));

    // A rejected batch is rejected whole: the valid entry beside the bad one is not written.
    let mut bad = thumbnail("cue-b", "hash");
    bad.width = 0;
    assert!(matches!(
        show.put_cue_thumbnails(&[thumbnail("cue-a", "hash"), bad]),
        Err(StoreError::Invalid(_))
    ));
    assert!(show.cue_thumbnail("cue-a").unwrap().is_none());

    let _ = fs::remove_file(path);
}

/// A show recorded before cue previews existed has no `cue_thumbnails` table at all. It must open,
/// read, and accept new previews without any operator-visible recovery step.
#[test]
fn a_show_saved_before_cue_previews_existed_opens_and_gains_the_table() {
    let path = temporary("cue-thumbnail-legacy");
    let (show, _) = ShowStore::create(&path, "Legacy Tour").unwrap();
    show.put_object("cue_list", "main", &serde_json::json!({"cues": []}), 0)
        .unwrap();
    drop(show);

    // Reduce the file to its pre-feature shape: no preview table, and the schema version the
    // previous release stamped. Without the version rollback this would not exercise the upgrade
    // path at all, because `open` only migrates a file whose version is behind the current one.
    let legacy = rusqlite::Connection::open(&path).unwrap();
    legacy
        .execute_batch("DROP TABLE cue_thumbnails; UPDATE schema_info SET version=6")
        .unwrap();
    assert!(
        !legacy
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='cue_thumbnails')",
                [],
                |row| row.get::<_, bool>(0),
            )
            .unwrap()
    );
    drop(legacy);

    let reopened = ShowStore::open(&path).unwrap();

    assert_eq!(reopened.objects("cue_list").unwrap().len(), 1);
    assert!(reopened.cue_thumbnail_index().unwrap().is_empty());
    reopened
        .put_cue_thumbnails(&[thumbnail("cue-a", "hash")])
        .unwrap();
    assert!(reopened.cue_thumbnail("cue-a").unwrap().is_some());

    let _ = fs::remove_file(path);
}
