use super::*;

fn records(manifest: &DynamicInstancePresetSources, value: f32) -> Vec<DynamicPresetSourceValues> {
    manifest
        .sources
        .iter()
        .map(|source| DynamicPresetSourceValues {
            occurrence: source.occurrence.unwrap(),
            preset_id: source.preset_id.clone(),
            address: source.address.clone(),
            values: manifest
                .ordered_targets
                .iter()
                .map(|target| DynamicValueFallback {
                    target: *target,
                    value: DynamicValue::Scalar(value),
                })
                .collect(),
        })
        .collect()
}

fn manifest(runtime: &DynamicRuntime) -> DynamicInstancePresetSources {
    let mut instances = runtime.preset_source_instances();
    assert_eq!(instances.len(), 1);
    instances.remove(0)
}

fn positions(
    targets: &[FixtureId; 2],
    reverse: bool,
    offset: f32,
) -> HashMap<FixtureId, SpatialPosition> {
    let xs = if reverse { [1.0, 0.0] } else { [0.0, 1.0] };
    targets
        .iter()
        .zip(xs)
        .map(|(target, x)| {
            (
                *target,
                SpatialPosition {
                    x: x + offset,
                    y: 0.0,
                    z: 0.0,
                },
            )
        })
        .collect()
}

fn running() -> (DynamicRuntime, Uuid, [FixtureId; 2]) {
    let targets = [FixtureId::new(), FixtureId::new()];
    let preset = DynamicValueSource::Preset {
        retained: None,
        preset_id: "ranked".into(),
        address: pan_address(),
        last_valid_by_target: vec![],
    };
    let mut dynamic = definition(ramp(pan_address(), preset.clone(), preset));
    dynamic.phase.ordering = PhaseOrdering::GridLinear { angle_degrees: 0.0 };
    let mut random = lane();
    random.legacy_mut().unwrap().mode = DynamicLaneMode::Random;
    let group = Uuid::new_v4();
    random.random_group_id = Some(group);
    dynamic.lanes.push(random);
    dynamic.random_groups.push(DynamicRandomGroup {
        id: group,
        seed: 17,
        range: DynamicRandomRange::LegacyScalar {
            low: source(0.0),
            high: source(1.0),
        },
        decision_interval_millis: 100,
        start_probability: 1.0,
        mean_duration_millis: 250,
        duration_spread_millis: 30,
        attack_ratio: 0.1,
        decay_ratio: 0.5,
    });
    let mut request = start_request(dynamic.id, controller(901, 1, false), targets[0], 0, false);
    request.target_scope.ordered_targets = targets.to_vec();
    request.stage_positions = positions(&targets, false, 0.0);
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([dynamic]).unwrap();
    let instance = runtime.start(request).unwrap();
    let initial = manifest(&runtime);
    assert!(
        runtime
            .install_preset_source_values(&initial, records(&initial, 10.0))
            .unwrap()
    );
    sampled(
        &mut runtime,
        instance,
        150,
        &TypedSources {
            current: Some(DynamicValue::Scalar(0.0)),
            preset: None,
            calls: Cell::new(0),
        },
    );
    runtime
        .set_controller_paused(instance, Uuid::from_u128(901), true, 150)
        .unwrap();
    assert!(!runtime.snapshot().instances[0].random_streams.is_empty());
    (runtime, instance, targets)
}

fn reconcile(
    runtime: &mut DynamicRuntime,
    instance: Uuid,
    targets: Vec<FixtureId>,
    positions: &HashMap<FixtureId, SpatialPosition>,
) -> bool {
    runtime
        .reconcile_instance_targets(
            instance,
            DynamicTargetScope {
                ordered_targets: targets,
            },
            positions,
            None,
        )
        .unwrap()
}

#[test]
fn changed_spatial_rank_rejects_same_target_manifest_without_rewinding_runtime() {
    let (mut runtime, instance, targets) = running();
    let stale = manifest(&runtime);
    let before = runtime.snapshot();
    assert!(reconcile(
        &mut runtime,
        instance,
        targets.to_vec(),
        &positions(&targets, true, 0.0)
    ));
    let current = manifest(&runtime);
    assert_eq!(current.ordered_targets, stale.ordered_targets);
    assert_eq!(
        current
            .sources
            .iter()
            .map(|source| source.id)
            .collect::<Vec<_>>(),
        stale
            .sources
            .iter()
            .map(|source| source.id)
            .collect::<Vec<_>>()
    );
    assert_ne!(current.dependency_generation, stale.dependency_generation);
    let mut expected = before;
    let after = runtime.snapshot();
    assert_ne!(
        after.instances[0].phase_by_lane_target,
        expected.instances[0].phase_by_lane_target
    );
    expected.instances[0].phase_by_lane_target = after.instances[0].phase_by_lane_target.clone();
    expected.instances[0].phase_by_target = after.instances[0].phase_by_target.clone();
    assert_eq!(
        after, expected,
        "reconciliation changes mapping only; paused clocks, controllers, Random and held values survive"
    );

    let refreshed = records(&current, 20.0);
    assert!(
        runtime
            .install_preset_source_values(&current, refreshed.clone())
            .unwrap()
    );
    assert!(
        !runtime
            .install_preset_source_values(&stale, records(&stale, 99.0))
            .unwrap()
    );
    assert_eq!(manifest(&runtime).last_valid, refreshed);

    // Translation changes the input positions, but not their relative rank or phase map.
    assert!(!reconcile(
        &mut runtime,
        instance,
        targets.to_vec(),
        &positions(&targets, true, 10.0)
    ));
    assert_eq!(
        manifest(&runtime).dependency_generation,
        current.dependency_generation
    );
    assert!(
        runtime
            .install_preset_source_values(&current, refreshed)
            .unwrap()
    );
}

#[test]
fn target_reorder_and_return_still_rejects_the_original_manifest() {
    let (mut runtime, instance, targets) = running();
    let initial = manifest(&runtime);
    let before = runtime.snapshot();
    let stable_positions = positions(&targets, false, 0.0);
    assert!(reconcile(
        &mut runtime,
        instance,
        vec![targets[1], targets[0]],
        &stable_positions
    ));
    let reversed = manifest(&runtime);
    assert_ne!(
        reversed.dependency_generation,
        initial.dependency_generation
    );
    assert_eq!(
        runtime.snapshot().instances[0].phase_by_lane_target,
        before.instances[0].phase_by_lane_target,
        "spatial phases did not change; target order alone invalidates the dependency"
    );
    assert!(reconcile(
        &mut runtime,
        instance,
        targets.to_vec(),
        &stable_positions
    ));
    let current = manifest(&runtime);
    assert_ne!(
        current.dependency_generation,
        reversed.dependency_generation
    );
    assert_ne!(current.dependency_generation, initial.dependency_generation);
    assert_eq!(
        runtime.snapshot(),
        before,
        "returning targets preserves all transport and Random state"
    );
    assert!(
        !runtime
            .install_preset_source_values(&initial, records(&initial, 99.0))
            .unwrap()
    );
    assert!(!reconcile(
        &mut runtime,
        instance,
        targets.to_vec(),
        &stable_positions
    ));
    assert_eq!(
        manifest(&runtime).dependency_generation,
        current.dependency_generation
    );
}

#[test]
fn failed_output_transaction_restores_dependency_generation_and_installed_preset_values() {
    let (mut runtime, instance, targets) = running();
    let original = manifest(&runtime);
    let before = runtime.snapshot();
    let mut provisional = None;
    let mut scratch = DynamicOutputFrameScratch::default();
    let failed: Result<(), &str> = runtime.with_output_frame_transaction(&mut scratch, |runtime| {
        assert!(reconcile(
            runtime,
            instance,
            targets.to_vec(),
            &positions(&targets, true, 0.0)
        ));
        let next = manifest(runtime);
        assert_ne!(next.dependency_generation, original.dependency_generation);
        assert!(
            runtime
                .install_preset_source_values(&next, records(&next, 20.0))
                .unwrap()
        );
        provisional = Some(next);
        Err("final encoding failed")
    });
    assert_eq!(failed, Err("final encoding failed"));
    assert_eq!(runtime.snapshot(), before);
    assert_eq!(
        manifest(&runtime).dependency_generation,
        original.dependency_generation
    );
    assert!(
        runtime
            .install_preset_source_values(&original, original.last_valid.clone())
            .unwrap()
    );
    let provisional = provisional.unwrap();
    assert!(
        !runtime
            .install_preset_source_values(&provisional, records(&provisional, 99.0))
            .unwrap()
    );

    runtime
        .with_output_frame_transaction(&mut scratch, |runtime| {
            assert!(reconcile(
                runtime,
                instance,
                targets.to_vec(),
                &positions(&targets, true, 0.0)
            ));
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_ne!(
        manifest(&runtime).dependency_generation,
        original.dependency_generation
    );
    assert!(
        !runtime
            .install_preset_source_values(&original, original.last_valid.clone())
            .unwrap()
    );
}
