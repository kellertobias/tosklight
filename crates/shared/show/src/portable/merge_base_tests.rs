//! An offline copy carries the source objects it started from (TL-858).
use super::MergeLineage;
use crate::ShowStore;
use light_core::ShowId;
use serde_json::json;
use std::{fs, path::PathBuf};
use uuid::Uuid;

fn temporary(name: &str) -> PathBuf {
    let directory = std::env::var_os("LIGHT_TMP_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    fs::create_dir_all(&directory).unwrap();
    directory.join(format!("light-merge-base-{name}-{}.show", Uuid::new_v4()))
}

fn lineage() -> MergeLineage {
    MergeLineage {
        source_show_id: ShowId(Uuid::new_v4()),
        source_show_name: "Main Show".into(),
        source_revision: 4,
        fork_id: Uuid::new_v4(),
        taken_at: "2026-10-31T10:00:00Z".into(),
    }
}

#[test]
fn an_ordinary_show_has_no_lineage_and_no_base() {
    let path = temporary("ordinary");
    let (show, _) = ShowStore::create(&path, "Ordinary").unwrap();
    show.put_object("group", "7", &json!({"name": "Front"}), 0)
        .unwrap();
    assert_eq!(show.merge_lineage().unwrap(), None);
    assert!(show.merge_base_objects().unwrap().is_empty());
    let _ = fs::remove_file(path);
}

#[test]
fn the_base_keeps_the_objects_of_the_moment_the_copy_was_taken_and_travels_with_the_file() {
    let path = temporary("copy");
    let (show, _) = ShowStore::create(&path, "Copy").unwrap();
    show.put_object("group", "7", &json!({"name": "Front"}), 0)
        .unwrap();
    let expected = lineage();
    assert_eq!(show.record_merge_base(&expected).unwrap(), 1);

    // Offline programming changes the copy, never its base.
    show.put_object("group", "7", &json!({"name": "Front wash"}), 1)
        .unwrap();
    show.put_object("group", "8", &json!({"name": "Back"}), 0)
        .unwrap();
    let base = show.merge_base_objects().unwrap();
    assert_eq!(base.len(), 1);
    assert_eq!((base[0].kind.as_str(), base[0].id.as_str()), ("group", "7"));
    assert_eq!(base[0].body, json!({"name": "Front"}));

    // Save As copies the whole file, so a renamed offline copy is still mergeable.
    let saved_as = temporary("saved-as");
    show.backup_to(&saved_as).unwrap();
    drop(show);
    let reopened = ShowStore::open(&saved_as).unwrap();
    assert_eq!(reopened.merge_lineage().unwrap(), Some(expected));
    assert_eq!(reopened.merge_base_objects().unwrap(), base);
    let _ = fs::remove_file(path);
    let _ = fs::remove_file(saved_as);
}
