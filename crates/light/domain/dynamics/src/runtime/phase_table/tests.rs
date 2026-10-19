use super::*;
use crate::*;

// Small copied scalar fixtures; no new production or test-only runtime hooks.
fn source(value: f32) -> ScalarSource {
    ScalarSource::Value { value }
}

fn lane() -> DynamicLane {
    DynamicLane {
        id: Uuid::new_v4(),
        body: crate::DynamicLaneBody::LegacyScalar(crate::LegacyScalarLaneBody {
            attribute: AttributeKey::intensity(),
            mode: DynamicLaneMode::Keyframes,
            keyframes: KeyframeConfiguration {
                points: vec![
                    DynamicKeyframe {
                        position: 0.0,
                        source: source(0.0),
                        interpolation: ScalarInterpolation::Linear,
                    },
                    DynamicKeyframe {
                        position: 0.5,
                        source: source(1.0),
                        interpolation: ScalarInterpolation::Linear,
                    },
                ],
                size: 1.0,
            },
            max_min: MaxMinConfiguration {
                minimum: source(0.0),
                maximum: source(1.0),
                function: PeriodicFunction::Sinus,
                size: 1.0,
                pwm: PwmShape::default(),
            },
            middle_amplitude: MiddleAmplitudeConfiguration {
                middle: ScalarSource::Current,
                amplitude: 0.5,
                function: PeriodicFunction::Sinus,
                size: 1.0,
                pwm: PwmShape::default(),
                invert_waveform: false,
            },
        }),
        speed_multiplier: Rational::ONE,
        width: 1.0,
        phase: None,
        random_group_id: None,
    }
}

fn definition(lane: DynamicLane) -> DynamicDefinition {
    DynamicDefinition {
        id: Uuid::new_v4(),
        pool_number: 1,
        revision: 1,
        name: "Test".into(),
        color: None,
        icon: None,
        target_binding: DynamicTargetBinding::Targetless,
        lanes: vec![lane],
        random_groups: vec![],
        phase_spread_mode: DynamicPhaseSpreadMode::Uniform,
        spatial_mapping: DynamicSpatialMappingOverride::default(),
        phase: PhaseDistribution {
            ordering: PhaseOrdering::Selection,
            offset_degrees: 0.0,
            span_degrees: 360.0,
            block_size: 1,
            repeats: 1,
            wings: false,
            anchors_degrees: vec![],
        },
        speed: DynamicSpeed::Fixed {
            duration_millis: 1_000,
        },
        overall_speed_multiplier: Rational::ONE,
        run_mode: DynamicRunMode::Loop,
        default_activation: ActivationPolicy::StartNow,
        activation_boundary: ActivationBoundary::Beat,
    }
}

fn controller(id: u128, priority: i16, paused: bool) -> DynamicController {
    DynamicController {
        id: Uuid::from_u128(id),
        source: DynamicControllerSource::Programmer {
            programmer_id: Uuid::from_u128(id + 100),
            instance_link: None,
        },
        priority,
        activated_at_millis: id as u64,
        size: 1.0,
        speed_multiplier: 1.0,
        phase_offset_degrees: 0.0,
        paused,
    }
}

fn targets() -> Vec<FixtureId> {
    (1..=3).map(|id| FixtureId(Uuid::from_u128(id))).collect()
}
fn fixture() -> (DynamicRuntime, Uuid, DynamicDefinition, Vec<FixtureId>) {
    let mut definition = definition(lane());
    definition.lanes.push(lane());
    definition.phase_spread_mode = DynamicPhaseSpreadMode::PerLane;
    definition.phase.ordering = PhaseOrdering::SourceOrder;
    let targets = targets();
    let mut runtime = DynamicRuntime::with_programming_contract_support(
        light_core::programming::DYNAMIC_PHASE_ORDERING_CONTRACT,
    );
    runtime.install_definitions([definition.clone()]).unwrap();
    let id = runtime
        .start(DynamicStartRequest {
            definition_id: definition.id,
            controller: controller(0, 1, false),
            target_scope: DynamicTargetScope {
                ordered_targets: targets.clone(),
            },
            stage_positions: HashMap::new(),
            inherited_spatial_mapping: None,
            now_millis: 0,
            activation_policy_override: None,
            activation_delay_millis: 0,
            activation_duration_millis: 0,
            reuse_matching_targetless: false,
        })
        .unwrap();
    (runtime, id, definition, targets)
}
fn bits(value: Option<&f32>) -> Option<u32> {
    value.map(|value| value.to_bits())
}
fn rows(table: &PhaseTable) -> HashMap<Key, f32> {
    table.iter().map(|(key, phase)| (*key, *phase)).collect()
}
// Frozen old fallback: hash get first, then +0.0 for an absent key.
fn old_phase(map: &HashMap<Key, f32>, key: Key) -> f32 {
    map.get(&key).copied().unwrap_or(0.0)
}
struct Sources;
impl ScalarSourceResolver for Sources {
    fn current(&self, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        Some(0.3)
    }
    fn preset(&self, _: &str, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        None
    }
}

#[test]
fn authoritative_rows_preserve_std_bits_holes_unknowns_and_guarded_lookup() {
    let (_, _, definition, targets) = fixture();
    let lanes = &definition.lanes;
    let unknown = (Uuid::from_u128(999), FixtureId(Uuid::from_u128(1000)));
    let map = HashMap::from([
        ((lanes[0].id, targets[0]), -0.0),
        ((lanes[0].id, targets[1]), 0.0),
        ((lanes[1].id, targets[0]), f32::from_bits(1)),
        ((lanes[1].id, targets[2]), -45.5),
        (unknown, 77.25),
    ]);
    let table = PhaseTable::new(map.clone(), &definition, &targets);
    assert!(table.dense.is_some());
    let fallback = PhaseTable {
        rows: table.rows.clone(),
        dense: None,
    };
    assert_eq!(table.iter().len(), map.len());
    assert!(
        table
            .rows
            .windows(2)
            .all(|a| (a[0].0.0, a[0].0.1.0) < (a[1].0.0, a[1].0.1.0))
    );
    for (target_index, target) in targets.iter().copied().enumerate() {
        for (lane_index, lane) in lanes.iter().enumerate() {
            let key = (lane.id, target);
            assert_eq!(bits(table.get(&key)), bits(map.get(&key)));
            assert_eq!(
                bits(table.get_indexed(lane_index, lane.id, target_index, target)),
                bits(map.get(&key))
            );
            assert_eq!(
                bits(fallback.get_indexed(lane_index, lane.id, target_index, target)),
                bits(map.get(&key))
            );
            // Reordered or out-of-range axes must use the exact key, never the wrong dense cell.
            assert_eq!(
                bits(table.get_indexed(1 - lane_index, lane.id, 2 - target_index, target)),
                bits(map.get(&key))
            );
            assert_eq!(
                bits(table.get_indexed(usize::MAX, lane.id, usize::MAX, target)),
                bits(map.get(&key))
            );
            assert_eq!(
                table
                    .get_indexed(lane_index, lane.id, target_index, target)
                    .copied()
                    .unwrap_or(0.0)
                    .to_bits(),
                old_phase(&map, key).to_bits()
            );
        }
    }
    assert_eq!(
        bits(table.get_indexed(0, unknown.0, 0, unknown.1)),
        bits(map.get(&unknown))
    );
    assert!(table.matches_projected(&map));
    let mut opposite_zero = map.clone();
    opposite_zero.insert((lanes[0].id, targets[0]), 0.0);
    assert!(table.matches_projected(&opposite_zero)); // Preserve original f32 equality, not bit equality.
    let mut nan = map;
    nan.insert(unknown, f32::from_bits(0x7fc00003));
    let nan_table = PhaseTable::new(nan.clone(), &definition, &targets);
    assert!(!nan_table.matches_projected(&nan));
    assert_eq!(bits(nan_table.get(&unknown)), Some(0x7fc00003));
}

#[test]
fn sparse_empty_huge_or_overflowing_dense_axes_have_lossless_binary_fallback() {
    assert_eq!(dense_cells(2, 3, 2), Some(6));
    assert_eq!(dense_cells(2, 3, 1), None);
    assert_eq!(dense_cells(0, 3, 9), None);
    assert_eq!(dense_cells(2, 0, 9), None);
    assert_eq!(dense_cells(257, 256, usize::MAX), None);
    assert_eq!(dense_cells(usize::MAX, 2, usize::MAX), None);
    if let Some(too_many) = (u32::MAX as usize).checked_add(1) {
        assert_eq!(dense_cells(1, 1, too_many), None);
    }
    let (_, _, mut definition, targets) = fixture();
    let key = (definition.lanes[0].id, targets[1]);
    for map in [HashMap::new(), HashMap::from([(key, -0.0)])] {
        let table = PhaseTable::new(map.clone(), &definition, &targets);
        assert!(table.dense.is_none());
        assert_eq!(bits(table.get(&key)), bits(map.get(&key)));
        assert_eq!(table.iter().len(), map.len());
    }
    let prototype = definition.lanes[0].clone();
    definition.lanes = vec![prototype; 257];
    let huge_targets = (0..256)
        .map(|id| FixtureId(Uuid::from_u128(id)))
        .collect::<Vec<_>>();
    let table = PhaseTable::new(HashMap::from([(key, 9.0)]), &definition, &huge_targets);
    assert!(table.dense.is_none());
    assert_eq!(table.get(&key), Some(&9.0));
}

#[test]
fn runtime_noop_preserves_signed_zero_arc_and_cold_cuts_survive_edit_and_rollback() {
    let (mut runtime, id, definition, targets) = fixture();
    let mut map = rows(&runtime.instances[&id].phase_by_lane_target);
    let key = (definition.lanes[0].id, targets[0]);
    map.insert(key, -0.0);
    let signed = Arc::new(PhaseTable::new(map, &definition, &targets));
    runtime.instances.get_mut(&id).unwrap().phase_by_lane_target = signed.clone();
    assert!(
        !runtime
            .reconcile_instance_targets(
                id,
                DynamicTargetScope {
                    ordered_targets: targets.clone()
                },
                &HashMap::new(),
                None
            )
            .unwrap()
    );
    assert!(Arc::ptr_eq(
        &runtime.instances[&id].phase_by_lane_target,
        &signed
    ));
    assert_eq!(bits(signed.get(&key)), Some((-0.0f32).to_bits()));
    let preview = runtime.fork_for_pending_preview();
    assert!(Arc::ptr_eq(
        &preview.instances[&id].phase_by_lane_target,
        &signed
    ));
    let mut scratch = DynamicOutputFrameScratch::default();
    let failure: Result<(), DynamicRuntimeError> =
        runtime.with_output_frame_transaction(&mut scratch, |runtime| {
            assert!(runtime.reconcile_instance_targets(
                id,
                DynamicTargetScope {
                    ordered_targets: targets.iter().rev().copied().collect()
                },
                &HashMap::new(),
                None
            )?);
            Err(DynamicRuntimeError::MissingInstance)
        });
    assert!(failure.is_err());
    assert!(Arc::ptr_eq(
        &runtime.instances[&id].phase_by_lane_target,
        &signed
    ));
    assert!(
        runtime
            .reconcile_instance_targets(
                id,
                DynamicTargetScope {
                    ordered_targets: targets.iter().rev().copied().collect()
                },
                &HashMap::new(),
                None
            )
            .unwrap()
    );
    assert!(!Arc::ptr_eq(
        &runtime.instances[&id].phase_by_lane_target,
        &signed
    ));
    assert!(Arc::ptr_eq(
        &preview.instances[&id].phase_by_lane_target,
        &signed
    ));
    assert_eq!(bits(signed.get(&key)), Some((-0.0f32).to_bits()));
}

#[test]
fn restored_duplicate_rows_last_win_unknown_rows_survive_and_invalid_restore_is_atomic() {
    let (runtime, id, _, targets) = fixture();
    let mut snapshot = runtime.snapshot();
    let stored = &mut snapshot.instances[0];
    let lanes = stored.definition.lanes.clone();
    let unknown = (Uuid::from_u128(999), FixtureId(Uuid::from_u128(1000)));
    stored.phase_by_target = vec![(targets[0], -0.0), (targets[0], 0.0)];
    stored.phase_by_lane_target = vec![
        (lanes[0].id, targets[0], -0.0),
        (unknown.0, unknown.1, 17.0),
        (lanes[1].id, targets[0], 1.0),
        (lanes[1].id, targets[0], f32::from_bits(1)),
    ];
    // Frozen original restore algorithm: expand every legacy target row, then overwrite in order.
    let mut old = stored
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
    old.extend(
        stored
            .phase_by_lane_target
            .iter()
            .map(|(lane, target, phase)| ((*lane, *target), *phase)),
    );
    let mut restored = DynamicRuntime::with_programming_contract_support(
        light_core::programming::DYNAMIC_PHASE_ORDERING_CONTRACT,
    );
    restored.restore_snapshot(snapshot).unwrap();
    let table = &restored.instances[&id].phase_by_lane_target;
    assert_eq!(table.iter().len(), old.len());
    for (key, value) in &old {
        assert_eq!(bits(table.get(key)), Some(value.to_bits()));
    }
    let mut expected = old
        .iter()
        .map(|((lane, target), phase)| (*lane, *target, phase.to_bits()))
        .collect::<Vec<_>>();
    expected.sort_by_key(|(lane, target, _)| (*lane, target.0));
    let before = restored.snapshot();
    assert_eq!(
        before.instances[0]
            .phase_by_lane_target
            .iter()
            .map(|(lane, target, phase)| (*lane, *target, phase.to_bits()))
            .collect::<Vec<_>>(),
        expected
    );
    let mut invalid = before.clone();
    invalid.instances[0].phase_by_lane_target[0].2 = f32::NAN;
    let arc = Arc::clone(&restored.instances[&id].phase_by_lane_target);
    assert!(restored.restore_snapshot(invalid).is_err());
    assert!(Arc::ptr_eq(
        &restored.instances[&id].phase_by_lane_target,
        &arc
    ));
    assert_eq!(restored.snapshot(), before);
}

#[test]
fn definition_and_target_reorder_fall_back_to_authoritative_keys_without_value_recompute() {
    let (mut runtime, id, mut definition, targets) = fixture();
    let old = Arc::clone(&runtime.instances[&id].phase_by_lane_target);
    let map = rows(&old);
    definition.revision += 1;
    definition.lanes.reverse();
    runtime.install_definitions([definition.clone()]).unwrap();
    assert!(Arc::ptr_eq(
        &runtime.instances[&id].phase_by_lane_target,
        &old
    ));
    // A retained map remains keyed by identity through a definition reorder.
    for now in [0, 1, 249, 501] {
        let samples = runtime.sample(id, now, 1000, 10, &Sources).unwrap();
        assert_eq!(samples.len(), 6);
        for sample in samples {
            let lane = definition
                .lanes
                .iter()
                .find(|lane| lane.id == sample.lane_id)
                .unwrap();
            let expected = DynamicEvaluator::new(&definition)
                .sample_lane(
                    lane,
                    DynamicEvaluationContext {
                        instance_id: id,
                        target: sample.target,
                        elapsed_millis: now,
                        cycle_duration_millis: 1000,
                        phase_degrees: old_phase(&map, (lane.id, sample.target)),
                        output_interval_millis: 10,
                        random_envelope: None,
                        sources: &Sources,
                    },
                )
                .unwrap();
            assert_eq!(sample.legacy().unwrap().value.to_bits(), expected.to_bits());
        }
    }
    let mut edited = definition;
    edited.revision += 1;
    edited.phase.ordering = PhaseOrdering::Spatial {
        shape: OverrideStage::Inherit,
    };
    runtime.install_definitions([edited]).unwrap();
    let before = runtime.snapshot();
    let arc = Arc::clone(&runtime.instances[&id].phase_by_lane_target);
    let mut invalid = SpatialSelectionMapping {
        projection: SpatialProjection::from_preset(ProjectionPreset::Top, Position3d::default()),
        shape: SpatialSelectionShape::Grid {
            angle_degrees: 0.0,
            direction: RankDirection::Ascending,
        },
    };
    invalid.projection.view_direction = Vector3::default();
    assert!(
        runtime
            .reconcile_instance_targets(
                id,
                DynamicTargetScope {
                    ordered_targets: targets
                },
                &HashMap::new(),
                Some(&invalid)
            )
            .is_err()
    );
    assert!(Arc::ptr_eq(
        &runtime.instances[&id].phase_by_lane_target,
        &arc
    ));
    assert_eq!(runtime.snapshot(), before);
}

#[test]
fn random_overrides_remain_first_and_paused_preview_keeps_exact_old_cut() {
    let (mut clean, id, mut definition, targets) = fixture();
    definition.revision += 1;
    definition.phase.ordering = PhaseOrdering::Spatial {
        shape: OverrideStage::Replace(DynamicSelectionShape::Random { seed: 91 }),
    };
    clean.install_definitions([definition.clone()]).unwrap();
    let mut poison = clean.fork_for_pending_preview();
    let old = Arc::clone(&clean.instances[&id].phase_by_lane_target);
    let changed = old
        .iter()
        .map(|(key, _)| (*key, 123.0))
        .collect::<HashMap<_, _>>();
    poison.instances.get_mut(&id).unwrap().phase_by_lane_target =
        Arc::new(PhaseTable::new(changed, &definition, &targets));
    assert!(
        !poison.instances[&id]
            .phase_by_lane_target
            .matches_projected(&rows(&old))
    );
    for now in [0, 499, 1000, 2500] {
        let samples = clean.sample(id, now, 1000, 10, &Sources).unwrap();
        assert_eq!(samples.len(), 6);
        assert_eq!(samples, poison.sample(id, now, 1000, 10, &Sources).unwrap());
    }
    clean.set_global_paused(true, 2500);
    poison.set_global_paused(true, 2500);
    let mut preview = poison.fork_for_pending_preview();
    let held = clean.sample(id, 3000, 1000, 10, &Sources).unwrap();
    assert_eq!(held.len(), 6);
    assert_eq!(held, poison.sample(id, 3000, 1000, 10, &Sources).unwrap());
    assert_eq!(held, preview.sample(id, 3000, 1000, 10, &Sources).unwrap());
}
