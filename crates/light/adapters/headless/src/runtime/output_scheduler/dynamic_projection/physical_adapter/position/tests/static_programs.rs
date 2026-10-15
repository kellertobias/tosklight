//! Static Position owners use the same captured mechanical cohort and native finalizer as
//! Dynamic owners. Neither moving geometry nor an output calculation rewrites their intent.
use super::programs::{destination_dynamic_rig, verify_live_native};
use super::*;

fn static_runtime() -> DynamicRuntime {
    DynamicRuntime::with_programming_contract_support(PROGRAMMING_CONTRACT_VERSION)
}

fn assert_static_row<'a>(
    published: &'a PublishedPhysicalFrame<PositionAdapter>,
    target: FixtureId,
    value: &AttributeValue,
) -> &'a PhysicalHeadResult<PositionAdapter> {
    assert!(published.requirements.is_empty());
    assert_eq!(published.results.len(), 1);
    let row = published
        .results
        .iter()
        .find(|row| row.target == target)
        .unwrap();
    assert_eq!(row.token, published.token);
    assert_eq!(row.owner, ProgrammingOwner::Position);
    assert_eq!(&row.value, value);
    let PositionRequest::Intent(requested) = &row.requested else {
        panic!("an empty static program is the original Intent, not an invented Dynamic program");
    };
    assert_eq!(requested, intent(value).unwrap());
    assert!(!row.quality.held);
    assert!(row.writes.iter().all(|write| !write.parked));
    for outcome in &row.achieved.outcomes {
        assert!(!outcome.missing_mount && !outcome.input_requirement);
        assert_eq!(outcome.result.status, PositionFitStatus::Fitted);
        assert!(outcome.result.angular_error_degrees.unwrap() < 0.04);
    }
    row
}

#[test]
fn actual_live_static_target_with_no_dynamics_tracks_an_independent_moving_mount() {
    let profile = moving_head();
    let root = FixtureId::new();
    let aim = FixtureId::new();
    let mount = FixtureId::new();
    let mut fixture = patched(&profile, root, 1);
    fixture.position_master = Some(mount.0);
    fixture.location.z = 3000;
    let rig = Rig::new(
        vec![
            fixture,
            point(
                aim,
                FixtureLocation {
                    x: 2000,
                    y: 5000,
                    z: 1000,
                },
            ),
            point(mount, FixtureLocation::default()),
        ],
        root,
    );
    rig.set(aim, "point.rotation.z", 0.75);
    let value = target(TargetReference::Point { point_id: aim.0 }, [1., 0., 0.]);
    rig.programmers.set(
        rig.session,
        root,
        ProgrammingOwner::Position.key(),
        value.clone(),
    );
    let lane = PhysicalAdapterLane::live(PositionAdapter::default());
    let mut runtime = static_runtime();
    let mut origins = DynamicSourceOrigins::default();
    let mut scratch = HybridFrameScratch::default();
    let capture = rig.capture();
    assert!(capture.dynamic_programmer_values().is_empty());
    assert!(capture.cue_dynamic_values().is_empty());
    // Staging a complete static owner must still obey the accepted-frame boundary.
    let foreign = rig.capture();
    assert!(
        prepare_live(
            &rig,
            &capture,
            &foreign,
            &lane,
            &mut runtime,
            &mut origins,
            &mut scratch
        )
        .is_err()
    );
    assert!(lane.continuity(root, ProgrammingOwner::Position).is_none());
    use crate::runtime::output_scheduler::slow_phase_trace::live_phases::family_preparation as diagnostics;
    let (before, calls, counts) = diagnostics::observe_for_test(|| {
        prepare_live(
            &rig,
            &capture,
            &capture,
            &lane,
            &mut runtime,
            &mut origins,
            &mut scratch,
        )
        .unwrap()
    });
    for stage in [
        diagnostics::Stage::PreparePrograms,
        diagnostics::Stage::ProgramInput,
        diagnostics::Stage::PositionCapture,
        diagnostics::Stage::PositionTracking,
    ] {
        assert_eq!(calls[stage as usize], 1);
    }
    assert_eq!(counts[diagnostics::Count::PositionPrograms as usize], 1);
    assert_eq!(counts[diagnostics::Count::PositionSamples as usize], 0);
    assert_eq!(
        counts[diagnostics::Count::PositionEmptyPrograms as usize],
        1
    );
    assert_eq!(
        counts[diagnostics::Count::PositionRequirementsOnly as usize],
        0
    );
    let row = assert_static_row(&before, root, &value);
    assert_eq!(row.writes.len(), 2);
    assert_eq!(row.achieved.outcomes.len(), 1);
    let expected = PositionFitRequest::Target {
        world: Some(RigidTransform::DESK_TO_PROFILE.point([2., 6., 1.])),
    };
    assert_eq!(row.achieved.outcomes[0].result.requested, Some(expected));
    verify_live_native(&rig, &capture, &lane, &before);
    let old_continuity = lane.continuity(root, ProgrammingOwner::Position).unwrap();
    let old_achieved = row.achieved.outcomes[0].result.achieved;
    let old_native = before
        .rendered
        .physical
        .instances
        .iter()
        .find(|instance| instance.instance_id == root.0)
        .unwrap()
        .native_raw
        .clone();

    rig.set(mount, "point.position.z", 0.505);
    let next = rig.capture();
    assert!(next.dynamic_programmer_values().is_empty());
    let after = prepare_live(
        &rig,
        &next,
        &next,
        &lane,
        &mut runtime,
        &mut origins,
        &mut scratch,
    )
    .unwrap();
    let row = assert_static_row(&after, root, &value);
    assert_eq!(row.achieved.outcomes[0].result.requested, Some(expected));
    assert_ne!(row.achieved.outcomes[0].result.achieved, old_achieved);
    verify_live_native(&rig, &next, &lane, &after);
    let new_native = &after
        .rendered
        .physical
        .instances
        .iter()
        .find(|instance| instance.instance_id == root.0)
        .unwrap()
        .native_raw;
    assert_ne!(
        new_native.as_ref(),
        old_native.as_ref(),
        "accepted motor commands follow mounting motion"
    );
    assert_ne!(
        lane.continuity(root, ProgrammingOwner::Position).unwrap(),
        old_continuity
    );
    let final_scalar = rig.engine.prepare_static_family_frame(&next, &[]);
    assert_eq!(
        final_scalar.value(root, &ProgrammingOwner::Position.key()),
        Some(&value),
        "rendering and moving mount leave the authored Target unchanged"
    );
}

#[test]
fn actual_live_static_target_fits_root_and_copy_from_their_distinct_world_mounts() {
    let (rig, copy) = destination_dynamic_rig();
    let value = target(TargetReference::Origin, [2., 6., 1.]);
    rig.programmers.set(
        rig.session,
        rig.root,
        ProgrammingOwner::Position.key(),
        value.clone(),
    );
    let lane = PhysicalAdapterLane::live(PositionAdapter::default());
    let mut runtime = static_runtime();
    let capture = rig.capture();
    assert!(capture.dynamic_programmer_values().is_empty());
    let published = prepare_live(
        &rig,
        &capture,
        &capture,
        &lane,
        &mut runtime,
        &mut DynamicSourceOrigins::default(),
        &mut HybridFrameScratch::default(),
    )
    .unwrap();
    let row = assert_static_row(&published, rig.root, &value);
    assert_eq!(row.writes.len(), 4);
    assert_eq!(row.achieved.outcomes.len(), 2);
    let requested = PositionFitRequest::Target {
        world: Some(RigidTransform::DESK_TO_PROFILE.point([2., 6., 1.])),
    };
    for destination in [rig.root, copy] {
        let outcome = row
            .achieved
            .outcomes
            .iter()
            .find(|outcome| outcome.destination == destination)
            .unwrap();
        assert_eq!(outcome.result.requested, Some(requested));
    }
    let root_result = row
        .achieved
        .outcomes
        .iter()
        .find(|outcome| outcome.destination == rig.root)
        .unwrap();
    let copy_result = row
        .achieved
        .outcomes
        .iter()
        .find(|outcome| outcome.destination == copy)
        .unwrap();
    assert_ne!(root_result.result.achieved, copy_result.result.achieved);
    let native = |destination: FixtureId| {
        published
            .rendered
            .physical
            .instances
            .iter()
            .find(|instance| instance.instance_id == destination.0)
            .unwrap()
            .native_raw
            .as_ref()
    };
    assert_ne!(native(rig.root), native(copy));
    let continuity = lane
        .continuity(rig.root, ProgrammingOwner::Position)
        .unwrap();
    assert_eq!(continuity.instances.len(), 2);
    for destination in [rig.root, copy] {
        let accepted = continuity
            .instances
            .iter()
            .find(|instance| instance.destination == destination)
            .unwrap();
        for &(index, _, _, raw) in &accepted.controls {
            assert_eq!(
                native(destination)[index as usize],
                raw,
                "accepted static continuity uses the final encoded native command"
            );
        }
    }
    verify_live_native(&rig, &capture, &lane, &published);
}

#[test]
fn static_position_registry_never_overwrites_an_active_legacy_pan_lane() {
    use light_dynamics::*;
    let rig = Rig::single();
    let base = target(TargetReference::Origin, [2., 6., 1.]);
    rig.programmers.set(
        rig.session,
        rig.root,
        ProgrammingOwner::Position.key(),
        base.clone(),
    );
    let mut definition = programs::position_definition(
        DynamicValueAddress::whole_family(ProgrammingOwner::Position, &base).unwrap(),
        [
            DynamicValue::Family(base.clone()),
            DynamicValue::Family(base.clone()),
        ],
    );
    definition.lanes[0].body = DynamicLaneBody::LegacyScalar(LegacyScalarLaneBody {
        attribute: AttributeKey("pan".into()),
        mode: DynamicLaneMode::Keyframes,
        keyframes: KeyframeConfiguration {
            points: [0., 0.5]
                .map(|position| DynamicKeyframe {
                    position,
                    source: ScalarSource::Value { value: 0.7 },
                    interpolation: ScalarInterpolation::Linear,
                })
                .to_vec(),
            size: 1.,
        },
        max_min: MaxMinConfiguration {
            minimum: ScalarSource::Value { value: 0. },
            maximum: ScalarSource::Value { value: 1. },
            function: PeriodicFunction::Sinus,
            size: 1.,
            pwm: Default::default(),
        },
        middle_amplitude: MiddleAmplitudeConfiguration {
            middle: ScalarSource::Current,
            amplitude: 1.,
            function: PeriodicFunction::Sinus,
            size: 1.,
            pwm: Default::default(),
            invert_waveform: false,
        },
    });
    let snapshot = rig.engine.snapshot();
    rig.engine
        .replace_snapshot(EngineSnapshot {
            dynamics: vec![definition.clone()].into(),
            revision: snapshot.revision + 1,
            ..snapshot.as_ref().clone()
        })
        .unwrap();
    rig.clock.advance_millis(1);
    assert!(rig.programmers.apply_dynamic_values(
        rig.session,
        &[DynamicProgrammerValueMutation::Set {
            fixture_id: rig.root,
            attribute: definition.lanes[0].output_owner(),
            value: DynamicSemanticValue::DynamicOn {
                instance_link: Uuid::new_v4(),
                lane_id: definition.lanes[0].id,
                dynamic: DynamicReference {
                    dynamic_id: Some(definition.id),
                    last_known_pool_number: definition.pool_number,
                    embedded_fallback: DynamicDefinitionSnapshot {
                        definition: Arc::new(definition.clone())
                    }
                },
                overrides: DynamicInstanceOverrides {
                    size: 1.,
                    speed_multiplier: Rational::ONE,
                    phase_offset_degrees: 0.
                },
                timing: Default::default()
            },
        }],
        None
    ));
    let mut runtime = static_runtime();
    runtime.install_definitions([definition]).unwrap();
    let lane = PhysicalAdapterLane::live(PositionAdapter::default());
    let capture = rig.capture();
    let published = prepare_live(
        &rig,
        &capture,
        &capture,
        &lane,
        &mut runtime,
        &mut DynamicSourceOrigins::default(),
        &mut HybridFrameScratch::default(),
    )
    .unwrap();
    assert!(
        published.results.is_empty(),
        "static Position cannot overwrite active scalar Pan output"
    );
    assert!(published.requirements.iter().any(|row| row.target == rig.root && row.owner == ProgrammingOwner::Position
        && matches!(row.reason, crate::runtime::output_scheduler::dynamic_projection::programming_projection::hybrid::HybridFamilyRequirementReason::LegacyOwnerOverlap)));
    assert!(
        lane.continuity(rig.root, ProgrammingOwner::Position)
            .is_none()
    );
    let instance = published
        .rendered
        .physical
        .instances
        .iter()
        .find(|instance| instance.instance_id == rig.root.0)
        .unwrap();
    assert_eq!(
        instance.native_raw[0],
        (0.7_f32 * 65535.0).round() as u32,
        "captured legacy Pan reaches final native output"
    );
    assert_eq!(
        rig.engine
            .prepare_static_family_frame(&capture, &[])
            .value(rig.root, &ProgrammingOwner::Position.key()),
        Some(&base)
    );
}

#[test]
fn actual_position_observer_counts_requirements_only_rows_and_error_boundaries() {
    use crate::runtime::output_scheduler::dynamic_projection::programming_projection::hybrid::HybridFamilyProgram;
    use crate::runtime::output_scheduler::slow_phase_trace::live_phases::family_preparation as diagnostics;

    // Real observer/captured engine inputs: duplicate capture fails before tracking; a
    // missing lane.begin fails inside tracking. Neither failure publishes a result.
    for (duplicate, begin_tracking) in [(false, true), (true, true), (false, false)] {
        let rig = Rig::single();
        let lane = PhysicalAdapterLane::live(PositionAdapter::default());
        let capture = rig.capture();
        let token = capture.frame_token();
        let mut scalar = rig.engine.prepare_static_family_frame(&capture, &[]);
        let geometry = rig
            .engine
            .observe_static_family_geometry(&capture, &mut scalar)
            .unwrap();
        let models = DynamicRuntime::default().captured_native_color_models();
        let frame = HybridFrameContext {
            capture: &capture,
            token: &token,
            scalar: &scalar,
            geometry: &geometry,
            native_models: models.as_ref(),
        };
        if begin_tracking {
            lane.begin(&token).unwrap();
        }
        let mut observer = PositionFrameObserver::new(&lane);
        observer.begin_frame(&token).unwrap();
        let value = angles(30., 15.);
        let program = || HybridFamilyProgram {
            target: rig.root,
            owner: ProgrammingOwner::Position,
            base: &value,
            samples: &[],
            has_requirements: true,
            frame,
        };
        let mut programs = vec![program()];
        if duplicate {
            programs.push(program());
        }
        let (result, calls, counts) =
            diagnostics::observe_for_test(|| observer.prepare_programs(frame, &programs));
        assert_eq!(result.is_ok(), !duplicate && begin_tracking);
        assert_eq!(calls[diagnostics::Stage::PositionCapture as usize], 1);
        assert_eq!(
            calls[diagnostics::Stage::PositionTracking as usize],
            u64::from(!duplicate)
        );
        assert_eq!(calls[diagnostics::Stage::ProgramInput as usize], 0);
        let submitted = if duplicate { 2 } else { 1 };
        assert_eq!(
            counts[diagnostics::Count::PositionPrograms as usize],
            submitted
        );
        assert_eq!(counts[diagnostics::Count::PositionSamples as usize], 0);
        assert_eq!(
            counts[diagnostics::Count::PositionEmptyPrograms as usize],
            submitted
        );
        assert_eq!(
            counts[diagnostics::Count::PositionRequirementsOnly as usize],
            submitted
        );
        assert!(
            lane.continuity(rig.root, ProgrammingOwner::Position)
                .is_none()
        );
    }
}
