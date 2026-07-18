#[test]
fn opening_a_show_repairs_and_persists_missing_logical_heads() {
    let directory = std::env::temp_dir().join(format!("light-head-repair-{}", Uuid::new_v4()));
    let path = directory.join("repair.show");
    std::fs::create_dir_all(&directory).unwrap();
    let show_id = default_show::initialise(&path).unwrap();
    let store = ShowStore::open(&path).unwrap();
    let object = store
        .objects("patched_fixture")
        .unwrap()
        .into_iter()
        .find(|object| object.body["fixture_number"] == 501)
        .unwrap();
    let mut fixture =
        serde_json::from_value::<light_fixture::PatchedFixture>(object.body.clone()).unwrap();
    fixture.logical_heads.clear();
    store
        .put_object(
            "patched_fixture",
            &object.id,
            &serde_json::to_value(&fixture).unwrap(),
            object.revision,
        )
        .unwrap();
    let entry = ShowEntry {
        id: show_id,
        name: "Repair".into(),
        path: path.display().to_string(),
        revision: 0,
        updated_at: String::new(),
        revision_copy: None,
    };
    reconcile_show_logical_heads(&entry).unwrap();
    let repaired = ShowStore::open(&path)
        .unwrap()
        .objects("patched_fixture")
        .unwrap()
        .into_iter()
        .find(|candidate| candidate.id == object.id)
        .unwrap();
    let repaired_fixture =
        serde_json::from_value::<light_fixture::PatchedFixture>(repaired.body).unwrap();
    assert_eq!(repaired_fixture.logical_heads.len(), 10);
    let ids = repaired_fixture
        .logical_heads
        .iter()
        .map(|head| head.fixture_id)
        .collect::<Vec<_>>();
    reconcile_show_logical_heads(&entry).unwrap();
    let stable = ShowStore::open(&path)
        .unwrap()
        .objects("patched_fixture")
        .unwrap()
        .into_iter()
        .find(|candidate| candidate.id == object.id)
        .unwrap();
    let stable_fixture =
        serde_json::from_value::<light_fixture::PatchedFixture>(stable.body).unwrap();
    assert_eq!(
        stable_fixture
            .logical_heads
            .iter()
            .map(|head| head.fixture_id)
            .collect::<Vec<_>>(),
        ids
    );
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn opening_a_legacy_show_migrates_embedded_profiles_and_patch_assignments() {
    let directory =
        std::env::temp_dir().join(format!("light-fixture-v2-repair-{}", Uuid::new_v4()));
    let path = directory.join("repair.show");
    std::fs::create_dir_all(&directory).unwrap();
    let show_id = default_show::initialise(&path).unwrap();
    let store = ShowStore::open(&path).unwrap();
    let object = store.objects("patched_fixture").unwrap().remove(0);
    let mut fixture = serde_json::from_value::<light_fixture::PatchedFixture>(object.body).unwrap();
    assert_eq!(fixture.definition.schema_version, 1);
    fixture.split_patches.clear();
    fixture.multipatch.push(light_fixture::MultiPatchInstance {
        id: Uuid::new_v4(),
        name: "Legacy balcony".into(),
        universe: Some(99),
        address: Some(100),
        split_patches: vec![],
        location: Default::default(),
        rotation: Default::default(),
    });
    store
        .put_object(
            "patched_fixture",
            &object.id,
            &serde_json::to_value(&fixture).unwrap(),
            object.revision,
        )
        .unwrap();
    let entry = ShowEntry {
        id: show_id,
        name: "Legacy fixture migration".into(),
        path: path.display().to_string(),
        revision: 0,
        updated_at: String::new(),
        revision_copy: None,
    };

    let loaded = load_engine_snapshot(&entry).unwrap();
    let migrated = loaded
        .fixtures
        .iter()
        .find(|candidate| candidate.fixture_id == fixture.fixture_id)
        .unwrap();
    assert_eq!(
        migrated.definition.schema_version,
        light_fixture::FIXTURE_PROFILE_SCHEMA_VERSION
    );
    assert!(migrated.definition.profile_snapshot.is_some());
    assert_eq!(migrated.split_patches[0].universe, fixture.universe);
    assert_eq!(migrated.split_patches[0].address, fixture.address);
    assert_eq!(migrated.multipatch[0].split_patches[0].universe, Some(99));
    assert_eq!(migrated.multipatch[0].split_patches[0].address, Some(100));

    let persisted = ShowStore::open(&path)
        .unwrap()
        .objects("patched_fixture")
        .unwrap()
        .into_iter()
        .find(|candidate| candidate.id == object.id)
        .unwrap();
    assert_eq!(persisted.body["definition"]["schema_version"], 2);
    assert!(persisted.body["definition"]["profile_snapshot"].is_object());
    assert_eq!(
        persisted.body["split_patches"][0]["universe"],
        fixture.universe.unwrap()
    );
    assert_eq!(
        persisted.body["multipatch"][0]["split_patches"][0]["universe"],
        99
    );
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn opening_a_legacy_show_persists_stable_cue_identities_once() {
    let directory = std::env::temp_dir().join(format!("light-cue-id-repair-{}", Uuid::new_v4()));
    let path = directory.join("repair.show");
    std::fs::create_dir_all(&directory).unwrap();
    let show_id = default_show::initialise(&path).unwrap();
    let store = ShowStore::open(&path).unwrap();
    let mut first_cue = light_playback::Cue::new(1.0);
    first_cue
        .group_changes
        .push(light_playback::GroupCueChange {
            group_id: "1".into(),
            attribute: light_core::AttributeKey::intensity(),
            value: Some(light_core::AttributeValue::Normalized(0.5)),
            automatic_restore: false,
            fade_millis: None,
            delay_millis: None,
        });
    let list = light_playback::CueList {
        id: light_core::CueListId::new(),
        name: "Legacy".into(),
        priority: 0,
        mode: light_playback::CueListMode::Sequence,
        looped: false,
        chaser_step_millis: 1_000,
        speed_group: None,
        intensity_priority_mode: light_playback::IntensityPriorityMode::Htp,
        wrap_mode: Some(light_playback::WrapMode::Off),
        restart_mode: light_playback::RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_multiplier: 1.0,
        cues: vec![first_cue, light_playback::Cue::new(2.0)],
    };
    let mut legacy = serde_json::to_value(list).unwrap();
    for cue in legacy["cues"].as_array_mut().unwrap() {
        cue.as_object_mut().unwrap().remove("id");
        cue.as_object_mut().unwrap().remove("cue_only");
        for change in cue["group_changes"].as_array_mut().unwrap() {
            change.as_object_mut().unwrap().remove("automatic_restore");
        }
    }
    store.put_object("cue_list", "legacy", &legacy, 0).unwrap();
    let entry = ShowEntry {
        id: show_id,
        name: "Repair".into(),
        path: path.display().to_string(),
        revision: 0,
        updated_at: String::new(),
        revision_copy: None,
    };
    let loaded = load_engine_snapshot(&entry).unwrap();
    assert!(!loaded.cue_lists[0].cues[0].cue_only);
    assert!(!loaded.cue_lists[0].cues[0].group_changes[0].automatic_restore);
    reconcile_show_cue_identities(&entry).unwrap();
    let repaired = ShowStore::open(&path)
        .unwrap()
        .objects("cue_list")
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    let ids = repaired.body["cues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|cue| cue["id"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    reconcile_show_cue_identities(&entry).unwrap();
    let stable = ShowStore::open(&path)
        .unwrap()
        .objects("cue_list")
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(stable.revision, repaired.revision);
    assert_eq!(
        stable.body["cues"]
            .as_array()
            .unwrap()
            .iter()
            .map(|cue| cue["id"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>(),
        ids
    );
    let _ = std::fs::remove_dir_all(directory);
}
use axum::{body::Body, http::Request};
use http_body_util::BodyExt;
use tower::ServiceExt;
