//! The same patch transport and PlanningDocument commit used by CAD history and desk rebase.
use super::apply_changes;
use crate::state::{DocumentReader, ObjectKey};
use light_application::PatchFixtureProjection;
use light_core::FixtureId;
use light_fixture::{
    FixtureProfile, PatchPolicy, PatchedFixturePatch, PatchedFixtureProfileReference,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use viz_document::PlanningDocument;

fn schema(path: &std::path::Path) -> i64 {
    rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .unwrap()
        .query_row("SELECT version FROM schema_info", [], |row| row.get(0))
        .unwrap()
}

#[test]
fn venue_rebase_and_history_patch_save_reopen_preserve_architecture_and_reader_floor() {
    architecture_roundtrip(
        json!({"kind":"wall","ceiling_height_millimetres":4000,"openings":[{
            "id":uuid::Uuid::new_v4(),"offset_millimetres":1000,"sill_millimetres":900,
            "width_millimetres":1400,"height_millimetres":1800,
            "opening":{"kind":"window","shape":"arched"}
        }]}),
    );
}

#[test]
fn stairs_rebase_and_history_patch_save_reopen_preserve_steps_and_reader_floor() {
    architecture_roundtrip(json!({"kind":"stairs","steps":16}));
}

fn architecture_roundtrip(architecture: Value) {
    let scratch = std::path::PathBuf::from(
        std::env::var_os("LIGHT_TMP_DIR").expect("canonical scratch directory"),
    )
    .join(format!("venue-sync-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&scratch).unwrap();
    let path = scratch.join("working.show");
    let saved = scratch.join("saved.show");
    let document = PlanningDocument::create(&path, "Venue sync").unwrap();
    assert_eq!(schema(&path), 10);
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Venue".into();
    profile.name = "Wall".into();
    profile.patch_policy = PatchPolicy::VisualOnly;
    profile.modes[0].splits[0].footprint = 0;
    let reference = PatchedFixtureProfileReference {
        profile_id: profile.id,
        profile_revision: u64::from(profile.revision),
        mode_id: profile.modes[0].id,
    };
    document
        .retain_fixture_profile(serde_json::to_value(&profile).unwrap())
        .unwrap();
    let fixture_id = FixtureId::new();

    let patch: PatchedFixturePatch = serde_json::from_value(json!({
        "fixture_id":fixture_id,"virtual_fixture_number":1,"name":"Venue wall",
        "split_patches":[{"split":1,"universe":null,"address":null}],
        "scenery_size_metres":{"x":6000,"y":4000,"z":200},
        "location":{"x":2000,"y":3000,"z":0},"rotation":{"x":0,"y":0,"z":35},
        "scenery_options":{"architecture":architecture,"colour_srgb":"#CACACA"}
    }))
    .unwrap();
    let input =
        light_patch_wire::patch_input(light_patch_wire::wire_fixture(&PatchFixtureProjection {
            fixture_revision: 1,
            profile: reference,
            patch,
        }));
    let original = serde_json::to_value(input).unwrap();
    let key = ObjectKey::new("patched_fixture", fixture_id.0.to_string());
    let profiles = BTreeMap::new();
    apply_changes(
        &document,
        &[(key.clone(), Some(original.clone()))],
        &[],
        &profiles,
    )
    .unwrap();
    assert_eq!(schema(&path), 11);
    let mut reader = DocumentReader::default();
    assert_eq!(
        reader.read(&document).unwrap().state.objects[&key]["scenery_options"]["architecture"],
        architecture
    );
    let mut changed = original.clone();
    if architecture["kind"] == "stairs" {
        changed["scenery_options"]["architecture"]["steps"] = json!(24);
    } else {
        changed["scenery_options"]["architecture"]["openings"][0]["width_millimetres"] =
            json!(1600);
    }
    changed["location"]["x"] = json!(4200);
    apply_changes(
        &document,
        &[(key.clone(), Some(changed.clone()))],
        &[],
        &profiles,
    )
    .unwrap();
    let snapshot = document.patch_snapshot().unwrap();
    assert_eq!(snapshot.fixtures[0].patch.location.x, 4200);
    // CAD Undo replays the full before fixture; Redo replays after, through this same patch service.
    apply_changes(
        &document,
        &[(key.clone(), Some(original.clone()))],
        &[],
        &profiles,
    )
    .unwrap();
    assert_eq!(
        document.patch_snapshot().unwrap().fixtures[0]
            .patch
            .location
            .x,
        2000
    );
    apply_changes(
        &document,
        &[(key.clone(), Some(changed.clone()))],
        &[],
        &profiles,
    )
    .unwrap();
    document.save_as(&saved).unwrap();
    let reopened = PlanningDocument::open(&saved).unwrap();
    let snapshot = reopened.patch_snapshot().unwrap();
    assert_eq!(snapshot.fixtures[0].patch.location.x, 4200);
    let restored: Value =
        serde_json::to_value(&snapshot.fixtures[0].patch.scenery_options).unwrap();
    assert_eq!(
        restored["architecture"],
        changed["scenery_options"]["architecture"]
    );
    assert_eq!(
        snapshot.fixtures[0]
            .patch
            .scenery_size_metres
            .as_ref()
            .unwrap()
            .x,
        6000.0
    );
    assert_eq!(schema(&saved), 11);
    drop(reopened);
    apply_changes(&document, &[(key.clone(), None)], &[], &profiles).unwrap();
    assert!(document.patch_snapshot().unwrap().fixtures.is_empty());
    assert_eq!(schema(&path), 11);
    apply_changes(&document, &[(key, Some(original))], &[], &profiles).unwrap();
    assert_eq!(schema(&path), 11);
    drop(document);
    std::fs::remove_dir_all(scratch).unwrap();
}
