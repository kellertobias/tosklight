use super::*;
use glam::Vec3;
use light_fixture::*;
fn fixture() -> crate::PatchedFixture {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../assets/fixture-library/cameo--root-par-6.toskfixture");
    let original = read_fixture_package(&std::fs::read(path).unwrap()).unwrap();
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Reference".into();
    profile.name = "Physical motion".into();
    profile.short_name = "Motion".into();
    profile.fixture_type = "moving-head".into();
    let mode = &mut profile.modes[0];
    let head = mode.heads[0].id;
    profile.geometry = GeometryGraph::template(GeometryTemplate::MovingHead, &[head]);
    profile.geometry.physical_contract = Some(GeometryPhysicalContract {
        version: 1,
        provenance: Default::default(),
        bracket: GeometryBracket::Hinge {
            node_id: profile.geometry.nodes[0].id,
            pivot: Default::default(),
            axis: Vector3 {
                x: 1.,
                y: 0.,
                z: 0.,
            },
        },
    });
    let mut bindings = Vec::new();
    for (i, (name, role, min, max)) in [
        ("pan", PositionAxisRole::Pan, -720., 720.),
        ("tilt", PositionAxisRole::Tilt, 0., 255.),
    ]
    .into_iter()
    .enumerate()
    {
        let mut c = original.modes[0].channels[i].clone();
        c.head_id = head;
        c.attribute = light_core::AttributeKey(name.into());
        c.fixture_attribute = c.attribute.clone();
        c.functions[0].attribute = c.attribute.clone();
        c.functions[0].behavior = ChannelFunctionBehavior::Continuous {
            physical_min: min,
            physical_max: max,
            unit: Some("deg".into()),
        };
        c.functions[0].angular_motion = Some(AngularMotion {
            kind: AngularMotionKind::AbsolutePosition,
            max_speed_degrees_per_second: Some(180.),
            acceleration_degrees_per_second_squared: Some(720.),
            deceleration_degrees_per_second_squared: Some(720.),
        });
        bindings.push(MotionFunctionBinding {
            node_id: profile.geometry.nodes[i + 1].id,
            channel_id: c.id,
            function_id: c.functions[0].id,
            role,
        });
        mode.channels.push(c);
    }
    mode.splits[0].footprint = 2;
    mode.position_physical = Some(PositionPhysicalModel {
        version: 1,
        revision: 0,
        bindings,
    });
    let lens = GeometryEmitter {
        id: Uuid::new_v4(),
        name: "Lens".into(),
        node_id: profile.geometry.nodes[2].id,
        head_id: None,
        origin: Vector3 {
            x: 0.,
            y: -600.,
            z: 100.,
        },
        orientation_degrees: Default::default(),
        beam_angle_degrees: 10.,
        field_angle_degrees: 20.,
        feather: 0.,
        focus: 1.,
        directional: true,
        layout: EmitterLayout::Point,
    };
    mode.emitter_heads = vec![EmitterHeadBinding {
        emitter_id: lens.id,
        head_id: head,
    }];
    profile.geometry.emitters = vec![lens];
    let mode_id = mode.id;
    profile.validate().unwrap();
    crate::PatchedFixture {
        fixture_id: Uuid::new_v4(),
        name: "Motion".into(),
        number: Some(1),
        profile: Arc::new(profile),
        mode_id,
        instances: vec![PhysicalInstance {
            instance_id: Uuid::new_v4(),
            name: "Motion".into(),
            split_patches: vec![(1, Some((1, 1)))],
            position: Vec3::ZERO,
            rotation_degrees: Vec3::ZERO,
            invert_pan: false,
            invert_tilt: false,
            bracket_angle: 45.,
            shaper_angle: None,
            installed_appearance: Default::default(),
            scenery_size_metres: None,
            scenery_options: Default::default(),
            model_scale: 1.,
            color_calibration: None,
            position_calibration: None,
        }],
    }
}
fn rig() -> crate::ScenePlan {
    crate::compile(&[fixture()])
}
/// Core timing only: no GPU or output scheduler claim. Run explicitly and retain stdout.
#[test]
#[ignore = "manual bounded physical decode and mechanical evaluation timing"]
fn bounded_physical_motion_timing() {
    for count in [300usize, 1000] {
        let mut fixture = fixture();
        let original = fixture.instances[0].clone();
        fixture.instances = (0..count)
            .map(|i| {
                let mut instance = original.clone();
                instance.instance_id = Uuid::from_u128(i as u128 + 1);
                instance.position = Vec3::new((i % 20) as f32, 5., (i / 20) as f32);
                instance.split_patches =
                    vec![(1, Some(((i / 256 + 1) as u16, (i % 256 * 2 + 1) as u16)))];
                instance
            })
            .collect();
        let plan = crate::compile(&[fixture]);
        let mut decoder = crate::Decoder::new(plan.bindings);
        let mut values = viz_scene::SceneValues::default();
        decoder.initialize_motion(&plan.scene, &mut values);
        let mut frames: Vec<_> = (1..=count.div_ceil(256))
            .map(|u| viz_dmx::UniverseFrame {
                logical_universe: u as u16,
                slots: [0; 512],
                received_micros: 0,
                stale: false,
            })
            .collect();
        let mut samples = Vec::with_capacity(600);
        for tick in 0..660 {
            let start = std::time::Instant::now();
            if tick % 3 == 0 {
                for frame in &mut frames {
                    for pair in frame.slots.chunks_mut(2) {
                        pair[0] = (tick % 256) as u8;
                        pair[1] = (255 - tick % 256) as u8;
                    }
                }
                decoder.apply(&plan.scene, &frames, &mut values, tick as f32 / 60.);
            }
            values.apply_calibrated_motion(&plan.scene, 1. / 60.);
            std::hint::black_box(&values);
            if tick >= 60 {
                samples.push(start.elapsed().as_secs_f64() * 1000.);
            }
        }
        samples.sort_by(f64::total_cmp);
        println!(
            "{}",
            serde_json::json!({"physical_instances": count, "samples":samples.len(), "mean_ms":samples.iter().sum::<f64>()/samples.len() as f64, "p95_ms":samples[samples.len()*95/100], "max_ms":samples[samples.len()-1], "includes_gpu":false})
        );
        assert_eq!(values.physical_positions.len(), count);
        assert!(
            values
                .emitters
                .iter()
                .all(|v| v.physical_pose.unwrap().local.is_some())
        );
    }
}

#[test]
fn physical_home_uses_defaults_and_keeps_buffers_and_held_pose() {
    let plan = rig();
    let decoder = crate::Decoder::new(plan.bindings);
    let mut values = viz_scene::SceneValues::default();
    decoder.initialize_motion(&plan.scene, &mut values);
    assert!(
        values.physical_positions[0]
            .axes
            .iter()
            .all(|a| a.has_position)
    );
    let targets: Vec<_> = values.physical_positions[0]
        .axes
        .iter()
        .map(|a| a.motion.target)
        .collect();
    let ptr = values.physical_positions[0].node_deltas.as_ptr();
    for _ in 0..1000 {
        values.apply_calibrated_motion(&plan.scene, 0.01);
    }
    assert_eq!(values.physical_positions[0].node_deltas.as_ptr(), ptr);
    assert!(values.emitters[0].physical_pose.unwrap().local.is_some());
    for (axis, target) in values.physical_positions[0].axes.iter().zip(&targets) {
        let Some(viz_scene::PhysicalMotionTarget::Position { degrees, .. }) = target else {
            panic!("authored absolute home");
        };
        assert_eq!(axis.motion.position_degrees, *degrees);
    }
    let held = values.physical_positions[0].axes[0].motion.position_degrees;
    let mut incoming = values.clone();
    incoming.retain_calibrated_motion_from(&values);
    incoming.take_calibrated_runtime_from(&mut values);
    incoming.apply_calibrated_motion(&plan.scene, 0.);
    assert_eq!(incoming.physical_positions[0].node_deltas.as_ptr(), ptr);
    assert_eq!(
        incoming.physical_positions[0].axes[0]
            .motion
            .position_degrees,
        held
    );
}

#[test]
fn repeated_lenses_of_one_head_keep_values_when_scene_order_changes() {
    let plan = rig();
    let mut before = plan.scene;
    before.emitters.push(before.emitters[0].clone());
    before.emitter_ids.push(Uuid::new_v4());
    let mut values = viz_scene::SceneValues::default();
    values.resize(2);
    values.emitters[0].intensity = 0.2;
    values.emitters[1].intensity = 0.8;
    let mut after = before.clone();
    after.emitters.swap(0, 1);
    after.emitter_ids.swap(0, 1);
    values.carry_over(&before, &after);
    assert_eq!(values.emitters[0].intensity, 0.8);
    assert_eq!(values.emitters[1].intensity, 0.2);
}

#[test]
fn native_commands_keep_unwrapped_motion_and_bracketed_lens_on_display_clock() {
    let plan = rig();
    assert_eq!(plan.scene.physical_positions.len(), 1);
    let mut decoder = crate::Decoder::new(plan.bindings);
    let mut values = viz_scene::SceneValues::default();
    let mut slots = [0; 512];
    slots[0] = 255;
    slots[1] = 90;
    decoder.apply(
        &plan.scene,
        &[viz_dmx::UniverseFrame {
            logical_universe: 1,
            slots,
            received_micros: 0,
            stale: false,
        }],
        &mut values,
        0.,
    );
    let commanded = values.physical_positions[0].axes[0].motion.target.unwrap();
    assert!(matches!(
        commanded,
        viz_scene::PhysicalMotionTarget::Position { degrees: 720., .. }
    ));
    values.apply_calibrated_motion(&plan.scene, 0.1);
    assert!(values.physical_positions[0].axes[0].motion.position_degrees < 20.);
    for _ in 0..100 {
        values.apply_calibrated_motion(&plan.scene, 0.1);
    }
    assert_eq!(
        values.physical_positions[0].axes[0].motion.position_degrees,
        720.
    );
    let pose = values.emitters[0].physical_pose.unwrap();
    assert_eq!(pose.flags, 0);
    assert!(!pose.nominal_motion);
    let local = pose.local.unwrap();
    let reference = glam::Quat::from_rotation_x(135_f32.to_radians());
    assert!((local.transform_vector3(Vec3::NEG_Y) - reference * Vec3::NEG_Y).length() < 1e-5);
    assert!(
        (local.transform_point3(Vec3::ZERO) - reference * Vec3::new(0., -0.6, 0.1)).length() < 1e-5
    );
    // A provider snapshot carries targets; it cannot reset renderer-owned settling.
    let mut next = viz_scene::SceneValues::default();
    decoder.apply(
        &plan.scene,
        &[viz_dmx::UniverseFrame {
            logical_universe: 1,
            slots,
            received_micros: 1,
            stale: false,
        }],
        &mut next,
        0.1,
    );
    next.retain_visual_motion_runtime_from(&values);
    next.take_calibrated_runtime_from(&mut values);
    next.apply_calibrated_motion(&plan.scene, 0.1);
    assert_eq!(
        next.physical_positions[0].axes[0].motion.position_degrees,
        720.
    );
}
