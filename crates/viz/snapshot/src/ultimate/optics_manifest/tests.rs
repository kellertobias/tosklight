use super::*;
use glam::{Vec2, Vec3};
use viz_scene::{
    EmitterInstance, EmitterKind, EmitterLayoutCells, EmitterOptics, FixtureInstance, GoboSlot,
};
fn open_gate() -> Gate {
    Gate {
        cos_outer: 0.9,
        params: [0.95, 0.1, 1.0, 0.0],
        frost: 0.0,
        shapers: [0.0; 4],
        angles: [0.0; 4],
        shaper_params: [0.0; 4],
        gobos: [[0.0, 0.0, -1.0, 0.0]; 8],
        prisms: [[0.0; 4]; 8],
        gobo_count: 1,
        prism_count: 1,
    }
}
#[test]
fn actual_asymmetric_artwork_rotates_and_wheels_mask_in_series() {
    let artwork = vec![GoboArtwork {
        edge: 2,
        mask: vec![0, 255, 0, 255],
    }];
    let mut gate = open_gate();
    gate.gobos[0] = [1.0, 0.0, 0.0, 0.0];
    let right = Vec2::new(0.5, 0.0);
    let left = Vec2::new(-0.5, 0.0);
    assert!(gate.transmission(right, &artwork) > 0.9);
    assert_eq!(gate.transmission(left, &artwork), 0.0);
    gate.gobos[0][1] = std::f32::consts::PI;
    assert!(gate.transmission(right, &artwork) < 1e-5);
    assert!(gate.transmission(left, &artwork) > 0.9);
    gate.gobo_count = 2;
    gate.gobos[1] = [1.0, 0.0, 0.0, 0.0];
    assert!(gate.transmission(right, &artwork) < 1e-5);
    assert!(gate.transmission(left, &artwork) < 1e-5);
}
#[test]
fn prism_copy_centres_blade_clip_and_frost_follow_native_gate_constants() {
    let mut gate = open_gate();
    gate.prisms[0] = [3.0, 0.0, 1.0, 0.6];
    assert!(gate.transmission(Vec2::ZERO, &[]) > 0.9);
    assert!(gate.transmission(Vec2::new(0.6, 0.0), &[]) > 0.9);
    assert_eq!(gate.transmission(Vec2::new(0.3, 0.0), &[]), 0.0);
    gate.frost = 1.0;
    assert!(gate.transmission(Vec2::new(0.3, 0.0), &[]) > 0.8);
    gate.prisms[0] = [0.0; 4];
    gate.frost = 0.0;
    gate.shapers[0] = 1.0;
    assert!(gate.transmission(Vec2::new(-0.3, 0.0), &[]) > 0.9);
    assert_eq!(gate.transmission(Vec2::new(0.3, 0.0), &[]), 0.0);
    gate.angles[0] = std::f32::consts::FRAC_PI_2;
    assert!(gate.transmission(Vec2::new(0.0, 0.3), &[]) > 0.9);
    assert_eq!(gate.transmission(Vec2::new(0.0, -0.3), &[]), 0.0);
}
fn rig() -> (Scene, SceneValues) {
    let mut scene = Scene::default();
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../assets/models/lamps/par-64-long-nose-silver.glb");
    scene
        .models
        .push(viz_scene::read_glb(&std::fs::read(path).unwrap()).unwrap());
    scene.fixtures.push(FixtureInstance {
        lamp_response: Default::default(),
        name: "Real model cells".into(),
        model: Some(0),
        position: Vec3::new(1.0, 5.0, 2.0),
        ..Default::default()
    });
    let emitter = EmitterInstance {
        fixture_index: 0,
        head_index: 0,
        label: "First head".into(),
        local_origin: Vec3::new(0.0, 0.0, -0.2),
        tilt_pivot: Vec3::ZERO,
        local_orientation_degrees: Vec3::ZERO,
        pan: None,
        tilt: None,
        beam_angle_degrees: 12.0,
        field_angle_degrees: 36.0,
        optics: EmitterOptics::default(),
        kind: EmitterKind::Beam,
        cells: EmitterLayoutCells {
            offsets: vec![Vec3::new(-0.1, 0.0, 0.0), Vec3::new(0.1, 0.0, 0.0)],
            ..EmitterLayoutCells::single()
        },
        laser: None,
        effect: None,
        live_shaper_angle_roles: [true; 4],
        shaper_roles: [true; 4],
        live_shaper_rotation_role: true,
    };
    scene.emitters.push(emitter.clone());
    let mut second = emitter;
    second.head_index = 1;
    second.local_origin.y = 0.2;
    scene.emitters.push(second);
    scene.gobo_artwork.push(GoboArtwork {
        edge: 2,
        mask: vec![0, 255, 0, 255],
    });
    for emitter in &mut scene.emitters {
        emitter.optics.gobo_wheel = vec![
            GoboSlot::default(),
            GoboSlot {
                artwork: Some(0),
                name: "Real asymmetric artwork".into(),
            },
        ];
    }
    scene.recompute_bounds();
    let mut values = SceneValues::default();
    values.resize(2);
    for value in &mut values.emitters {
        value.intensity = 1.0;
        value.colour = [0.2, 0.4, 0.8];
        value.gobo = 1.0;
        value.gobo_rotation = 0.25;
        value.shaper_blades[0] = 0.25;
        value.shaper_rotation_degrees = 30.0;
        value.shaper_blade_angles_degrees[0] = 20.0;
    }
    (scene, values)
}
#[test]
fn real_glb_multi_head_cell_origins_and_optical_transforms_use_native_frame() {
    let (scene, values) = rig();
    let manifest = OpticsManifest::build(&scene, &values, &ViewConfiguration::default(), 12.5);
    assert!(manifest.reasons().is_empty(), "{:?}", manifest.reasons());
    let native = viz_render::semantic_lights(&scene, &values);
    assert_eq!(manifest.lights.len(), 4);
    for (light, native) in manifest.lights.iter().zip(native) {
        assert_eq!(light.origin, crate::to_z_up(native.origin.to_array()));
        assert_eq!(light.direction, crate::to_z_up(native.direction.to_array()));
        assert_eq!(light.emitter_index, native.emitter_index);
        assert_eq!(light.cell_index, native.cell_index);
        assert_eq!(light.colour, native.colour.to_array());
        assert_eq!(light.cone_radians, native.outer_half_angle * 2.0);
    }
    assert_ne!(manifest.lights[0].origin, manifest.lights[1].origin);
    assert_eq!(
        manifest.masks.len(),
        1,
        "identical optics share an angular image across heads/cells"
    );
    assert!((manifest.masks[0].gobos[0][1] - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
    assert!((manifest.masks[0].angles[0] - 20.0_f32.to_radians()).abs() < 1e-6);
    assert_eq!(manifest.asset_files(), vec!["optics-mask-0000.png"]);
}
#[test]
fn malformed_active_native_artwork_refuses_instead_of_substituting_an_open_gobo() {
    let (mut scene, values) = rig();
    scene.gobo_artwork[0].mask.clear();
    let manifest = OpticsManifest::build(&scene, &values, &ViewConfiguration::default(), 1.0);
    assert!(
        manifest
            .reasons()
            .iter()
            .any(|reason| reason.contains("invalid dimensions"))
    );
    assert!(manifest.lights.is_empty());
}
#[test]
fn cancelled_optical_asset_job_does_not_open_or_create_output_files() {
    let (scene, values) = rig();
    let manifest = OpticsManifest::build(&scene, &values, &ViewConfiguration::default(), 1.0);
    let cancelled = AtomicBool::new(true);
    assert_eq!(
        manifest.write_assets_with_cancel(Path::new("unused-owned-optics-directory"), &cancelled),
        Err(UltimateError::Cancelled)
    );
}

#[path = "shaper_tests.rs"]
mod shaper_tests;
