use super::*;

fn backups(path: &std::path::Path) -> Vec<PathBuf> {
    let prefix = format!(
        "{}.pre-canonical-",
        path.file_name().unwrap().to_string_lossy()
    );
    std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with(&prefix))
        .map(|entry| entry.path())
        .collect()
}

#[test]
fn legacy_open_migrates_once_before_first_numeric_pose_write_and_retains_recovery() {
    let rig = rig("canonical-open");
    let mut command = patch_one(rig.document.show_id(), rig.profile.clone());
    command.fixtures[0].patch.location.x = -2500;
    rig.document.patch_fixtures(command.clone()).unwrap();
    let store = ShowStore::open(&rig.path).unwrap();
    let legacy =
        serde_json::json!({"family":"Color","name":"Legacy color","future_note":{"kept":true}});
    store.put_object("preset", "2.100", &legacy, 0).unwrap();
    let opaque = serde_json::json!({"vendor":"untouched","position":17});
    store
        .put_object("future_vendor", "opaque", &opaque, 0)
        .unwrap();
    store
        .set_metadata_values(&[("light.programming_contract", "3")])
        .unwrap();
    let before = store.portable_document().unwrap();
    drop(store);
    let opened = PlanningDocument::open(&rig.path).unwrap();
    assert_eq!(opened.show_id(), rig.document.show_id());
    assert_eq!(
        ShowStore::open(&rig.path)
            .unwrap()
            .metadata_value("light.programming_contract")
            .unwrap()
            .as_deref(),
        Some("3")
    );
    let migrated = opened.objects("preset").unwrap().remove(0).body;
    assert!(migrated["instance_id"].as_str().is_some());
    assert_eq!(migrated["future_note"], legacy["future_note"]);
    assert_eq!(opened.objects("future_vendor").unwrap()[0].body, opaque);
    let recovery = backups(&rig.path);
    assert_eq!(recovery.len(), 1);
    let recovered = ShowStore::open(&recovery[0])
        .unwrap()
        .portable_document()
        .unwrap();
    assert_eq!(recovered.revision(), before.revision());
    assert_eq!(recovered.object("preset", "2.100").unwrap().body(), &legacy);
    command.fixtures[0].patch.location.x = -3000;
    opened
        .patch_fixtures(command)
        .expect("first numeric pose patch is canonical");
    assert_eq!(
        opened.patch_snapshot().unwrap().fixtures[0]
            .patch
            .location
            .x,
        -3000
    );
    let revision = opened.portable_revision().unwrap();
    drop(opened);
    let reopened = PlanningDocument::open(&rig.path).unwrap();
    assert_eq!(reopened.portable_revision().unwrap(), revision);
    assert_eq!(reopened.objects("preset").unwrap()[0].body, migrated);
    assert_eq!(backups(&rig.path), recovery);
    for path in recovery {
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn rejected_canonical_migration_leaves_all_portable_objects_and_revision_unchanged() {
    let rig = rig("canonical-invalid");
    let store = ShowStore::open(&rig.path).unwrap();
    store
        .put_object("preset", "2.1", &serde_json::json!({"family":"Color"}), 0)
        .unwrap();
    store
        .put_object("preset", "2.2", &serde_json::json!({"family":"invalid"}), 0)
        .unwrap();
    let before = store.portable_document().unwrap();
    drop(store);
    assert!(PlanningDocument::open(&rig.path).is_err());
    let after = ShowStore::open(&rig.path)
        .unwrap()
        .portable_document()
        .unwrap();
    assert_eq!(after.revision(), before.revision());
    for object in before.objects() {
        assert_eq!(
            after
                .object(object.key().kind(), object.key().id())
                .unwrap()
                .body(),
            object.body()
        );
    }
    assert!(backups(&rig.path).is_empty());
    assert!(
        rig.document.patch_snapshot().is_ok(),
        "previous document remains available"
    );
}

#[test]
fn future_contract_refused_before_open_without_rewriting_source() {
    let rig = rig("canonical-future");
    let store = ShowStore::open(&rig.path).unwrap();
    store
        .set_metadata_values(&[("light.programming_contract", "65535")])
        .unwrap();
    drop(store);
    let original = std::fs::read(&rig.path).unwrap();
    assert!(
        PlanningDocument::open(&rig.path)
            .err()
            .unwrap()
            .to_string()
            .contains("65535")
    );
    assert_eq!(std::fs::read(&rig.path).unwrap(), original);
    assert!(backups(&rig.path).is_empty());
}

#[test]
fn missing_show_open_does_not_create_a_database() {
    let path = temp_path("canonical-missing");
    assert!(PlanningDocument::open(&path).is_err());
    assert!(!path.exists());
}

#[test]
fn legacy_semantic_programming_refused_before_open_without_rewriting_source() {
    let rig = rig("canonical-old-semantic");
    let store = ShowStore::open(&rig.path).unwrap();
    let fixture = FixtureId::new().0.to_string();
    let body = serde_json::json!({"family":"Position","values":{fixture:{"pan":{"kind":"normalized","value":0.5}}}});
    store.put_object("preset", "3.1", &body, 0).unwrap();
    drop(store);
    let original = std::fs::read(&rig.path).unwrap();
    let error = PlanningDocument::open(&rig.path).err().unwrap().to_string();
    assert!(error.contains("pan"), "{error}");
    assert_eq!(std::fs::read(&rig.path).unwrap(), original);
    assert!(backups(&rig.path).is_empty());
}
