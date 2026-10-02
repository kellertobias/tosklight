use super::*;
use crate::forward::{CompiledPositionForward, PositionInstallation};
use light_core::spatial::RigidTransform as R;

fn model(p: &FixtureProfile, install: PositionInstallation<'_>) -> CompiledPositionFitting {
    CompiledPositionFitting::compile(p, p.modes[0].id, install)
        .unwrap()
        .unwrap()
}
fn native(p: &FixtureProfile, angles: [f64; 2]) -> Vec<u32> {
    let m = &p.modes[0];
    let mut raw = vec![0; m.channels.len()];
    for (i, channel) in m.channels.iter().enumerate() {
        if let Some(role) = m
            .position_physical
            .as_ref()
            .unwrap()
            .bindings
            .iter()
            .find(|b| b.channel_id == channel.id)
            .map(|b| b.role)
        {
            let j = if role == PositionAxisRole::Pan { 0 } else { 1 };
            let mapping = CompiledPhysicalMapping::compile(channel, &channel.functions[0])
                .unwrap()
                .unwrap();
            raw[i] = mapping.raw_for_physical(angles[j]).unwrap().raw;
        }
    }
    raw
}
fn target(
    p: &FixtureProfile,
    angles: [f64; 2],
    mount: R,
    install: PositionInstallation<'_>,
) -> [f64; 3] {
    let forward = CompiledPositionForward::compile(p, p.modes[0].id, install)
        .unwrap()
        .unwrap();
    let mut poses = forward.create_output();
    forward
        .evaluate_pose(
            &angles.map(Some),
            mount,
            &mut forward.create_workspace(),
            &mut poses,
        )
        .unwrap();
    poses[0].world.unwrap().point([0., -10., 0.])
}
fn run(
    m: &CompiledPositionFitting,
    raw: &[u32],
    r: PositionFitRequest,
    previous: [f64; 2],
    mount: R,
) -> (PositionFitWorkspace, Vec<PositionFitResult>) {
    let mut ws = m.create_workspace();
    let mut out = m.create_output();
    m.fit(
        PositionFitInput {
            current_raw: raw,
            available: &vec![true; raw.len()],
            requests: &[Some(r)],
            previous: &previous.map(Some),
            mount,
        },
        &mut ws,
        &mut out,
    )
    .unwrap();
    (ws, out)
}
#[test]
fn angles_keep_turns_and_calibration_is_applied_once() {
    let p = example();
    let m = model(&p, PositionInstallation::default());
    let raw = native(&p, [0., 0.]);
    for pan in [-720., -360., 0., 360., 720.] {
        let (ws, out) = run(
            &m,
            &raw,
            PositionFitRequest::Angles { pan, tilt: 45. },
            [0., 0.],
            R::IDENTITY,
        );
        assert_eq!(out[0].status, PositionFitStatus::Fitted);
        assert_eq!(
            out[0].requested,
            Some(PositionFitRequest::Angles { pan, tilt: 45. })
        );
        assert!((out[0].achieved.unwrap()[0] - pan).abs() < 0.023);
        assert_eq!(out[0].writes.iter().flatten().count(), 2);
        assert_eq!(ws.proposed_raw()[0], out[0].writes[0].unwrap().raw);
    }
    let calibration = crate::InstalledPositionCalibration {
        pan_zero_degrees: 31.,
        tilt_zero_degrees: -17.,
        ..Default::default()
    };
    let m = model(
        &p,
        PositionInstallation {
            calibration: Some(&calibration),
            invert_pan: true,
            ..Default::default()
        },
    );
    let (_, out) = run(
        &m,
        &raw,
        PositionFitRequest::Angles {
            pan: 700.,
            tilt: -400.,
        },
        [0., 0.],
        R::IDENTITY,
    );
    let achieved = out[0].achieved.unwrap();
    assert!((achieved[0] - 700.).abs() < 0.023);
    assert!((achieved[1] + 400.).abs() < 0.023);
    let (_, out) = run(
        &m,
        &raw,
        PositionFitRequest::Angles {
            pan: 9999.,
            tilt: -9999.,
        },
        [0., 0.],
        R::IDENTITY,
    );
    assert!(out[0].clipped);
    assert_eq!(out[0].achieved, Some([751., -737.]));
}
#[test]
fn targets_keep_nearest_turn_and_follow_independent_mount() {
    let p = example();
    let m = model(&p, PositionInstallation::default());
    let raw = native(&p, [700., 45.]);
    let fixed = target(
        &p,
        [700., 45.],
        R::IDENTITY,
        PositionInstallation::default(),
    );
    let (_, first) = run(
        &m,
        &raw,
        PositionFitRequest::Target { world: Some(fixed) },
        [700., 45.],
        R::IDENTITY,
    );
    assert_eq!(first[0].status, PositionFitStatus::Fitted);
    assert!((first[0].achieved.unwrap()[0] - 700.).abs() < 0.023);
    assert!(first[0].angular_error_degrees.unwrap() < 0.03);
    let mount = R::translation([2., 1., -1.])
        .unwrap()
        .compose(R::euler_xyz([20., -35., 50.]).unwrap());
    let (_, next) = run(
        &m,
        &raw,
        PositionFitRequest::Target { world: Some(fixed) },
        [700., 45.],
        mount,
    );
    assert_eq!(next[0].status, PositionFitStatus::Fitted, "{next:?}");
    assert!(next[0].angular_error_degrees.unwrap() < 0.04);
    assert_ne!(first[0].achieved, next[0].achieved);
    assert_eq!(first[0].requested, next[0].requested);
}
#[test]
fn invalid_layouts_are_atomic_and_scratch_is_model_bound() {
    let p = example();
    let m = model(&p, PositionInstallation::default());
    let raw = native(&p, [0., 0.]);
    let mut ws = m.create_workspace();
    let mut out = m.create_output();
    let old = out.clone();
    assert_eq!(
        m.fit(
            PositionFitInput {
                current_raw: &raw,
                available: &[true, true],
                requests: &[],
                previous: &[None, None],
                mount: R::IDENTITY
            },
            &mut ws,
            &mut out
        ),
        Err(PositionFitInputError::RequestLayout)
    );
    assert_eq!(out, old);
    let other = model(&p, PositionInstallation::default());
    assert_eq!(
        other.fit(
            PositionFitInput {
                current_raw: &raw,
                available: &[true, true],
                requests: &[None],
                previous: &[None, None],
                mount: R::IDENTITY
            },
            &mut ws,
            &mut out
        ),
        Err(PositionFitInputError::WorkspaceLayout)
    );
    assert_eq!(out, old);
}
#[test]
fn missing_and_coincident_targets_hold_native_output() {
    let p = example();
    let m = model(&p, PositionInstallation::default());
    let raw = native(&p, [300., 10.]);
    let (ws, out) = run(
        &m,
        &raw,
        PositionFitRequest::Target { world: None },
        [300., 10.],
        R::IDENTITY,
    );
    assert_eq!(out[0].status, PositionFitStatus::MissingTarget);
    assert_eq!(ws.proposed_raw(), raw);
    let pose = out[0].pose.unwrap();
    let exact = out[0].achieved.unwrap();
    let (ws, out) = run(
        &m,
        &raw,
        PositionFitRequest::Target {
            world: Some(pose.point([0.; 3])),
        },
        exact,
        R::IDENTITY,
    );
    assert_eq!(out[0].status, PositionFitStatus::CoincidentTarget);
    assert_eq!(ws.proposed_raw(), raw);
}
#[test]
fn unavailable_native_inputs_never_reappear_in_achieved_pose() {
    let p = example();
    let m = model(&p, PositionInstallation::default());
    let raw = native(&p, [30., 20.]);
    let mut ws = m.create_workspace();
    let mut out = m.create_output();
    m.fit(
        PositionFitInput {
            current_raw: &raw,
            available: &[false, true],
            requests: &[Some(PositionFitRequest::Angles {
                pan: 40.,
                tilt: 50.,
            })],
            previous: &[Some(30.), Some(20.)],
            mount: R::IDENTITY,
        },
        &mut ws,
        &mut out,
    )
    .unwrap();
    assert_eq!(out[0].status, PositionFitStatus::UnavailableInput);
    assert!(out[0].pose.is_none());
    assert!(out[0].achieved.is_none());
    assert_eq!(ws.proposed_raw(), raw);
    assert!(
        out[0]
            .flags
            .contains(crate::forward::PositionForwardFlags::UNKNOWN_AXIS)
    );
}
#[test]
fn shared_outputs_withhold_conflicting_complete_families_and_passive_holds() {
    let mut p = example();
    let mut lens = p.geometry.emitters[0].clone();
    lens.id = Uuid::new_v4();
    let head = p.modes[0].heads[0].id;
    p.modes[0].emitter_heads.push(EmitterHeadBinding {
        emitter_id: lens.id,
        head_id: head,
    });
    p.geometry.emitters.push(lens);
    let m = model(&p, PositionInstallation::default());
    let raw = native(&p, [0., 0.]);
    for second in [
        Some(PositionFitRequest::Angles {
            pan: -90.,
            tilt: 50.,
        }),
        Some(PositionFitRequest::Target { world: None }),
    ] {
        let mut ws = m.create_workspace();
        let mut out = m.create_output();
        m.fit(
            PositionFitInput {
                current_raw: &raw,
                available: &[true, true],
                requests: &[
                    Some(PositionFitRequest::Angles {
                        pan: 90.,
                        tilt: 50.,
                    }),
                    second,
                ],
                previous: &[Some(0.), Some(0.)],
                mount: R::IDENTITY,
            },
            &mut ws,
            &mut out,
        )
        .unwrap();
        assert_eq!(out[0].status, PositionFitStatus::OwnershipConflict);
        assert_eq!(out[0].writes, [None; 2]);
        assert_eq!(ws.proposed_raw(), raw);
    }
    let mut ws = m.create_workspace();
    let mut out = m.create_output();
    m.fit(
        PositionFitInput {
            current_raw: &raw,
            available: &[true, true],
            requests: &[Some(PositionFitRequest::Angles {
                pan: 90.,
                tilt: 50.,
            }); 2],
            previous: &[Some(0.), Some(0.)],
            mount: R::IDENTITY,
        },
        &mut ws,
        &mut out,
    )
    .unwrap();
    assert!(out.iter().all(|o| o.status == PositionFitStatus::Fitted));
}
#[test]
fn improper_or_nonfinite_mounts_fail_without_mutating_outputs() {
    let p = example();
    let m = model(&p, PositionInstallation::default());
    let raw = native(&p, [0., 0.]);
    let bad:R=serde_json::from_value(serde_json::json!({"rotation":[[-1.,0.,0.],[0.,1.,0.],[0.,0.,1.]],"translation":[0.,0.,0.]})).unwrap();
    let mut ws = m.create_workspace();
    let mut out = m.create_output();
    let before = out.clone();
    assert_eq!(
        m.fit(
            PositionFitInput {
                current_raw: &raw,
                available: &[true, true],
                requests: &[None],
                previous: &[None, None],
                mount: bad
            },
            &mut ws,
            &mut out
        ),
        Err(PositionFitInputError::InvalidMount)
    );
    assert_eq!(out, before);
}
#[test]
fn compound_geometry_bracket_and_point_offset_use_the_moving_lens_origin() {
    let mut p = example();
    p.geometry.nodes[1].transform.rotation_degrees = Vector3 {
        x: 15.,
        y: 25.,
        z: -20.,
    };
    p.geometry.nodes[1].pivot = Vector3 {
        x: 30.,
        y: 140.,
        z: 80.,
    };
    p.geometry.nodes[2].motion.as_mut().unwrap().axis = Vector3 {
        x: 1.,
        y: 0.2,
        z: 0.3,
    };
    p.geometry.emitters[0].orientation_degrees = Vector3 {
        x: 10.,
        y: 15.,
        z: -7.,
    };
    p.geometry.physical_contract.as_mut().unwrap().bracket = GeometryBracket::Hinge {
        node_id: p.geometry.nodes[0].id,
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
    let install = PositionInstallation {
        bracket_degrees: 38.,
        ..Default::default()
    };
    let m = model(&p, install);
    let raw = native(&p, [25., 45.]);
    let mount = R::translation([2., -1., 3.])
        .unwrap()
        .compose(R::euler_xyz([30., -40., 80.]).unwrap());
    let reference = p
        .geometry
        .reference_lens_pose(
            p.geometry.emitters[0].id,
            &[(p.geometry.nodes[1].id, 35.), (p.geometry.nodes[2].id, 55.)]
                .into_iter()
                .collect(),
            38.,
        )
        .unwrap();
    let point = R::translation(mount.compose(reference).point([0., -8., 0.]))
        .unwrap()
        .compose(R::euler_xyz([17., 29., 41.]).unwrap());
    let offset = [0.13, -0.07, 0.08];
    let fixed = point.point(offset);
    let (ws, out) = run(
        &m,
        &raw,
        PositionFitRequest::Target { world: Some(fixed) },
        [25., 45.],
        mount,
    );
    assert_eq!(out[0].status, PositionFitStatus::Fitted, "{out:?}");
    assert!(out[0].angular_error_degrees.unwrap() < 0.04);
    let q = out[0].achieved.unwrap();
    let reference = p
        .geometry
        .reference_lens_pose(
            p.geometry.emitters[0].id,
            &[
                (p.geometry.nodes[1].id, q[0]),
                (p.geometry.nodes[2].id, q[1]),
            ]
            .into_iter()
            .collect(),
            38.,
        )
        .unwrap();
    let expected = mount.compose(reference);
    let actual = ws.forward()[0].world.unwrap();
    for basis in [[0.; 3], [1., 0., 0.], [0., -1., 0.], [0., 0., 1.]] {
        for (a, b) in actual.point(basis).into_iter().zip(expected.point(basis)) {
            assert!((a - b).abs() < 1e-9);
        }
    }
}
#[test]
fn unreachable_limited_target_holds_and_antiparallel_seed_is_not_exact() {
    let mut p = example();
    for c in &mut p.modes[0].channels {
        c.functions[0].behavior = ChannelFunctionBehavior::Continuous {
            physical_min: -20.,
            physical_max: 20.,
            unit: Some("deg".into()),
        };
    }
    let m = model(&p, PositionInstallation::default());
    let raw = native(&p, [0., 0.]);
    let (ws, out) = run(
        &m,
        &raw,
        PositionFitRequest::Target {
            world: Some([0., 10., 0.]),
        },
        [0., 0.],
        R::IDENTITY,
    );
    assert_eq!(out[0].status, PositionFitStatus::UnreachableTarget);
    assert_eq!(ws.proposed_raw(), raw);
    let p = example();
    let m = model(&p, PositionInstallation::default());
    let raw = native(&p, [0., 0.]);
    let (_, out) = run(
        &m,
        &raw,
        PositionFitRequest::Target {
            world: Some([0., 10., 0.]),
        },
        [0., 0.],
        R::IDENTITY,
    );
    assert_eq!(out[0].status, PositionFitStatus::Fitted, "{out:?}");
    assert!(out[0].angular_error_degrees.unwrap() < 0.04);
    assert!(out[0].achieved.unwrap()[1].abs() > 90.);
}
#[test]
fn singular_pan_keeps_the_previous_turn() {
    let mut p = example();
    p.geometry.emitters[0].origin = Vector3::default();
    let m = model(&p, PositionInstallation::default());
    let raw = native(&p, [697., 20.]);
    let (_, out) = run(
        &m,
        &raw,
        PositionFitRequest::Target {
            world: Some([0., -10., 0.]),
        },
        [697., 20.],
        R::IDENTITY,
    );
    assert_eq!(out[0].status, PositionFitStatus::Fitted, "{out:?}");
    assert!((out[0].achieved.unwrap()[0] - 697.).abs() < 0.025);
}
#[test]
fn scratch_and_continuity_are_independent_for_physical_copies() {
    let p = example();
    let m = model(&p, PositionInstallation::default());
    let raw = native(&p, [0., 20.]);
    let request = PositionFitRequest::Target {
        world: Some(target(
            &p,
            [0., 40.],
            R::IDENTITY,
            PositionInstallation::default(),
        )),
    };
    let (a, outa) = run(&m, &raw, request, [710., 35.], R::IDENTITY);
    let (b, outb) = run(&m, &raw, request, [-350., 35.], R::IDENTITY);
    assert_eq!(outa[0].status, PositionFitStatus::Fitted);
    assert_eq!(outb[0].status, PositionFitStatus::Fitted);
    assert!((outa[0].achieved.unwrap()[0] - 720.).abs() < 0.03);
    assert!((outb[0].achieved.unwrap()[0] + 360.).abs() < 0.03);
    assert_ne!(a.proposed_raw(), b.proposed_raw());
}
#[test]
fn velocity_only_and_even_zero_simultaneous_velocity_never_become_absolute_aim() {
    let mut p = example();
    let binding = p.modes[0].position_physical.as_ref().unwrap().bindings[0].clone();
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
            channel_id: speed.id,
            function_id: speed.functions[0].id,
            ..binding
        });
    p.modes[0].channels.push(speed);
    for velocity_only in [false, true] {
        if velocity_only {
            p.modes[0]
                .position_physical
                .as_mut()
                .unwrap()
                .bindings
                .remove(0);
        }
        let m = model(&p, PositionInstallation::default());
        let raw = vec![32768, 32768, 0];
        for request in [
            PositionFitRequest::Angles {
                pan: 90.,
                tilt: 40.,
            },
            PositionFitRequest::Target {
                world: Some([3., -4., 2.]),
            },
        ] {
            let (ws, out) = run(&m, &raw, request, [90., 40.], R::IDENTITY);
            assert_eq!(
                out[0].status,
                PositionFitStatus::VelocityAuthority,
                "{out:?}"
            );
            assert_eq!(ws.proposed_raw(), raw);
            assert!(out[0].achieved.is_none());
            assert!(out[0].pose.is_none());
        }
    }
}
#[test]
fn nonlinear_u32_angle_writes_survive_exact_dmx_bytes() {
    let mut p = example();
    for (i, c) in p.modes[0].channels.iter_mut().enumerate() {
        c.resolution = ChannelResolution::U32;
        c.secondary_slots = vec![(i * 4 + 2) as u16, (i * 4 + 3) as u16, (i * 4 + 4) as u16];
        c.functions[0].dmx_from = 700;
        c.functions[0].dmx_to = u32::MAX - 900;
        c.functions[0].physical_mapping = Some(PhysicalMappingCalibration {
            quality: PhysicalDataQuality::Measured,
            source: Some("Synthetic nonlinear reference".into()),
            samples: vec![
                PhysicalMappingPoint {
                    raw: 700,
                    physical: 720.,
                },
                PhysicalMappingPoint {
                    raw: 2_000_000_000,
                    physical: 600.,
                },
                PhysicalMappingPoint {
                    raw: u32::MAX - 900,
                    physical: -720.,
                },
            ],
            ..Default::default()
        });
    }
    p.modes[0].splits[0].footprint = 8;
    let m = model(&p, PositionInstallation::default());
    let raw = native(&p, [0., 0.]);
    let (ws, out) = run(
        &m,
        &raw,
        PositionFitRequest::Angles {
            pan: 631.123456789,
            tilt: -421.987654321,
        },
        [0., 0.],
        R::IDENTITY,
    );
    assert_eq!(out[0].status, PositionFitStatus::Fitted);
    let plan = p.modes[0].compile_encoding_plan().unwrap();
    let mut frame = [0; 512];
    plan.encode_split_by_index(
        &mut frame,
        1,
        1,
        &out[0]
            .writes
            .iter()
            .flatten()
            .map(|w| (w.channel_index, w.raw))
            .collect::<Vec<_>>(),
    )
    .unwrap();
    for i in 0..2 {
        let decoded = u32::from_be_bytes(frame[i * 4..i * 4 + 4].try_into().unwrap());
        assert_eq!(decoded, ws.proposed_raw()[i]);
        assert_eq!(decoded, out[0].writes[i].unwrap().raw);
    }
    let q = out[0].achieved.unwrap();
    assert!((q[0] - 631.123456789).abs() < 1e-6);
    assert!((q[1] + 421.987654321).abs() < 1e-6);
}
#[test]
fn disjoint_functions_use_exact_ids_and_do_not_write_a_gap() {
    let mut p = example();
    let c = &mut p.modes[0].channels[0];
    let mut other = c.functions[0].clone();
    other.id = Uuid::new_v4();
    c.functions[0].dmx_from = 0;
    c.functions[0].dmx_to = 20000;
    c.functions[0].behavior = ChannelFunctionBehavior::Continuous {
        physical_min: -720.,
        physical_max: -100.,
        unit: Some("deg".into()),
    };
    other.dmx_from = 40000;
    other.dmx_to = 65535;
    other.behavior = ChannelFunctionBehavior::Continuous {
        physical_min: 100.,
        physical_max: 720.,
        unit: Some("deg".into()),
    };
    let id = other.id;
    let cid = c.id;
    c.functions.push(other);
    let old = p.modes[0].position_physical.as_ref().unwrap().bindings[0].clone();
    p.modes[0]
        .position_physical
        .as_mut()
        .unwrap()
        .bindings
        .push(MotionFunctionBinding {
            channel_id: cid,
            function_id: id,
            ..old
        });
    let m = model(&p, PositionInstallation::default());
    let (_, out) = run(
        &m,
        &[30000, 32768],
        PositionFitRequest::Angles {
            pan: 500.,
            tilt: 20.,
        },
        [0., 0.],
        R::IDENTITY,
    );
    assert_eq!(out[0].status, PositionFitStatus::Fitted);
    let w = out[0].writes[0].unwrap();
    assert_eq!(w.function_id, id);
    assert!(w.raw >= 40000);
    let (_, out) = run(
        &m,
        &[30000, 32768],
        PositionFitRequest::Angles { pan: 0., tilt: 20. },
        [0., 0.],
        R::IDENTITY,
    );
    assert!(out[0].clipped);
    assert!((out[0].achieved.unwrap()[0]).abs() >= 100.);
    assert!(!(20001..40000).contains(&out[0].writes[0].unwrap().raw));
}
#[test]
fn inactive_velocity_function_on_same_channel_can_switch_to_absolute_function() {
    let mut p = example();
    let c = &mut p.modes[0].channels[0];
    let mut speed = c.functions[0].clone();
    speed.id = Uuid::new_v4();
    c.functions[0].dmx_to = 40000;
    speed.dmx_from = 40001;
    speed.dmx_to = 65535;
    speed.behavior = ChannelFunctionBehavior::Continuous {
        physical_min: -360.,
        physical_max: 360.,
        unit: Some("deg/s".into()),
    };
    speed.angular_motion.as_mut().unwrap().kind = AngularMotionKind::AngularVelocity;
    let id = speed.id;
    c.functions.push(speed);
    let b = p.modes[0].position_physical.as_ref().unwrap().bindings[0].clone();
    p.modes[0]
        .position_physical
        .as_mut()
        .unwrap()
        .bindings
        .push(MotionFunctionBinding {
            function_id: id,
            ..b
        });
    let m = model(&p, PositionInstallation::default());
    let (_, out) = run(
        &m,
        &[50000, 32768],
        PositionFitRequest::Angles {
            pan: 90.,
            tilt: 40.,
        },
        [90., 40.],
        R::IDENTITY,
    );
    assert_eq!(out[0].status, PositionFitStatus::Fitted);
    assert!(out[0].writes[0].unwrap().raw <= 40000);
}
#[test]
fn target_chooses_reachable_flipped_head_branch() {
    let mut p = example();
    p.geometry.emitters[0].origin = Vector3::default();
    p.modes[0].channels[1].functions[0].behavior = ChannelFunctionBehavior::Continuous {
        physical_min: 0.,
        physical_max: 100.,
        unit: Some("deg".into()),
    };
    let m = model(&p, PositionInstallation::default());
    let raw = native(&p, [0., 5.]);
    let fixed = target(&p, [0., -40.], R::IDENTITY, PositionInstallation::default());
    let (_, out) = run(
        &m,
        &raw,
        PositionFitRequest::Target { world: Some(fixed) },
        [0., 5.],
        R::IDENTITY,
    );
    assert_eq!(out[0].status, PositionFitStatus::Fitted, "{out:?}");
    assert!((out[0].achieved.unwrap()[0].abs() - 180.).abs() < 0.025);
    assert!((out[0].achieved.unwrap()[1] - 40.).abs() < 0.025);
}
#[test]
fn unbound_translation_ancestor_requires_explicit_captured_value() {
    let mut p = example();
    let mut slide = p.geometry.nodes[0].clone();
    slide.id = Uuid::new_v4();
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
    let m = model(&p, PositionInstallation::default());
    let raw = native(&p, [20., 10.]);
    let mut ws = m.create_workspace();
    let mut out = m.create_output();
    let mut previous = m
        .create_commands()
        .iter()
        .map(|c| c.role.map(|_| 20.))
        .collect::<Vec<_>>();
    let request = Some(PositionFitRequest::Angles {
        pan: 35.,
        tilt: 55.,
    });
    m.fit(
        PositionFitInput {
            current_raw: &raw,
            available: &[true, true],
            requests: &[request],
            previous: &previous,
            mount: R::IDENTITY,
        },
        &mut ws,
        &mut out,
    )
    .unwrap();
    assert_eq!(out[0].status, PositionFitStatus::UnknownAxis);
    assert!(out[0].pose.is_none());
    for q in &mut previous {
        if q.is_none() {
            *q = Some(250.);
        }
    }
    m.fit(
        PositionFitInput {
            current_raw: &raw,
            available: &[true, true],
            requests: &[request],
            previous: &previous,
            mount: R::IDENTITY,
        },
        &mut ws,
        &mut out,
    )
    .unwrap();
    assert_eq!(out[0].status, PositionFitStatus::Fitted);
    assert!(out[0].pose.is_some());
}
#[test]
fn target_ranks_encoded_function_accuracy_instead_of_continuous_accuracy() {
    let mut p = example();
    let desired = target(&p, [31., 42.], R::IDENTITY, PositionInstallation::default());
    let c = &mut p.modes[0].channels[0];
    let mut fine = c.functions[0].clone();
    fine.id = Uuid::new_v4();
    c.functions[0].dmx_to = 2;
    fine.dmx_from = 100;
    fine.dmx_to = 65535;
    let fid = fine.id;
    c.functions.push(fine);
    let b = p.modes[0].position_physical.as_ref().unwrap().bindings[0].clone();
    p.modes[0]
        .position_physical
        .as_mut()
        .unwrap()
        .bindings
        .push(MotionFunctionBinding {
            function_id: fid,
            ..b
        });
    let m = model(&p, PositionInstallation::default());
    let (_, out) = run(
        &m,
        &[1, 32768],
        PositionFitRequest::Target {
            world: Some(desired),
        },
        [0., 40.],
        R::IDENTITY,
    );
    assert_eq!(out[0].status, PositionFitStatus::Fitted, "{out:?}");
    assert_eq!(out[0].writes[0].unwrap().function_id, fid);
    assert!(out[0].angular_error_degrees.unwrap() < 0.04);
}
#[test]
fn excessive_target_function_pairs_remain_passive_and_angles_still_work() {
    let mut p = example();
    let old = p.modes[0]
        .position_physical
        .as_ref()
        .unwrap()
        .bindings
        .clone();
    p.modes[0]
        .position_physical
        .as_mut()
        .unwrap()
        .bindings
        .clear();
    for (j, b) in old.iter().enumerate() {
        let original = p.modes[0].channels[j].functions[0].clone();
        p.modes[0].channels[j].functions.clear();
        for i in 0..9 {
            let mut f = original.clone();
            f.id = Uuid::new_v4();
            f.dmx_from = i * 7000;
            f.dmx_to = i * 7000 + 6999;
            p.modes[0]
                .position_physical
                .as_mut()
                .unwrap()
                .bindings
                .push(MotionFunctionBinding {
                    function_id: f.id,
                    ..b.clone()
                });
            p.modes[0].channels[j].functions.push(f);
        }
    }
    let m = model(&p, PositionInstallation::default());
    let raw = [3500, 3500];
    let (ws, out) = run(
        &m,
        &raw,
        PositionFitRequest::Target {
            world: Some([3., -4., 5.]),
        },
        [0., 0.],
        R::IDENTITY,
    );
    assert_eq!(out[0].status, PositionFitStatus::SolverCapacity);
    assert_eq!(ws.proposed_raw(), raw);
    let (_, out) = run(
        &m,
        &raw,
        PositionFitRequest::Angles {
            pan: 42.,
            tilt: 17.,
        },
        [0., 0.],
        R::IDENTITY,
    );
    assert_eq!(out[0].status, PositionFitStatus::Fitted);
}
#[test]
fn ambiguous_requested_head_protects_shared_native_controls_from_a_valid_peer() {
    let mut p = example();
    let pan = p.geometry.nodes[1].clone();
    let tilt = p.geometry.nodes[2].clone();
    let mut extra_pan = pan.clone();
    extra_pan.id = Uuid::new_v4();
    extra_pan.parent_id = Some(pan.id);
    p.geometry.nodes[2].parent_id = Some(extra_pan.id);
    let mut peer_tilt = tilt;
    peer_tilt.id = Uuid::new_v4();
    peer_tilt.parent_id = Some(pan.id);
    let mut peer = p.geometry.emitters[0].clone();
    peer.id = Uuid::new_v4();
    peer.node_id = peer_tilt.id;
    let head = p.modes[0].heads[0].id;
    p.modes[0].emitter_heads.push(EmitterHeadBinding {
        emitter_id: peer.id,
        head_id: head,
    });
    p.geometry.emitters.push(peer);
    let bindings = p.modes[0]
        .position_physical
        .as_ref()
        .unwrap()
        .bindings
        .clone();
    p.modes[0]
        .position_physical
        .as_mut()
        .unwrap()
        .bindings
        .extend([
            MotionFunctionBinding {
                node_id: extra_pan.id,
                ..bindings[0].clone()
            },
            MotionFunctionBinding {
                node_id: peer_tilt.id,
                ..bindings[1].clone()
            },
        ]);
    p.geometry.nodes.extend([extra_pan, peer_tilt]);
    let m = model(&p, PositionInstallation::default());
    let raw = native(&p, [0., 0.]);
    let mut ws = m.create_workspace();
    let mut out = m.create_output();
    let previous = vec![Some(0.); m.create_commands().len()];
    m.fit(
        PositionFitInput {
            current_raw: &raw,
            available: &[true, true],
            requests: &[
                Some(PositionFitRequest::Target { world: None }),
                Some(PositionFitRequest::Angles {
                    pan: 90.,
                    tilt: 45.,
                }),
            ],
            previous: &previous,
            mount: R::IDENTITY,
        },
        &mut ws,
        &mut out,
    )
    .unwrap();
    assert_eq!(out[0].status, PositionFitStatus::AmbiguousAxes);
    assert_eq!(out[1].status, PositionFitStatus::OwnershipConflict);
    assert_eq!(ws.proposed_raw(), raw);
}

/// Fixture-math microbenchmark only: excludes Engine, output scheduling, GPU and transport.
/// Explicitly run with --ignored --nocapture; not a physical or packaged performance gate.
#[test]
#[ignore]
fn moving_target_fitting_workload_evidence() {
    let p = example();
    let m = model(&p, PositionInstallation::default());
    let mut instances = (0..300)
        .map(|_| {
            (
                native(&p, [30., 45.]),
                [Some(30.), Some(45.)],
                m.create_workspace(),
                m.create_output(),
            )
        })
        .collect::<Vec<_>>();
    let mut frame_times = Vec::new();
    let mut max_evaluations = 0;
    for frame in 0..60 {
        let mount = R::translation([0., f64::from(frame) * 0.001, 0.]).unwrap();
        let aim = target(
            &p,
            [30. + f64::from(frame) * 0.05, 45.],
            R::IDENTITY,
            PositionInstallation::default(),
        );
        let start = std::time::Instant::now();
        for (raw, previous, ws, out) in &mut instances {
            m.fit(
                PositionFitInput {
                    current_raw: raw,
                    available: &[true, true],
                    requests: &[Some(PositionFitRequest::Target { world: Some(aim) })],
                    previous,
                    mount,
                },
                ws,
                out,
            )
            .unwrap();
            assert_eq!(out[0].status, PositionFitStatus::Fitted);
            max_evaluations = max_evaluations.max(ws.candidate_evaluations());
            raw.copy_from_slice(ws.proposed_raw());
            *previous = out[0].achieved.unwrap().map(Some);
        }
        frame_times.push(start.elapsed().as_secs_f64() * 1000.);
    }
    frame_times.sort_by(f64::total_cmp);
    eprintln!(
        "fixture_only_moving_300_p95_ms={:.3};max_candidate_evaluations={max_evaluations}",
        frame_times[57]
    );
    assert!(max_evaluations <= 4096);
}
#[test]
fn analytic_joint_derivatives_match_independent_central_differences() {
    let mut p = example();
    p.geometry.nodes[1].transform.rotation_degrees = Vector3 {
        x: 15.,
        y: 25.,
        z: -20.,
    };
    p.geometry.nodes[1].pivot = Vector3 {
        x: 30.,
        y: 140.,
        z: 80.,
    };
    p.geometry.nodes[2].motion.as_mut().unwrap().axis = Vector3 {
        x: 1.,
        y: 0.2,
        z: 0.3,
    };
    p.geometry.emitters[0].orientation_degrees = Vector3 {
        x: 10.,
        y: 15.,
        z: -7.,
    };
    p.geometry.physical_contract.as_mut().unwrap().bracket = GeometryBracket::Hinge {
        node_id: p.geometry.nodes[0].id,
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
    let mut slide = p.geometry.nodes[0].clone();
    slide.id = Uuid::new_v4();
    slide.motion = Some(GeometryMotion {
        attribute: None,
        kind: GeometryMotionKind::Translation,
        axis: Vector3 {
            x: 0.2,
            y: 0.3,
            z: 1.,
        },
        physical_min: 0.,
        physical_max: 1000.,
        max_speed_per_second: None,
        acceleration_per_second_squared: None,
        deceleration_per_second_squared: None,
    });
    p.geometry.nodes[0].parent_id = Some(slide.id);
    p.geometry.nodes.push(slide);
    let calibration = crate::InstalledPositionCalibration {
        pan_zero_degrees: 31.,
        tilt_zero_degrees: -17.,
        ..Default::default()
    };
    let forward = CompiledPositionForward::compile(
        &p,
        p.modes[0].id,
        PositionInstallation {
            calibration: Some(&calibration),
            invert_pan: true,
            bracket_degrees: 38.,
            ..Default::default()
        },
    )
    .unwrap()
    .unwrap();
    let commands = forward.create_commands();
    let axes = commands
        .iter()
        .map(|a| {
            Some(match a.role {
                Some(PositionAxisRole::Pan) => 35.,
                Some(PositionAxisRole::Tilt) => 55.,
                None => 250.,
            })
        })
        .collect::<Vec<_>>();
    let pair = [
        commands
            .iter()
            .position(|a| a.role == Some(PositionAxisRole::Pan))
            .unwrap(),
        commands
            .iter()
            .position(|a| a.role == Some(PositionAxisRole::Tilt))
            .unwrap(),
    ];
    let mount = R::translation([2., -1., 3.])
        .unwrap()
        .compose(R::euler_xyz([30., -40., 80.]).unwrap());
    let (pose, tangents) = forward
        .fitting_lens_geometry(0, &forward.fitting_ancestry(0), &axes, mount, pair)
        .unwrap();
    let cross = |a: [f64; 3], b: [f64; 3]| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let origin = pose.point([0.; 3]);
    let direction = pose.direction([0., -1., 0.]);
    for j in 0..2 {
        let mut before = axes.clone();
        let mut after = axes.clone();
        before[pair[j]] = Some(axes[pair[j]].unwrap() - 0.0001);
        after[pair[j]] = Some(axes[pair[j]].unwrap() + 0.0001);
        let mut output = forward.create_output();
        let mut scratch = forward.create_workspace();
        forward
            .evaluate_pose(&before, mount, &mut scratch, &mut output)
            .unwrap();
        let a = output[0].world.unwrap();
        forward
            .evaluate_pose(&after, mount, &mut scratch, &mut output)
            .unwrap();
        let b = output[0].world.unwrap();
        let analytic_origin = cross(
            tangents[j].radians_per_degree,
            std::array::from_fn(|i| origin[i] - tangents[j].pivot[i]),
        );
        let analytic_direction = cross(tangents[j].radians_per_degree, direction);
        for i in 0..3 {
            assert!(
                ((b.point([0.; 3])[i] - a.point([0.; 3])[i]) / 0.0002 - analytic_origin[i]).abs()
                    < 1e-8
            );
            assert!(
                ((b.direction([0., -1., 0.])[i] - a.direction([0., -1., 0.])[i]) / 0.0002
                    - analytic_direction[i])
                    .abs()
                    < 1e-8
            );
        }
    }
}

#[test]
fn fitting_metadata_preserves_order_ancestor_identity_and_complete_driver_ownership() {
    let mut p = example();
    let pan_node = p.geometry.nodes[1].id;
    let tilt_node = p.geometry.nodes[2].id;
    let head = p.modes[0].heads[0].id;
    let mut slide = p.geometry.nodes[0].clone();
    slide.id = Uuid::new_v4();
    let slide_node = slide.id;
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

    // Two function drivers on the same velocity channel must produce one ownership slot.
    let binding = p.modes[0].position_physical.as_ref().unwrap().bindings[0].clone();
    let mut speed = p.modes[0].channels[0].clone();
    speed.id = Uuid::new_v4();
    speed.split = 2;
    speed.resolution = ChannelResolution::U32;
    speed.secondary_slots = vec![2, 3, 4];
    speed.functions[0].id = Uuid::new_v4();
    speed.functions[0].dmx_to = u32::MAX / 2;
    speed.functions[0].behavior = ChannelFunctionBehavior::Continuous {
        physical_min: 0.,
        physical_max: 360.,
        unit: Some("deg/s".into()),
    };
    speed.functions[0].angular_motion.as_mut().unwrap().kind = AngularMotionKind::AngularVelocity;
    let mut second = speed.functions[0].clone();
    second.id = Uuid::new_v4();
    second.dmx_from = u32::MAX / 2 + 1;
    second.dmx_to = u32::MAX;
    speed.functions.push(second);
    for function in &speed.functions {
        p.modes[0]
            .position_physical
            .as_mut()
            .unwrap()
            .bindings
            .push(MotionFunctionBinding {
                channel_id: speed.id,
                function_id: function.id,
                ..binding.clone()
            });
    }
    p.modes[0].splits.push(FixtureSplit {
        number: 2,
        footprint: 4,
    });
    p.modes[0].channels.push(speed);

    // A Pan-only emitter remains unsupported, but its shared controls must remain visible.
    let mut pan_only = p.geometry.emitters[0].clone();
    pan_only.id = Uuid::new_v4();
    pan_only.node_id = pan_node;
    p.modes[0].emitter_heads.push(EmitterHeadBinding {
        emitter_id: pan_only.id,
        head_id: head,
    });
    p.geometry.emitters.push(pan_only);
    let m = model(&p, PositionInstallation::default());
    let commands = m.create_commands();
    assert_eq!(m.axes().len(), commands.len());
    for (index, axis) in m.axes().iter().enumerate() {
        assert_eq!(axis.command_index, index);
        assert_eq!(
            (axis.node_id, axis.role),
            (commands[index].node_id, commands[index].role)
        );
    }
    let pan = m.axes().iter().position(|a| a.node_id == pan_node).unwrap();
    let tilt = m
        .axes()
        .iter()
        .position(|a| a.node_id == tilt_node)
        .unwrap();
    let slide = m
        .axes()
        .iter()
        .position(|a| a.node_id == slide_node)
        .unwrap();
    assert_eq!(m.axes()[slide].role, None);
    assert!(m.axes()[slide].controls.is_empty());
    assert_eq!(m.axes()[pan].controls.len(), 2);
    assert_eq!(
        m.axes()[pan].controls[1],
        PositionFitControlMetadata {
            channel_index: 2,
            channel_id: p.modes[0].channels[2].id,
            split: 2,
            raw_max: u32::MAX,
        }
    );
    let out = m.create_output();
    assert_eq!(m.emitters().len(), out.len());
    for (index, emitter) in m.emitters().enumerate() {
        assert_eq!(emitter.emitter_index, index);
        assert_eq!(emitter.emitter_id, out[index].emitter_id);
        assert_eq!(emitter.head_id, Some(head));
        assert_eq!(m.emitter(index), Some(emitter));
    }
    assert_eq!(m.emitter(out.len()), None);
    let emitter = m.emitter(0).unwrap();
    assert_eq!(emitter.command_indices, Some([pan, tilt]));
    assert_eq!(emitter.ancestor_axes, &[tilt, pan, slide]);
    assert_eq!(
        emitter
            .controls
            .iter()
            .map(|c| c.channel_index)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    for control in emitter.controls {
        let channel = &p.modes[0].channels[control.channel_index as usize];
        assert_eq!(
            (control.channel_id, control.split, control.raw_max),
            (channel.id, channel.split, channel.resolution.max_raw())
        );
    }
    let unsupported = m.emitter(1).unwrap();
    assert_eq!(unsupported.command_indices, None);
    assert_eq!(unsupported.ancestor_axes, &[pan, slide]);
    assert_eq!(
        unsupported
            .controls
            .iter()
            .map(|c| c.channel_index)
            .collect::<Vec<_>>(),
        vec![0, 2]
    );
}

#[test]
fn achieved_axis_accessor_uses_final_native_decode_and_preserves_unknowns_and_atomic_errors() {
    let p = example();
    let calibration = crate::InstalledPositionCalibration {
        pan_zero_degrees: 31.,
        tilt_zero_degrees: -17.,
        ..Default::default()
    };
    let m = model(
        &p,
        PositionInstallation {
            calibration: Some(&calibration),
            invert_pan: true,
            ..Default::default()
        },
    );
    let mut ws = m.create_workspace();
    assert!(ws.achieved_axes().iter().all(Option::is_none));
    let raw = native(&p, [0., 0.]);
    let mut out = m.create_output();
    m.fit(
        PositionFitInput {
            current_raw: &raw,
            available: &[true, true],
            requests: &[Some(PositionFitRequest::Angles {
                pan: 450.,
                tilt: 30.,
            })],
            previous: &[Some(0.), Some(0.)],
            mount: R::IDENTITY,
        },
        &mut ws,
        &mut out,
    )
    .unwrap();
    assert_eq!(out[0].status, PositionFitStatus::Fitted);
    let pair = m.emitter(0).unwrap().command_indices.unwrap();
    let achieved = out[0].achieved.unwrap();
    assert_eq!(ws.achieved_axes()[pair[0]], Some(achieved[0]));
    assert_eq!(ws.achieved_axes()[pair[1]], Some(achieved[1]));
    assert!((achieved[0] - 450.).abs() < 0.023);
    assert!((achieved[1] - 30.).abs() < 0.023);
    let accepted = ws.achieved_axes().to_vec();
    assert_eq!(
        m.fit(
            PositionFitInput {
                current_raw: &raw,
                available: &[true, true],
                requests: &[None],
                previous: &[None],
                mount: R::IDENTITY,
            },
            &mut ws,
            &mut out
        ),
        Err(PositionFitInputError::JointLayout)
    );
    assert_eq!(ws.achieved_axes(), accepted);
    m.fit(
        PositionFitInput {
            current_raw: &raw,
            available: &[false, true],
            requests: &[None],
            previous: &accepted,
            mount: R::IDENTITY,
        },
        &mut ws,
        &mut out,
    )
    .unwrap();
    assert_eq!(ws.achieved_axes()[pair[0]], None);
    assert!(ws.achieved_axes()[pair[1]].is_some());

    let mut velocity = p.clone();
    let function = &mut velocity.modes[0].channels[0].functions[0];
    function.angular_motion.as_mut().unwrap().kind = AngularMotionKind::AngularVelocity;
    function.behavior = ChannelFunctionBehavior::Continuous {
        physical_min: 0.,
        physical_max: 360.,
        unit: Some("deg/s".into()),
    };
    let m = model(&velocity, PositionInstallation::default());
    let mut ws = m.create_workspace();
    let mut out = m.create_output();
    m.fit(
        PositionFitInput {
            current_raw: &[0, raw[1]],
            available: &[true, true],
            requests: &[None],
            previous: &[Some(450.), Some(30.)],
            mount: R::IDENTITY,
        },
        &mut ws,
        &mut out,
    )
    .unwrap();
    assert_eq!(
        ws.achieved_axes()[m.emitter(0).unwrap().command_indices.unwrap()[0]],
        None
    );
}
