use axum::{body::Body, http::Request};
use http_body_util::BodyExt;
use tower::ServiceExt;

fn migration_test_entry(
    path: &FsPath,
    id: light_core::ShowId,
    name: &str,
) -> ShowEntry {
    ShowEntry {
        id,
        name: name.into(),
        path: path.display().to_string(),
        revision: 0,
        updated_at: String::new(),
        revision_copy: None,
    }
}

fn migration_backup_files(data_dir: &FsPath) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(data_dir.join("backups")) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains("-migration-") && name.ends_with(".show"))
        })
        .collect()
}

fn checkpoint_show_file(path: &FsPath) {
    let connection = rusqlite::Connection::open(path).unwrap();
    connection
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .unwrap();
}

fn settle_sqlite_wal_before_file_baseline(path: &FsPath) {
    checkpoint_show_file(path);
    // Opening the repository can perform one-time SQLite WAL/main-file bookkeeping. Settle that
    // physical representation before the byte digest baseline without reading or writing an
    // individual show object.
    drop(ShowStore::open(path).unwrap());
    checkpoint_show_file(path);
}

fn show_file_digest(path: &FsPath) -> [u8; 32] {
    Sha256::digest(std::fs::read(path).unwrap()).into()
}

fn overwrite_show_object_body_without_revision(
    path: &FsPath,
    kind: &str,
    id: &str,
    body: &serde_json::Value,
) {
    let connection = rusqlite::Connection::open(path).unwrap();
    let changed = connection
        .execute(
            "UPDATE objects SET body_json=?1 WHERE kind=?2 AND id=?3",
            rusqlite::params![serde_json::to_string(body).unwrap(), kind, id],
        )
        .unwrap();
    assert_eq!(changed, 1);
}

fn legacy_multipatch_without_appearance(id: Uuid, name: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "name": name,
        "universe": null,
        "address": null,
        "split_patches": [],
        "location": {"x": 1200, "y": -300, "z": 4500},
        "rotation": {"x": 0.0, "y": 90.0, "z": 0.0},
        "invert_pan": false,
        "invert_tilt": false,
        "bracket_angle": 12.5,
        "shaper_angle": null
    })
}

#[test]
fn startup_defaults_absent_installed_appearance_without_rewriting_the_legacy_fields() {
    let data_dir =
        std::env::temp_dir().join(format!("light-appearance-startup-{}", Uuid::new_v4()));
    let path = data_dir.join("shows/legacy-appearance.show");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let show_id = default_show::initialise(&path).unwrap();
    let store = ShowStore::open(&path).unwrap();
    let object = store
        .objects("patched_fixture")
        .unwrap()
        .into_iter()
        .find(|object| {
            object.body["multipatch"]
                .as_array()
                .is_some_and(|instances| !instances.is_empty())
        })
        .unwrap();
    let fixture_id = object.id.clone();
    let copy_id = Uuid::new_v4();
    let mut legacy = object.body;
    legacy.as_object_mut().unwrap().remove("installed_appearance");
    legacy["multipatch"] = serde_json::json!([
        legacy_multipatch_without_appearance(copy_id, "Legacy copy")
    ]);
    store
        .put_object("patched_fixture", &fixture_id, &legacy, object.revision)
        .unwrap();
    let source_revision = store.portable_revision().unwrap();
    drop(store);
    let entry = migration_test_entry(&path, show_id, "Legacy appearance defaults");

    let first = Engine::new(ProgrammerRegistry::default());
    assert_eq!(
        compile_active_show_for_startup(&first, &entry, &data_dir, 5),
        None
    );
    let first_snapshot = first.snapshot();
    let fixture = first_snapshot
        .fixtures
        .iter()
        .find(|fixture| fixture.fixture_id.0.to_string() == fixture_id)
        .unwrap();
    assert_eq!(
        fixture.installed_appearance,
        light_fixture::InstalledFixtureAppearance::default()
    );
    assert_eq!(fixture.multipatch.len(), 1);
    assert_eq!(fixture.multipatch[0].id, copy_id);
    assert_eq!(
        fixture.multipatch[0].installed_appearance,
        light_fixture::InstalledFixtureAppearance::default()
    );

    let store = ShowStore::open(&path).unwrap();
    let document = store.portable_document().unwrap();
    let migrated_revision = document.revision();
    assert!(migrated_revision > source_revision);
    let migrated_object = document.object("patched_fixture", &fixture_id).unwrap();
    let migrated_object_revision = migrated_object.revision();
    let migrated_body = migrated_object.body().clone();
    assert!(migrated_body.get("definition").is_none());
    assert!(migrated_body.get("installed_appearance").is_none());
    assert!(
        migrated_body["multipatch"][0]
            .get("installed_appearance")
            .is_none()
    );
    drop(store);
    let backups = migration_backup_files(&data_dir);
    assert_eq!(backups.len(), 1);

    let reopened = Engine::new(ProgrammerRegistry::default());
    assert_eq!(
        compile_active_show_for_startup(&reopened, &entry, &data_dir, 5),
        None
    );
    let reopened_snapshot = reopened.snapshot();
    let reopened_fixture = reopened_snapshot
        .fixtures
        .iter()
        .find(|fixture| fixture.fixture_id.0.to_string() == fixture_id)
        .unwrap();
    assert_eq!(
        reopened_fixture.installed_appearance,
        light_fixture::InstalledFixtureAppearance::default()
    );
    assert_eq!(
        reopened_fixture.multipatch[0].installed_appearance,
        light_fixture::InstalledFixtureAppearance::default()
    );
    let reopened_store = ShowStore::open(&path).unwrap();
    let reopened_document = reopened_store.portable_document().unwrap();
    assert_eq!(reopened_document.revision(), migrated_revision);
    let reopened_object = reopened_document
        .object("patched_fixture", &fixture_id)
        .unwrap();
    assert_eq!(reopened_object.revision(), migrated_object_revision);
    assert_eq!(reopened_object.body(), &migrated_body);
    assert_eq!(migration_backup_files(&data_dir), backups);
    drop(reopened_store);
    std::fs::remove_dir_all(data_dir).unwrap();
}

#[test]
fn startup_defaults_absent_installed_appearance_in_a_lean_show_without_any_rewrite() {
    let data_dir =
        std::env::temp_dir().join(format!("light-lean-appearance-startup-{}", Uuid::new_v4()));
    let path = data_dir.join("shows/lean-appearance.show");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let show_id = default_show::initialise(&path).unwrap();
    let bootstrap_entry = migration_test_entry(&path, show_id, default_show::name());

    // Establish the already-lean compatibility input independently. The assertions below begin
    // only after the raw appearance fields are removed, and use a separate empty backup root.
    let bootstrap_data_dir = data_dir.join("bootstrap");
    let bootstrap = Engine::new(ProgrammerRegistry::default());
    assert_eq!(
        compile_active_show_for_startup(&bootstrap, &bootstrap_entry, &bootstrap_data_dir, 5),
        None
    );
    std::fs::remove_dir_all(&bootstrap_data_dir).unwrap();
    let entry = migration_test_entry(&path, show_id, "Lean appearance compatibility");

    let store = ShowStore::open(&path).unwrap();
    let object = store
        .objects("patched_fixture")
        .unwrap()
        .into_iter()
        .find(|object| {
            object.body["multipatch"]
                .as_array()
                .is_some_and(|instances| !instances.is_empty())
        })
        .unwrap();
    let object_id = object.id.clone();
    let fixture_id = object.body["fixture_id"].as_str().unwrap().to_owned();
    let source_revision = store.portable_revision().unwrap();
    let source_object_revision = object.revision;
    let copy_id = Uuid::parse_str(object.body["multipatch"][0]["id"].as_str().unwrap()).unwrap();
    let mut lean = object.body;
    assert!(lean.get("definition").is_none());
    lean.as_object_mut().unwrap().remove("installed_appearance");
    lean["multipatch"][0]
        .as_object_mut()
        .unwrap()
        .remove("installed_appearance");
    drop(store);
    overwrite_show_object_body_without_revision(&path, "patched_fixture", &object_id, &lean);
    settle_sqlite_wal_before_file_baseline(&path);
    let source_store = ShowStore::open(&path).unwrap();
    assert_eq!(source_store.portable_revision().unwrap(), source_revision);
    let stored = source_store
        .objects("patched_fixture")
        .unwrap()
        .into_iter()
        .find(|candidate| candidate.id == object_id)
        .unwrap();
    assert_eq!(stored.revision, source_object_revision);
    assert_eq!(stored.body, lean);
    drop(source_store);
    settle_sqlite_wal_before_file_baseline(&path);
    let source_file_digest = show_file_digest(&path);
    assert!(migration_backup_files(&data_dir).is_empty());

    let first = Engine::new(ProgrammerRegistry::default());
    assert_eq!(
        compile_active_show_for_startup(&first, &entry, &data_dir, 5),
        None
    );
    let first_snapshot = first.snapshot();
    let first_fixture = first_snapshot
        .fixtures
        .iter()
        .find(|fixture| fixture.fixture_id.0.to_string() == fixture_id)
        .unwrap();
    assert_eq!(
        first_fixture.installed_appearance,
        light_fixture::InstalledFixtureAppearance::default()
    );
    assert!(!first_fixture.multipatch.is_empty());
    assert_eq!(first_fixture.multipatch[0].id, copy_id);
    assert_eq!(
        first_fixture.multipatch[0].installed_appearance,
        light_fixture::InstalledFixtureAppearance::default()
    );
    assert_eq!(show_file_digest(&path), source_file_digest);
    let first_store = ShowStore::open(&path).unwrap();
    assert_eq!(first_store.portable_revision().unwrap(), source_revision);
    let first_object = first_store
        .objects("patched_fixture")
        .unwrap()
        .into_iter()
        .find(|candidate| candidate.id == object_id)
        .unwrap();
    assert_eq!(first_object.revision, source_object_revision);
    assert_eq!(first_object.body, lean);
    assert!(migration_backup_files(&data_dir).is_empty());
    drop(first_store);

    let reopened = Engine::new(ProgrammerRegistry::default());
    assert_eq!(
        compile_active_show_for_startup(&reopened, &entry, &data_dir, 5),
        None
    );
    assert_eq!(show_file_digest(&path), source_file_digest);
    let reopened_snapshot = reopened.snapshot();
    let reopened_fixture = reopened_snapshot
        .fixtures
        .iter()
        .find(|fixture| fixture.fixture_id.0.to_string() == fixture_id)
        .unwrap();
    assert_eq!(
        reopened_fixture.installed_appearance,
        light_fixture::InstalledFixtureAppearance::default()
    );
    assert_eq!(
        reopened_fixture.multipatch[0].installed_appearance,
        light_fixture::InstalledFixtureAppearance::default()
    );
    let reopened_store = ShowStore::open(&path).unwrap();
    assert_eq!(reopened_store.portable_revision().unwrap(), source_revision);
    let reopened_object = reopened_store
        .objects("patched_fixture")
        .unwrap()
        .into_iter()
        .find(|candidate| candidate.id == object_id)
        .unwrap();
    assert_eq!(reopened_object.revision, source_object_revision);
    assert_eq!(reopened_object.body, lean);
    assert!(migration_backup_files(&data_dir).is_empty());
    drop(reopened_store);
    std::fs::remove_dir_all(data_dir).unwrap();
}

fn assert_malformed_installed_appearance_enters_recovery(
    case: &str,
    mutate: impl FnOnce(&mut serde_json::Value),
) {
    let data_dir = std::env::temp_dir().join(format!(
        "light-malformed-appearance-{case}-{}",
        Uuid::new_v4()
    ));
    let path = data_dir.join("shows/malformed-appearance.show");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let show_id = default_show::initialise(&path).unwrap();
    let store = ShowStore::open(&path).unwrap();
    let object = store
        .objects("patched_fixture")
        .unwrap()
        .into_iter()
        .find(|object| {
            object.body["multipatch"]
                .as_array()
                .is_some_and(|instances| !instances.is_empty())
        })
        .unwrap();
    let object_id = object.id.clone();
    let source_object_revision = object.revision;
    let source_revision = store.portable_revision().unwrap();
    let mut malformed = object.body;
    mutate(&mut malformed);
    drop(store);
    overwrite_show_object_body_without_revision(
        &path,
        "patched_fixture",
        &object_id,
        &malformed,
    );
    settle_sqlite_wal_before_file_baseline(&path);
    let source_store = ShowStore::open(&path).unwrap();
    assert_eq!(source_store.portable_revision().unwrap(), source_revision);
    let stored_object = source_store
        .objects("patched_fixture")
        .unwrap()
        .into_iter()
        .find(|candidate| candidate.id == object_id)
        .unwrap();
    assert_eq!(stored_object.revision, source_object_revision);
    assert_eq!(stored_object.body, malformed);
    drop(source_store);
    settle_sqlite_wal_before_file_baseline(&path);
    let source_file_digest = show_file_digest(&path);
    let entry = migration_test_entry(&path, show_id, "Malformed installed appearance");

    let engine = Engine::new(ProgrammerRegistry::default());
    let error = compile_active_show_for_startup(&engine, &entry, &data_dir, 5)
        .expect("malformed installed appearance should enter recovery mode");
    assert!(error.contains("might be corrupted or incompatible"));
    assert!(error.contains("invalid portable patched fixture"));
    assert!(engine.snapshot().fixtures.is_empty());
    assert_eq!(show_file_digest(&path), source_file_digest);

    let reopened = ShowStore::open(&path).unwrap();
    assert_eq!(reopened.portable_revision().unwrap(), source_revision);
    let retained = reopened
        .objects("patched_fixture")
        .unwrap()
        .into_iter()
        .find(|candidate| candidate.id == object_id)
        .unwrap();
    assert_eq!(retained.revision, source_object_revision);
    assert_eq!(retained.body, malformed);
    assert!(migration_backup_files(&data_dir).is_empty());
    drop(reopened);
    std::fs::remove_dir_all(data_dir).unwrap();
}

#[test]
fn malformed_root_cct_and_multipatch_gel_hex_enter_recovery_without_mutating_the_show() {
    assert_malformed_installed_appearance_enters_recovery("root-cct", |body| {
        body["installed_appearance"]["color_temperature_kelvin"] = serde_json::json!(999);
    });
    assert_malformed_installed_appearance_enters_recovery("multipatch-gel", |body| {
        body["multipatch"][0]["installed_appearance"] = serde_json::json!({
            "gel": {
                "type": "custom",
                "name": "Lowercase blue",
                "color_srgb": "#80a0ff"
            }
        });
    });
}

#[test]
fn startup_migrates_legacy_patch_to_lean_once_and_reopens_at_the_relocated_address() {
    let data_dir =
        std::env::temp_dir().join(format!("light-lean-startup-{}", Uuid::new_v4()));
    let path = data_dir.join("shows/default.show");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let show_id = default_show::initialise(&path).unwrap();
    let store = ShowStore::open(&path).unwrap();
    let mut retained_id = None;
    for object in store.objects("patched_fixture").unwrap() {
        let mut body = object.body;
        body["universe"] = serde_json::json!(1);
        if body["name"] == "Back Profile 1" {
            retained_id = Some(object.id.clone());
            body["address"] = serde_json::json!(500);
            body["split_patches"] = serde_json::json!([{
                "split": 1,
                "universe": 1,
                "address": 500
            }]);
            body["future_fixture"] = serde_json::json!({"kept": [3, 1, 2]});
            body["definition"]["future_schema_one"] =
                serde_json::json!({"kept": true});
        }
        store
            .put_object("patched_fixture", &object.id, &body, object.revision)
            .unwrap();
    }
    let retained_id = retained_id.unwrap();
    let source_revision = store.portable_revision().unwrap().value();
    drop(store);
    let entry = migration_test_entry(&path, show_id, default_show::name());

    let first_engine = Engine::new(ProgrammerRegistry::default());
    assert_eq!(
        compile_active_show_for_startup(&first_engine, &entry, &data_dir, 5),
        None
    );
    let first_fixture = first_engine
        .snapshot()
        .fixtures
        .iter()
        .find(|fixture| fixture.name == "Back Profile 1")
        .cloned()
        .unwrap();
    assert_eq!((first_fixture.universe, first_fixture.address), (Some(2), Some(1)));
    assert_eq!(
        (
            first_fixture.split_patches[0].universe,
            first_fixture.split_patches[0].address
        ),
        (Some(2), Some(1))
    );

    let store = ShowStore::open(&path).unwrap();
    let document = store.portable_document().unwrap();
    let migrated_revision = document.revision().value();
    let migrated_patch_revision = document.patch_revision().value();
    let body = document
        .object("patched_fixture", &retained_id)
        .unwrap()
        .body()
        .clone();
    let record = light_fixture::PortablePatchedFixtureRecord::decode(body.clone()).unwrap();
    assert!(!record.is_legacy_inline());
    assert!(body.get("definition").is_none());
    assert_eq!(body["future_fixture"], serde_json::json!({"kept": [3, 1, 2]}));
    assert!(
        body[light_fixture::RETAINED_LEGACY_DEFINITION_FIELDS]
            .as_array()
            .unwrap()
            .iter()
            .any(|field| field["json_pointer"] == "/future_schema_one"
                && field["value"] == serde_json::json!({"kept": true}))
    );
    let patch = record.patch().unwrap();
    assert_eq!((patch.universe, patch.address), (Some(2), Some(1)));
    assert_eq!(
        (
            patch.split_patches[0].universe,
            patch.split_patches[0].address
        ),
        (Some(2), Some(1))
    );
    assert!(!document.fixture_profile_revisions().is_empty());
    drop(store);

    let backups = migration_backup_files(&data_dir);
    assert_eq!(backups.len(), 1);
    assert!(backups[0]
        .file_name()
        .unwrap()
        .to_string_lossy()
        .contains(&format!("source-revision-{source_revision}")));

    let group_override = serde_json::json!({"name": "Front", "fixtures": []});
    let overridden = load_engine_snapshot_with_override(
        &entry,
        Some(("group", "7", &group_override)),
    )
    .unwrap();
    assert!(overridden.fixtures.iter().any(|fixture| fixture.name == "Back Profile 1"));
    assert!(overridden.groups.iter().any(|group| group.id == "7"));

    let reopened_engine = Engine::new(ProgrammerRegistry::default());
    assert_eq!(
        compile_active_show_for_startup(&reopened_engine, &entry, &data_dir, 5),
        None
    );
    let reopened = reopened_engine
        .snapshot()
        .fixtures
        .iter()
        .find(|fixture| fixture.name == "Back Profile 1")
        .cloned()
        .unwrap();
    assert_eq!((reopened.universe, reopened.address), (Some(2), Some(1)));
    let store = ShowStore::open(&path).unwrap();
    assert_eq!(store.portable_revision().unwrap().value(), migrated_revision);
    assert_eq!(
        store.portable_patch_revision().unwrap().value(),
        migrated_patch_revision
    );
    assert_eq!(migration_backup_files(&data_dir), backups);
    drop(store);
    std::fs::remove_dir_all(data_dir).unwrap();
}

#[test]
fn startup_moves_assigned_group_master_to_playback_and_reopens_without_a_sidecar() {
    let data_dir =
        std::env::temp_dir().join(format!("light-group-master-startup-{}", Uuid::new_v4()));
    let path = data_dir.join("shows/group-master.show");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let show_id = default_show::initialise(&path).unwrap();
    let store = ShowStore::open(&path).unwrap();
    store
        .put_object(
            "group",
            "900",
            &serde_json::json!({
                "id": "900",
                "name": "Portable master",
                "fixtures": [],
                "master": 0.375,
                "playback_fader": 99,
                "future_group": {"kept": true}
            }),
            0,
        )
        .unwrap();
    store
        .put_object(
            "playback",
            "900",
            &serde_json::json!({
                "number": 900,
                "name": "Portable master",
                "target": {"type": "group", "group_id": "900"},
                "future_playback": {"kept": true}
            }),
            0,
        )
        .unwrap();
    let source_revision = store.portable_revision().unwrap().value();
    drop(store);
    let entry = migration_test_entry(&path, show_id, "Group Master transfer");

    let first = Engine::new(ProgrammerRegistry::default());
    assert_eq!(
        compile_active_show_for_startup(&first, &entry, &data_dir, 5),
        None
    );
    assert_eq!(first.group_master("900"), Some(0.375));

    let store = ShowStore::open(&path).unwrap();
    let document = store.portable_document().unwrap();
    let migrated_revision = document.revision();
    let group = document.object("group", "900").unwrap().body();
    assert!(group.get("master").is_none());
    assert!(group.get("playback_fader").is_none());
    assert_eq!(group["future_group"], serde_json::json!({"kept": true}));
    let playback = document.object("playback", "900").unwrap().body();
    assert_eq!(playback["target"]["initial_master"], 0.375);
    assert_eq!(
        playback["future_playback"],
        serde_json::json!({"kept": true})
    );
    drop(store);
    let backups = migration_backup_files(&data_dir);
    assert_eq!(backups.len(), 1);
    assert!(backups[0]
        .file_name()
        .unwrap()
        .to_string_lossy()
        .contains(&format!("source-revision-{source_revision}")));
    let backup = ShowStore::open(&backups[0]).unwrap().portable_document().unwrap();
    assert_eq!(backup.object("group", "900").unwrap().body()["master"], 0.375);
    assert_eq!(
        backup.object("group", "900").unwrap().body()["playback_fader"],
        99
    );

    let reopened = Engine::new(ProgrammerRegistry::default());
    assert_eq!(
        compile_active_show_for_startup(&reopened, &entry, &data_dir, 5),
        None
    );
    assert_eq!(reopened.group_master("900"), Some(0.375));
    assert_eq!(
        ShowStore::open(&path).unwrap().portable_revision().unwrap(),
        migrated_revision
    );
    assert_eq!(migration_backup_files(&data_dir), backups);
    std::fs::remove_dir_all(data_dir).unwrap();
}

#[test]
fn failed_legacy_candidate_leaves_the_stored_document_unchanged_and_enters_recovery() {
    let data_dir =
        std::env::temp_dir().join(format!("light-failed-migration-{}", Uuid::new_v4()));
    let path = data_dir.join("shows/damaged.show");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let show_id = default_show::initialise(&path).unwrap();
    let store = ShowStore::open(&path).unwrap();
    let object = store.objects("patched_fixture").unwrap().remove(0);
    let mut damaged = object.body;
    damaged["fixture_id"] = serde_json::json!("not-a-uuid");
    store
        .put_object(
            "patched_fixture",
            &object.id,
            &damaged,
            object.revision,
        )
        .unwrap();
    let source_revision = store.portable_revision().unwrap().value();
    let source_profiles = store
        .portable_document()
        .unwrap()
        .fixture_profile_revisions()
        .len();
    drop(store);
    let entry = migration_test_entry(&path, show_id, "Damaged Legacy Show");
    let engine = Engine::new(ProgrammerRegistry::default());

    let error = compile_active_show_for_startup(&engine, &entry, &data_dir, 5)
        .expect("invalid show should enter recovery mode");
    assert!(error.contains("might be corrupted or incompatible"));
    assert!(error.contains("Damaged Legacy Show"));
    assert!(engine.snapshot().fixtures.is_empty());
    let store = ShowStore::open(&path).unwrap();
    assert_eq!(store.portable_revision().unwrap().value(), source_revision);
    let document = store.portable_document().unwrap();
    assert_eq!(
        document
            .object("patched_fixture", &object.id)
            .unwrap()
            .body(),
        &damaged
    );
    assert_eq!(document.fixture_profile_revisions().len(), source_profiles);
    assert!(migration_backup_files(&data_dir).is_empty());
    drop(store);
    std::fs::remove_dir_all(data_dir).unwrap();
}

#[test]
fn missing_active_show_enters_recovery_instead_of_aborting_startup() {
    let data_dir = std::env::temp_dir().join(format!("light-missing-show-{}", Uuid::new_v4()));
    let engine = Engine::new(ProgrammerRegistry::default());
    let path = data_dir.join("missing.show");
    let entry = migration_test_entry(&path, light_core::ShowId::new(), "Damaged Show");
    let error = compile_active_show_for_startup(&engine, &entry, &data_dir, 5)
        .expect("invalid show should enter recovery mode");
    assert!(error.contains("might be corrupted or incompatible"));
    assert!(error.contains("Damaged Show"));
    assert!(engine.snapshot().fixtures.is_empty());
}
