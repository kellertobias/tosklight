//! Accepted Position writes enter the existing engine projection before its final overlays.
//! This tests the gated staging API; it does not enable the production scheduler contract.
use super::*;
use light_engine::{PositionNativeWrite, RenderOptions, RenderResult};

fn projection_rig() -> (Rig, FixtureId) {
    let profile = moving_head();
    let root = FixtureId::new();
    let copy = FixtureId::new();
    let mut fixture = patched(&profile, root, 1);
    fixture.invert_pan = true;
    fixture.position_calibration = Some(InstalledPositionCalibration {
        pan_zero_degrees: 31.,
        tilt_zero_degrees: -17.,
        ..Default::default()
    });
    fixture.multipatch = vec![MultiPatchInstance {
        id: copy.0,
        universe: Some(1),
        address: Some(10),
        invert_tilt: true,
        position_calibration: Some(InstalledPositionCalibration {
            pan_zero_degrees: -41.,
            tilt_zero_degrees: 23.,
            ..Default::default()
        }),
        ..Default::default()
    }];
    let rig = Rig::new(vec![fixture], root);
    rig.programmers.set(
        rig.session,
        root,
        ProgrammingOwner::Position.key(),
        angles(31., 17.),
    );
    // Existing scalar controls are intentionally different. Staged native controls own output.
    rig.set(root, "pan", 0.1);
    rig.set(root, "tilt", 0.9);
    (rig, copy)
}

fn fitted_writes(rig: &Rig, requests: &[(FixtureId, AttributeValue)]) -> Vec<PositionNativeWrite> {
    let resolved = rig.resolve(requests);
    rig.verify(&resolved);
    resolved
        .results
        .iter()
        .zip(requests)
        .flat_map(|(result, (target, _))| {
            result.writes.iter().map(move |write| PositionNativeWrite {
                target: *target,
                instance_id: write.slot.destination.0,
                channel_index: write.slot.channel_index,
                channel_id: write.channel_id,
                function_id: write.function_id,
                split: write.slot.split,
                raw: write.raw,
            })
        })
        .collect()
}

fn expected_instance(writes: &[PositionNativeWrite], instance: Uuid) -> Vec<u32> {
    let mut values = vec![0; 2];
    for write in writes.iter().filter(|write| write.instance_id == instance) {
        values[write.channel_index as usize] = write.raw;
    }
    values
}

fn assert_physical(physical: &light_engine::PhysicalForwardFrame, writes: &[PositionNativeWrite]) {
    for instance in &physical.instances {
        assert_eq!(
            instance.native_raw.as_ref(),
            expected_instance(writes, instance.instance_id),
            "physical forward calculation consumes the exact encoded native commands"
        );
    }
}

fn assert_native_output(
    rendered: &RenderResult,
    rig: &Rig,
    copy: FixtureId,
    writes: &[PositionNativeWrite],
) {
    let bytes = &rendered.universes[&1];
    for (instance, start) in [(rig.root.0, 0), (copy.0, 9)] {
        let actual = vec![
            u32::from(u16::from_be_bytes([bytes[start], bytes[start + 1]])),
            u32::from(u16::from_be_bytes([bytes[start + 2], bytes[start + 3]])),
        ];
        assert_eq!(
            actual,
            expected_instance(writes, instance),
            "installation inversion and calibration must not be applied to fitted Raw commands again"
        );
    }
    assert_physical(&rendered.physical, writes);
}

#[test]
fn staged_fitted_position_writes_reach_live_preview_and_preload_native_output_once() {
    let (rig, copy) = projection_rig();
    let writes = fitted_writes(&rig, &[(rig.root, angles(450., 30.))]);
    assert_ne!(
        expected_instance(&writes, rig.root.0),
        expected_instance(&writes, copy.0)
    );
    let capture = rig.capture();
    let token = capture.frame_token();
    let mut preview = rig.engine.prepare_static_family_frame(&capture, &[]);
    preview
        .project_position_native(&capture, &token, &writes)
        .unwrap();
    let previewed = rig
        .engine
        .preview_static_family_frame(&capture, preview)
        .unwrap();
    assert_native_output(&previewed, &rig, copy, &writes);
    let mut live = rig.engine.prepare_static_family_frame(&capture, &[]);
    live.project_position_native(&capture, &token, &writes)
        .unwrap();
    let rendered = rig
        .engine
        .render_static_family_frame(&capture, live)
        .unwrap();
    assert_native_output(&rendered, &rig, copy, &writes);
    assert_eq!(rendered.universes, previewed.universes);

    // A distinct pending branch owns its own token and staged commands; it never uses Live's token.
    let capture = rig.capture();
    let input = rig.engine.prepare_preload_frame(&capture, None);
    let mut state = light_engine::PreloadFrameState::default();
    let branch = PreloadBranch::AfterRelease;
    let token = input.frame_token(&state, branch);
    let mut pending = rig
        .engine
        .prepare_preload_static_family_frame(&input, &[], &state, branch);
    pending
        .project_position_native(&capture, &token, &writes)
        .unwrap();
    let rendered = rig
        .engine
        .render_prepared_preload_families(&input, None, pending, &mut state)
        .unwrap();
    assert_physical(&rendered.projection.physical, &writes);
}

#[test]
fn rejected_native_position_batches_are_atomic_against_the_actual_rendered_baseline() {
    let (rig, _) = projection_rig();
    let writes = fitted_writes(&rig, &[(rig.root, angles(450., 30.))]);
    let capture = rig.capture();
    let token = capture.frame_token();
    let baseline = rig
        .engine
        .preview_static_family_frame(
            &capture,
            rig.engine.prepare_static_family_frame(&capture, &[]),
        )
        .unwrap();
    let mut invalid = Vec::new();
    let mut batch = writes.clone();
    batch.last_mut().unwrap().target = FixtureId::new();
    invalid.push(("foreign owner", batch));
    let mut batch = writes.clone();
    batch.last_mut().unwrap().instance_id = Uuid::new_v4();
    invalid.push(("foreign instance", batch));
    let mut batch = writes.clone();
    batch.last_mut().unwrap().channel_id = Uuid::new_v4();
    invalid.push(("foreign channel UUID", batch));
    let mut batch = writes.clone();
    batch.last_mut().unwrap().channel_index = 99;
    invalid.push(("unknown channel index", batch));
    let mut batch = writes.clone();
    batch.last_mut().unwrap().split = 2;
    invalid.push(("wrong split", batch));
    let mut batch = writes.clone();
    batch.last_mut().unwrap().function_id = Some(Uuid::new_v4());
    invalid.push(("foreign function binding", batch));
    let mut batch = writes.clone();
    batch.last_mut().unwrap().raw = u32::from(u16::MAX) + 1;
    invalid.push(("outside full-width range", batch));
    let mut batch = writes.clone();
    batch.pop();
    invalid.push(("incomplete axis footprint", batch));
    let mut batch = writes.clone();
    batch.retain(|write| write.instance_id == rig.root.0);
    invalid.push(("missing whole multipatch instance", batch));
    let mut batch = writes.clone();
    batch.push(writes[0].clone());
    invalid.push(("duplicate owner channel", batch));
    for (reason, batch) in invalid {
        let mut frame = rig.engine.prepare_static_family_frame(&capture, &[]);
        assert!(
            frame
                .project_position_native(&capture, &token, &batch)
                .is_err(),
            "must reject {reason}"
        );
        let rendered = rig
            .engine
            .preview_static_family_frame(&capture, frame)
            .unwrap();
        assert_eq!(
            rendered.universes, baseline.universes,
            "{reason} must not partially stage earlier valid writes"
        );
        for (actual, expected) in rendered
            .physical
            .instances
            .iter()
            .zip(&baseline.physical.instances)
        {
            assert_eq!(
                actual.native_raw, expected.native_raw,
                "{reason} must preserve physical baseline"
            );
        }
    }
    let foreign = rig.capture();
    for foreign_capture in [false, true] {
        let mut frame = rig.engine.prepare_static_family_frame(&capture, &[]);
        let result = if foreign_capture {
            frame.project_position_native(&foreign, &token, &writes)
        } else {
            frame.project_position_native(&capture, &foreign.frame_token(), &writes)
        };
        assert!(
            result.is_err(),
            "foreign capture or token is rejected before staging"
        );
        let rendered = rig
            .engine
            .preview_static_family_frame(&capture, frame)
            .unwrap();
        assert_eq!(rendered.universes, baseline.universes);
    }
}

fn shared_projection_rig() -> (Rig, [FixtureId; 2]) {
    let mut profile = moving_head();
    profile.modes[0].heads[0].master_shared = true;
    let head_ids = [Uuid::new_v4(), Uuid::new_v4()];
    for (index, id) in head_ids.iter().enumerate() {
        profile.modes[0].heads.push(FixtureHead {
            id: *id,
            name: format!("Lens {index}"),
            master_shared: false,
        });
    }
    profile.modes[0].emitter_heads[0].head_id = head_ids[0];
    let mut second = profile.geometry.emitters[0].clone();
    second.id = Uuid::new_v4();
    profile.modes[0].emitter_heads.push(EmitterHeadBinding {
        emitter_id: second.id,
        head_id: head_ids[1],
    });
    profile.geometry.emitters.push(second);
    let root = FixtureId::new();
    let owners = [FixtureId::new(), FixtureId::new()];
    let mut fixture = patched(&profile, root, 1);
    fixture.logical_heads = head_ids
        .into_iter()
        .zip(owners)
        .enumerate()
        .map(|(index, (head, owner))| PatchedHead {
            profile_head_id: Some(head),
            head_index: (index + 1) as u16,
            fixture_id: owner,
        })
        .collect();
    let rig = Rig::new(vec![fixture], root);
    for owner in owners {
        rig.programmers.set(
            rig.session,
            owner,
            ProgrammingOwner::Position.key(),
            angles(0., 0.),
        );
    }
    (rig, owners)
}

#[test]
fn native_position_shared_equal_controls_deduplicate_and_conflicts_reject_atomically() {
    let (rig, owners) = shared_projection_rig();
    let writes = fitted_writes(&rig, &owners.map(|owner| (owner, angles(45., 30.))));
    assert_eq!(
        writes.len(),
        4,
        "two owners each submit a complete shared footprint"
    );
    let capture = rig.capture();
    let token = capture.frame_token();
    let mut frame = rig.engine.prepare_static_family_frame(&capture, &[]);
    frame
        .project_position_native(&capture, &token, &writes)
        .unwrap();
    let rendered = rig
        .engine
        .preview_static_family_frame(&capture, frame)
        .unwrap();
    assert_physical(&rendered.physical, &writes);
    let baseline = rig
        .engine
        .preview_static_family_frame(
            &capture,
            rig.engine.prepare_static_family_frame(&capture, &[]),
        )
        .unwrap();
    let mut conflict = writes.clone();
    let second_pan = conflict
        .iter_mut()
        .find(|write| write.target == owners[1] && write.channel_index == 0)
        .unwrap();
    second_pan.raw += 1;
    let mut frame = rig.engine.prepare_static_family_frame(&capture, &[]);
    assert!(
        frame
            .project_position_native(&capture, &token, &conflict)
            .is_err()
    );
    let rendered = rig
        .engine
        .preview_static_family_frame(&capture, frame)
        .unwrap();
    assert_eq!(
        rendered.universes, baseline.universes,
        "a late shared conflict cannot install its preceding owner's writes"
    );
}

#[test]
fn final_control_loss_overrides_staged_position_but_partial_fade_retains_exact_raw_commands() {
    for (policy, progress, safe) in [
        (SignalLossPolicy::ImmediateSafe, 0., true),
        (
            SignalLossPolicy::FadeToSafe {
                duration_millis: 1000,
            },
            1.,
            true,
        ),
        (
            SignalLossPolicy::FadeToSafe {
                duration_millis: 1000,
            },
            0.5,
            false,
        ),
    ] {
        let (rig, copy) = projection_rig();
        let snapshot = rig.engine.snapshot();
        let mut fixtures = snapshot.fixtures.as_ref().clone();
        fixtures[0].definition.signal_loss_policy = policy;
        for attribute in ["pan", "tilt"] {
            fixtures[0].definition.safe_values.insert(
                AttributeKey(attribute.into()),
                AttributeValue::Normalized(0.),
            );
        }
        rig.engine
            .replace_snapshot(EngineSnapshot {
                fixtures: fixtures.into(),
                revision: snapshot.revision + 1,
                ..snapshot.as_ref().clone()
            })
            .unwrap();
        let writes = fitted_writes(&rig, &[(rig.root, angles(450., 30.))]);
        let options = RenderOptions {
            control_loss_progress: Some(progress),
            ..Default::default()
        };
        let capture = rig.engine.prepare_output_frame(options);
        let token = capture.frame_token();
        let baseline = rig
            .engine
            .preview_static_family_frame(
                &capture,
                rig.engine.prepare_static_family_frame(&capture, &[]),
            )
            .unwrap();
        let mut frame = rig.engine.prepare_static_family_frame(&capture, &[]);
        frame
            .project_position_native(&capture, &token, &writes)
            .unwrap();
        let rendered = rig
            .engine
            .render_static_family_frame(&capture, frame)
            .unwrap();
        if safe {
            assert_eq!(
                rendered.universes, baseline.universes,
                "ImmediateSafe and a completed Fade keep final authority over native staged commands"
            );
            for (actual, expected) in rendered
                .physical
                .instances
                .iter()
                .zip(&baseline.physical.instances)
            {
                assert_eq!(actual.native_raw, expected.native_raw);
            }
        } else {
            assert_native_output(&rendered, &rig, copy, &writes);
            assert_ne!(
                rendered.universes, baseline.universes,
                "existing Raw fade semantics hold the exact native command until the safe endpoint"
            );
        }
    }
}

fn alias_rig() -> Rig {
    let mut profile = moving_head();
    for channel in &mut profile.modes[0].channels {
        channel.attribute = AttributeKey(format!("native.{}", channel.fixture_attribute.0).into());
    }
    profile.validate().unwrap();
    let root = FixtureId::new();
    let rig = Rig::new(vec![patched(&profile, root, 1)], root);
    rig.programmers.set(
        rig.session,
        root,
        ProgrammingOwner::Position.key(),
        angles(30., 40.),
    );
    rig.set(root, "pan", 0.1);
    rig.set(root, "tilt", 0.9);
    rig
}

#[test]
fn fixture_facing_safety_aliases_replace_native_candidates_and_partial_fades_hold_raw() {
    for progress in [0.5, 1.] {
        let rig = alias_rig();
        let snapshot = rig.engine.snapshot();
        let mut fixtures = snapshot.fixtures.as_ref().clone();
        fixtures[0].definition.signal_loss_policy = SignalLossPolicy::FadeToSafe {
            duration_millis: 1000,
        };
        for key in ["pan", "tilt"] {
            fixtures[0]
                .definition
                .safe_values
                .insert(AttributeKey(key.into()), AttributeValue::Normalized(0.));
        }
        rig.engine
            .replace_snapshot(EngineSnapshot {
                fixtures: fixtures.into(),
                revision: snapshot.revision + 1,
                ..snapshot.as_ref().clone()
            })
            .unwrap();
        let writes = fitted_writes(&rig, &[(rig.root, angles(450., 30.))]);
        let capture = rig.engine.prepare_output_frame(RenderOptions {
            control_loss_progress: Some(progress),
            ..Default::default()
        });
        let baseline = rig
            .engine
            .preview_static_family_frame(
                &capture,
                rig.engine.prepare_static_family_frame(&capture, &[]),
            )
            .unwrap();
        let mut frame = rig.engine.prepare_static_family_frame(&capture, &[]);
        frame
            .project_position_native(&capture, &capture.frame_token(), &writes)
            .unwrap();
        let rendered = rig
            .engine
            .render_static_family_frame(&capture, frame)
            .unwrap();
        if progress == 1. {
            assert_eq!(
                rendered.universes, baseline.universes,
                "fixture-facing safety alias owns the endpoint"
            );
            assert_eq!(
                rendered.physical.instances[0].native_raw,
                baseline.physical.instances[0].native_raw
            );
        } else {
            assert_physical(&rendered.physical, &writes);
            assert_ne!(
                rendered.universes, baseline.universes,
                "exact Raw input holds until fade completion"
            );
        }
    }
}

#[test]
fn explicit_scalar_freeze_aliases_keep_authority_over_staged_position() {
    let rig = alias_rig();
    let snapshot = rig.engine.snapshot();
    let mut fixtures = snapshot.fixtures.as_ref().clone();
    fixtures[0].freeze.targets.insert(
        rig.root,
        FrozenFixtureTarget {
            families: vec![FreezeFamily::Position],
            values: [
                (AttributeKey("pan".into()), AttributeValue::Normalized(0.2)),
                (AttributeKey("tilt".into()), AttributeValue::Normalized(0.8)),
            ]
            .into(),
            ..Default::default()
        },
    );
    rig.engine
        .replace_snapshot(EngineSnapshot {
            fixtures: fixtures.into(),
            revision: snapshot.revision + 1,
            ..snapshot.as_ref().clone()
        })
        .unwrap();
    let writes = fitted_writes(&rig, &[(rig.root, angles(450., 30.))]);
    let capture = rig.capture();
    let baseline = rig
        .engine
        .preview_static_family_frame(
            &capture,
            rig.engine.prepare_static_family_frame(&capture, &[]),
        )
        .unwrap();
    let mut frame = rig.engine.prepare_static_family_frame(&capture, &[]);
    frame
        .project_position_native(&capture, &capture.frame_token(), &writes)
        .unwrap();
    let rendered = rig
        .engine
        .render_static_family_frame(&capture, frame)
        .unwrap();
    assert_eq!(
        rendered.universes, baseline.universes,
        "explicit held aliases outrank derived candidates"
    );
    assert_eq!(
        rendered.physical.instances[0].native_raw,
        baseline.physical.instances[0].native_raw
    );
    assert_ne!(
        rendered.physical.instances[0].native_raw.as_ref(),
        expected_instance(&writes, rig.root.0)
    );
}
