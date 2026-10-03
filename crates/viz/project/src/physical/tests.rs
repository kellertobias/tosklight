use super::*;
use glam::Vec3;
use light_fixture::{OpticalEmitterBand, OpticalSource};
fn rig() -> crate::ScenePlan {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../assets/fixture-library/cameo--root-par-6.toskfixture");
    let mut profile = light_fixture::read_fixture_package(&std::fs::read(path).unwrap()).unwrap();
    let mode = profile
        .modes
        .iter_mut()
        .find(|m| m.channels.len() == 7)
        .unwrap();
    let OpticalSource::Additive { emitters } =
        &mut mode.color_physical.as_mut().unwrap().paths[0].source
    else {
        unreachable!()
    };
    for emitter in emitters {
        emitter.xyz = match emitter.name.to_lowercase().as_str() {
            "red" => Some(light_core::Xyz {
                x: 0.4124564,
                y: 0.2126729,
                z: 0.0193339,
            }),
            "green" => Some(light_core::Xyz {
                x: 0.3575761,
                y: 0.7151522,
                z: 0.119192,
            }),
            "blue" => Some(light_core::Xyz {
                x: 0.1804375,
                y: 0.072175,
                z: 0.9503041,
            }),
            _ if emitter.band != OpticalEmitterBand::Ultraviolet => Some(light_core::Xyz {
                x: 0.,
                y: 0.,
                z: 0.,
            }),
            _ => None,
        };
    }
    let mode_id = mode.id;
    crate::compile(&[crate::PatchedFixture {
        fixture_id: Uuid::new_v4(),
        name: "UV reference".into(),
        number: Some(1),
        profile: Arc::new(profile),
        mode_id,
        instances: vec![PhysicalInstance {
            instance_id: Uuid::new_v4(),
            name: "UV reference".into(),
            split_patches: vec![(1, Some((1, 1)))],
            position: Vec3::ZERO,
            rotation_degrees: Vec3::ZERO,
            invert_pan: false,
            invert_tilt: false,
            bracket_angle: 0.,
            shaper_angle: None,
            installed_appearance: Default::default(),
            scenery_size_metres: None,
            scenery_options: Default::default(),
            model_scale: 1.,
            color_calibration: None,
            position_calibration: None,
        }],
    }])
}
fn frame(rgbuv: [u8; 6]) -> viz_dmx::UniverseFrame {
    let mut slots = [0; DMX_SLOTS];
    slots[..6].copy_from_slice(&rgbuv);
    viz_dmx::UniverseFrame {
        logical_universe: 1,
        slots,
        received_micros: 0,
        stale: false,
    }
}
#[test]
fn native_uv_activity_survives_unknown_visible_spill_without_fabricated_violet() {
    let plan = rig();
    let mut decoder = crate::Decoder::new(plan.bindings);
    let mut values = viz_scene::SceneValues::default();
    decoder.apply(&plan.scene, &[frame([0, 0, 0, 0, 0, 255])], &mut values, 0.);
    let v = &values.emitters[0];
    assert_eq!(v.colour, [0.; 3]);
    assert_eq!(v.uv_drive, 1.);
    assert!(!v.physical_color.unwrap().visible_complete);
    decoder.apply(
        &plan.scene,
        &[frame([128, 0, 0, 0, 0, 255])],
        &mut values,
        0.1,
    );
    let v = &mut values.emitters[0];
    assert!((v.colour[0] - 128. / 255.).abs() < 0.001);
    assert!(v.colour[1] < 0.0001 && v.colour[2] < 0.0001);
    assert!(!v.physical_color.unwrap().visible_complete);
    // A legacy color-wheel palette must not overwrite calibrated visible output on a tick.
    v.colour_wheel_palette = vec![[0., 0., 1.]];
    v.colour_wheel_motion.set_target(0, 1, 10., 20., 20.);
    let expected = v.colour;
    values.apply_physical_motion(0.1);
    assert_eq!(values.emitters[0].colour, expected);
    decoder.apply(
        &plan.scene,
        &[frame([128, 0, 0, 0, 0, 0])],
        &mut values,
        0.2,
    );
    assert!(values.emitters[0].physical_color.unwrap().visible_complete);
}
#[test]
fn absent_native_input_is_not_authoritative_zero() {
    let plan = rig();
    let binding = plan.bindings[0].physical.as_ref().unwrap();
    let mut runtime = PhysicalRuntime::new(binding.plan.clone());
    runtime.update(&HashMap::new());
    let mut value = EmitterValues::default();
    runtime.apply(binding, &mut value, Some(1.), &plan.scene.emitters[0]);
    assert!(!value.physical_color.unwrap().visible_complete);
    assert_eq!(value.colour, [0.; 3]);
}
#[test]
fn unpatched_native_values_drive_color_and_keep_uv_distinct() {
    let mut plan = rig();
    for binding in &mut plan.bindings {
        binding.universes.clear();
        let physical = binding.physical.as_mut().unwrap();
        let shared = Arc::make_mut(&mut physical.plan);
        shared.channels.fill(None);
        shared.universes = Box::default();
    }
    let p = plan.bindings[0].physical.as_ref().unwrap().plan.clone();
    let mut decoder = crate::Decoder::new(plan.bindings.clone());
    assert!(decoder.required_universes().is_empty());
    let mut values = viz_scene::SceneValues::default();
    let raw = [128, 0, 0, 0, 0, 255, 0];
    let record = || crate::NativeInstanceValues {
        fixture_id: p.fixture_id,
        instance_id: p.instance_id,
        native_identity: &p.native_identity,
        raw: &raw,
        owned_channels: None,
    };
    assert_eq!(
        decoder.apply_native(
            &plan.scene,
            [crate::NativeInstanceValues {
                native_identity: "stale",
                ..record()
            }],
            false,
            &mut values,
            0.
        ),
        0
    );
    assert_eq!(
        decoder.apply_native(&plan.scene, [record()], false, &mut values, 0.1),
        1
    );
    assert!((values.emitters[0].colour[0] - 128. / 255.).abs() < 0.001);
    assert_eq!(values.emitters[0].intensity, 1.);
    assert_eq!(values.emitters[0].uv_drive, 1.);
    assert!(!values.emitters[0].physical_color.unwrap().visible_complete);
}

#[test]
fn patched_live_slots_win_native_while_preload_and_clear_redecode_without_new_dmx() {
    let plan = rig();
    let p = plan.bindings[0].physical.as_ref().unwrap().plan.clone();
    let mut decoder = crate::Decoder::new(plan.bindings.clone());
    let mut values = viz_scene::SceneValues::default();
    decoder.apply(&plan.scene, &[frame([255, 0, 0, 0, 0, 0])], &mut values, 0.);
    let raw = [0, 255, 0, 0, 0, 0, 0];
    let record = || crate::NativeInstanceValues {
        fixture_id: p.fixture_id,
        instance_id: p.instance_id,
        native_identity: &p.native_identity,
        raw: &raw,
        owned_channels: None,
    };
    decoder.apply_native(&plan.scene, [record()], false, &mut values, 0.1);
    assert!(values.emitters[0].colour[0] > 0.99 && values.emitters[0].colour[1] < 0.001);
    decoder.apply_native(&plan.scene, [record()], true, &mut values, 0.2);
    assert!(values.emitters[0].colour[1] > 0.99 && values.emitters[0].colour[0] < 0.001);
    decoder.apply_native(&plan.scene, [record()], false, &mut values, 0.3);
    assert!(values.emitters[0].colour[0] > 0.99 && values.emitters[0].colour[1] < 0.001);
    let bad = [0, 256, 0, 0, 0, 0, 0];
    assert_eq!(
        decoder.apply_native(
            &plan.scene,
            [crate::NativeInstanceValues {
                raw: &bad,
                ..record()
            }],
            true,
            &mut values,
            0.4
        ),
        0
    );
}

#[test]
fn partial_native_preload_preserves_same_fixture_live_overrides_and_uv() {
    let plan = rig();
    let p = plan.bindings[0].physical.as_ref().unwrap().plan.clone();
    let mut decoder = crate::Decoder::new(plan.bindings.clone());
    let mut values = viz_scene::SceneValues::default();
    decoder.apply(
        &plan.scene,
        &[frame([201, 173, 0, 0, 0, 0])],
        &mut values,
        0.,
    );
    let raw = [64, 0, 0, 0, 0, 197, 0];
    let mask = [true, false, false, false, false, true, false];
    let record = || crate::NativeInstanceValues {
        fixture_id: p.fixture_id,
        instance_id: p.instance_id,
        native_identity: &p.native_identity,
        raw: &raw,
        owned_channels: Some(&mask),
    };
    decoder.apply_native(&plan.scene, [record()], false, &mut values, 0.1);
    decoder.apply_native(&plan.scene, [record()], true, &mut values, 0.1);
    assert!((values.emitters[0].colour[0] - 64. / 255.).abs() < 0.001);
    assert!((values.emitters[0].colour[1] - 173. / 255.).abs() < 0.001);
    assert!((values.emitters[0].uv_drive - 197. / 255.).abs() < 0.001);
    let before = values.emitters[0].colour;
    assert_eq!(
        decoder.apply_native(
            &plan.scene,
            [crate::NativeInstanceValues {
                owned_channels: Some(&[true]),
                ..record()
            }],
            true,
            &mut values,
            0.2
        ),
        0
    );
    assert_eq!(values.emitters[0].colour, before);
    decoder.apply_native(&plan.scene, [record()], false, &mut values, 0.3);
    assert!((values.emitters[0].colour[0] - 201. / 255.).abs() < 0.001);
    assert!((values.emitters[0].colour[1] - 173. / 255.).abs() < 0.001);
    assert_eq!(values.emitters[0].uv_drive, 0.);
}

#[test]
fn native_fills_missing_split_but_never_replaces_a_patched_universe_that_has_not_arrived() {
    let mut plan = rig();
    let b = plan.bindings[0].physical.as_mut().unwrap();
    let p = Arc::make_mut(&mut b.plan);
    for channel in &mut p.channels[1..] {
        *channel = None;
    }
    let p = b.plan.clone();
    let mut decoder = crate::Decoder::new(plan.bindings.clone());
    let mut values = viz_scene::SceneValues::default();
    let raw = [255, 0, 255, 0, 0, 0, 0];
    let record = || crate::NativeInstanceValues {
        fixture_id: p.fixture_id,
        instance_id: p.instance_id,
        native_identity: &p.native_identity,
        raw: &raw,
        owned_channels: None,
    };
    decoder.apply_native(&plan.scene, [record()], false, &mut values, 0.);
    assert!(!values.emitters[0].physical_color.unwrap().visible_complete);
    decoder.apply(&plan.scene, &[frame([64, 0, 0, 0, 0, 0])], &mut values, 0.1);
    decoder.apply_native(&plan.scene, [record()], false, &mut values, 0.1);
    assert!(values.emitters[0].physical_color.unwrap().visible_complete);
    assert!((values.emitters[0].colour[0] - 64. / 255.).abs() < 0.001);
    assert!(values.emitters[0].colour[2] > 0.99);
}

#[test]
fn missing_unrelated_dimmer_input_does_not_hide_known_color() {
    let mut plan = rig();
    let binding = plan.bindings[0].physical.as_mut().unwrap();
    let physical = Arc::make_mut(&mut binding.plan);
    // RGBWAU occupy the first six controls; dimmer is not an optical path input.
    physical.channels[6] = None;
    let mut decoder = crate::Decoder::new(plan.bindings.clone());
    let mut values = viz_scene::SceneValues::default();
    decoder.apply(
        &plan.scene,
        &[frame([255, 0, 255, 0, 0, 0])],
        &mut values,
        0.,
    );
    let achieved = values.emitters[0].physical_color.unwrap();
    assert!(achieved.visible_complete);
    assert!(values.emitters[0].colour[0] > 0.99 && values.emitters[0].colour[2] > 0.99);
}

#[test]
fn replacing_with_legacy_mode_releases_old_prediction_ownership() {
    let plan = rig();
    let old = plan.bindings[0].physical.as_ref().unwrap();
    let mut replacement = (*old.plan).clone();
    replacement.color_declared = false;
    replacement.color = None;
    replacement.position_declared = false;
    replacement.position = None;
    let runtime = PhysicalRuntime::new(Arc::new(replacement));
    let mut value = EmitterValues::default();
    value.physical_color = Some(PhysicalColorState::default());
    value.physical_pose = Some(viz_scene::PhysicalPoseState {
        local: Some(glam::Mat4::IDENTITY),
        ..Default::default()
    });
    runtime.apply(old, &mut value, Some(1.), &plan.scene.emitters[0]);
    assert!(value.physical_color.is_none());
    assert!(value.physical_pose.is_none());
}
