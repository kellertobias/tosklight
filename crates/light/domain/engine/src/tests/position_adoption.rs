//! Runtime-only command-pose adoption, verified from the ordinary engine renderer and forward
//! model. Synthetic profiles establish ownership/frame semantics, not lamp measurements.
use super::*;
use light_core::programming::JointAngles;
use light_fixture::{
    AngularMotion, AngularMotionKind, ChannelFunctionBehavior, EmitterHeadBinding, EmitterLayout,
    GeometryBracket, GeometryEmitter, GeometryPhysicalContract, InstalledPositionCalibration,
    MotionFunctionBinding, OpticalProvenance, PositionAxisRole, PositionPhysicalModel, Vector3,
};
use uuid::Uuid;

fn mover() -> PatchedFixture {
    let (mut fixture, _) = schema_v2_fixture(&[
        ("pan", false, false, false, false, false),
        ("tilt", false, false, false, false, false),
    ]);
    let mut profile = fixture
        .definition
        .profile_snapshot
        .as_deref()
        .unwrap()
        .clone();
    let head = profile.modes[0].heads[0].id;
    profile.geometry = GeometryGraph::template(GeometryTemplate::MovingHead, &[head]);
    profile.geometry.physical_contract = Some(GeometryPhysicalContract {
        version: 1,
        provenance: OpticalProvenance::default(),
        bracket: GeometryBracket::Fixed,
    });
    let mode = &mut profile.modes[0];
    mode.splits[0].footprint = 4;
    let mut bindings = Vec::new();
    for (index, role) in [PositionAxisRole::Pan, PositionAxisRole::Tilt]
        .into_iter()
        .enumerate()
    {
        let channel = &mut mode.channels[index];
        channel.resolution = ChannelResolution::U16;
        channel.secondary_slots = vec![2 + 2 * index as u16];
        channel.default_raw = 32768;
        channel.highlight_raw = 65535;
        channel.functions = vec![ChannelFunction::continuous(
            channel.attribute.0.to_string(),
            channel.attribute.clone(),
            65535,
        )];
        channel.functions[0].behavior = ChannelFunctionBehavior::Continuous {
            physical_min: -720.,
            physical_max: 720.,
            unit: Some("deg".into()),
        };
        channel.functions[0].angular_motion = Some(AngularMotion {
            kind: AngularMotionKind::AbsolutePosition,
            max_speed_degrees_per_second: None,
            acceleration_degrees_per_second_squared: None,
            deceleration_degrees_per_second_squared: None,
        });
        bindings.push(MotionFunctionBinding {
            node_id: profile.geometry.nodes[index + 1].id,
            channel_id: channel.id,
            function_id: channel.functions[0].id,
            role,
        });
    }
    mode.position_physical = Some(PositionPhysicalModel {
        kinematics: Default::default(),
        version: 1,
        revision: 1,
        bindings,
    });
    let emitter = GeometryEmitter {
        id: Uuid::new_v4(),
        name: "Lens".into(),
        node_id: profile.geometry.nodes[2].id,
        head_id: None,
        origin: Vector3::default(),
        orientation_degrees: Vector3::default(),
        beam_angle_degrees: 10.,
        field_angle_degrees: 20.,
        feather: 0.,
        focus: 1.,
        directional: true,
        layout: EmitterLayout::Point,
    };
    mode.emitter_heads = vec![EmitterHeadBinding {
        emitter_id: emitter.id,
        head_id: head,
    }];
    profile.geometry.emitters = vec![emitter];
    let mode_id = mode.id;
    fixture.definition = profile.resolved_definition(mode_id).unwrap();
    fixture
}

fn redefine(fixture: &mut PatchedFixture, edit: impl FnOnce(&mut FixtureProfile)) {
    let mut profile = fixture
        .definition
        .profile_snapshot
        .as_deref()
        .unwrap()
        .clone();
    let mode = fixture.definition.mode_id.unwrap();
    edit(&mut profile);
    fixture.definition = profile.resolved_definition(mode).unwrap();
}

fn engine(fixture: PatchedFixture) -> (Engine, ProgrammerRegistry, SessionId) {
    let programmers = ProgrammerRegistry::default();
    let session = SessionId::new();
    programmers.start(session);
    let engine = Engine::new(programmers.clone());
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    (engine, programmers, session)
}

fn set(
    programmers: &ProgrammerRegistry,
    session: SessionId,
    target: FixtureId,
    attribute: &str,
    value: f32,
) {
    programmers.set(
        session,
        target,
        AttributeKey(attribute.into()),
        AttributeValue::Normalized(value),
    );
}

#[test]
fn adoption_preserves_unwrapped_commands_and_common_independently_inverted_calibrated_copy() {
    let mut fixture = mover();
    let root = fixture.fixture_id;
    let calibration = InstalledPositionCalibration {
        pan_zero_degrees: 15.,
        tilt_zero_degrees: -10.,
        ..Default::default()
    };
    fixture.position_calibration = Some(calibration.clone());
    let copy = Uuid::new_v4();
    fixture.multipatch.push(MultiPatchInstance {
        id: copy,
        universe: Some(1),
        address: Some(10),
        invert_pan: true,
        position_calibration: Some(calibration),
        ..Default::default()
    });
    let (engine, programmers, session) = engine(fixture);
    set(&programmers, session, root, "pan", 1.);
    set(&programmers, session, root, "tilt", 0.);
    let rendered = engine.render(Default::default()).unwrap();
    assert_eq!(
        rendered.physical.instances[0].native_raw.as_ref(),
        &[65535, 0]
    );
    assert_eq!(rendered.physical.instances[1].native_raw.as_ref(), &[0, 0]);
    assert_eq!(
        engine.position_angles_from_physical(rendered.generation, &rendered.physical, root),
        Some(JointAngles {
            pan_degrees: 735.,
            tilt_degrees: -730.
        })
    );
    assert_eq!(
        engine.position_angles_from_physical(
            rendered.generation,
            &rendered.physical,
            FixtureId(copy)
        ),
        None,
        "physical copies are not selectable programming owners"
    );
    assert_eq!(
        engine.position_angles_from_physical(
            rendered.generation,
            &rendered.physical,
            FixtureId::new()
        ),
        None
    );
}

#[test]
fn adoption_withholds_divergent_copy_even_when_optical_angles_are_equivalent() {
    let mut fixture = mover();
    let root = fixture.fixture_id;
    fixture.multipatch.push(MultiPatchInstance {
        id: Uuid::new_v4(),
        universe: Some(1),
        address: Some(10),
        position_calibration: Some(InstalledPositionCalibration {
            pan_zero_degrees: 360.,
            ..Default::default()
        }),
        ..Default::default()
    });
    let (engine, programmers, session) = engine(fixture);
    set(&programmers, session, root, "pan", 1.);
    set(&programmers, session, root, "tilt", 0.);
    let rendered = engine.render(Default::default()).unwrap();
    let pans = rendered
        .physical
        .instances
        .iter()
        .map(|instance| {
            instance
                .axes()
                .iter()
                .find(|axis| axis.role == Some(PositionAxisRole::Pan))
                .unwrap()
                .absolute_degrees()
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(pans, [720., 1080.]);
    assert_eq!(
        engine.position_angles_from_physical(rendered.generation, &rendered.physical, root),
        None
    );
}

#[test]
fn adoption_requires_current_generation_and_exact_physical_layout_and_complete_axes() {
    let fixture = mover();
    let root = fixture.fixture_id;
    let (engine, _, _) = engine(fixture.clone());
    let rendered = engine.render(Default::default()).unwrap();
    assert!(
        engine
            .position_angles_from_physical(rendered.generation, &rendered.physical, root)
            .is_some()
    );
    assert_eq!(
        engine.position_angles_from_physical(rendered.generation + 1, &rendered.physical, root),
        None
    );
    let (foreign, _, _) = engine_fixture(fixture.clone());
    let foreign_frame = foreign.render(Default::default()).unwrap();
    assert_eq!(
        engine.position_angles_from_physical(rendered.generation, &foreign_frame.physical, root),
        None,
        "a foreign physical layout is rejected even with the caller's current generation number"
    );
    let current = engine.generation.load_full();
    let incomplete = current.physical_projection().take_frame();
    assert_eq!(
        engine.position_angles_from_physical(rendered.generation, &incomplete, root),
        None
    );
    engine
        .replace_snapshot(engine.snapshot().as_ref().clone())
        .unwrap();
    let replacement = engine.render(Default::default()).unwrap();
    assert_eq!(
        engine.position_angles_from_physical(rendered.generation, &rendered.physical, root),
        None
    );
    assert_eq!(
        engine.position_angles_from_physical(replacement.generation, &rendered.physical, root),
        None,
        "a same-show reload cannot reinterpret an older physical layout"
    );
    assert!(
        engine
            .position_angles_from_physical(replacement.generation, &replacement.physical, root)
            .is_some()
    );
}

// Avoid shadowing the engine constructor in a test which also names its real engine handle.
fn engine_fixture(fixture: PatchedFixture) -> (Engine, ProgrammerRegistry, SessionId) {
    engine(fixture)
}

#[test]
fn adoption_withholds_velocity_missing_role_and_unsupported_position_model() {
    for kind in 0..4 {
        let mut fixture = mover();
        let root = fixture.fixture_id;
        redefine(&mut fixture, |profile| {
            let mode = &mut profile.modes[0];
            match kind {
                0 => {
                    let function = &mut mode.channels[0].functions[0];
                    function.angular_motion.as_mut().unwrap().kind =
                        AngularMotionKind::AngularVelocity;
                    if let ChannelFunctionBehavior::Continuous { unit, .. } = &mut function.behavior
                    {
                        *unit = Some("deg/s".into());
                    }
                }
                1 => {
                    mode.position_physical.as_mut().unwrap().bindings.pop();
                }
                2 => {
                    mode.position_physical = None;
                    profile.geometry.physical_contract = None;
                }
                _ => {
                    let mut extra_pan = profile.geometry.nodes[1].clone();
                    extra_pan.id = Uuid::new_v4();
                    extra_pan.name = "Ambiguous second Pan".into();
                    extra_pan.parent_id = Some(profile.geometry.nodes[2].id);
                    let mut channel = mode.channels[0].clone();
                    channel.id = Uuid::new_v4();
                    channel.secondary_slots = vec![6];
                    channel.functions[0].id = Uuid::new_v4();
                    mode.position_physical
                        .as_mut()
                        .unwrap()
                        .bindings
                        .push(MotionFunctionBinding {
                            node_id: extra_pan.id,
                            channel_id: channel.id,
                            function_id: channel.functions[0].id,
                            role: PositionAxisRole::Pan,
                        });
                    mode.channels.push(channel);
                    mode.splits[0].footprint = 6;
                    profile.geometry.emitters[0].node_id = extra_pan.id;
                    profile.geometry.nodes.push(extra_pan);
                }
            }
        });
        let (engine, _, _) = engine(fixture);
        let rendered = engine.render(Default::default()).unwrap();
        assert!(rendered.physical.instances[0].complete);
        assert_eq!(
            engine.position_angles_from_physical(rendered.generation, &rendered.physical, root),
            None,
            "unknown or unsupported command pair kind {kind}"
        );
    }
}

#[test]
fn adoption_uses_each_owned_logical_emitter_with_shared_ancestral_pan() {
    let mut fixture = mover();
    let root = fixture.fixture_id;
    let head_ids = [Uuid::new_v4(), Uuid::new_v4()];
    redefine(&mut fixture, |profile| {
        profile.modes[0].heads[0].master_shared = true;
        profile.modes[0].heads[0].name = "Shared Pan".into();
        for (index, id) in head_ids.into_iter().enumerate() {
            profile.modes[0].heads.push(FixtureHead {
                id,
                name: format!("Lens {}", index + 1),
                master_shared: false,
            });
        }
        profile.modes[0].channels[1].head_id = head_ids[0];
        let mut tilt = profile.geometry.nodes[2].clone();
        tilt.id = Uuid::new_v4();
        tilt.name = "Second Tilt".into();
        tilt.parent_id = Some(profile.geometry.nodes[1].id);
        let mut channel = profile.modes[0].channels[1].clone();
        channel.id = Uuid::new_v4();
        channel.head_id = head_ids[1];
        channel.secondary_slots = vec![6];
        channel.functions[0].id = Uuid::new_v4();
        profile.modes[0]
            .position_physical
            .as_mut()
            .unwrap()
            .bindings
            .push(MotionFunctionBinding {
                node_id: tilt.id,
                channel_id: channel.id,
                function_id: channel.functions[0].id,
                role: PositionAxisRole::Tilt,
            });
        profile.modes[0].channels.push(channel);
        let mut lens = profile.geometry.emitters[0].clone();
        lens.id = Uuid::new_v4();
        lens.name = "Second lens".into();
        lens.node_id = tilt.id;
        profile.geometry.nodes.push(tilt);
        profile.geometry.emitters.push(lens);
        profile.modes[0].emitter_heads = profile
            .geometry
            .emitters
            .iter()
            .zip(head_ids)
            .map(|(lens, head_id)| EmitterHeadBinding {
                emitter_id: lens.id,
                head_id,
            })
            .collect();
        profile.modes[0].splits[0].footprint = 6;
    });
    let owners = [FixtureId::new(), FixtureId::new()];
    fixture.logical_heads = head_ids
        .into_iter()
        .zip(owners)
        .enumerate()
        .map(|(index, (profile_head_id, fixture_id))| PatchedHead {
            profile_head_id: Some(profile_head_id),
            fixture_id,
            head_index: (index + 1) as u16,
        })
        .collect();
    let (engine, programmers, session) = engine(fixture);
    set(&programmers, session, root, "pan", 1.);
    set(&programmers, session, owners[0], "tilt", 0.);
    set(&programmers, session, owners[1], "tilt", 1.);
    let rendered = engine.render(Default::default()).unwrap();
    assert_eq!(
        rendered.physical.instances[0].native_raw.as_ref(),
        &[65535, 0, 65535]
    );
    for (owner, tilt) in [(owners[0], -720.), (owners[1], 720.)] {
        assert_eq!(
            engine.position_angles_from_physical(rendered.generation, &rendered.physical, owner),
            Some(JointAngles {
                pan_degrees: 720.,
                tilt_degrees: tilt
            })
        );
    }
    assert_eq!(
        engine.position_angles_from_physical(rendered.generation, &rendered.physical, root),
        None,
        "shared motor ownership alone is not an invented complete root emitter pose"
    );
}

#[test]
fn observed_projection_stamps_its_captured_generation_before_and_after_reload() {
    let fixture = mover();
    let root = fixture.fixture_id;
    let (engine, programmers, session) = engine(fixture);
    set(&programmers, session, root, "pan", 1.);
    set(&programmers, session, root, "tilt", 0.);
    let capture = engine.prepare_observer_frame(Default::default());
    let generation = capture.generation();
    let observed = engine.observe_prepared_frame(&capture, &[]);
    let project = |source: &crate::ObservedSourceFrame| {
        engine
            .profile_observed_source_frame(source, Default::default(), &Default::default())
            .unwrap()
    };
    let projected = project(&observed);
    assert_eq!(
        projected.physical.instances[0].native_raw.as_ref(),
        &[65535, 0]
    );
    assert_eq!(
        engine.position_angles_from_physical(generation, &projected.physical, root),
        Some(JointAngles {
            pan_degrees: 720.,
            tilt_degrees: -720.
        })
    );
    engine
        .replace_snapshot(engine.snapshot().as_ref().clone())
        .unwrap();
    let fresh_capture = engine.prepare_observer_frame(Default::default());
    let fresh_generation = fresh_capture.generation();
    assert_ne!(generation, fresh_generation);
    let held = project(&observed);
    assert_eq!(held.physical.instances[0].native_raw.as_ref(), &[65535, 0]);
    assert!(held.physical.belongs_to_generation(generation));
    assert!(!held.physical.belongs_to_generation(fresh_generation));
    assert_eq!(
        engine.position_angles_from_physical(generation, &held.physical, root),
        None
    );
    assert_eq!(
        engine.position_angles_from_physical(fresh_generation, &held.physical, root),
        None
    );
    let fresh_observed = engine.observe_prepared_frame(&fresh_capture, &[]);
    let fresh = project(&fresh_observed);
    assert_eq!(
        engine.position_angles_from_physical(fresh_generation, &fresh.physical, root),
        Some(JointAngles {
            pan_degrees: 720.,
            tilt_degrees: -720.
        })
    );
}

#[test]
fn native_freeze_captures_divergent_physical_copies_without_approximating_one_angle_pair() {
    let mut fixture = mover();
    let root = fixture.fixture_id;
    let copy = Uuid::new_v4();
    fixture.multipatch.push(MultiPatchInstance {
        id: copy,
        invert_pan: true,
        position_calibration: Some(InstalledPositionCalibration {
            pan_zero_degrees: 360.,
            ..Default::default()
        }),
        ..Default::default()
    });
    let (engine, programmers, session) = engine(fixture);
    set(&programmers, session, root, "pan", 1.);
    set(&programmers, session, root, "tilt", 0.);
    let rendered = engine.render(Default::default()).unwrap();
    assert_eq!(
        engine.position_angles_from_physical(rendered.generation, &rendered.physical, root),
        None
    );
    let freeze = engine
        .position_freeze_from_physical(rendered.generation, &rendered.physical, root)
        .unwrap();
    assert_eq!(
        freeze
            .instances
            .iter()
            .map(|instance| instance.instance_id)
            .collect::<Vec<_>>(),
        [root.0, copy]
    );
    let words = freeze
        .instances
        .iter()
        .map(|instance| {
            instance
                .controls
                .iter()
                .map(|control| control.raw)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(words, [vec![65535, 0], vec![0, 0]]);
    assert_ne!(
        freeze.instances[0].controls[0].signature,
        freeze.instances[1].controls[0].signature
    );
    assert_eq!(
        engine.position_freeze_from_physical(
            rendered.generation,
            &rendered.physical,
            FixtureId(copy)
        ),
        None
    );
    engine
        .replace_snapshot(engine.snapshot().as_ref().clone())
        .unwrap();
    assert_eq!(
        engine.position_freeze_from_physical(rendered.generation, &rendered.physical, root),
        None
    );
}

#[test]
fn native_freeze_preserves_full_u32_words_and_rejects_angular_velocity() {
    let mut fixture = mover();
    let root = fixture.fixture_id;
    redefine(&mut fixture, |profile| {
        let mode = &mut profile.modes[0];
        mode.splits[0].footprint = 8;
        for (index, channel) in mode.channels.iter_mut().enumerate() {
            channel.resolution = ChannelResolution::U32;
            channel.secondary_slots = (2 + 4 * index as u16..=4 + 4 * index as u16).collect();
            channel.functions[0].dmx_to = u32::MAX;
        }
    });
    let (engine, programmers, session) = engine(fixture);
    for (attribute, raw) in [("pan", u32::MAX - 1), ("tilt", 17)] {
        programmers.set(
            session,
            root,
            AttributeKey(attribute.into()),
            AttributeValue::RawDmxExact(raw),
        );
    }
    let rendered = engine.render(Default::default()).unwrap();
    let freeze = engine
        .position_freeze_from_physical(rendered.generation, &rendered.physical, root)
        .unwrap();
    assert_eq!(
        freeze.instances[0]
            .controls
            .iter()
            .map(|control| control.raw)
            .collect::<Vec<_>>(),
        [u32::MAX - 1, 17]
    );
    let mut velocity = mover();
    let velocity_root = velocity.fixture_id;
    redefine(&mut velocity, |profile| {
        let function = &mut profile.modes[0].channels[0].functions[0];
        function.angular_motion.as_mut().unwrap().kind = AngularMotionKind::AngularVelocity;
        if let ChannelFunctionBehavior::Continuous { unit, .. } = &mut function.behavior {
            *unit = Some("deg/s".into());
        }
    });
    let (velocity_engine, _, _) = engine_fixture(velocity);
    let rendered = velocity_engine.render(Default::default()).unwrap();
    assert_eq!(
        velocity_engine.position_freeze_from_physical(
            rendered.generation,
            &rendered.physical,
            velocity_root
        ),
        None
    );
}

#[path = "position_adoption/freeze_replay.rs"]
mod freeze_replay;

#[path = "position_adoption/family_native.rs"]
mod family_native;
#[path = "position_adoption/root_emitter.rs"]
mod root_emitter;

#[path = "position_adoption/commanded_readout.rs"]
mod commanded_readout;

#[path = "position_adoption/axis_role_inversion.rs"]
mod axis_role_inversion;

#[path = "position_adoption/cue_fade_start.rs"]
mod cue_fade_start;
