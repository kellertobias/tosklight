use super::*;
use crate::forward::{
    CompiledPositionForward, PositionForwardFlags, PositionForwardInputError, PositionInstallation,
};
use light_core::spatial::RigidTransform as R;

fn near(a: [f64; 3], b: [f64; 3]) {
    for i in 0..3 {
        assert!((a[i] - b[i]).abs() < 1e-9, "{a:?} != {b:?}");
    }
}
fn same_pose(a: R, b: R) {
    near(a.point([0.; 3]), b.point([0.; 3]));
    for basis in [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]] {
        near(a.direction(basis), b.direction(basis));
    }
}
fn compile(p: &FixtureProfile, install: PositionInstallation<'_>) -> CompiledPositionForward {
    CompiledPositionForward::compile(p, p.modes[0].id, install)
        .unwrap()
        .unwrap()
}

#[test]
fn compiled_compound_poses_match_independent_reference_after_node_reordering() {
    let mut p = example();
    p.geometry.nodes[1].transform.rotation_degrees = Vector3 {
        x: 15.,
        y: 25.,
        z: -11.,
    };
    p.geometry.nodes[1].pivot = Vector3 {
        x: 50.,
        y: -320.,
        z: 7.,
    };
    p.geometry.nodes[2].transform.translation = Vector3 {
        x: 13.,
        y: -2.,
        z: 40.,
    };
    p.geometry.nodes[2].motion.as_mut().unwrap().axis = Vector3 {
        x: 1.,
        y: 0.2,
        z: -0.1,
    };
    p.geometry.emitters[0].orientation_degrees = Vector3 {
        x: 2.,
        y: 7.,
        z: -5.,
    };
    p.geometry.nodes.reverse();
    let compiled = compile(&p, PositionInstallation::default());
    let mut commands = compiled.create_commands();
    let mut workspace = compiled.create_workspace();
    let mut result = compiled.create_output();
    let mount = R::translation([1., 2., 3.])
        .unwrap()
        .compose(R::euler_xyz([30., -20., 70.]).unwrap());
    for raw in [[0, 0], [65535, 65535], [10000, 50000], [32768, 32768]] {
        compiled.decode_commands(&raw, &mut commands).unwrap();
        let q = commands
            .iter()
            .map(|c| c.absolute_degrees())
            .collect::<Vec<_>>();
        let angles = commands
            .iter()
            .zip(&q)
            .map(|(c, q)| (c.node_id, q.unwrap()))
            .collect();
        compiled
            .evaluate_pose(&q, mount, &mut workspace, &mut result)
            .unwrap();
        let expected = p
            .geometry
            .reference_lens_pose(p.geometry.emitters[0].id, &angles, 0.)
            .unwrap();
        same_pose(result[0].local.unwrap(), expected);
        same_pose(result[0].world.unwrap(), mount.compose(expected));
    }
}

#[test]
fn installation_defaults_and_axis_overrides_replace_once_without_wrapping() {
    let p = example();
    let mut calibration = InstalledPositionCalibration {
        pan_zero_degrees: 30.,
        tilt_zero_degrees: -20.,
        ..Default::default()
    };
    let instance = compile(
        &p,
        PositionInstallation {
            calibration: Some(&calibration),
            invert_pan: true,
            ..Default::default()
        },
    );
    let mut commands = instance.create_commands();
    instance
        .decode_commands(&[0, 65535], &mut commands)
        .unwrap();
    assert_eq!(commands[0].absolute_degrees(), Some(-690.));
    assert_eq!(commands[1].absolute_degrees(), Some(-740.));
    calibration.axis_overrides = Some(InstalledAxisOverrides {
        version: 1,
        source_identity: p
            .position_calibration_identity(p.modes[0].id)
            .unwrap()
            .unwrap(),
        axes: vec![InstalledAxisCalibration {
            node_id: commands[0].node_id,
            zero_degrees: 1080.,
            invert: false,
        }],
    });
    let override_plan = compile(
        &p,
        PositionInstallation {
            calibration: Some(&calibration),
            invert_pan: true,
            ..Default::default()
        },
    );
    override_plan
        .decode_commands(&[0, 65535], &mut commands)
        .unwrap();
    assert_eq!(commands[0].absolute_degrees(), Some(1800.));
    let mut other = p.clone();
    other.geometry.nodes[1].pivot.x += 1.;
    let stale = compile(
        &other,
        PositionInstallation {
            calibration: Some(&calibration),
            invert_pan: true,
            ..Default::default()
        },
    );
    stale.decode_commands(&[0, 65535], &mut commands).unwrap();
    assert_eq!(commands[0].absolute_degrees(), Some(-690.));
    assert!(
        stale.create_output()[0]
            .flags
            .contains(PositionForwardFlags::STALE_CALIBRATION)
    );
    // A second physical copy has its own correction rather than inheriting the first instance.
    let copy = compile(&p, PositionInstallation::default());
    copy.decode_commands(&[0, 65535], &mut commands).unwrap();
    assert_eq!(commands[0].absolute_degrees(), Some(720.));
}

#[test]
fn velocity_uses_sign_only_and_simultaneous_authority_is_explicit() {
    let mut p = example();
    let node = p.modes[0].position_physical.as_ref().unwrap().bindings[0].node_id;
    let mut speed = p.modes[0].channels[0].clone();
    speed.id = Uuid::new_v4();
    speed.functions[0].id = Uuid::new_v4();
    speed.functions[0].behavior = ChannelFunctionBehavior::Continuous {
        physical_min: 0.,
        physical_max: 360.,
        unit: Some("deg/s".into()),
    };
    speed.functions[0].angular_motion.as_mut().unwrap().kind = AngularMotionKind::AngularVelocity;
    p.modes[0]
        .position_physical
        .as_mut()
        .unwrap()
        .bindings
        .push(MotionFunctionBinding {
            node_id: node,
            channel_id: speed.id,
            function_id: speed.functions[0].id,
            role: PositionAxisRole::Pan,
        });
    p.modes[0].channels.push(speed);
    let calibration = InstalledPositionCalibration {
        pan_zero_degrees: 90.,
        ..Default::default()
    };
    let plan = compile(
        &p,
        PositionInstallation {
            calibration: Some(&calibration),
            invert_pan: true,
            ..Default::default()
        },
    );
    let mut commands = plan.create_commands();
    plan.decode_commands(&[0, 0, 65535], &mut commands).unwrap();
    assert_eq!(commands[0].velocity.unwrap().value, -360.);
    assert_eq!(commands[0].absolute.unwrap().value, -630.);
    assert!(commands[0].conflict());
    assert_eq!(commands[0].absolute_degrees(), None);
    plan.decode_commands(&[0, 0, 0], &mut commands).unwrap();
    assert!(
        commands[0].conflict(),
        "zero speed is not a declaration of hardware priority"
    );
    p.modes[0]
        .position_physical
        .as_mut()
        .unwrap()
        .bindings
        .remove(0);
    let velocity_only = compile(
        &p,
        PositionInstallation {
            calibration: Some(&calibration),
            invert_pan: true,
            ..Default::default()
        },
    );
    velocity_only
        .decode_commands(&[0, 0, 65535], &mut commands)
        .unwrap();
    assert!(!commands[0].conflict());
    assert!(commands[0].absolute_degrees().is_none());
}

#[test]
fn bracket_moves_lens_and_model_delta_without_double_neutral_transform() {
    let mut p = example();
    let root = p.geometry.nodes[0].id;
    p.geometry.physical_contract.as_mut().unwrap().bracket = GeometryBracket::Hinge {
        node_id: root,
        pivot: Vector3 {
            x: 0.,
            y: -350.,
            z: 0.,
        },
        axis: Vector3 {
            x: 1.,
            y: 0.,
            z: 0.,
        },
    };
    let plan = compile(
        &p,
        PositionInstallation {
            bracket_degrees: 90.,
            ..Default::default()
        },
    );
    let mut workspace = plan.create_workspace();
    let mut output = plan.create_output();
    plan.evaluate_pose(
        &[Some(0.), Some(0.)],
        R::IDENTITY,
        &mut workspace,
        &mut output,
    )
    .unwrap();
    near(output[0].world.unwrap().point([0.; 3]), [0., -0.45, -0.25]);
    let (_, delta) = plan.node_delta(&workspace, 0).unwrap();
    near(delta.point([0., -0.6, 0.1]), [0., -0.45, -0.25]);
    p.geometry.physical_contract.as_mut().unwrap().bracket = GeometryBracket::Unknown;
    let unknown = compile(
        &p,
        PositionInstallation {
            bracket_degrees: 90.,
            ..Default::default()
        },
    );
    let mut workspace = unknown.create_workspace();
    unknown
        .evaluate_pose(
            &[Some(0.), Some(0.)],
            R::IDENTITY,
            &mut workspace,
            &mut output,
        )
        .unwrap();
    assert!(output[0].world.is_none());
    assert!(
        output[0]
            .flags
            .contains(PositionForwardFlags::UNSUPPORTED_BRACKET)
    );
}

#[test]
fn unknown_axis_only_invalidates_its_descendants_and_fixed_lenses_need_no_position_model() {
    let mut p = example();
    let mut fixed = p.geometry.emitters[0].clone();
    fixed.id = Uuid::new_v4();
    fixed.node_id = p.geometry.nodes[0].id;
    let head_id = p.modes[0].heads[0].id;
    p.modes[0].emitter_heads.push(EmitterHeadBinding {
        emitter_id: fixed.id,
        head_id,
    });
    p.geometry.emitters.push(fixed);
    let plan = compile(&p, PositionInstallation::default());
    let mut workspace = plan.create_workspace();
    let mut output = plan.create_output();
    plan.evaluate_pose(&[Some(0.), None], R::IDENTITY, &mut workspace, &mut output)
        .unwrap();
    assert!(output[0].world.is_none());
    assert!(output[1].world.is_some());
    p.modes[0].position_physical = None;
    let unbound = compile(&p, PositionInstallation::default());
    let mut commands = unbound.create_commands();
    unbound.decode_commands(&[0, 0], &mut commands).unwrap();
    assert!(
        commands
            .iter()
            .all(|c| c.absolute.is_none() && c.velocity.is_none())
    );
    for n in &mut p.geometry.nodes {
        n.motion = None;
    }
    let fixed = compile(&p, PositionInstallation::default());
    assert!(fixed.create_commands().is_empty());
    let mut workspace = fixed.create_workspace();
    fixed
        .evaluate_pose(&[], R::IDENTITY, &mut workspace, &mut output)
        .unwrap();
    assert!(output.iter().all(|o| o.world.is_some()));
}

#[test]
fn function_gaps_and_invalid_input_do_not_become_clamped_commands() {
    let mut p = example();
    p.modes[0].channels[0].functions[0].dmx_from = 100;
    let plan = compile(&p, PositionInstallation::default());
    let mut commands = plan.create_commands();
    plan.decode_commands(&[0, 0], &mut commands).unwrap();
    assert!(commands[0].absolute.is_none());
    let before = commands.clone();
    assert_eq!(
        plan.decode_commands(&[0, 70000], &mut commands),
        Err(PositionForwardInputError::RawOutOfRange)
    );
    assert_eq!(commands, before);
    let mut workspace = plan.create_workspace();
    let mut output = plan.create_output();
    let before = output.clone();
    assert_eq!(
        plan.evaluate_pose(
            &[Some(f64::NAN), Some(0.)],
            R::IDENTITY,
            &mut workspace,
            &mut output
        ),
        Err(PositionForwardInputError::NonfiniteAxis)
    );
    assert_eq!(output, before);
}

#[test]
fn installed_evidence_limits_result_quality_and_invalid_motion_limits_are_rejected() {
    let mut p = example();
    p.modes[0].channels[0].functions[0].physical_mapping = Some(PhysicalMappingCalibration {
        quality: PhysicalDataQuality::Measured,
        source: Some("Synthetic mapping".into()),
        ..Default::default()
    });
    let c = InstalledPositionCalibration {
        pan_zero_degrees: 30.,
        quality: PhysicalDataQuality::Estimated,
        ..Default::default()
    };
    let plan = compile(
        &p,
        PositionInstallation {
            calibration: Some(&c),
            ..Default::default()
        },
    );
    let mut out = plan.create_commands();
    plan.decode_commands(&[0, 0], &mut out).unwrap();
    assert_eq!(
        out[0].absolute.unwrap().quality,
        PhysicalDataQuality::Estimated
    );
    p.modes[0].channels[0].functions[0]
        .angular_motion
        .as_mut()
        .unwrap()
        .max_speed_degrees_per_second = Some(-2.);
    assert!(
        CompiledPositionForward::compile(&p, p.modes[0].id, PositionInstallation::default())
            .is_err()
    );
}

#[test]
fn translation_ancestor_and_compound_point_mount_match_reference() {
    let mut p = example();
    let mut slide = p.geometry.nodes[0].clone();
    slide.id = Uuid::new_v4();
    slide.transform.rotation_degrees = Vector3 {
        x: 11.,
        y: 22.,
        z: 33.,
    };
    slide.motion = Some(GeometryMotion {
        attribute: None,
        kind: GeometryMotionKind::Translation,
        axis: Vector3 {
            x: 1.,
            y: 0.,
            z: 0.,
        },
        physical_min: 0.,
        physical_max: 1000.,
        max_speed_per_second: None,
        acceleration_per_second_squared: None,
        deceleration_per_second_squared: None,
    });
    p.geometry.nodes[0].parent_id = Some(slide.id);
    p.geometry.nodes.push(slide);
    let plan = compile(&p, PositionInstallation::default());
    let commands = plan.create_commands();
    let positions = commands
        .iter()
        .map(|c| Some(if c.role.is_none() { 250. } else { 30. }))
        .collect::<Vec<_>>();
    let reference = commands
        .iter()
        .zip(&positions)
        .map(|(c, q)| (c.node_id, q.unwrap()))
        .collect();
    let point = R::translation([0., 0., 3.])
        .unwrap()
        .compose(R::euler_xyz([20., 0., 45.]).unwrap());
    let relative_mount = R::translation([2., -1., 0.])
        .unwrap()
        .compose(R::euler_xyz([0., 30., 0.]).unwrap());
    let mount = point.compose(relative_mount).desk_pose_to_profile();
    let mut workspace = plan.create_workspace();
    let mut output = plan.create_output();
    plan.evaluate_pose(&positions, mount, &mut workspace, &mut output)
        .unwrap();
    let local = p
        .geometry
        .reference_lens_pose(p.geometry.emitters[0].id, &reference, 0.)
        .unwrap();
    same_pose(output[0].world.unwrap(), mount.compose(local));
}
