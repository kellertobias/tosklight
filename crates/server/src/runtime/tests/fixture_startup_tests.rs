#[test]
fn startup_fixture_library_migrates_schema_v1_and_loads_transferable_packages_once() {
    let data_dir = std::env::temp_dir().join(format!(
        "light-startup-fixture-migration-{}",
        Uuid::new_v4()
    ));
    let family = format!("Startup family {}", Uuid::new_v4());
    let rows = schema_v1_dimmer_rows("Startup Legacy", &family);
    assert_eq!(rows.len(), 2);
    let expected_profile_id = rows[0].0.id;
    seed_schema_v1_fixture_database(&data_dir, &rows);

    let package_dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/fixture-library");
    let package_count = std::fs::read_dir(&package_dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "toskfixture")
        })
        .count();
    let library = open_fixture_library_for_startup(&data_dir, Some(&package_dir)).unwrap();
    let profiles = library.profiles().unwrap();
    let migrated = profiles
        .iter()
        .find(|profile| profile.id == expected_profile_id)
        .unwrap();
    assert_eq!(migrated.schema_version, 2);
    assert_eq!(migrated.revision, 1);
    assert_eq!(migrated.manufacturer, "Startup Legacy");
    assert_eq!(migrated.name, family);
    assert_eq!(
        migrated
            .modes
            .iter()
            .map(|mode| mode.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Coarse", "Fine"]
    );
    assert!(
        profiles
            .iter()
            .all(|profile| profile.reserved_source.is_none())
    );
    let vendor_profiles = profiles
        .iter()
        .filter(|profile| profile.manufacturer == "ROBE")
        .collect::<Vec<_>>();
    assert_eq!(profiles.len(), package_count + 1);
    assert_eq!(vendor_profiles.len(), 5);
    assert!(vendor_profiles.iter().any(|profile| {
        profile.name == "Robin 600X LEDWash"
            && profile
                .modes
                .iter()
                .map(|mode| mode.splits[0].footprint)
                .collect::<Vec<_>>()
                == vec![37, 21, 15, 10, 37, 25]
    }));
    let legacy_sources = library
        .profile_legacy_sources(expected_profile_id, 1)
        .unwrap();
    assert_eq!(legacy_sources.len(), 2);
    for (definition, source) in &rows {
        let expected_json = serde_json::to_string(definition).unwrap();
        assert_eq!(
            library.export_json(definition.id, 1).unwrap().as_deref(),
            Some(expected_json.as_str())
        );
        assert_eq!(
            library.source_gdtf(definition.id, 1).unwrap().as_deref(),
            Some(source.as_slice())
        );
    }
    let initial = serde_json::to_value(&profiles).unwrap();
    drop(library);

    let reopened = open_fixture_library_for_startup(&data_dir, Some(&package_dir)).unwrap();
    assert_eq!(
        serde_json::to_value(reopened.profiles().unwrap()).unwrap(),
        initial
    );
    assert!(reopened.migration_warnings().unwrap().is_empty());
    drop(reopened);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn startup_fixture_library_keeps_malformed_and_conflicting_schema_v1_evidence() {
    let data_dir =
        std::env::temp_dir().join(format!("light-startup-fixture-recovery-{}", Uuid::new_v4()));
    let family = format!("Conflict family {}", Uuid::new_v4());
    let mut rows = schema_v1_dimmer_rows("Startup Recovery", &family);
    rows[1].0.physical.width_millimetres = Some(500.0);
    seed_schema_v1_fixture_database(&data_dir, &rows);
    let malformed_id = light_core::FixtureId::new();
    let malformed_source = b"retained-malformed-startup-gdtf";
    let connection = rusqlite::Connection::open(data_dir.join("fixtures.sqlite")).unwrap();
    connection.execute(
            "INSERT INTO fixture_definitions(id,revision,manufacturer,model,mode,definition_json,source_gdtf) VALUES(?1,1,'Broken','Broken','Broken','{',?2)",
            rusqlite::params![malformed_id.0.to_string(), malformed_source.as_slice()],
        ).unwrap();
    drop(connection);

    let library = open_fixture_library_for_startup(&data_dir, None).unwrap();
    let warnings = library.migration_warnings().unwrap();
    assert!(warnings.iter().any(|warning| {
        warning.contains(&malformed_id.0.to_string())
            && warning.contains("could not be migrated")
            && warning.contains("original definition and GDTF source were retained")
    }));
    assert!(warnings.iter().any(|warning| {
        warning.contains("Startup Recovery")
            && warning.contains(&family)
            && warning.contains("conflicting fixture-level metadata")
            && warning.contains("retained as separate profiles")
    }));
    assert_eq!(
        library.export_json(malformed_id, 1).unwrap().as_deref(),
        Some("{")
    );
    assert_eq!(
        library.source_gdtf(malformed_id, 1).unwrap().as_deref(),
        Some(malformed_source.as_slice())
    );
    for (definition, source) in &rows {
        let expected_json = serde_json::to_string(definition).unwrap();
        assert_eq!(
            library.export_json(definition.id, 1).unwrap().as_deref(),
            Some(expected_json.as_str())
        );
        assert_eq!(
            library.source_gdtf(definition.id, 1).unwrap().as_deref(),
            Some(source.as_slice())
        );
    }
    let profiles = serde_json::to_value(library.profiles().unwrap()).unwrap();
    drop(library);

    let reopened = open_fixture_library_for_startup(&data_dir, None).unwrap();
    assert_eq!(reopened.migration_warnings().unwrap(), warnings);
    assert_eq!(
        serde_json::to_value(reopened.profiles().unwrap()).unwrap(),
        profiles
    );
    assert_eq!(
        reopened.export_json(malformed_id, 1).unwrap().as_deref(),
        Some("{")
    );
    assert_eq!(
        reopened.source_gdtf(malformed_id, 1).unwrap().as_deref(),
        Some(malformed_source.as_slice())
    );
    drop(reopened);
    let _ = std::fs::remove_dir_all(data_dir);
}
