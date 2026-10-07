use super::*;

fn mapping() -> SpatialSelectionMapping {
    SpatialSelectionMapping {
        projection: SpatialProjection::from_preset(ProjectionPreset::Top, Position3d::default()),
        shape: SpatialSelectionShape::Grid {
            angle_degrees: 0.0,
            direction: RankDirection::Ascending,
        },
    }
}

fn spatial_runtime() -> (
    DynamicRuntime,
    Uuid,
    [FixtureId; 3],
    HashMap<FixtureId, SpatialPosition>,
) {
    let targets = [FixtureId::new(), FixtureId::new(), FixtureId::new()];
    let positions = targets
        .iter()
        .enumerate()
        .map(|(index, target)| {
            (
                *target,
                SpatialPosition {
                    x: index as f32,
                    y: 0.0,
                    z: 0.0,
                },
            )
        })
        .collect::<HashMap<_, _>>();
    let definition = definition(lane());
    let mut runtime = installed(&definition);
    let mut request = start_request(
        definition.id,
        controller(2401, 1, false),
        targets[0],
        100,
        false,
    );
    request.target_scope.ordered_targets = targets[..2].to_vec();
    request.stage_positions = positions.clone();
    request.inherited_spatial_mapping = Some(mapping());
    let id = runtime.start(request).unwrap();
    (runtime, id, targets, positions)
}

#[test]
fn delayed_accepted_start_retains_original_phase_origin_during_replay() {
    let definition = definition(lane());
    let mut live = installed(&definition);
    let mut replay = live.fork_for_pending_preview();
    let mut baseline = live.fork_for_pending_preview();
    let start = live.begin_control_recording(NonZeroUsize::new(8).unwrap());
    let mut request = start_request(
        definition.id,
        controller(2400, 1, false),
        FixtureId::new(),
        100,
        false,
    );
    request.controller.activated_at_millis = 100;
    request.activation_delay_millis = 50;
    request.activation_duration_millis = 200;
    let id = live
        .apply_recorded_control(timed(500, DynamicControl::Start(Box::new(request.clone()))))
        .unwrap()
        .instance_id
        .unwrap();
    baseline.start_with_instance_identity(request, id).unwrap();
    let batch = live.controls_since(start).unwrap().unwrap();
    assert_eq!(batch.operation_times().collect::<Vec<_>>(), vec![500]);
    let mut cursor = start;
    let mut scratch = DynamicOutputFrameScratch::default();
    replay_dynamic_controls(&mut replay, &mut scratch, &mut cursor, &batch, None).unwrap();
    assert_eq!(cursor, batch.to());
    let snapshot = replay.snapshot();
    assert_eq!(snapshot.instances[0].started_at_millis, 100);
    assert_eq!(
        snapshot.instances[0].controller_transitions[0].activation_started_at_millis,
        100
    );
    assert_eq!(snapshot, baseline.snapshot());
    for now in [550, 650, 900] {
        let expected = baseline
            .sample(id, now, 1000, 10, &Sources { current: 0.2 })
            .unwrap();
        assert!(!expected.is_empty());
        assert_eq!(
            replay
                .sample(id, now, 1000, 10, &Sources { current: 0.2 })
                .unwrap(),
            expected
        );
        assert_eq!(
            live.sample(id, now, 1000, 10, &Sources { current: 0.2 })
                .unwrap(),
            expected
        );
    }
}

#[test]
fn recorded_targets_and_spatial_ranks_replay_owned_inputs_without_logging_noops() {
    let (mut live, id, targets, mut positions) = spatial_runtime();
    let mut pending = live.fork_for_pending_preview();
    let start = live.begin_control_recording(NonZeroUsize::new(8).unwrap());
    let mut inherited = mapping();
    let scope = || DynamicTargetScope {
        ordered_targets: targets.to_vec(),
    };
    assert!(
        live.reconcile_instance_targets_recorded(id, scope(), &positions, Some(&inherited), 200)
            .unwrap()
    );
    let first = live.snapshot();
    let accepted = live.control_cursor();
    for at in [201, 202] {
        assert!(
            !live
                .reconcile_instance_targets_recorded(id, scope(), &positions, Some(&inherited), at)
                .unwrap()
        );
    }
    assert_eq!(live.control_cursor(), accepted);
    positions.get_mut(&targets[0]).unwrap().x = 2.0;
    positions.get_mut(&targets[1]).unwrap().x = 0.0;
    positions.get_mut(&targets[2]).unwrap().x = 1.0;
    assert!(
        live.reconcile_instance_targets_recorded(id, scope(), &positions, Some(&inherited), 300)
            .unwrap()
    );
    let second = live.snapshot();
    assert_ne!(
        first.instances[0].phase_by_lane_target,
        second.instances[0].phase_by_lane_target
    );
    inherited.shape = SpatialSelectionShape::Grid {
        angle_degrees: 0.0,
        direction: RankDirection::Descending,
    };
    assert!(
        live.reconcile_instance_targets_recorded(id, scope(), &positions, Some(&inherited), 400)
            .unwrap()
    );
    let third = live.snapshot();
    assert_ne!(
        second.instances[0].phase_by_lane_target,
        third.instances[0].phase_by_lane_target
    );
    let batch = live.controls_since(start).unwrap().unwrap();
    assert_eq!(
        batch.operation_times().collect::<Vec<_>>(),
        vec![200, 300, 400]
    );
    // Neither later edits to borrowed caller inputs nor producer pruning can alter this batch.
    positions.clear();
    inherited.projection.view_direction = Vector3::default();
    live.acknowledge_controls(batch.to()).unwrap().unwrap();
    let mut cursor = start;
    let mut scratch = DynamicOutputFrameScratch::default();
    for (count, expected) in [(1, first), (2, second), (3, third)] {
        let mut branch = pending.fork_for_pending_preview();
        let mut branch_cursor = start;
        replay_dynamic_controls(
            &mut branch,
            &mut scratch,
            &mut branch_cursor,
            &batch.prefix(count).unwrap(),
            None,
        )
        .unwrap();
        assert_eq!(branch.snapshot(), expected);
    }
    replay_dynamic_controls(&mut pending, &mut scratch, &mut cursor, &batch, None).unwrap();
    assert_eq!(pending.snapshot(), live.snapshot());
}

#[test]
fn target_reconciliation_failure_and_post_sample_rejection_restore_state_and_cursor() {
    let (mut runtime, id, targets, positions) = spatial_runtime();
    let start = runtime.begin_control_recording(NonZeroUsize::new(8).unwrap());
    let before = runtime.snapshot();
    let mut invalid = mapping();
    invalid.projection.view_direction = Vector3::default();
    let scope = || DynamicTargetScope {
        ordered_targets: targets.to_vec(),
    };
    assert!(matches!(
        runtime.reconcile_instance_targets_recorded(id, scope(), &positions, Some(&invalid), 200),
        Err(DynamicRuntimeError::InvalidSpatialMapping(_))
    ));
    assert_eq!(runtime.snapshot(), before);
    assert_eq!(runtime.control_cursor(), Some(start));
    let mut scratch = DynamicOutputFrameScratch::default();
    let failed: Result<(), DynamicRuntimeError> =
        runtime.with_output_frame_transaction(&mut scratch, |runtime| {
            assert!(runtime.reconcile_instance_targets_recorded(
                id,
                scope(),
                &positions,
                Some(&mapping()),
                200
            )?);
            runtime.reconcile_instance_targets_recorded(
                id,
                scope(),
                &positions,
                Some(&invalid),
                201,
            )?;
            Ok(())
        });
    assert!(matches!(
        failed,
        Err(DynamicRuntimeError::InvalidSpatialMapping(_))
    ));
    assert_eq!(runtime.snapshot(), before);
    assert_eq!(runtime.control_cursor(), Some(start));
    assert!(runtime.controls_since(start).unwrap().unwrap().is_empty());
    let sampled_then_changed: Result<(), DynamicRuntimeError> = runtime
        .with_output_frame_transaction(&mut scratch, |runtime| {
            runtime.sample(id, 250, 1000, 10, &Sources { current: 0.2 })?;
            runtime.reconcile_instance_targets_recorded(
                id,
                scope(),
                &positions,
                Some(&mapping()),
                251,
            )?;
            Ok(())
        });
    assert!(matches!(
        sampled_then_changed,
        Err(DynamicRuntimeError::InvalidReplay(_))
    ));
    assert_eq!(runtime.snapshot(), before);
    assert_eq!(runtime.committed_sample_boundary(), None);
    assert_eq!(runtime.control_cursor(), Some(start));
    assert!(runtime.controls_since(start).unwrap().unwrap().is_empty());
}

#[test]
fn unrecorded_target_reconciliation_keeps_direct_mutation_semantics() {
    let (mut runtime, id, targets, positions) = spatial_runtime();
    let mut baseline = runtime.fork_for_pending_preview();
    let scope = || DynamicTargetScope {
        ordered_targets: targets.to_vec(),
    };
    let inherited = mapping();
    assert_eq!(
        runtime
            .reconcile_instance_targets_recorded(id, scope(), &positions, Some(&inherited), 200)
            .unwrap(),
        baseline
            .reconcile_instance_targets(id, scope(), &positions, Some(&inherited))
            .unwrap()
    );
    assert_eq!(runtime.snapshot(), baseline.snapshot());
    assert_eq!(runtime.control_cursor(), None);
}
