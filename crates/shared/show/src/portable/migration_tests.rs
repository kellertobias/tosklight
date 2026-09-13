use crate::ShowStore;
use serde_json::{Value, json};
use std::{fs, path::PathBuf};
use uuid::Uuid;

fn temporary(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("light-migration-{name}-{}.show", Uuid::new_v4()))
}

/// A standalone record exactly as the original demo generator wrote it.
fn demo_truss_segment() -> Value {
    json!({
        "chords": 4,
        "id": "planned-demo-venue-1-2",
        "kind": "truss",
        "name": "Back Truss Segment 2",
        "position": {"x": -1, "y": 4, "z": 4.15},
        "rotation_degrees": {"x": 0, "y": 0, "z": 0},
        "size": {"x": 2, "y": 0.3, "z": 0.3}
    })
}

/// A show holding the demo's standalone record, one record nothing generated, and one patched
/// fixture of a profile with `policy` (and a crowd when `crowd` is set).
fn legacy_show(name: &str, policy: &str, crowd: bool) -> PathBuf {
    let path = temporary(name);
    let (show, _) = ShowStore::create(&path, "Legacy venue show").unwrap();
    show.put_object("venue", "planned-demo-venue-1-2", &demo_truss_segment(), 0)
        .unwrap();
    show.put_object(
        "venue",
        "hand-made-riser",
        &json!({"name": "Riser", "kind": "riser", "position": {"x": 0, "y": 2, "z": 0.4}}),
        0,
    )
    .unwrap();
    let profile_id = Uuid::new_v4().to_string();
    let mut profile = json!({"id": profile_id, "patch_policy": policy});
    if crowd {
        profile["crowd"] = json!({"modes": []});
    }
    show.conn
        .execute(
            "INSERT INTO fixture_profile_revisions(profile_id,revision,content_digest,profile_json)
             VALUES (?1,1,'digest',?2)",
            (profile_id.as_str(), profile.to_string()),
        )
        .unwrap();
    show.put_object(
        "patched_fixture",
        &Uuid::new_v4().to_string(),
        &json!({"name": "Back Truss", "profile_id": profile_id, "profile_revision": 1}),
        0,
    )
    .unwrap();
    written_by_schema_seven(&show);
    path
}

/// Every show written before the cleanup existed is schema 7.
fn written_by_schema_seven(show: &ShowStore) {
    show.conn
        .execute_batch("UPDATE schema_info SET version=7;")
        .unwrap();
}

fn venue_ids(path: &PathBuf) -> Vec<String> {
    let store = ShowStore::open(path).unwrap();
    let mut ids: Vec<_> = store
        .objects("venue")
        .unwrap()
        .into_iter()
        .map(|object| object.id)
        .collect();
    ids.sort();
    ids
}

#[test]
fn demo_venue_records_go_once_the_patch_carries_the_scenery() {
    let path = legacy_show("superseded", "visual_only", false);

    assert_eq!(
        venue_ids(&path),
        ["hand-made-riser"],
        "the generator's stale copy goes; a record it did not write stays"
    );
    assert_eq!(
        venue_ids(&path),
        ["hand-made-riser"],
        "opening again changes nothing"
    );
    let store = ShowStore::open(&path).unwrap();
    let version: i64 = store
        .conn
        .query_row("SELECT version FROM schema_info", [], |row| row.get(0))
        .unwrap();
    assert_eq!(version, 9);
    drop(store);
    let _ = fs::remove_file(path);
}

#[test]
fn demo_venue_records_stay_while_the_patch_has_no_scenery_of_its_own() {
    for (name, policy, crowd) in [("lights", "dmx", false), ("crowd", "visual_only", true)] {
        let path = legacy_show(name, policy, crowd);
        assert_eq!(
            venue_ids(&path),
            ["hand-made-riser", "planned-demo-venue-1-2"],
            "{name}: nothing else draws this scenery"
        );
        let _ = fs::remove_file(path);
    }
}

#[test]
fn an_inline_patched_fixture_that_is_visual_only_counts_as_scenery() {
    let path = temporary("inline");
    let (show, _) = ShowStore::create(&path, "Inline venue show").unwrap();
    show.put_object("venue", "planned-demo-venue-1-2", &demo_truss_segment(), 0)
        .unwrap();
    show.put_object(
        "patched_fixture",
        &Uuid::new_v4().to_string(),
        &json!({"name": "Back Truss", "definition": {"profile_snapshot": {"patch_policy": "visual_only"}}}),
        0,
    )
    .unwrap();
    written_by_schema_seven(&show);
    drop(show);

    assert!(venue_ids(&path).is_empty());
    let _ = fs::remove_file(path);
}

#[test]
fn venue_multipatch_copies_become_venue_objects_of_their_own() {
    let path = temporary("venue-copies");
    let (show, _) = ShowStore::create(&path, "Venue copies").unwrap();
    let venue = Uuid::new_v4().to_string();
    let lights = Uuid::new_v4().to_string();
    for (profile_id, policy) in [(&venue, "visual_only"), (&lights, "dmx")] {
        show.conn
            .execute(
                "INSERT INTO fixture_profile_revisions(profile_id,revision,content_digest,profile_json)
                 VALUES (?1,1,'digest',?2)",
                (profile_id.as_str(), json!({"id": profile_id, "patch_policy": policy}).to_string()),
            )
            .unwrap();
    }
    let copy = |id: &str, name: &str, x: i64| {
        json!({"id": id, "name": name, "split_patches": [], "location": {"x": x, "y": 0, "z": 4150},
               "rotation": {"x": 0, "y": 0, "z": 0}, "scenery_size_metres": {"x": 2000, "y": 340, "z": 340}})
    };
    let truss_id = Uuid::new_v4().to_string();
    let span_two = Uuid::new_v4().to_string();
    let span_three = Uuid::new_v4().to_string();
    show.put_object(
        "patched_fixture",
        &truss_id,
        &json!({"fixture_id": truss_id, "virtual_fixture_number": 7, "name": "Back Truss Segment 1",
                "profile_id": venue, "profile_revision": 1, "layer_id": "trusses",
                "location": {"x": -3000, "y": 0, "z": 4150},
                "logical_heads": [{"fixture_id": Uuid::new_v4().to_string(), "head_index": 0}],
                "multipatch": [copy(&span_two, "Back Truss Segment 2", -1000),
                               copy(&span_three, "Back Truss Segment 3", 1000)]}),
        0,
    )
    .unwrap();
    let wash_id = Uuid::new_v4().to_string();
    let wash_copy = Uuid::new_v4().to_string();
    show.put_object(
        "patched_fixture",
        &wash_id,
        &json!({"fixture_id": wash_id, "fixture_number": 1, "name": "Wash", "profile_id": lights,
                "profile_revision": 1, "multipatch": [copy(&wash_copy, "Wash copy", 0)]}),
        0,
    )
    .unwrap();
    show.conn
        .execute_batch("UPDATE schema_info SET version=8;")
        .unwrap();
    drop(show);

    let store = ShowStore::open(&path).unwrap();
    let body = |id: &str| -> Value {
        store
            .objects("patched_fixture")
            .unwrap()
            .into_iter()
            .find(|object| object.id == id)
            .map(|object| object.body)
            .unwrap_or_else(|| panic!("{id} is missing"))
    };
    assert_eq!(body(&truss_id)["multipatch"], json!([]));
    let second = body(&span_two);
    assert_eq!(second["name"], "Back Truss Segment 2");
    assert_eq!(second["fixture_id"], span_two.as_str());
    assert_eq!(second["virtual_fixture_number"], 8);
    assert_eq!(second["location"]["x"], -1000);
    assert_eq!(second["scenery_size_metres"]["x"], 2000);
    assert_eq!(second["layer_id"], "trusses");
    assert_eq!(second["multipatch"], json!([]));
    assert_ne!(
        second["logical_heads"][0]["fixture_id"],
        body(&truss_id)["logical_heads"][0]["fixture_id"],
        "a separated object's head is its own identity"
    );
    assert_eq!(body(&span_three)["virtual_fixture_number"], 9);
    assert_eq!(
        body(&wash_id)["multipatch"].as_array().map(Vec::len),
        Some(1),
        "a lighting fixture keeps its copies"
    );
    drop(store);
    let _ = fs::remove_file(path);
}
