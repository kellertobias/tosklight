use super::*;
use crate::{
    EffectiveAxisCalibration, InstalledAxisCalibration, InstalledAxisOverrides,
    InstalledPositionCalibration, PositionCalibrationContext,
};
fn example() -> FixtureProfile {
    let mut p = FixtureProfile::blank();
    p.manufacturer = "Reference".into();
    p.name = "Two-axis".into();
    p.short_name = "Two-axis".into();
    let m = &mut p.modes[0];
    m.splits[0].footprint = 4;
    let h = m.heads[0].id;
    p.geometry = GeometryGraph::template(GeometryTemplate::MovingHead, &[h]);
    p.geometry.physical_contract = Some(GeometryPhysicalContract {
        version: 1,
        provenance: OpticalProvenance::default(),
        bracket: GeometryBracket::Fixed,
    });
    let mut bindings = Vec::new();
    for (i, role) in [PositionAxisRole::Pan, PositionAxisRole::Tilt]
        .into_iter()
        .enumerate()
    {
        let mut c = channel(h, ChannelResolution::U16, vec![(2 + i * 2) as u16]);
        c.attribute = AttributeKey(if i == 0 { "pan" } else { "tilt" }.into());
        c.fixture_attribute = c.attribute.clone();
        c.functions[0].attribute = c.attribute.clone();
        c.functions[0].behavior = ChannelFunctionBehavior::Continuous {
            physical_min: 720.0,
            physical_max: -720.0,
            unit: Some("deg".into()),
        };
        c.functions[0].angular_motion = Some(AngularMotion {
            kind: AngularMotionKind::AbsolutePosition,
            max_speed_degrees_per_second: None,
            acceleration_degrees_per_second_squared: None,
            deceleration_degrees_per_second_squared: None,
        });
        bindings.push(MotionFunctionBinding {
            node_id: p.geometry.nodes[i + 1].id,
            channel_id: c.id,
            function_id: c.functions[0].id,
            role,
        });
        m.channels.push(c);
    }
    m.position_physical = Some(PositionPhysicalModel {
        kinematics: Default::default(),
        version: 1,
        revision: 0,
        bindings,
    });
    let e = GeometryEmitter {
        id: Uuid::new_v4(),
        name: "Lens".into(),
        node_id: p.geometry.nodes[2].id,
        head_id: None,
        origin: Vector3 {
            x: 0.0,
            y: -600.0,
            z: 100.0,
        },
        orientation_degrees: Vector3::default(),
        beam_angle_degrees: 10.0,
        field_angle_degrees: 20.0,
        feather: 0.0,
        focus: 1.0,
        directional: true,
        layout: EmitterLayout::Point,
    };
    m.emitter_heads = vec![EmitterHeadBinding {
        emitter_id: e.id,
        head_id: h,
    }];
    p.geometry.emitters = vec![e];
    p
}
#[test]
fn physical_contract_round_trip_keeps_ids_and_requires_exact_supported_bindings() {
    let p = example();
    p.validate().unwrap();
    let copy: FixtureProfile = serde_json::from_value(serde_json::to_value(&p).unwrap()).unwrap();
    assert_eq!(
        copy.position_calibration_identity(copy.modes[0].id)
            .unwrap(),
        p.position_calibration_identity(p.modes[0].id).unwrap()
    );
    assert_eq!(copy.geometry.nodes[1].id, p.geometry.nodes[1].id);
    for velocity in [false, true] {
        let mut p = example();
        if velocity {
            let f = &mut p.modes[0].channels[0].functions[0];
            f.angular_motion.as_mut().unwrap().kind = AngularMotionKind::AngularVelocity;
            if let ChannelFunctionBehavior::Continuous { unit, .. } = &mut f.behavior {
                *unit = Some("deg/s".into())
            }
        }
        p.validate().unwrap();
        p.modes[0].channels[0].behavior = ChannelBehavior::Static;
        assert!(p.validate().unwrap_err().to_string().contains("static"));
    }
    let mut p = example();
    p.modes[0].position_physical.as_mut().unwrap().bindings[0].function_id = Uuid::new_v4();
    assert!(p.validate().is_err());
    let mut p = example();
    p.geometry.physical_contract.as_mut().unwrap().version = 2;
    assert!(p.validate().is_err());
    let mut p = example();
    p.geometry.nodes[1].motion.as_mut().unwrap().axis = Vector3::default();
    assert!(p.validate().is_err());
    let mut p = example();
    p.geometry.nodes[0].transform.scale.x = 2.0;
    assert!(p.validate().is_err());
}
#[test]
fn calibration_identity_ignores_cosmetics_and_focus_but_tracks_real_kinematics() {
    let p = example();
    let id = p.position_calibration_identity(p.modes[0].id).unwrap();
    let mut cosmetic = p.clone();
    cosmetic.geometry.nodes[1].name = "Renamed".into();
    cosmetic.geometry.emitters[0].focus = 0.1;
    cosmetic.geometry.emitters[0].beam_angle_degrees = 5.0;
    cosmetic.geometry.emitters[0].name = "Other".into();
    cosmetic
        .geometry
        .physical_contract
        .as_mut()
        .unwrap()
        .provenance
        .source = Some("Notes".into());
    cosmetic.geometry.nodes.reverse();
    cosmetic.modes[0]
        .position_physical
        .as_mut()
        .unwrap()
        .bindings
        .reverse();
    assert_eq!(
        id,
        cosmetic
            .position_calibration_identity(p.modes[0].id)
            .unwrap()
    );
    let mut changed = p.clone();
    changed.geometry.nodes[1].pivot.x = 50.0;
    assert_ne!(
        id,
        changed
            .position_calibration_identity(p.modes[0].id)
            .unwrap()
    );
    let mut changed = p.clone();
    if let ChannelFunctionBehavior::Continuous { physical_min, .. } =
        &mut changed.modes[0].channels[0].functions[0].behavior
    {
        *physical_min = 540.0
    };
    assert_ne!(
        id,
        changed
            .position_calibration_identity(p.modes[0].id)
            .unwrap()
    );
    let mut changed = p.clone();
    changed.id = FixtureId::new();
    assert_ne!(
        id,
        changed
            .position_calibration_identity(p.modes[0].id)
            .unwrap()
    );
}
#[test]
fn installed_axis_override_replaces_defaults_once_and_remains_unwrapped() {
    let p = example();
    let m = &p.modes[0];
    let context = PositionCalibrationContext::new(&p, m.id).unwrap().unwrap();
    let node = p.geometry.nodes[1].id;
    let calibration = InstalledPositionCalibration {
        pan_zero_degrees: 999.0,
        axis_overrides: Some(InstalledAxisOverrides {
            version: 1,
            source_identity: context.identity.clone(),
            axes: vec![InstalledAxisCalibration {
                node_id: node,
                zero_degrees: 30.0,
                invert: true,
            }],
        }),
        ..Default::default()
    };
    let effective = calibration
        .effective_axis(&context, node, PositionAxisRole::Pan, true, false)
        .unwrap();
    let mapping = CompiledPhysicalMapping::compile(&m.channels[0], &m.channels[0].functions[0])
        .unwrap()
        .unwrap();
    let native = effective.calibrated_to_physical(390.0);
    assert_eq!(native, -360.0);
    let encoded = mapping.raw_for_physical(native).unwrap();
    assert_eq!(encoded.raw, 49151);
    assert!((effective.physical_to_calibrated(encoded.physical) - 389.9945).abs() < 0.001);
    let other = EffectiveAxisCalibration {
        zero_degrees: -15.0,
        invert: false,
    };
    assert_eq!(other.physical_to_calibrated(native), -375.0);
    assert!(
        calibration
            .effective_axis(
                &context,
                Uuid::new_v4(),
                PositionAxisRole::Pan,
                false,
                false
            )
            .is_err()
    );
    assert!(
        calibration
            .effective_axis(&context, node, PositionAxisRole::Tilt, false, false)
            .is_err()
    );
    let mut replacement = p.clone();
    replacement.id = FixtureId::new();
    let current = PositionCalibrationContext::new(&replacement, m.id)
        .unwrap()
        .unwrap();
    assert!(
        calibration
            .effective_axis(&current, node, PositionAxisRole::Pan, false, false)
            .is_err()
    );
    let restored: InstalledPositionCalibration =
        serde_json::from_value(serde_json::to_value(&calibration).unwrap()).unwrap();
    assert_eq!(restored, calibration);
}
#[test]
fn bracket_and_lens_use_profile_geometry_without_artwork() {
    let mut p = example();
    let root = p.geometry.nodes[0].id;
    let id = p.geometry.emitters[0].id;
    p.geometry.physical_contract.as_mut().unwrap().bracket = GeometryBracket::Hinge {
        node_id: root,
        pivot: Vector3 {
            x: 0.0,
            y: -350.0,
            z: 0.0,
        },
        axis: Vector3 {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        },
    };
    let pose = p
        .geometry
        .reference_lens_pose(id, &HashMap::new(), 90.0)
        .unwrap();
    let xyz = pose.point([0.0; 3]);
    for (a, b) in xyz.into_iter().zip([0.0, -0.45, -0.25]) {
        assert!((a - b).abs() < 1e-10)
    }
    let expected = pose;
    p.geometry.nodes[2].glb_node = Some("Unrelated artwork".into());
    assert_eq!(
        expected,
        p.geometry
            .reference_lens_pose(id, &HashMap::new(), 90.0)
            .unwrap()
    );
    // Without an authored hinge the bracket turns the whole lamp about its own X axis at its
    // origin, as the Stage draws it: +90° maps (x, y, z) to (x, −z, y).
    p.geometry.physical_contract.as_mut().unwrap().bracket = GeometryBracket::Unknown;
    let level = p
        .geometry
        .reference_lens_pose(id, &HashMap::new(), 0.0)
        .unwrap()
        .point([0.0; 3]);
    let turned = p
        .geometry
        .reference_lens_pose(id, &HashMap::new(), 90.0)
        .unwrap()
        .point([0.0; 3]);
    for (a, b) in turned.into_iter().zip([level[0], -level[2], level[1]]) {
        assert!((a - b).abs() < 1e-10)
    }
}
#[test]
fn shared_pan_can_reach_other_heads_only_through_shared_control() {
    let mut p = example();
    let m = &mut p.modes[0];
    let other = Uuid::new_v4();
    m.heads.push(FixtureHead {
        id: other,
        name: "Second".into(),
        master_shared: false,
    });
    m.emitter_heads[0].head_id = other;
    p.validate().unwrap();
    p.modes[0].heads[0].master_shared = false;
    assert!(p.validate().unwrap_err().to_string().contains("cross-head"));
}

#[path = "position_physical_forward.rs"]
mod forward;

#[path = "position_fitting.rs"]
mod fitting;
