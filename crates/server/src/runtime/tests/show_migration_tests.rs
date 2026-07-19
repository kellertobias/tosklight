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
