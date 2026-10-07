use super::*;
use crate::binding::ChannelRef;
use crate::plan::ColourBinding;
use glam::Vec3;
use light_core::AttributeKey;
use light_fixture::{AngularMotion, AngularMotionKind, ChannelFunction, ChannelFunctionBehavior};
use viz_scene::{EmitterInstance, EmitterLayoutCells, EmitterOptics, FixtureBody, FixtureInstance};

pub(super) fn channel(slot: u16) -> ChannelRef {
    ChannelRef {
        logical_universe: 1,
        slots: vec![slot],
        max_raw: 255,
        invert: false,
        physical_min: 0.0,
        physical_max: 1.0,
        physical_unit: None,
        snap: false,
        default_raw: 0,
        functions: Vec::new(),
    }
}

#[test]
fn independent_optical_wheels_keep_selection_and_position_velocity_rotation() {
    use crate::plan::OpticalWheelBinding;
    let rotation = |slot, kind, max| {
        let mut channel = channel(slot);
        channel.functions = vec![ChannelFunction {
            id: uuid::Uuid::new_v4(),
            name: "rotation".into(),
            dmx_from: 0,
            dmx_to: 255,
            attribute: AttributeKey("gobo.2.rotation".into()),
            priority: 0,
            physical_mapping: None,
            angular_motion: Some(AngularMotion {
                kind,
                max_speed_degrees_per_second: Some(720.0),
                acceleration_degrees_per_second_squared: Some(10000.0),
                deceleration_degrees_per_second_squared: Some(10000.0),
            }),
            behavior: ChannelFunctionBehavior::Continuous {
                physical_min: 0.0,
                physical_max: max,
                unit: Some("deg".into()),
            },
        }];
        channel
    };
    let binding = EmitterBinding {
        gobo_wheels: vec![
            OpticalWheelBinding {
                selection: Some(channel(1)),
                rotation: Some(rotation(3, AngularMotionKind::AbsolutePosition, 360.0)),
            },
            OpticalWheelBinding {
                selection: Some(channel(2)),
                rotation: Some(rotation(4, AngularMotionKind::AngularVelocity, 90.0)),
            },
        ],
        prism_wheels: vec![OpticalWheelBinding {
            selection: Some(channel(5)),
            rotation: Some(rotation(6, AngularMotionKind::AngularVelocity, 45.0)),
        }],
        universes: vec![1],
        ..EmitterBinding::default()
    };
    let mut decoder = Decoder::new(vec![binding]);
    let scene = scene(&[EmitterKind::Beam]);
    let mut values = SceneValues::default();
    decoder.apply(
        &scene,
        &[frame(&[
            (0, 64),
            (1, 200),
            (2, 128),
            (3, 255),
            (4, 180),
            (5, 255),
        ])],
        &mut values,
        0.0,
    );
    assert_eq!(values.emitters[0].gobo_wheels[0].slot(8), 2);
    assert_eq!(values.emitters[0].gobo_wheels[1].slot(8), 6);
    assert_eq!(values.emitters[0].prism_wheels[0].slot(4), 2);
    values.apply_physical_motion(1.0);
    assert!(
        values.emitters[0].gobo_wheels[0]
            .rotation_motion
            .position_degrees
            > 0.0
    );
    assert!(
        values.emitters[0].gobo_wheels[1]
            .rotation_motion
            .position_degrees
            > 0.0
    );
    let previous = values.clone();
    decoder.apply(
        &scene,
        &[frame(&[(0, 0), (1, 255), (2, 128), (3, 255)])],
        &mut values,
        1.0,
    );
    values.retain_visual_motion_runtime_from(&previous);
    let before = values.emitters[0].gobo_wheels[1]
        .rotation_motion
        .position_degrees;
    values.apply_physical_motion(0.1);
    assert!(
        values.emitters[0].gobo_wheels[1]
            .rotation_motion
            .position_degrees
            > before
    );
    assert_eq!(values.emitters[0].gobo_wheels[0].slot(8), 0);
    assert_eq!(values.emitters[0].gobo_wheels[1].slot(8), 7);
}

fn camera_channel(first_slot: u16, bytes: usize) -> ChannelRef {
    ChannelRef {
        logical_universe: 1,
        slots: (first_slot..first_slot + bytes as u16).collect(),
        max_raw: match bytes {
            2 => 0xffff,
            3 => 0x00ff_ffff,
            _ => unreachable!("camera channels are U16 or U24"),
        },
        invert: false,
        physical_min: 0.0,
        physical_max: 1.0,
        physical_unit: None,
        snap: false,
        default_raw: 0,
        functions: Vec::new(),
    }
}

fn external_camera_binding() -> ExternalCameraBinding {
    ExternalCameraBinding {
        fixture_id: uuid::Uuid::from_u128(1),
        instance_id: uuid::Uuid::from_u128(2),
        label: "Camera 1".into(),
        x: camera_channel(1, 3),
        y: camera_channel(4, 3),
        z: camera_channel(7, 3),
        yaw: camera_channel(10, 2),
        pitch: camera_channel(12, 2),
        roll: camera_channel(14, 2),
        zoom: camera_channel(16, 2),
        universes: vec![1],
    }
}

pub(super) fn emitter(kind: EmitterKind) -> EmitterInstance {
    EmitterInstance {
        fixture_index: 0,
        head_index: 0,
        label: "Main".into(),
        local_origin: Vec3::ZERO,
        tilt_pivot: Vec3::ZERO,
        local_orientation_degrees: Vec3::ZERO,
        pan: None,
        tilt: None,
        beam_angle_degrees: 10.0,
        field_angle_degrees: 30.0,
        optics: EmitterOptics::default(),
        kind,
        cells: EmitterLayoutCells::single(),
        laser: None,
        effect: None,
        live_shaper_angle_roles: [false; 4],
        shaper_roles: [false; 4],
        live_shaper_rotation_role: false,
    }
}

pub(super) fn scene(kinds: &[EmitterKind]) -> Scene {
    let mut scene = Scene::default();
    scene.fixtures.push(FixtureInstance {
        instance_id: uuid::Uuid::nil(),
        fixture_id: uuid::Uuid::nil(),
        name: "Test".into(),
        number: None,
        position: Vec3::ZERO,
        rotation_degrees: Vec3::ZERO,
        bracket_degrees: 0.0,
        shaper_degrees: None,
        installed_colour: [1.0; 3],
        installed_shaper_angles_degrees: [0.0; 4],
        body: FixtureBody::default(),
        patched: true,
        ..FixtureInstance::default()
    });
    for kind in kinds {
        scene.emitters.push(emitter(*kind));
    }
    scene
}

pub(super) fn frame(values: &[(usize, u8)]) -> UniverseFrame {
    let mut slots = [0_u8; DMX_SLOTS];
    for (index, value) in values {
        slots[*index] = *value;
    }
    UniverseFrame {
        logical_universe: 1,
        slots,
        received_micros: 1_000,
        stale: false,
    }
}

#[test]
fn an_intensity_channel_drives_the_emitter_level() {
    let binding = EmitterBinding {
        intensity: Some(channel(1)),
        universes: vec![1],
        ..EmitterBinding::default()
    };
    let mut decoder = Decoder::new(vec![binding]);
    let scene = scene(&[EmitterKind::Beam]);
    let mut values = SceneValues::default();
    decoder.apply(&scene, &[frame(&[(0, 255)])], &mut values, 0.0);
    assert_eq!(values.emitters[0].intensity, 1.0);
    decoder.apply(&scene, &[frame(&[(0, 0)])], &mut values, 0.0);
    assert_eq!(values.emitters[0].intensity, 0.0);
}

#[test]
fn external_camera_decodes_the_exact_seventeen_slot_contract_and_holds_stale_pose() {
    let mut decoder = Decoder::with_external_camera(Vec::new(), Some(external_camera_binding()));
    let scene = Scene::default();
    let mut values = SceneValues::default();
    let mut slots = [0_u8; DMX_SLOTS];
    // X = exact zero, Y = +1 m, Z = minimum. Orientation = -360, near zero, +360.
    slots[0..3].copy_from_slice(&[0x80, 0x00, 0x00]);
    slots[3..6].copy_from_slice(&[0x80, 0x07, 0xd0]);
    slots[6..9].copy_from_slice(&[0x00, 0x00, 0x00]);
    slots[9..11].copy_from_slice(&[0x00, 0x00]);
    slots[11..13].copy_from_slice(&[0x80, 0x00]);
    slots[13..15].copy_from_slice(&[0xff, 0xff]);
    slots[15..17].copy_from_slice(&[0x00, 0x00]);
    let live = UniverseFrame {
        logical_universe: 1,
        slots,
        received_micros: 1_000,
        stale: false,
    };
    assert_eq!(
        decoder.apply(&scene, std::slice::from_ref(&live), &mut values, 0.0),
        1
    );
    let camera = values.external_camera.expect("camera decoded");
    assert_eq!(camera.position_metres, [0.0, 1.0, -4_194.304]);
    assert_eq!(camera.yaw_degrees, -360.0);
    assert!(camera.pitch_degrees.abs() < 0.006);
    assert_eq!(camera.roll_degrees, 360.0);
    assert!((camera.focal_length_millimetres - 18.0).abs() < 1e-5);
    assert!((camera.vertical_fov_degrees - 67.380_135).abs() < 1e-4);
    assert!(!camera.stale);

    let stale = UniverseFrame {
        stale: true,
        ..live
    };
    decoder.apply(&scene, &[stale], &mut values, 1.0);
    let held = values.external_camera.expect("last pose retained");
    assert_eq!(held.position_metres, camera.position_metres);
    assert!(held.stale);

    Decoder::new(Vec::new()).reconcile_external_camera(&mut values);
    let unpatched = values
        .external_camera
        .expect("unpatch retains the last pose");
    assert_eq!(unpatched.position_metres, camera.position_metres);
    assert!(!unpatched.patched);
    assert!(unpatched.stale);
}

#[test]
fn colour_channels_act_as_a_virtual_dimmer_without_an_intensity_channel() {
    let binding = EmitterBinding {
        colour: ColourBinding {
            red: Some(channel(1)),
            green: Some(channel(2)),
            blue: Some(channel(3)),
            ..ColourBinding::default()
        },
        universes: vec![1],
        ..EmitterBinding::default()
    };
    let mut decoder = Decoder::new(vec![binding]);
    let scene = scene(&[EmitterKind::Beam]);
    let mut values = SceneValues::default();
    decoder.apply(
        &scene,
        &[frame(&[(0, 255), (1, 0), (2, 0)])],
        &mut values,
        0.0,
    );
    assert_eq!(values.emitters[0].intensity, 1.0);
    assert_eq!(values.emitters[0].colour, [1.0, 0.0, 0.0]);
}

#[test]
fn additive_colour_level_multiplies_the_physical_dimmer() {
    let binding = EmitterBinding {
        intensity: Some(channel(1)),
        colour: ColourBinding {
            red: Some(channel(2)),
            green: Some(channel(3)),
            blue: Some(channel(4)),
            ..ColourBinding::default()
        },
        universes: vec![1],
        ..EmitterBinding::default()
    };
    let mut decoder = Decoder::new(vec![binding]);
    let scene = scene(&[EmitterKind::Beam]);
    let mut values = SceneValues::default();
    for dimmer in [0_u8, 64, 128, 255] {
        for red in [0_u8, 64, 128, 255] {
            decoder.apply(&scene, &[frame(&[(0, dimmer), (1, red)])], &mut values, 0.0);
            let expected = f32::from(dimmer) * f32::from(red) / (255.0 * 255.0);
            assert!((values.emitters[0].intensity - expected).abs() < 1e-6);
            if red > 0 {
                assert_eq!(values.emitters[0].colour, [1.0, 0.0, 0.0]);
            }
        }
    }
}

#[test]
fn rgb_cells_follow_master_without_squaring_colour_or_cell_dimmers() {
    for per_cell_dimmer in [false, true] {
        let binding = EmitterBinding {
            intensity: Some(channel(1)),
            cells: vec![ColourBinding {
                red: Some(channel(2)),
                intensity: per_cell_dimmer.then(|| channel(1)),
                ..ColourBinding::default()
            }],
            universes: vec![1],
            ..EmitterBinding::default()
        };
        let mut decoder = Decoder::new(vec![binding]);
        let scene = scene(&[EmitterKind::Emissive]);
        let mut values = SceneValues::default();
        for dimmer in [0_u8, 128, 255] {
            decoder.apply(&scene, &[frame(&[(0, dimmer), (1, 128)])], &mut values, 0.0);
            let expected = f32::from(dimmer) * 128.0 / (255.0 * 255.0);
            assert!((values.emitters[0].cells[0].intensity - expected).abs() < 1e-6);
        }
    }
}

#[test]
fn axis_inversion_flips_pan_and_tilt() {
    let binding = EmitterBinding {
        pan: Some(channel(1)),
        tilt: Some(channel(2)),
        invert_pan: true,
        universes: vec![1],
        ..EmitterBinding::default()
    };
    let mut decoder = Decoder::new(vec![binding]);
    let scene = scene(&[EmitterKind::Beam]);
    let mut values = SceneValues::default();
    decoder.apply(&scene, &[frame(&[(0, 255), (1, 255)])], &mut values, 0.0);
    assert_eq!(values.emitters[0].pan, 0.0);
    assert_eq!(values.emitters[0].tilt, 1.0);
}

#[test]
fn pan_uses_a_functions_exact_raw_span_and_declared_dynamics() {
    let mut pan = channel(1);
    pan.default_raw = 128;
    pan.functions = vec![ChannelFunction {
        id: uuid::Uuid::nil(),
        name: "finite pan".into(),
        dmx_from: 64,
        dmx_to: 191,
        attribute: AttributeKey("pan".into()),
        priority: 0,
        physical_mapping: None,
        angular_motion: Some(AngularMotion {
            kind: AngularMotionKind::AbsolutePosition,
            max_speed_degrees_per_second: Some(180.0),
            acceleration_degrees_per_second_squared: Some(360.0),
            deceleration_degrees_per_second_squared: Some(240.0),
        }),
        behavior: ChannelFunctionBehavior::Continuous {
            physical_min: -270.0,
            physical_max: 270.0,
            unit: Some("deg".into()),
        },
    }];
    let binding = EmitterBinding {
        pan: Some(pan),
        universes: vec![1],
        ..EmitterBinding::default()
    };
    let mut rig = scene(&[EmitterKind::Beam]);
    rig.emitters[0].pan = Some(MotionAxis {
        axis: Vec3::Y,
        min_degrees: -270.0,
        max_degrees: 270.0,
    });
    let mut values = SceneValues::default();
    let mut decoder = Decoder::new(vec![binding]);
    decoder.initialize_motion(&rig, &mut values);
    assert!(matches!(
        values.emitters[0].pan_motion.target,
        Some(PhysicalMotionTarget::Position { degrees, .. }) if degrees.abs() < 3.0
    ));
    decoder.apply(&rig, &[frame(&[(0, 191)])], &mut values, 0.0);
    assert_eq!(values.emitters[0].pan_motion.position_degrees, 0.0);
    assert_eq!(
        values.emitters[0].pan_motion.target,
        Some(PhysicalMotionTarget::Position {
            degrees: 270.0,
            max_speed: 180.0,
            acceleration: 360.0,
            deceleration: 240.0,
        })
    );
}

#[test]
fn legacy_pan_gets_fast_physical_fallback_instead_of_teleporting() {
    let binding = EmitterBinding {
        pan: Some(channel(1)),
        universes: vec![1],
        ..EmitterBinding::default()
    };
    let mut rig = scene(&[EmitterKind::Beam]);
    rig.emitters[0].pan = Some(MotionAxis {
        axis: Vec3::Y,
        min_degrees: -270.0,
        max_degrees: 270.0,
    });
    let mut values = SceneValues::default();
    Decoder::new(vec![binding]).apply(&rig, &[frame(&[(0, 255)])], &mut values, 0.0);
    assert_eq!(values.emitters[0].pan_motion.position_degrees, 0.0);
    values.apply_physical_motion(0.1);
    assert!(values.emitters[0].pan_motion.position_degrees > 0.0);
    assert!(values.emitters[0].pan_motion.position_degrees < 270.0);
}

#[test]
fn shaper_angles_decode_in_physical_degrees() {
    let mut blade = channel(1);
    blade.physical_min = -90.0;
    blade.physical_max = 90.0;
    let mut rotation = channel(2);
    rotation.physical_min = -180.0;
    rotation.physical_max = 180.0;
    let binding = EmitterBinding {
        shaper_blade_angles: [Some(blade), None, None, None],
        shaper_rotation: Some(rotation),
        universes: vec![1],
        ..EmitterBinding::default()
    };
    let mut decoder = Decoder::new(vec![binding]);
    let scene = scene(&[EmitterKind::Beam]);
    let mut values = SceneValues::default();
    decoder.apply(&scene, &[frame(&[(0, 255), (1, 0)])], &mut values, 0.0);
    assert_eq!(values.emitters[0].shaper_blade_angles_degrees[0], 90.0);
    assert_eq!(values.emitters[0].shaper_rotation_degrees, -180.0);
}

#[test]
fn hazer_output_is_decoded_but_never_touches_the_atmosphere() {
    let hazer = EmitterBinding {
        fog: Some(channel(1)),
        universes: vec![1],
        ..EmitterBinding::default()
    };
    let mut decoder = Decoder::new(vec![hazer]);
    let scene = scene(&[EmitterKind::Atmosphere]);
    let mut values = SceneValues::default();
    values.atmosphere.density = 0.5;
    decoder.apply(&scene, &[frame(&[(0, 255)])], &mut values, 0.0);
    assert_eq!(
        values.emitters[0].intensity, 1.0,
        "the hazer output decodes"
    );
    assert_eq!(
        values.atmosphere.density, 0.5,
        "haze is the renderer's own setting, not the hazer's output"
    );
}

#[test]
fn only_emitters_reading_a_changed_universe_are_re_decoded() {
    let first = EmitterBinding {
        intensity: Some(channel(1)),
        universes: vec![1],
        ..EmitterBinding::default()
    };
    let mut second = EmitterBinding {
        intensity: Some(channel(1)),
        universes: vec![7],
        ..EmitterBinding::default()
    };
    if let Some(channel) = second.intensity.as_mut() {
        channel.logical_universe = 7;
    }
    let mut decoder = Decoder::new(vec![first, second]);
    let scene = scene(&[EmitterKind::Beam, EmitterKind::Beam]);
    let mut values = SceneValues::default();
    let touched = decoder.apply(&scene, &[frame(&[(0, 255)])], &mut values, 0.0);
    assert_eq!(touched, 1);
    assert_eq!(values.emitters[0].intensity, 1.0);
    assert_eq!(values.emitters[1].intensity, 0.0);
}

#[test]
fn a_strobe_channel_gates_the_shutter_over_time() {
    let mut strobe = channel(2);
    strobe.physical_max = 20.0;
    let binding = EmitterBinding {
        intensity: Some(channel(1)),
        strobe: Some(strobe),
        universes: vec![1],
        ..EmitterBinding::default()
    };
    let mut decoder = Decoder::new(vec![binding]);
    let scene = scene(&[EmitterKind::Beam]);
    let mut values = SceneValues::default();
    let packet = frame(&[(0, 255), (1, 128)]);
    decoder.apply(&scene, std::slice::from_ref(&packet), &mut values, 0.0);
    assert!(values.emitters[0].strobe_hz > 0.5);
    let on = values.emitters[0].shutter;
    let half_period = 0.5 / values.emitters[0].strobe_hz;
    decoder.apply(&scene, &[packet], &mut values, half_period);
    let off = values.emitters[0].shutter;
    assert!(on != off, "the gate must change across the strobe period");
}
