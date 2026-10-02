use super::*;
use light_core::FixtureId;
use light_engine::EngineSnapshot;
use light_fixture::PatchedFixture;
use light_psn_wire::{PsnTrackerData, PsnVector3, encode_data_frame};

fn point() -> PatchedFixture {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../assets/fixture-library/tosklight--3d-point.toskfixture");
    let profile = light_fixture::read_fixture_package(&std::fs::read(path).unwrap()).unwrap();
    PatchedFixture {
        fixture_id: FixtureId::new(),
        definition: profile.resolved_definition(profile.modes[0].id).unwrap(),
        name: "Point".into(),
        layer_id: "default".into(),
        model_scale: None,
        scenery_options: Default::default(),
        scenery_size_metres: None,
        note: None,
        position_master: None,
        fixture_number: None,
        virtual_fixture_number: None,
        universe: None,
        address: None,
        split_patches: vec![],
        direct_control: None,
        internal_bindings: Default::default(),
        location: Default::default(),
        rotation: Default::default(),
        logical_heads: vec![],
        group_masters_enabled: true,
        grand_master_enabled: true,
        invert_pan: false,
        invert_tilt: false,
        position_calibration: None,
        color_calibration: None,
        bracket_angle: 0.0,
        shaper_angle: None,
        installed_appearance: Default::default(),
        move_in_black_enabled: true,
        highlight_overrides: Default::default(),
        freeze: Default::default(),
        move_in_black_delay_millis: 0,
        multipatch: vec![],
    }
}

#[test]
fn point_location_dependencies_refresh_for_patch_changes_but_not_frames_or_unrelated_edits() {
    let point = point();
    let point_id = point.fixture_id;
    let resource = super::super::service::PsnResource::new();
    resource.install(PsnConfiguration {
        enabled: true,
        bindings: vec![super::super::config::PsnBinding {
            id: uuid::Uuid::new_v4(),
            tracker_id: 1,
            point_fixture_id: point_id.0,
            enabled: true,
        }],
        ..Default::default()
    });
    let mut snapshot = EngineSnapshot {
        fixtures: vec![point].into(),
        ..Default::default()
    };
    let mut cache = PointLocationCache::default();
    assert!(cache.refresh(&resource, &snapshot));
    for sample in 1..=100 {
        for bytes in encode_data_frame(
            sample,
            sample as u8,
            &[PsnTrackerData {
                id: 1,
                position: Some(PsnVector3 {
                    x: 10.0,
                    y: 0.0,
                    z: 0.0,
                }),
                ..Default::default()
            }],
        ) {
            resource.observe("127.0.0.1:56565".parse().unwrap(), &bytes, sample);
        }
        assert!(!cache.refresh(&resource, &snapshot));
        let frame = resource.tick(sample);
        assert_eq!(frame.status.placements[0].position_metres[0], 10.0);
        assert_eq!(frame.overrides[0].value.normalized(), Some(0.55));
    }
    snapshot.revision += 1;
    assert!(!cache.refresh(&resource, &snapshot));
    Arc::make_mut(&mut snapshot.fixtures)[0].location.x = 5_000;
    assert!(cache.refresh(&resource, &snapshot));
    let moved_mount = resource.tick(101);
    assert_eq!(moved_mount.overrides[0].value.normalized(), Some(0.525));
    assert_eq!(moved_mount.status.placements[0].position_metres[0], 10.0);
    Arc::make_mut(&mut snapshot.fixtures).clear();
    assert!(cache.refresh(&resource, &snapshot));
    assert!(resource.tick(102).overrides.is_empty());
}
