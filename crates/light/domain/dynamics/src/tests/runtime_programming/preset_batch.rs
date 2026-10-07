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

fn batch(
    manifests: &[DynamicInstancePresetSources],
    value: f32,
) -> Vec<(DynamicInstancePresetSources, Vec<DynamicPresetSourceValues>)> {
    manifests
        .iter()
        .map(|manifest| (manifest.clone(), records(manifest, value)))
        .collect()
}

fn sources() -> TypedSources {
    TypedSources {
        current: Some(DynamicValue::Scalar(0.0)),
        preset: None,
        calls: Cell::new(0),
    }
}

fn running() -> (DynamicRuntime, Vec<(Uuid, Uuid)>) {
    let preset = DynamicValueSource::Preset {
        retained: None,
        preset_id: "batch".into(),
        address: pan_address(),
        last_valid_by_target: vec![],
    };
    let mut dynamic = definition(ramp(pan_address(), preset.clone(), preset));
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
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([dynamic.clone()]).unwrap();
    let mut instances = Vec::new();
    for index in 1..=2 {
        let control = controller(index, 1, false);
        let mut request = start_request(dynamic.id, control.clone(), FixtureId::new(), 0, false);
        request.target_scope.ordered_targets.push(FixtureId::new());
        instances.push((runtime.start(request).unwrap(), control.id));
    }
    for manifest in runtime.preset_source_instances() {
        assert!(
            runtime
                .install_preset_source_values(&manifest, records(&manifest, 10.0))
                .unwrap()
        );
    }
    for &(instance, _) in &instances {
        sampled(&mut runtime, instance, 150, &sources());
    }
    (runtime, instances)
}

#[test]
fn invalid_member_never_publishes_any_prepared_table() {
    let (runtime, _) = running();
    let manifests = runtime.preset_source_instances();
    let before = runtime.snapshot();
    for invalid_index in 0..2 {
        let mut values = batch(&manifests, 20.0);
        values[invalid_index].1[0].values[0].value = DynamicValue::Scalar(f32::NAN);
        assert!(matches!(
            runtime.prepare_preset_source_values(values),
            Err(DynamicRuntimeError::InvalidSnapshot(_))
        ));
        assert_eq!(runtime.snapshot(), before);
    }
}

#[test]
fn duplicate_then_stale_then_payload_precedence_is_independent_of_entry_order() {
    let (mut runtime, _) = running();
    let manifests = runtime.preset_source_instances();
    assert!(runtime.invalidate_preset_source_dependencies(manifests[0].instance_id));
    let before = runtime.snapshot();
    for reverse in [false, true] {
        let mut values = batch(&manifests, 20.0);
        values[1].1[0].values[0].value = DynamicValue::Scalar(f32::NAN);
        if reverse {
            values.reverse();
        }
        assert!(
            runtime
                .prepare_preset_source_values(values)
                .unwrap()
                .is_none()
        );
        let mut duplicates = batch(&manifests, 20.0);
        duplicates.push(duplicates[0].clone());
        if reverse {
            duplicates.reverse();
        }
        let error = runtime
            .prepare_preset_source_values(duplicates)
            .unwrap_err();
        assert!(error.to_string().contains("duplicate instance"));
        assert_eq!(runtime.snapshot(), before);
    }
    let empty = runtime.prepare_preset_source_values([]).unwrap().unwrap();
    assert!(runtime.install_prepared_preset_source_values(empty));
    assert_eq!(runtime.snapshot(), before);
}

#[test]
fn preparation_checks_missing_instances_ordered_targets_and_exact_source_ids() {
    let (runtime, _) = running();
    let manifests = runtime.preset_source_instances();
    let before = runtime.snapshot();
    for changed_field in 0..3 {
        let mut values = batch(&manifests, 20.0);
        match changed_field {
            0 => values[1].0.instance_id = Uuid::new_v4(),
            1 => values[1].0.ordered_targets.reverse(),
            2 => Arc::make_mut(&mut values[1].0.sources[0]).id = Uuid::new_v4(),
            _ => unreachable!(),
        }
        assert!(
            runtime
                .prepare_preset_source_values(values)
                .unwrap()
                .is_none()
        );
        assert_eq!(runtime.snapshot(), before);
    }
}

#[test]
fn one_stale_member_rejects_the_complete_prepared_batch() {
    let (mut runtime, _) = running();
    let manifests = runtime.preset_source_instances();
    let prepared = runtime
        .prepare_preset_source_values(batch(&manifests, 20.0))
        .unwrap()
        .unwrap();
    assert!(runtime.invalidate_preset_source_dependencies(manifests[1].instance_id));
    let before = runtime.snapshot();
    let current = runtime.preset_source_instances();
    assert!(!runtime.install_prepared_preset_source_values(prepared));
    assert_eq!(runtime.snapshot(), before);
    for (before, after) in current.iter().zip(runtime.preset_source_instances()) {
        assert_eq!(before.dependency_generation, after.dependency_generation);
        assert_eq!(before.last_valid, after.last_valid);
    }
}

#[test]
fn recompiled_source_ids_stale_the_token_even_when_dependency_generation_is_unchanged() {
    let (mut runtime, instances) = running();
    let manifests = runtime.preset_source_instances();
    let prepared = runtime
        .prepare_preset_source_values(batch(&manifests, 20.0))
        .unwrap()
        .unwrap();
    let mut definition = runtime
        .instance_definition(instances[0].0)
        .unwrap()
        .as_ref()
        .clone();
    definition.name = "Recompiled source generation".into();
    runtime.install_definitions([definition]).unwrap();
    let current = runtime.preset_source_instances();
    assert_eq!(
        current[0].dependency_generation,
        manifests[0].dependency_generation
    );
    assert_ne!(current[0].sources[0].id, manifests[0].sources[0].id);
    let before = runtime.snapshot();
    assert!(!runtime.install_prepared_preset_source_values(prepared));
    assert_eq!(runtime.snapshot(), before);
}

#[test]
fn successful_batch_preserves_latest_pause_clocks_random_controls_and_held_history() {
    let (mut runtime, instances) = running();
    let manifests = runtime.preset_source_instances();
    let prepared = runtime
        .prepare_preset_source_values(batch(&manifests, 20.0))
        .unwrap()
        .unwrap();
    let at_preparation = runtime.snapshot();
    let mut held = Vec::new();
    for &(instance, control) in &instances {
        sampled(&mut runtime, instance, 650, &sources());
        runtime
            .update_controller(control, None, Some(1.5), Some(45.0))
            .unwrap();
        runtime
            .set_controller_paused(instance, control, true, 650)
            .unwrap();
        held.push(sampled(&mut runtime, instance, 700, &sources()));
    }
    let mut expected = runtime.snapshot();
    assert_ne!(expected, at_preparation);
    for instance in &mut expected.instances {
        assert!(!instance.random_streams.is_empty());
        assert!(!instance.last_sample_values.is_empty());
        assert_eq!(instance.paused_at_millis, Some(650));
        let manifest = manifests
            .iter()
            .find(|row| row.instance_id == instance.id)
            .unwrap();
        instance.preset_source_values = records(manifest, 20.0);
    }
    assert!(runtime.install_prepared_preset_source_values(prepared));
    assert_eq!(runtime.snapshot(), expected);
    for ((instance, control), mut held) in instances.into_iter().zip(held) {
        // StartNow pause holds transport phase, not source resolution. A newly installed
        // Preset is read at that same phase; Random and the unanimated Current partner stay.
        for sample in &mut held {
            if let DynamicSampleExpression::Programming { address, value, .. } =
                &mut sample.expression
                && address.component == Some(ProgrammingComponent::Pan)
            {
                *value = DynamicValue::Scalar(20.0);
            }
        }
        assert_eq!(sampled(&mut runtime, instance, 900, &sources()), held);
        runtime
            .set_controller_paused(instance, control, false, 900)
            .unwrap();
        let live = sampled(&mut runtime, instance, 1000, &sources());
        let pan = live
            .iter()
            .find(|sample| {
                sample
                    .expression
                    .programming_leaf()
                    .is_some_and(|(address, _)| {
                        address.component == Some(ProgrammingComponent::Pan)
                    })
            })
            .unwrap();
        assert_eq!(typed_value(pan), &DynamicValue::Scalar(20.0));
    }
}

#[test]
fn batch_publication_rolls_back_with_sampling_and_can_commit_after_retry() {
    let (mut runtime, instances) = running();
    let manifests = runtime.preset_source_instances();
    let before = runtime.snapshot();
    let prepared = runtime
        .prepare_preset_source_values(batch(&manifests, 20.0))
        .unwrap()
        .unwrap();
    let mut scratch = DynamicOutputFrameScratch::default();
    let failed: Result<(), &str> = runtime.with_output_frame_transaction(&mut scratch, |runtime| {
        for &(instance, _) in &instances {
            sampled(runtime, instance, 650, &sources());
        }
        assert!(runtime.install_prepared_preset_source_values(prepared));
        runtime.set_global_paused(true, 650);
        Err("final projection failed")
    });
    assert_eq!(failed, Err("final projection failed"));
    assert_eq!(runtime.snapshot(), before);
    let retry = runtime
        .prepare_preset_source_values(batch(&manifests, 20.0))
        .unwrap()
        .unwrap();
    runtime
        .with_output_frame_transaction(&mut scratch, |runtime| {
            assert!(runtime.install_prepared_preset_source_values(retry));
            Ok::<_, ()>(())
        })
        .unwrap();
    let mut expected = before;
    for instance in &mut expected.instances {
        let manifest = manifests
            .iter()
            .find(|row| row.instance_id == instance.id)
            .unwrap();
        instance.preset_source_values = records(manifest, 20.0);
    }
    assert_eq!(runtime.snapshot(), expected);
}

#[test]
fn preset_readiness_follows_dependency_changes_restore_and_definition_rebinding() {
    let (mut runtime, instances) = running();
    assert!(!runtime.has_pending_preset_sources());
    assert!(runtime.pending_preset_source_instances().is_empty());
    let instance = instances[0].0;
    runtime.invalidate_preset_source_dependencies(instance);
    let pending = runtime.pending_preset_source_instances();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].instance_id, instance);
    let token = runtime
        .prepare_preset_source_values(batch(&pending, 30.0))
        .unwrap()
        .unwrap();
    assert!(runtime.install_prepared_preset_source_values(token));
    assert!(!runtime.has_pending_preset_sources());
    let saved = runtime.snapshot();
    let mut restored = runtime.fork_for_preview();
    restored.restore_snapshot(saved).unwrap();
    assert!(restored.has_pending_preset_sources());
    assert_eq!(restored.pending_preset_source_instances().len(), 2);
    assert!(!runtime.has_pending_preset_sources());

    let mut definition = runtime.snapshot().instances[0].definition.clone();
    definition.revision += 1;
    definition.name = "Recompiled dependency".into();
    runtime.install_definitions([definition]).unwrap();
    assert!(runtime.has_pending_preset_sources());
    assert_eq!(runtime.pending_preset_source_instances().len(), 2);
}

#[test]
fn unavailable_preparation_is_fresh_and_frame_failure_restores_readiness() {
    let (mut runtime, instances) = running();
    runtime.invalidate_preset_source_dependencies(instances[0].0);
    let pending = runtime.pending_preset_source_instances();
    let mut scratch = DynamicOutputFrameScratch::default();
    let failed: Result<(), &str> = runtime.with_output_frame_transaction(&mut scratch, |runtime| {
        let token = runtime
            .prepare_preset_source_values(vec![(pending[0].clone(), vec![])])
            .unwrap()
            .unwrap();
        assert!(runtime.install_prepared_preset_source_values(token));
        assert!(!runtime.has_pending_preset_sources());
        Err("encoding failed")
    });
    assert!(failed.is_err());
    assert!(runtime.has_pending_preset_sources());
    assert_eq!(
        runtime.pending_preset_source_instances()[0].dependency_generation,
        pending[0].dependency_generation
    );
    let token = runtime
        .prepare_preset_source_values(vec![(pending[0].clone(), vec![])])
        .unwrap()
        .unwrap();
    assert!(runtime.install_prepared_preset_source_values(token));
    for _ in 0..5 {
        assert!(!runtime.has_pending_preset_sources());
        assert!(runtime.pending_preset_source_instances().is_empty());
    }
}
