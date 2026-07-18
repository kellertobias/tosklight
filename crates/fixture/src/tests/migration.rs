use super::*;

#[test]
fn embedded_legacy_patch_migrates_to_portable_profile_and_explicit_split_assignments() {
    let mut legacy = definition(4);
    legacy.revision = 7;
    let intensity = Parameter {
        attribute: AttributeKey::intensity(),
        components: vec![ChannelComponent {
            offset: 0,
            byte_order: ByteOrder::MsbFirst,
        }],
        default: 0.0,
        virtual_dimmer: false,
        metadata: ParameterMetadata::default(),
        capabilities: vec![Capability {
            name: "Open".into(),
            dmx_from: 1,
            dmx_to: 255,
            preset_family: Some("beam".into()),
        }],
    };
    let emitter_parameter = |name: &str, offset| Parameter {
        attribute: AttributeKey(format!("color.emitter.{name}")),
        components: vec![ChannelComponent {
            offset,
            byte_order: ByteOrder::MsbFirst,
        }],
        default: 0.0,
        virtual_dimmer: false,
        metadata: ParameterMetadata::default(),
        capabilities: vec![],
    };
    legacy.heads[0].parameters = vec![
        intensity,
        emitter_parameter("red", 1),
        emitter_parameter("green", 2),
        emitter_parameter("blue", 3),
    ];
    legacy.color_calibration = Some(ColorCalibration {
        emitters: vec![
            EmitterCalibration {
                name: "red".into(),
                xyz: Xyz {
                    x: 1.0,
                    y: 0.0,
                    z: 0.0,
                },
                limit: 0.8,
            },
            EmitterCalibration {
                name: "green".into(),
                xyz: Xyz {
                    x: 0.0,
                    y: 1.0,
                    z: 0.0,
                },
                limit: 0.9,
            },
            EmitterCalibration {
                name: "blue".into(),
                xyz: Xyz {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                },
                limit: 1.0,
            },
        ],
        correction_matrix: [[0.9, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.1]],
    });
    let instance_id = Uuid::new_v4();
    let mut fixture = PatchedFixture {
        fixture_id: FixtureId::new(),
        fixture_number: Some(1),
        virtual_fixture_number: None,
        name: "Legacy".into(),
        definition: legacy,
        universe: Some(2),
        address: Some(101),
        split_patches: vec![],
        layer_id: default_patch_layer(),
        direct_control: None,
        location: Default::default(),
        rotation: Default::default(),
        logical_heads: vec![],
        multipatch: vec![MultiPatchInstance {
            id: instance_id,
            name: "Balcony".into(),
            universe: Some(3),
            address: Some(201),
            split_patches: vec![],
            location: Default::default(),
            rotation: Default::default(),
        }],
        move_in_black_enabled: true,
        move_in_black_delay_millis: 0,
        highlight_overrides: BTreeMap::new(),
    };

    assert!(migrate_patched_fixture_to_v2(&mut fixture).unwrap());
    assert_eq!(
        fixture.definition.schema_version,
        FIXTURE_PROFILE_SCHEMA_VERSION
    );
    assert_eq!(fixture.definition.revision, 7);
    assert!(fixture.definition.profile_snapshot.is_some());
    let migrated_mode = &fixture.definition.profile_snapshot.as_ref().unwrap().modes[0];
    let ColorSystem::Additive { emitters } = &migrated_mode.color_systems[0].system else {
        panic!("legacy additive calibration was not converted")
    };
    assert_eq!(emitters.len(), 3);
    assert_eq!(emitters[0].maximum_level, 0.8);
    assert_eq!(emitters[0].response_curve, 1.0);
    assert!(emitters.iter().all(|emitter| emitter.visible));
    assert_eq!(migrated_mode.color_systems[0].correction_matrix[0][0], 0.9);
    assert_eq!(
        fixture.definition.heads[0].parameters[0].capabilities[0].name,
        "Open"
    );
    assert_eq!(
        fixture.split_patches,
        vec![SplitPatch {
            split: 1,
            universe: Some(2),
            address: Some(101),
        }]
    );
    assert_eq!(
        fixture.multipatch[0].split_patches,
        vec![SplitPatch {
            split: 1,
            universe: Some(3),
            address: Some(201),
        }]
    );
    assert!(!migrate_patched_fixture_to_v2(&mut fixture).unwrap());
}

#[test]
fn legacy_library_migration_combines_compatible_modes_and_retains_sources() {
    let path = std::env::temp_dir().join(format!(
        "fixture-profile-migration-{}.sqlite",
        Uuid::new_v4()
    ));
    let connection = Connection::open(&path).unwrap();
    connection.execute_batch("CREATE TABLE fixture_definitions(id TEXT NOT NULL,revision INTEGER NOT NULL,manufacturer TEXT NOT NULL,model TEXT NOT NULL,mode TEXT NOT NULL,definition_json TEXT NOT NULL,source_gdtf BLOB,PRIMARY KEY(id,revision));").unwrap();
    let mut coarse = definition(1);
    coarse.manufacturer = "Acme".into();
    coarse.model = "Orbit".into();
    coarse.name = "Orbit".into();
    coarse.mode = "Coarse".into();
    let mut fine = coarse.clone();
    fine.id = FixtureId::new();
    fine.mode = "Fine".into();
    for fixture in [&coarse, &fine] {
        connection.execute(
            "INSERT INTO fixture_definitions(id,revision,manufacturer,model,mode,definition_json,source_gdtf) VALUES(?1,1,?2,?3,?4,?5,?6)",
            params![fixture.id.0.to_string(), fixture.manufacturer, fixture.model, fixture.mode, serde_json::to_string(fixture).unwrap(), b"retained-gdtf".as_slice()],
        ).unwrap();
    }
    drop(connection);
    let library = FixtureLibrary::open(&path).unwrap();
    let profiles = library.profiles().unwrap();
    assert_eq!(profiles.len(), 1);
    assert_eq!(
        profiles[0]
            .modes
            .iter()
            .map(|mode| mode.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Coarse", "Fine"]
    );
    let sources = library.profile_legacy_sources(profiles[0].id, 1).unwrap();
    assert_eq!(sources.len(), 2);
    assert!(
        sources
            .iter()
            .all(|(_, json, source)| json.contains("Orbit")
                && source.as_deref() == Some(b"retained-gdtf".as_slice()))
    );
    let _ = std::fs::remove_file(path);
}
