use super::*;

// Frozen std-map restoration and keyed lookup, independent of the runtime's private hasher.
// Preserve legacy expansion followed by explicit-row overwrite (including duplicate rows).
fn old_phases(stored: &DynamicInstanceSnapshot) -> HashMap<(Uuid, FixtureId), f32> {
    let mut phases = stored
        .definition
        .lanes
        .iter()
        .flat_map(|lane| {
            stored
                .phase_by_target
                .iter()
                .map(move |(target, phase)| ((lane.id, *target), *phase))
        })
        .collect::<HashMap<_, _>>();
    phases.extend(
        stored
            .phase_by_lane_target
            .iter()
            .map(|(lane, target, phase)| ((*lane, *target), *phase)),
    );
    phases
}

fn old_get(phases: &HashMap<(Uuid, FixtureId), f32>, lane: Uuid, target: FixtureId) -> f32 {
    phases.get(&(lane, target)).copied().unwrap_or(0.0)
}

fn phase_bits(snapshot: &DynamicRuntimeSnapshot) -> Vec<(Uuid, FixtureId, u32)> {
    snapshot.instances[0]
        .phase_by_lane_target
        .iter()
        .map(|(lane, target, phase)| (*lane, *target, phase.to_bits()))
        .collect()
}

fn restored(snapshot: DynamicRuntimeSnapshot) -> DynamicRuntime {
    let mut runtime = DynamicRuntime::with_programming_contract_support(
        light_core::programming::DYNAMIC_PHASE_ORDERING_CONTRACT,
    );
    runtime.restore_snapshot(snapshot).unwrap();
    runtime
}

#[test]
fn phase_hash_restoration_and_samples_match_frozen_std_lookup_bitwise() {
    let (dynamic, targets, positions, inherited) = setup(vec![
        ordering(json!({"type":"source_order"})),
        ordering(json!({"type":"source_order"})),
    ]);
    let lanes = dynamic.lanes.clone();
    let (runtime, instance) = start(dynamic.clone(), targets.clone(), positions, inherited);
    let mut snapshot = runtime.snapshot();
    let stored = &mut snapshot.instances[0];
    // The third target is deliberately absent: it must still sample at +0 degrees.
    stored.phase_by_target = vec![(targets[0], -0.0), (targets[1], 30.0)];
    stored.phase_by_lane_target = vec![
        (lanes[0].id, targets[1], 45.0),
        (lanes[0].id, targets[1], -90.0),
        (lanes[1].id, targets[0], 0.0),
        (lanes[1].id, targets[1], f32::from_bits(1)),
    ];
    stored.controllers[0].phase_offset_degrees = -17.0;
    let old = old_phases(stored);
    let fx = old
        .clone()
        .into_iter()
        .collect::<rustc_hash::FxHashMap<_, _>>();
    for lane in &lanes {
        for target in &targets {
            assert_eq!(
                fx.get(&(lane.id, *target))
                    .copied()
                    .unwrap_or(0.0)
                    .to_bits(),
                old_get(&old, lane.id, *target).to_bits()
            );
        }
    }
    assert_eq!(
        old_get(&old, lanes[0].id, targets[0]).to_bits(),
        (-0.0f32).to_bits()
    );
    assert_eq!(
        old_get(&old, lanes[1].id, targets[0]).to_bits(),
        0.0f32.to_bits()
    );
    assert_eq!(
        old_get(&old, lanes[0].id, targets[2]).to_bits(),
        0.0f32.to_bits()
    );
    let mut expected = old
        .iter()
        .map(|((lane, target), phase)| (*lane, *target, phase.to_bits()))
        .collect::<Vec<_>>();
    expected.sort_by_key(|(lane, target, _)| (*lane, target.0));
    let mut runtime = restored(snapshot);
    assert_eq!(phase_bits(&runtime.snapshot()), expected);
    let sources = Sources { current: 0.3 };
    for now in [0, 1, 249, 500, 999, 1001] {
        let samples = runtime.sample(instance, now, 1000, 10, &sources).unwrap();
        assert_eq!(samples.len(), lanes.len() * targets.len());
        for sample in samples {
            let lane = lanes.iter().find(|lane| lane.id == sample.lane_id).unwrap();
            let expected = DynamicEvaluator::new(&dynamic)
                .sample_lane(
                    lane,
                    DynamicEvaluationContext {
                        instance_id: instance,
                        target: sample.target,
                        elapsed_millis: now,
                        cycle_duration_millis: 1000,
                        phase_degrees: old_get(&old, lane.id, sample.target) + -17.0,
                        output_interval_millis: 10,
                        random_envelope: None,
                        sources: &sources,
                    },
                )
                .unwrap();
            assert_eq!(sample.legacy().unwrap().value.to_bits(), expected.to_bits());
        }
    }
    let cut = runtime.snapshot();
    let encoded = serde_json::to_vec(&cut).unwrap();
    assert_eq!(
        serde_json::to_vec(&restored(cut.clone()).snapshot()).unwrap(),
        encoded
    );
    assert_eq!(
        phase_bits(&restored(cut.clone()).snapshot()),
        phase_bits(&cut)
    );
}

#[test]
fn phase_hash_mapping_edits_noops_invalid_edits_and_transaction_rollback() {
    let (mut definition, targets, positions, mut inherited) = setup(vec![
        spatial(json!({"type":"inherit"})),
        ordering(json!({"type":"source_order"})),
    ]);
    definition.spatial_mapping = DynamicSpatialMappingOverride::default();
    let (mut runtime, instance) = start(
        definition.clone(),
        targets.clone(),
        positions.clone(),
        inherited.clone(),
    );
    let scope = || DynamicTargetScope {
        ordered_targets: targets.clone(),
    };
    assert!(
        !runtime
            .reconcile_instance_targets(instance, scope(), &positions, Some(&inherited))
            .unwrap()
    );
    let before = runtime.snapshot();
    let mut preview = runtime.fork_for_pending_preview();
    inherited.shape = SpatialSelectionShape::Grid {
        angle_degrees: 0.0,
        direction: RankDirection::Descending,
    };
    assert!(
        runtime
            .reconcile_instance_targets(instance, scope(), &positions, Some(&inherited))
            .unwrap()
    );
    assert_ne!(phase_bits(&before), phase_bits(&runtime.snapshot()));
    assert_eq!(phase_bits(&preview.snapshot()), phase_bits(&before));
    assert!(
        preview
            .reconcile_instance_targets(instance, scope(), &positions, Some(&inherited))
            .unwrap()
    );
    assert_eq!(
        phase_bits(&preview.snapshot()),
        phase_bits(&runtime.snapshot())
    );
    let stable = runtime.snapshot();
    let mut invalid = inherited.clone();
    invalid.projection.view_direction = Vector3::default();
    assert!(matches!(
        runtime.reconcile_instance_targets(instance, scope(), &positions, Some(&invalid)),
        Err(DynamicRuntimeError::InvalidSpatialMapping(_))
    ));
    assert_eq!(runtime.snapshot(), stable);
    let mut scratch = DynamicOutputFrameScratch::default();
    let failed: Result<(), DynamicRuntimeError> =
        runtime.with_output_frame_transaction(&mut scratch, |runtime| {
            runtime.reconcile_instance_targets(
                instance,
                DynamicTargetScope {
                    ordered_targets: targets.iter().rev().copied().collect(),
                },
                &positions,
                Some(&inherited),
            )?;
            runtime.reconcile_instance_targets(instance, scope(), &positions, Some(&invalid))?;
            Ok(())
        });
    assert!(failed.is_err());
    assert_eq!(runtime.snapshot(), stable);
    // Reorder lane definitions and targets, then rebuild through the original projector.
    definition.revision += 1;
    definition.lanes.reverse();
    runtime.install_definitions([definition]).unwrap();
    assert!(
        runtime
            .reconcile_instance_targets(
                instance,
                DynamicTargetScope {
                    ordered_targets: targets.iter().rev().copied().collect()
                },
                &positions,
                Some(&inherited)
            )
            .unwrap()
    );
    let mut roundtrip = restored(runtime.snapshot());
    for now in [100, 250, 501] {
        let samples = runtime
            .sample(instance, now, 1000, 10, &Sources { current: 0.2 })
            .unwrap();
        assert_eq!(samples.len(), 6);
        assert_eq!(
            samples,
            roundtrip
                .sample(instance, now, 1000, 10, &Sources { current: 0.2 })
                .unwrap()
        );
    }
}

#[test]
fn phase_hash_random_override_and_paused_preview_ignore_poisoned_static_phases() {
    let random = || spatial(json!({"type":"replace","value":{"type":"random","seed":91}}));
    let (dynamic, targets, positions, inherited) = setup(vec![random(), random()]);
    let (runtime, instance) = start(dynamic, targets, positions, inherited);
    let baseline = runtime.snapshot();
    let mut poison = baseline.clone();
    for (_, _, phase) in &mut poison.instances[0].phase_by_lane_target {
        *phase = 123.0;
    }
    assert_ne!(phase_bits(&baseline), phase_bits(&poison));
    let mut clean = restored(baseline);
    let mut changed = restored(poison);
    for now in [0, 499, 1000, 2500] {
        let samples = clean
            .sample(instance, now, 1000, 10, &Sources { current: 0.0 })
            .unwrap();
        assert_eq!(samples.len(), 6);
        assert_eq!(
            samples,
            changed
                .sample(instance, now, 1000, 10, &Sources { current: 0.0 })
                .unwrap()
        );
    }
    clean.set_global_paused(true, 2500);
    changed.set_global_paused(true, 2500);
    let mut preview = changed.fork_for_pending_preview();
    let held = clean
        .sample(instance, 3000, 1000, 10, &Sources { current: 0.0 })
        .unwrap();
    assert_eq!(held.len(), 6);
    assert_eq!(
        held,
        changed
            .sample(instance, 3000, 1000, 10, &Sources { current: 0.0 })
            .unwrap()
    );
    assert_eq!(
        held,
        preview
            .sample(instance, 3000, 1000, 10, &Sources { current: 0.0 })
            .unwrap()
    );
    clean.set_global_paused(false, 3500);
    changed.set_global_paused(false, 3500);
    let resumed = clean
        .sample(instance, 4000, 1000, 10, &Sources { current: 0.0 })
        .unwrap();
    assert_eq!(resumed.len(), 6);
    assert_eq!(
        resumed,
        changed
            .sample(instance, 4000, 1000, 10, &Sources { current: 0.0 })
            .unwrap()
    );
}
