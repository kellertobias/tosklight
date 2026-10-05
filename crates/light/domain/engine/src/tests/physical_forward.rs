use super::*;
use light_fixture::{
    ColorPhysicalModel, HeadOpticalPath, InstalledColorCalibration, InstalledColorPathCalibration,
    InstalledEmitterCalibration, NativeColorBinding, OpticalEmitter, OpticalEmitterBand,
    OpticalProvenance, OpticalSource, PhysicalDataQuality,
};

fn physical_fixture() -> PatchedFixture {
    let mut fixture = calibrated_visual_fixture(FixtureId::new());
    let mut profile = fixture
        .definition
        .profile_snapshot
        .as_deref()
        .unwrap()
        .clone();
    let mode = &mut profile.modes[0];
    let ColorSystem::Additive { emitters } = &mode.color_systems[0].system else {
        unreachable!()
    };
    let source = emitters
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let c = mode.channels.iter().find(|c| c.id == e.channel_id).unwrap();
            OpticalEmitter {
                id: uuid::Uuid::new_v4(),
                name: e.name.clone(),
                binding: NativeColorBinding {
                    channel_id: c.id,
                    function_id: c.functions[0].id,
                },
                xyz: if i == 2 { None } else { Some(e.xyz) },
                spectrum: vec![],
                band: if i == 2 {
                    OpticalEmitterBand::Ultraviolet
                } else {
                    OpticalEmitterBand::Visible
                },
                native_reversed: c.invert,
                maximum_level: 1.,
                response_exponent: e.response_curve,
                provenance: OpticalProvenance {
                    quality: PhysicalDataQuality::Estimated,
                    ..Default::default()
                },
            }
        })
        .collect();
    mode.color_physical = Some(ColorPhysicalModel {
        version: 1,
        revision: 1,
        paths: vec![HeadOpticalPath {
            id: uuid::Uuid::new_v4(),
            head_id: mode.heads[0].id,
            controls: mode
                .channels
                .iter()
                .filter(|c| c.attribute.0.starts_with("color."))
                .map(|c| c.id)
                .collect(),
            source: OpticalSource::Additive { emitters: source },
            filters: vec![],
            measurements: vec![],
        }],
    });
    let mode_id = mode.id;
    fixture.definition = profile.resolved_definition(mode_id).unwrap();
    fixture
}
fn gain(fixture: &PatchedFixture, gain: f32) -> InstalledColorCalibration {
    let profile = fixture.definition.profile_snapshot.as_deref().unwrap();
    let mode = profile.mode(fixture.definition.mode_id.unwrap()).unwrap();
    let path = &mode.color_physical.as_ref().unwrap().paths[0];
    let OpticalSource::Additive { emitters } = &path.source else {
        unreachable!()
    };
    InstalledColorCalibration {
        version: 1,
        revision: 1,
        paths: vec![InstalledColorPathCalibration {
            source_identity: profile
                .native_color_identity(mode.id, path.head_id)
                .unwrap(),
            emitters: vec![InstalledEmitterCalibration {
                emitter_id: emitters[0].id,
                output_gain: gain,
                provenance: OpticalProvenance {
                    quality: PhysicalDataQuality::Estimated,
                    ..Default::default()
                },
            }],
            measurements: vec![],
        }],
    }
}
fn engine_with(fixture: PatchedFixture) -> (Engine, SessionId) {
    let p = ProgrammerRegistry::default();
    let session = SessionId::new();
    p.start(session);
    for (name, value) in [
        ("intensity", 1.),
        ("color.red", 0.8),
        ("color.green", 0.2),
        ("color.blue", 1.),
    ] {
        p.set(
            session,
            fixture.fixture_id,
            AttributeKey(name.into()),
            AttributeValue::Normalized(value),
        );
    }
    let engine = Engine::new(p);
    engine
        .replace_snapshot(EngineSnapshot {
            fixtures: vec![fixture].into(),
            revision: 1,
            ..Default::default()
        })
        .unwrap();
    (engine, session)
}
#[test]
fn final_native_predictions_distinguish_copy_calibration_and_unknown_uv() {
    let mut fixture = physical_fixture();
    fixture.color_calibration = Some(gain(&fixture, 0.5));
    let copy = MultiPatchInstance {
        id: uuid::Uuid::new_v4(),
        universe: Some(1),
        address: Some(10),
        color_calibration: Some(gain(&fixture, 0.25)),
        ..Default::default()
    };
    fixture.multipatch = vec![
        copy,
        MultiPatchInstance {
            id: uuid::Uuid::new_v4(),
            ..Default::default()
        },
    ];
    let (engine, _) = engine_with(fixture);
    let output = engine.render(RenderOptions::default()).unwrap();
    let physical = &output.physical.instances;
    assert_eq!(physical.len(), 3);
    assert!(physical.iter().all(|i| i.complete));
    assert_eq!(
        physical[0].native_raw.as_ref(),
        &output.universes[&1][0..4]
            .iter()
            .map(|v| u32::from(*v))
            .collect::<Vec<_>>()
    );
    assert_eq!(physical[0].native_raw, physical[1].native_raw);
    assert_eq!(physical[0].native_raw, physical[2].native_raw);
    let a = &physical[0].colors()[0];
    let b = &physical[1].colors()[0];
    let c = &physical[2].colors()[0];
    assert!((a.known_xyz.x - b.known_xyz.x * 2.).abs() < 1e-7);
    assert!((c.known_xyz.x - a.known_xyz.x * 2.).abs() < 1e-7);
    assert!(!a.visible_complete);
    assert_eq!(a.uv_drive_max, 1.);
    assert!(a.known_xyz.y > 0.);
    let raw_red = f64::from(physical[0].native_raw[1]);
    assert!((f64::from(a.known_xyz.x) - (1. - raw_red / 255.).powi(2) * 0.5).abs() < 1e-7);
}
#[test]
fn unpatched_uv_only_copies_retain_native_activity_without_visible_white() {
    let mut fixture = physical_fixture();
    fixture.universe = None;
    fixture.address = None;
    fixture.multipatch.push(MultiPatchInstance {
        id: uuid::Uuid::new_v4(),
        ..Default::default()
    });
    let id = fixture.fixture_id;
    let (engine, session) = engine_with(fixture);
    engine.programmers.set(
        session,
        id,
        AttributeKey("color.red".into()),
        AttributeValue::Normalized(0.),
    );
    engine.programmers.set(
        session,
        id,
        AttributeKey("color.green".into()),
        AttributeValue::Normalized(0.),
    );
    let output = engine.render(RenderOptions::default()).unwrap();
    assert!(output.universes.is_empty());
    for instance in &output.physical.instances {
        assert!(instance.complete);
        let color = &instance.colors()[0];
        assert_eq!(
            color.known_xyz,
            Xyz {
                x: 0.,
                y: 0.,
                z: 0.
            }
        );
        assert_eq!(color.uv_drive_max, 1.);
        assert!(!color.visible_complete);
    }
}
#[test]
fn frame_loans_reuse_nested_uv_and_native_buffers_without_mutating_held_results() {
    let fixture = physical_fixture();
    let id = fixture.fixture_id;
    let (engine, session) = engine_with(fixture);
    let first = engine.render(RenderOptions::default()).unwrap();
    let original = first.physical.instances[0].colors()[0].clone();
    let second = engine.render(RenderOptions::default()).unwrap();
    let raw = second.physical.instances[0].native_raw.as_ptr();
    let uv = second.physical.instances[0].colors()[0]
        .uv_emitters
        .as_ptr();
    drop(second);
    engine.programmers.set(
        session,
        id,
        AttributeKey("color.blue".into()),
        AttributeValue::Normalized(0.),
    );
    for _ in 0..20 {
        let result = engine.render(RenderOptions::default()).unwrap();
        assert_eq!(result.physical.instances[0].native_raw.as_ptr(), raw);
        assert_eq!(
            result.physical.instances[0].colors()[0]
                .uv_emitters
                .as_ptr(),
            uv
        );
        assert!(result.physical.instances[0].colors()[0].visible_complete);
    }
    assert_eq!(first.physical.instances[0].colors()[0], original);
}

#[test]
fn preload_projection_matches_live_native_state_for_every_physical_copy() {
    let mut fixture = physical_fixture();
    fixture.multipatch.push(MultiPatchInstance {
        id: uuid::Uuid::new_v4(),
        color_calibration: Some(gain(&fixture, 0.3)),
        ..Default::default()
    });
    let (engine, _) = engine_with(fixture);
    let options = RenderOptions {
        grand_master: 0.37,
        ..Default::default()
    };
    let live = engine.render(options).unwrap();
    // A projection is handed finalized output parameters (masters included, 2026-10-05).
    let preview = engine
        .profile_visualization_projection(live.resolved_values.values(), options)
        .unwrap();
    for (a, b) in live
        .physical
        .instances
        .iter()
        .zip(&preview.physical.instances)
    {
        assert_eq!(a.instance_id, b.instance_id);
        assert_eq!(a.native_raw, b.native_raw);
        assert_eq!(a.colors(), b.colors());
    }
}
#[test]
fn color_path_sees_shared_channels_even_when_shared_head_resolves_last() {
    let mut fixture = physical_fixture();
    let mut profile = fixture
        .definition
        .profile_snapshot
        .as_deref()
        .unwrap()
        .clone();
    let mode = &mut profile.modes[0];
    let shared = mode.heads[0].id;
    let child = uuid::Uuid::new_v4();
    mode.heads.insert(
        0,
        FixtureHead {
            id: child,
            name: "First child".into(),
            master_shared: false,
        },
    );
    mode.heads[1].master_shared = true;
    mode.channels[1].head_id = child;
    mode.channels[2].head_id = child;
    assert_eq!(mode.channels[3].head_id, shared);
    // Legacy systems need own-head channels; physical paths explicitly admit shared controls.
    mode.color_systems.clear();
    mode.color_physical.as_mut().unwrap().paths[0].head_id = child;
    let mode_id = mode.id;
    fixture.definition = profile.resolved_definition(mode_id).unwrap();
    let child_fixture = FixtureId::new();
    fixture.logical_heads = vec![PatchedHead {
        profile_head_id: Some(child),
        head_index: 0,
        fixture_id: child_fixture,
    }];
    let (engine, session) = engine_with(fixture);
    engine.programmers.set(
        session,
        child_fixture,
        AttributeKey("color.green".into()),
        AttributeValue::Normalized(0.2),
    );
    let result = engine.render(RenderOptions::default()).unwrap();
    let head = &result.physical.instances[0].colors()[0];
    assert_eq!(head.head_id, child);
    assert_eq!(head.uv_drive_max, 1.);
    assert!(head.known_xyz.y > 0.);
    assert!(!head.visible_complete);
}
#[test]
fn encoded_noncontiguous_fine_bytes_reproduce_the_same_native_color_prediction() {
    for (resolution, secondary) in [
        (ChannelResolution::U16, vec![5]),
        (ChannelResolution::U24, vec![5, 6]),
        (ChannelResolution::U32, vec![5, 6, 7]),
    ] {
        let mut fixture = physical_fixture();
        let mut profile = fixture
            .definition
            .profile_snapshot
            .as_deref()
            .unwrap()
            .clone();
        let mode = &mut profile.modes[0];
        mode.channels[1].resolution = resolution;
        mode.channels[1].secondary_slots = secondary.clone();
        mode.channels[1].functions[0].dmx_to = resolution.max_raw();
        mode.channels[1].highlight_raw = resolution.max_raw();
        mode.splits[0].footprint = 4 + secondary.len() as u16;
        let mode_id = mode.id;
        fixture.definition = profile.resolved_definition(mode_id).unwrap();
        let profile = fixture
            .definition
            .profile_snapshot
            .as_deref()
            .unwrap()
            .clone();
        let (engine, _) = engine_with(fixture);
        let result = engine.render(RenderOptions::default()).unwrap();
        let actual = &result.physical.instances[0];
        let bytes = &result.universes[&1];
        let mut raw = u32::from(bytes[1]);
        for slot in secondary {
            raw = (raw << 8) | u32::from(bytes[usize::from(slot) - 1]);
        }
        assert_eq!(raw, actual.native_raw[1]);
        let forward =
            light_fixture::forward::CompiledColorForward::compile(&profile, mode_id, None)
                .unwrap()
                .unwrap();
        let mut out = forward.create_output();
        let mut decoded = actual.native_raw.to_vec();
        decoded[1] = raw;
        forward.evaluate(&decoded, &mut out).unwrap();
        assert_eq!(out, actual.colors());
    }
}
#[test]
fn non_dmx_profile_receives_physical_prediction() {
    let mut fixture = physical_fixture();
    let mut profile = fixture
        .definition
        .profile_snapshot
        .as_deref()
        .unwrap()
        .clone();
    profile.patch_policy = light_fixture::PatchPolicy::VisualOnly;
    let mode = &mut profile.modes[0];
    mode.channels.clear();
    mode.color_systems.clear();
    mode.splits[0].footprint = 0;
    let path = &mut mode.color_physical.as_mut().unwrap().paths[0];
    path.controls.clear();
    path.source = OpticalSource::Fixed {
        xyz: Some(Xyz {
            x: 0.2,
            y: 0.3,
            z: 0.1,
        }),
        spectrum: vec![],
        provenance: OpticalProvenance::default(),
    };
    fixture.definition = profile
        .resolved_definition(fixture.definition.mode_id.unwrap())
        .unwrap();
    fixture.universe = None;
    fixture.address = None;
    fixture.fixture_number = None;
    fixture.virtual_fixture_number = Some(1);
    let (engine, _) = engine_with(fixture);
    let result = engine.render(RenderOptions::default()).unwrap();
    assert!(result.universes.is_empty());
    assert_eq!(result.physical.instances.len(), 1);
    assert!(
        result
            .physical
            .instances
            .iter()
            .all(|i| i.complete && i.colors()[0].known_xyz.y == 0.3)
    );
}

#[test]
fn portable_compaction_keeps_full_authoring_calibration_identity() {
    use light_fixture::{
        PatchedFixtureCompiler, PortablePatchedFixtureRecord, ResolvedFixtureProfileRevision,
        fixture_profile_content_digest,
    };
    let mut fixture = physical_fixture();
    let mut profile = fixture
        .definition
        .profile_snapshot
        .as_deref()
        .unwrap()
        .clone();
    let mut other = profile.modes[0].clone();
    other.id = uuid::Uuid::new_v4();
    other.name = "Other mode retained in authoring identity".into();
    profile.modes.push(other);
    fixture.definition = profile
        .resolved_definition(fixture.definition.mode_id.unwrap())
        .unwrap();
    fixture.color_calibration = Some(gain(&fixture, 0.5));
    fixture.multipatch.push(MultiPatchInstance {
        id: uuid::Uuid::new_v4(),
        color_calibration: Some(gain(&fixture, 0.25)),
        ..Default::default()
    });
    let json = serde_json::to_value(&profile).unwrap();
    let resolved = ResolvedFixtureProfileRevision::new(
        profile.id,
        profile.revision.into(),
        fixture_profile_content_digest(&json).unwrap(),
        json,
    );
    let record = PortablePatchedFixtureRecord::from_runtime_fixture(&fixture).unwrap();
    let mut compiler = PatchedFixtureCompiler::new(move |_| Some(resolved.clone()));
    let compact = compiler.compile(&record).unwrap();
    assert_eq!(
        compact
            .definition
            .profile_snapshot
            .as_ref()
            .unwrap()
            .modes
            .len(),
        1
    );
    assert!(compact.definition.runtime_color_context.is_some());
    let (original, _) = engine_with(fixture);
    let (reloaded, _) = engine_with(compact.clone());
    let a = original.render(RenderOptions::default()).unwrap();
    let b = reloaded.render(RenderOptions::default()).unwrap();
    for (a, b) in a.physical.instances.iter().zip(&b.physical.instances) {
        assert_eq!(a.colors(), b.colors());
        assert_eq!(
            b.colors()[0].flags.0 & 64,
            0,
            "valid calibration is not stale"
        );
    }
    // Runtime authority cannot be smuggled through serialized show data.
    let serialized = serde_json::to_value(&compact.definition).unwrap();
    assert!(serialized.get("runtime_color_context").is_none());
    // Nor may a caller reuse the trusted context for a changed optical channel.
    let mut changed = compact
        .definition
        .profile_snapshot
        .as_deref()
        .unwrap()
        .clone();
    changed.modes[0].channels[1].invert = !changed.modes[0].channels[1].invert;
    assert!(
        compact
            .definition
            .runtime_color_context
            .as_ref()
            .unwrap()
            .validate_runtime_profile(&changed, changed.modes[0].id)
            .is_err()
    );
}

#[test]
fn preload_forward_prediction_honors_captured_color_freeze() {
    let mut fixture = physical_fixture();
    fixture.freeze = FixtureFreezeState {
        targets: HashMap::from([(
            fixture.fixture_id,
            FrozenFixtureTarget {
                position_native: None,
                full: false,
                families: vec![FreezeFamily::Color],
                values: HashMap::from([
                    (
                        AttributeKey("color.red".into()),
                        AttributeValue::Normalized(0.25),
                    ),
                    (
                        AttributeKey("color.green".into()),
                        AttributeValue::Normalized(0.5),
                    ),
                    (
                        AttributeKey("color.blue".into()),
                        AttributeValue::Normalized(0.),
                    ),
                ]),
            },
        )]),
    };
    let (engine, _) = engine_with(fixture);
    let options = RenderOptions::default();
    let live = engine.render(options).unwrap();
    let preload = engine
        .profile_visualization_projection(&engine.resolved_values(), options)
        .unwrap();
    assert_eq!(
        live.physical.instances[0].native_raw,
        preload.physical.instances[0].native_raw
    );
    assert_eq!(
        live.physical.instances[0].colors(),
        preload.physical.instances[0].colors()
    );
    assert_eq!(preload.physical.instances[0].colors()[0].uv_drive_max, 0.);
}

/// TL-553: a pooled instance whose final native values are unchanged keeps its forward results
/// instead of re-evaluating them. Every frame must equal a fresh evaluation of its values.
#[test]
fn unchanged_native_values_reuse_forward_results_identical_to_a_fresh_evaluation() {
    let fixture = physical_fixture();
    let id = fixture.fixture_id;
    let (engine, session) = engine_with(fixture);
    let generation = engine.generation.load_full();
    let fresh = |instance: &crate::PhysicalInstanceOutput| {
        let index = crate::physical_projection::PhysicalProjectionIndex::compile(
            &generation.snapshot_arc(),
        );
        let mut frame = index.take_frame();
        let raw = (0u32..)
            .zip(instance.native_raw.iter().copied())
            .collect::<Vec<_>>();
        index.evaluate(id, 0, &raw, &mut frame).unwrap();
        let output = &frame.instances[0];
        (
            output.colors().to_vec(),
            output.axes().to_vec(),
            output.lenses().to_vec(),
            output.optics().to_vec(),
        )
    };
    let mut held = Vec::new();
    for step in 0..16 {
        if step % 4 == 3 {
            engine.programmers.set(
                session,
                id,
                AttributeKey("color.blue".into()),
                AttributeValue::Normalized(step as f32 / 16.),
            );
        }
        let result = engine.render(RenderOptions::default()).unwrap();
        let instance = &result.physical.instances[0];
        assert!(instance.complete, "step {step}");
        let observed = (
            instance.colors().to_vec(),
            instance.axes().to_vec(),
            instance.lenses().to_vec(),
            instance.optics().to_vec(),
        );
        assert_eq!(observed, fresh(instance), "step {step}");
        // Holding some frames makes the pool hand out different loans with older values.
        if step % 5 == 0 {
            held.push(result);
        }
    }
}

/// A rejected layout clears the reuse, so the next valid values are evaluated again.
#[test]
fn a_rejected_physical_layout_never_reuses_stale_forward_results() {
    let fixture = physical_fixture();
    let id = fixture.fixture_id;
    let (engine, _) = engine_with(fixture);
    let generation = engine.generation.load_full();
    let index =
        crate::physical_projection::PhysicalProjectionIndex::compile(&generation.snapshot_arc());
    let mut frame = index.take_frame();
    let channels = frame.instances[0].native_raw.len() as u32;
    let raw = |level: u32| (0..channels).map(|i| (i, level)).collect::<Vec<_>>();
    index.evaluate(id, 0, &raw(255), &mut frame).unwrap();
    let bright = frame.instances[0].colors().to_vec();
    index.evaluate(id, 0, &raw(0), &mut frame).unwrap();
    let dark = frame.instances[0].colors().to_vec();
    assert_ne!(bright, dark);
    // A duplicate channel is rejected after the earlier channels were already stored.
    let mut invalid = raw(255);
    invalid[1] = (0, 255);
    assert!(index.evaluate(id, 0, &invalid, &mut frame).is_err());
    assert!(!frame.instances[0].complete);
    index.evaluate(id, 0, &raw(255), &mut frame).unwrap();
    assert!(frame.instances[0].complete);
    assert_eq!(frame.instances[0].colors(), bright);
}

/// TL-639: the output path only captures native values; forward models run on first read, on
/// whichever thread reads, and give exactly the results of an evaluation of those values.
#[test]
fn forward_results_are_evaluated_on_first_read_and_equal_a_fresh_evaluation() {
    let fixture = physical_fixture();
    let id = fixture.fixture_id;
    let (engine, session) = engine_with(fixture);
    let generation = engine.generation.load_full();
    let index =
        crate::physical_projection::PhysicalProjectionIndex::compile(&generation.snapshot_arc());
    for step in 0..6 {
        engine.programmers.set(
            session,
            id,
            AttributeKey("color.blue".into()),
            AttributeValue::Normalized(step as f32 / 6.),
        );
        let result = std::sync::Arc::new(engine.render(RenderOptions::default()).unwrap());
        let instance = &result.physical.instances[0];
        assert!(instance.complete, "step {step}");
        assert!(
            !instance.forward_evaluated(),
            "the render evaluated no forward model"
        );
        let reader = std::sync::Arc::clone(&result);
        let observed = std::thread::spawn(move || {
            let instance = &reader.physical.instances[0];
            (
                instance.colors().to_vec(),
                instance.axes().to_vec(),
                instance.lenses().to_vec(),
                instance.optics().to_vec(),
            )
        })
        .join()
        .unwrap();
        assert!(instance.forward_evaluated());
        let mut frame = index.take_frame();
        let raw = (0u32..)
            .zip(instance.native_raw.iter().copied())
            .collect::<Vec<_>>();
        index.evaluate(id, 0, &raw, &mut frame).unwrap();
        let fresh = &frame.instances[0];
        assert_eq!(
            observed,
            (
                fresh.colors().to_vec(),
                fresh.axes().to_vec(),
                fresh.lenses().to_vec(),
                fresh.optics().to_vec(),
            ),
            "step {step}"
        );
    }
}

/// TL-639: changed native values discard the held results even when nobody read them, and an
/// unread frame that is recycled with changed values never shows a stale result.
#[test]
fn changed_native_values_are_never_read_through_stale_forward_results() {
    let fixture = physical_fixture();
    let id = fixture.fixture_id;
    let (engine, session) = engine_with(fixture);
    let read = |level: f32| {
        engine.programmers.set(
            session,
            id,
            AttributeKey("color.red".into()),
            AttributeValue::Normalized(level),
        );
        let result = engine.render(RenderOptions::default()).unwrap();
        result.physical.instances[0].colors()[0].clone()
    };
    let bright = read(1.);
    let dark = read(0.);
    assert_ne!(bright, dark);
    // Rendered but unread, then recycled with the earlier values.
    engine.programmers.set(
        session,
        id,
        AttributeKey("color.red".into()),
        AttributeValue::Normalized(0.5),
    );
    drop(engine.render(RenderOptions::default()).unwrap());
    assert_eq!(read(1.), bright);
    assert_eq!(read(0.), dark);
}

/// TL-639: instances of one profile snapshot and mode with identical installation inputs share
/// one compiled forward model; a different calibration or a separate snapshot compiles its own.
#[test]
fn identical_installations_share_compiled_forward_models() {
    let first = physical_fixture();
    let mut second = first.clone();
    second.fixture_id = FixtureId::new();
    let mut calibrated = first.clone();
    calibrated.fixture_id = FixtureId::new();
    calibrated.color_calibration = Some(gain(&first, 0.4));
    let mut separate = first.clone();
    separate.fixture_id = FixtureId::new();
    separate.definition.profile_snapshot = Some(std::sync::Arc::new(
        first
            .definition
            .profile_snapshot
            .as_deref()
            .unwrap()
            .clone(),
    ));
    let snapshot = EngineSnapshot {
        fixtures: vec![
            first.clone(),
            second.clone(),
            calibrated.clone(),
            separate.clone(),
        ]
        .into(),
        revision: 1,
        ..Default::default()
    };
    let index = crate::physical_projection::PhysicalProjectionIndex::compile(&snapshot);
    let model = |fixture: &PatchedFixture| {
        index
            .color_forward(fixture.fixture_id, fixture.fixture_id.0)
            .unwrap() as *const _
    };
    assert_eq!(model(&first), model(&second));
    assert_ne!(model(&first), model(&calibrated));
    assert_ne!(model(&first), model(&separate));
    // A shared model evaluates exactly like the model compiled for that fixture alone.
    let alone = crate::physical_projection::PhysicalProjectionIndex::compile(&EngineSnapshot {
        fixtures: vec![second.clone()].into(),
        revision: 1,
        ..Default::default()
    });
    let mut shared_frame = index.take_frame();
    let mut alone_frame = alone.take_frame();
    let channels = shared_frame.instances[1].native_raw.len() as u32;
    let raw = (0..channels).map(|i| (i, 40 + i * 7)).collect::<Vec<_>>();
    index
        .evaluate(second.fixture_id, 0, &raw, &mut shared_frame)
        .unwrap();
    alone
        .evaluate(second.fixture_id, 0, &raw, &mut alone_frame)
        .unwrap();
    assert_eq!(
        shared_frame.instances[1].colors(),
        alone_frame.instances[0].colors()
    );
    assert_eq!(
        shared_frame.instances[1].optics(),
        alone_frame.instances[0].optics()
    );
}
