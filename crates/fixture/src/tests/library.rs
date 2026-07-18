use super::*;

#[test]
fn fixture_json_round_trips_through_library() {
    let path =
        std::env::temp_dir().join(format!("fixture-library-{}.sqlite", uuid::Uuid::new_v4()));
    let library = FixtureLibrary::open(&path).unwrap();
    let fixture = definition(1);
    let json = serde_json::to_string(&fixture).unwrap();
    library.import_json(&json).unwrap();
    assert_eq!(library.export_json(fixture.id, 1).unwrap().unwrap(), json);
    let profiles = library.profiles().unwrap();
    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].modes[0].name, "Mode");
    let _ = std::fs::remove_file(path);
}

#[test]
fn profile_revisions_are_atomic_and_server_assigned() {
    let path = std::env::temp_dir().join(format!("fixture-profiles-{}.sqlite", Uuid::new_v4()));
    let library = FixtureLibrary::open(&path).unwrap();
    let mut draft = FixtureProfile::blank();
    draft.manufacturer = "Acme".into();
    draft.name = "Orbit".into();
    draft.short_name = "Orbit".into();
    let first = library.save_profile(draft, 0).unwrap();
    assert_eq!(first.revision, 1);
    assert!(
        library
            .set_profile_source_gdtf(first.id, 1, b"original-gdtf-archive")
            .unwrap()
    );
    assert_eq!(
        library.profile_source_gdtf(first.id, 1).unwrap().as_deref(),
        Some(b"original-gdtf-archive".as_slice())
    );
    let mut edit = first.clone();
    edit.notes = "Second revision".into();
    let second = library.save_profile(edit, 1).unwrap();
    assert_eq!(second.revision, 2);
    assert_eq!(
        library.profile_source_gdtf(first.id, 2).unwrap().as_deref(),
        Some(b"original-gdtf-archive".as_slice()),
        "new immutable revisions retain the original import archive"
    );
    assert_eq!(library.profile(first.id, 1).unwrap().unwrap().notes, "");
    assert_eq!(
        library.profile(first.id, 2).unwrap().unwrap().notes,
        "Second revision"
    );
    assert!(matches!(
        library.save_profile(second, 1),
        Err(FixtureError::RevisionConflict {
            expected: 1,
            current: 2
        })
    ));
    let _ = std::fs::remove_file(path);
}

#[test]
fn failed_or_conflicting_legacy_migration_keeps_startup_available() {
    let path = std::env::temp_dir().join(format!(
        "fixture-profile-recovery-{}.sqlite",
        Uuid::new_v4()
    ));
    let connection = Connection::open(&path).unwrap();
    connection.execute_batch("CREATE TABLE fixture_definitions(id TEXT NOT NULL,revision INTEGER NOT NULL,manufacturer TEXT NOT NULL,model TEXT NOT NULL,mode TEXT NOT NULL,definition_json TEXT NOT NULL,source_gdtf BLOB,PRIMARY KEY(id,revision));").unwrap();
    let mut first = definition(1);
    first.manufacturer = "Acme".into();
    first.model = "Conflict".into();
    first.name = "Conflict".into();
    let mut second = first.clone();
    second.id = FixtureId::new();
    second.mode = "Different metadata".into();
    second.physical.width_millimetres = Some(500.0);
    for fixture in [&first, &second] {
        connection.execute(
            "INSERT INTO fixture_definitions(id,revision,manufacturer,model,mode,definition_json) VALUES(?1,1,?2,?3,?4,?5)",
            params![fixture.id.0.to_string(), fixture.manufacturer, fixture.model, fixture.mode, serde_json::to_string(fixture).unwrap()],
        ).unwrap();
    }
    let invalid_id = FixtureId::new();
    connection.execute(
        "INSERT INTO fixture_definitions(id,revision,manufacturer,model,mode,definition_json,source_gdtf) VALUES(?1,1,'Broken','Broken','Broken','{',?2)",
        params![invalid_id.0.to_string(), b"original-source".as_slice()],
    ).unwrap();
    drop(connection);
    let library = FixtureLibrary::open(&path).unwrap();
    assert_eq!(library.profiles().unwrap().len(), 2);
    let warnings = library.migration_warnings().unwrap();
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("conflicting fixture-level metadata"))
    );
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("could not be migrated"))
    );
    assert_eq!(
        library.export_json(invalid_id, 1).unwrap().as_deref(),
        Some("{")
    );
    assert_eq!(
        library.source_gdtf(invalid_id, 1).unwrap().as_deref(),
        Some(b"original-source".as_slice())
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn transferable_package_startup_is_idempotent_updates_clean_installs_and_preserves_edits() {
    let root = std::env::temp_dir().join(format!("fixture-package-startup-{}", Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    let package_path = root.join("test-lamp.toskfixture");
    let database_path = root.join("fixtures.sqlite");
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "Transfer Lamp".into();
    profile.short_name = "Transfer".into();
    fs::write(&package_path, write_fixture_package(&profile).unwrap()).unwrap();

    let library = FixtureLibrary::open(&database_path).unwrap();
    assert_eq!(
        library.load_fixture_package_directory(&root).unwrap(),
        FixturePackageLoadReport {
            installed: 1,
            ..FixturePackageLoadReport::default()
        }
    );
    assert_eq!(
        library.load_fixture_package_directory(&root).unwrap(),
        FixturePackageLoadReport {
            unchanged: 1,
            ..FixturePackageLoadReport::default()
        }
    );
    let installed = library.profiles().unwrap().remove(0);
    assert_eq!(installed.revision, 1);
    assert!(installed.reserved_source.is_none());

    profile.notes = "package update".into();
    fs::write(&package_path, write_fixture_package(&profile).unwrap()).unwrap();
    assert_eq!(
        library
            .load_fixture_package_directory(&root)
            .unwrap()
            .updated,
        1
    );
    let mut operator = library.profiles().unwrap().remove(0);
    assert_eq!(operator.revision, 2);
    operator.notes = "operator edit".into();
    let operator = library.save_profile(operator, 2).unwrap();
    profile.notes = "later package update".into();
    fs::write(&package_path, write_fixture_package(&profile).unwrap()).unwrap();
    assert_eq!(
        library
            .load_fixture_package_directory(&root)
            .unwrap()
            .preserved_operator_revisions,
        1
    );
    assert_eq!(library.profiles().unwrap().remove(0).notes, operator.notes);

    let exported = library
        .export_fixture_package(operator.id, operator.revision)
        .unwrap()
        .unwrap();
    let second = FixtureLibrary::open(root.join("second.sqlite")).unwrap();
    let imported = second.import_fixture_package(&exported).unwrap();
    assert_eq!(imported.id, operator.id);
    assert_eq!(imported.notes, "operator edit");
    assert!(imported.reserved_source.is_none());
    drop(second);
    drop(library);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn transferable_package_retires_matching_unmarked_legacy_generic_rows() {
    let root =
        std::env::temp_dir().join(format!("fixture-package-legacy-catalog-{}", Uuid::new_v4()));
    fs::create_dir_all(&root).unwrap();
    let database_path = root.join("fixtures.sqlite");
    let connection = Connection::open(&database_path).unwrap();
    connection.execute_batch("CREATE TABLE fixture_definitions(id TEXT NOT NULL,revision INTEGER NOT NULL,manufacturer TEXT NOT NULL,model TEXT NOT NULL,mode TEXT NOT NULL,definition_json TEXT NOT NULL,source_gdtf BLOB,PRIMARY KEY(id,revision));").unwrap();
    let mut legacy = definition(1);
    legacy.manufacturer = "Generic".into();
    legacy.name = "Legacy Package Lamp".into();
    legacy.model = legacy.name.clone();
    connection.execute(
        "INSERT INTO fixture_definitions(id,revision,manufacturer,model,mode,definition_json) VALUES(?1,1,?2,?3,?4,?5)",
        params![legacy.id.0.to_string(), legacy.manufacturer, legacy.model, legacy.mode, serde_json::to_string(&legacy).unwrap()],
    ).unwrap();
    drop(connection);

    let library = FixtureLibrary::open(&database_path).unwrap();
    let mut packaged = library.profiles().unwrap().remove(0);
    assert!(packaged.reserved_source.is_none());
    packaged.notes = "now supplied as a package".into();
    fs::write(
        root.join("generic--legacy-package-lamp.toskfixture"),
        write_fixture_package(&packaged).unwrap(),
    )
    .unwrap();
    assert_eq!(
        library
            .load_fixture_package_directory(&root)
            .unwrap()
            .updated,
        1
    );
    assert!(library.definitions().unwrap().is_empty());
    assert!(
        library
            .profile_legacy_sources(packaged.id, 1)
            .unwrap()
            .is_empty()
    );
    drop(library);
    let _ = fs::remove_dir_all(root);
}
