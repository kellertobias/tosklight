use super::*;

#[test]
fn random_each_loop_is_recomputed_per_lane_without_reordering_uniform_lanes() {
    let stable = lane();
    let stable_id = stable.id;
    let mut random = lane();
    let random_id = random.id;
    let mut random_phase = selection_phase(0.0);
    random_phase.ordering = PhaseOrdering::RandomEachLoop { seed: 91 };
    random.phase = Some(random_phase);
    let mut dynamic = definition(stable);
    dynamic.phase_spread_mode = DynamicPhaseSpreadMode::PerLane;
    dynamic.lanes.push(random);
    let definition_id = dynamic.id;
    let targets = (1..=6)
        .map(|value| FixtureId(Uuid::from_u128(value)))
        .collect::<Vec<_>>();
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([dynamic]).unwrap();
    let instance = runtime
        .start(DynamicStartRequest {
            definition_id,
            controller: controller(0, 1, false),
            target_scope: DynamicTargetScope {
                ordered_targets: targets,
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
    let values = |runtime: &mut DynamicRuntime, lane_id, now| {
        runtime
            .sample(instance, now, 1_000, 10, &Sources { current: 0.0 })
            .unwrap()
            .into_iter()
            .filter(|sample| sample.lane_id == lane_id)
            .map(|sample| (sample.target, sample.value))
            .collect::<HashMap<_, _>>()
    };
    let stable_first = values(&mut runtime, stable_id, 0);
    let random_first = values(&mut runtime, random_id, 0);
    assert!((1..=32).any(|loop_index| {
        let now = loop_index * 1_000;
        values(&mut runtime, stable_id, now) == stable_first
            && values(&mut runtime, random_id, now) != random_first
    }));
}

#[test]
fn uniform_random_each_loop_shares_one_permutation_across_different_lane_speeds() {
    let first = lane();
    let first_id = first.id;
    let mut second = lane();
    let second_id = second.id;
    second.speed_multiplier = Rational {
        numerator: 2,
        denominator: 1,
    };
    let mut dynamic = definition(first);
    dynamic.phase.ordering = PhaseOrdering::RandomEachLoop { seed: 117 };
    dynamic.lanes.push(second);
    let definition_id = dynamic.id;
    let targets = (1..=6)
        .map(|value| FixtureId(Uuid::from_u128(value)))
        .collect::<Vec<_>>();
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([dynamic]).unwrap();
    let instance = runtime
        .start(DynamicStartRequest {
            definition_id,
            controller: controller(0, 1, false),
            target_scope: DynamicTargetScope {
                ordered_targets: targets,
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

    for loop_index in 0..=32 {
        let samples = runtime
            .sample(
                instance,
                loop_index * 1_000,
                1_000,
                10,
                &Sources { current: 0.0 },
            )
            .unwrap();
        let lane_values = |lane_id| {
            samples
                .iter()
                .filter(|sample| sample.lane_id == lane_id)
                .map(|sample| (sample.target, sample.value))
                .collect::<HashMap<_, _>>()
        };
        assert_eq!(
            lane_values(first_id),
            lane_values(second_id),
            "uniform phase spread must use one permutation for loop {loop_index}"
        );
    }
}

#[test]
fn explicit_multi_anchor_phase_and_spatial_ties_follow_the_operator_contract() {
    let targets = (0..8).map(|_| FixtureId::new()).collect::<Vec<_>>();
    let mut distribution = definition(lane()).phase;
    distribution.anchors_degrees = vec![0.0, 360.0, 0.0];
    let phases = project_phase(&distribution, &targets, &HashMap::new(), 0);
    assert_eq!(
        phases.iter().map(|phase| phase.degrees).collect::<Vec<_>>(),
        [0.0, 90.0, 180.0, 270.0, 270.0, 180.0, 90.0, 0.0]
    );

    distribution.anchors_degrees.clear();
    distribution.ordering = PhaseOrdering::GridLinear { angle_degrees: 0.0 };
    let mut positions = HashMap::new();
    positions.insert(
        targets[0],
        SpatialPosition {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
    );
    positions.insert(
        targets[1],
        SpatialPosition {
            x: 0.0,
            y: 0.0,
            z: 2.0,
        },
    );
    positions.insert(
        targets[2],
        SpatialPosition {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        },
    );
    let phases = project_phase(&distribution, &targets[..4], &positions, 0);
    assert_eq!(phases[0].degrees, 0.0);
    assert_eq!(phases[1].degrees, 0.0, "exact spatial ties share one rank");
    assert_eq!(phases[2].degrees, 120.0);
    assert_eq!(
        phases[3].degrees, 240.0,
        "a missing Stage position is appended in stored order"
    );
}

#[test]
fn radial_in_keeps_missing_stage_positions_after_positioned_targets() {
    let targets = (0..4).map(|_| FixtureId::new()).collect::<Vec<_>>();
    let mut distribution = definition(lane()).phase;
    distribution.ordering = PhaseOrdering::RadialIn {
        center_x: 0.0,
        center_z: 0.0,
    };
    let positions = HashMap::from([
        (
            targets[0],
            SpatialPosition {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
        ),
        (
            targets[1],
            SpatialPosition {
                x: 3.0,
                y: 0.0,
                z: 0.0,
            },
        ),
    ]);

    let phases = project_phase(&distribution, &targets, &positions, 0);

    assert_eq!(phases[0].target, targets[1]);
    assert_eq!(phases[1].target, targets[0]);
    assert_eq!(
        phases[2..]
            .iter()
            .map(|phase| phase.target)
            .collect::<Vec<_>>(),
        targets[2..],
        "missing Stage positions remain appended in stored order"
    );
}

#[test]
fn lane_width_compresses_the_curve_without_stretching_its_value_range() {
    let mut narrow = lane();
    narrow.width = 0.5;
    let definition = definition(narrow.clone());
    let evaluator = DynamicEvaluator::new(&definition);
    let target = FixtureId::new();
    let sample = |elapsed| {
        evaluator
            .sample_lane(
                &narrow,
                DynamicEvaluationContext {
                    instance_id: Uuid::nil(),
                    target,
                    elapsed_millis: elapsed,
                    cycle_duration_millis: 1_000,
                    phase_degrees: 0.0,
                    output_interval_millis: 10,
                    random_envelope: None,
                    sources: &Sources { current: 0.0 },
                },
            )
            .unwrap()
    };

    assert_eq!(sample(0), 0.0);
    assert!((sample(500) - 1.0).abs() < 0.0001);
    assert_eq!(sample(750), 0.0);
}

#[test]
fn pwm_always_uses_the_full_curve_width() {
    let mut pwm = lane();
    pwm.mode = DynamicLaneMode::MaxMin;
    pwm.width = 0.25;
    pwm.max_min.function = PeriodicFunction::Pwm;
    let definition = definition(pwm.clone());
    let evaluator = DynamicEvaluator::new(&definition);
    let value = evaluator
        .sample_lane(
            &pwm,
            DynamicEvaluationContext {
                instance_id: Uuid::nil(),
                target: FixtureId::new(),
                elapsed_millis: 100,
                cycle_duration_millis: 1_000,
                phase_degrees: 0.0,
                output_interval_millis: 10,
                random_envelope: None,
                sources: &Sources { current: 0.0 },
            },
        )
        .unwrap();

    assert!((value - 1.0).abs() < 0.0001);
}

#[test]
fn random_each_loop_reorders_targets_at_runtime_loop_boundaries() {
    let targets = (1..=6)
        .map(|value| FixtureId(Uuid::from_u128(value)))
        .collect::<Vec<_>>();
    let mut dynamic = definition(lane());
    dynamic.phase.ordering = PhaseOrdering::RandomEachLoop { seed: 73 };
    let definition_id = dynamic.id;
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([dynamic]).unwrap();
    let instance = runtime
        .start(DynamicStartRequest {
            definition_id,
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
    let values = |runtime: &mut DynamicRuntime, now| {
        runtime
            .sample(instance, now, 1_000, 10, &Sources { current: 0.0 })
            .unwrap()
            .into_iter()
            .map(|sample| (sample.target, sample.value))
            .collect::<HashMap<_, _>>()
    };

    let first_loop = values(&mut runtime, 0);
    assert!(
        (1..=32).any(|loop_index| { values(&mut runtime, loop_index * 1_000) != first_loop }),
        "the deterministic ordering must be recomputed at runtime loop boundaries"
    );
}

#[test]
fn random_groups_are_repeatable_per_instance_and_independent_between_instances() {
    let mut lane = lane();
    lane.mode = DynamicLaneMode::Random;
    let group_id = Uuid::new_v4();
    lane.random_group_id = Some(group_id);
    let mut definition = definition(lane.clone());
    definition.random_groups.push(DynamicRandomGroup {
        id: group_id,
        seed: 7,
        low: source(0.0),
        high: source(1.0),
        decision_interval_millis: 100,
        start_probability: 0.5,
        mean_duration_millis: 80,
        duration_spread_millis: 10,
        attack_ratio: 0.1,
        decay_ratio: 0.1,
    });
    validate_definition(&definition).unwrap();
    let evaluator = DynamicEvaluator::new(&definition);
    let target = FixtureId::new();
    let values = |instance| {
        (0..20)
            .map(|step| {
                evaluator.sample_lane(
                    &lane,
                    DynamicEvaluationContext {
                        instance_id: instance,
                        target,
                        elapsed_millis: step * 25,
                        cycle_duration_millis: 1_000,
                        phase_degrees: 0.0,
                        output_interval_millis: 10,
                        random_envelope: None,
                        sources: &Sources { current: 0.0 },
                    },
                )
            })
            .collect::<Vec<_>>()
    };
    let first = values(Uuid::from_u128(1));
    assert_eq!(first, values(Uuid::from_u128(1)));
    assert_ne!(first, values(Uuid::from_u128(2)));
}

#[test]
fn runtime_random_pulses_cross_decision_boundaries_without_overlapping_restart() {
    let mut random_lane = lane();
    random_lane.mode = DynamicLaneMode::Random;
    let group_id = Uuid::new_v4();
    random_lane.random_group_id = Some(group_id);
    let mut definition = definition(random_lane);
    definition.random_groups.push(DynamicRandomGroup {
        id: group_id,
        seed: 11,
        low: source(0.0),
        high: source(1.0),
        decision_interval_millis: 100,
        start_probability: 1.0,
        mean_duration_millis: 250,
        duration_spread_millis: 0,
        attack_ratio: 0.0,
        decay_ratio: 0.5,
    });
    let target = FixtureId::new();
    let definition_id = definition.id;
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition]).unwrap();
    let instance = runtime
        .start(start_request(
            definition_id,
            controller(90, 1, false),
            target,
            0,
            false,
        ))
        .unwrap();
    let at = |runtime: &mut DynamicRuntime, elapsed| {
        runtime
            .sample(instance, elapsed, 1_000, 10, &Sources { current: 0.0 })
            .unwrap()[0]
            .value
    };
    assert_eq!(at(&mut runtime, 0), 1.0);
    let crossing = at(&mut runtime, 150);
    assert!(
        (crossing - 0.8).abs() < 0.001,
        "the pulse which began at zero must decay through the 100ms decision boundary"
    );
    assert!(
        at(&mut runtime, 240) < crossing,
        "the active pulse continues decaying instead of starting an overlapping pulse"
    );
}

#[test]
fn runtime_snapshot_round_trip_preserves_epoch_pause_controllers_and_random_index() {
    let mut random_lane = lane();
    random_lane.mode = DynamicLaneMode::Random;
    let group_id = Uuid::new_v4();
    random_lane.random_group_id = Some(group_id);
    let mut definition = definition(random_lane);
    definition.random_groups.push(DynamicRandomGroup {
        id: group_id,
        seed: 19,
        low: source(0.0),
        high: source(1.0),
        decision_interval_millis: 100,
        start_probability: 0.75,
        mean_duration_millis: 180,
        duration_spread_millis: 20,
        attack_ratio: 0.1,
        decay_ratio: 0.2,
    });
    let definition_id = definition.id;
    let target = FixtureId::new();
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition]).unwrap();
    let instance = runtime
        .start(start_request(
            definition_id,
            controller(93, 4, false),
            target,
            1_000,
            false,
        ))
        .unwrap();
    runtime
        .sample(instance, 1_450, 1_000, 10, &Sources { current: 0.0 })
        .unwrap();
    runtime.set_global_paused(true, 1_500);

    let full_snapshot = runtime.snapshot();
    let output_snapshot = runtime.output_projection_snapshot();
    assert_eq!(output_snapshot.global_paused, full_snapshot.global_paused);
    assert_eq!(
        output_snapshot.instances.len(),
        full_snapshot.instances.len()
    );
    let full_instance = &full_snapshot.instances[0];
    let output_instance = &output_snapshot.instances[0];
    assert_eq!(output_instance.id, full_instance.id);
    assert_eq!(output_instance.definition, full_instance.definition);
    assert_eq!(output_instance.targets, full_instance.targets);
    assert_eq!(output_instance.controllers, full_instance.controllers);
    assert_eq!(
        output_instance.controller_transitions,
        full_instance.controller_transitions
    );
    assert_eq!(
        output_instance.pending_until_millis,
        full_instance.pending_until_millis
    );
    assert_eq!(output_instance.completed, full_instance.completed);
    assert!(output_instance.phase_by_target.is_empty());
    assert!(output_instance.phase_by_lane_target.is_empty());
    assert!(output_instance.random_streams.is_empty());
    assert!(output_instance.last_sample_values.is_empty());
    assert!(output_instance.synchronized_hold_values.is_empty());

    let serialized = serde_json::to_string(&full_snapshot).unwrap();
    let snapshot: DynamicRuntimeSnapshot = serde_json::from_str(&serialized).unwrap();
    let mut restored = DynamicRuntime::default();
    restored.restore_snapshot(snapshot).unwrap();
    assert_eq!(restored.snapshot(), runtime.snapshot());

    runtime.set_global_paused(false, 1_750);
    restored.set_global_paused(false, 1_750);
    assert_eq!(
        restored
            .sample(instance, 2_000, 1_000, 10, &Sources { current: 0.0 })
            .unwrap(),
        runtime
            .sample(instance, 2_000, 1_000, 10, &Sources { current: 0.0 })
            .unwrap()
    );
}

#[test]
fn malformed_runtime_snapshot_is_rejected_without_discarding_valid_definitions_or_live_state() {
    let target = FixtureId::new();
    let definition = definition(lane());
    let definition_id = definition.id;
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition]).unwrap();
    let instance = runtime
        .start(start_request(
            definition_id,
            controller(96, 3, false),
            target,
            1_000,
            false,
        ))
        .unwrap();
    let before = runtime.snapshot();
    let mut malformed = before.clone();
    malformed.instances[0].controllers.clear();

    assert!(matches!(
        runtime.restore_snapshot(malformed),
        Err(DynamicRuntimeError::InvalidSnapshot(_))
    ));
    assert_eq!(
        runtime.snapshot(),
        before,
        "recovery leaves the last valid runtime untouched"
    );
    let controller_id = before.instances[0].controllers[0].id;
    runtime
        .off_controller(instance, controller_id, 1_500, 0, 0)
        .unwrap();
    assert!(
        runtime
            .start(start_request(
                definition_id,
                controller(97, 3, false),
                target,
                2_000,
                false,
            ))
            .is_ok(),
        "the valid portable definition remains installed after runtime recovery"
    );
}

#[test]
fn preload_definition_pin_keeps_live_revision_until_atomic_unpin() {
    let target = FixtureId::new();
    let original = definition(lane());
    let definition_id = original.id;
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([original.clone()]).unwrap();
    let instance = runtime
        .start(start_request(
            definition_id,
            controller(94, 1, false),
            target,
            0,
            false,
        ))
        .unwrap();
    let sample = |runtime: &mut DynamicRuntime| {
        runtime
            .sample(instance, 0, 1_000, 10, &Sources { current: 0.0 })
            .unwrap()[0]
            .value
    };
    assert_eq!(sample(&mut runtime), 0.0);

    let mut edited = original;
    edited.revision = 2;
    for point in &mut edited.lanes[0].keyframes.points {
        point.source = source(1.0);
    }
    runtime.set_definitions_pinned(true);
    runtime.install_definitions([edited]).unwrap();
    assert_eq!(
        sample(&mut runtime),
        0.0,
        "Live keeps the effective pre-Preload definition"
    );

    runtime.set_definitions_pinned(false);
    assert_eq!(
        sample(&mut runtime),
        1.0,
        "GO/unpin hot-swaps the definition without restarting the instance"
    );
}

#[test]
fn speed_group_join_and_next_boundary_use_authoritative_transport_phase() {
    let target = FixtureId::new();
    let mut joined_definition = definition(lane());
    joined_definition.speed = DynamicSpeed::SpeedGroup {
        group: SpeedGroup::A,
        beats_per_cycle: Rational {
            numerator: 4,
            denominator: 1,
        },
    };
    joined_definition.default_activation = ActivationPolicy::JoinSyncNow;
    let joined_id = joined_definition.id;
    let mut runtime = DynamicRuntime::default();
    runtime
        .install_definitions([joined_definition.clone()])
        .unwrap();
    runtime
        .start(start_request(
            joined_id,
            controller(91, 1, false),
            target,
            1_000,
            false,
        ))
        .unwrap();
    let transport = DynamicSpeedTransport {
        effective_bpm: 60.0,
        phase_origin_millis: 0,
        phase_reference_millis: 1_250,
        beat_phase: 0.25,
        phase_advancing: true,
    };
    let joined = runtime.sample_all(1_250, 10, &[transport; 5], &Sources { current: 0.0 });
    assert!(
        (joined[0].value - 0.625).abs() < 0.001,
        "Join sync now must sample the Speed Group epoch instead of a local start epoch"
    );

    let mut pending_definition = joined_definition;
    pending_definition.id = Uuid::new_v4();
    pending_definition.default_activation = ActivationPolicy::NextBoundary;
    let pending_id = pending_definition.id;
    let mut pending = DynamicRuntime::default();
    pending.install_definitions([pending_definition]).unwrap();
    pending
        .start(start_request(
            pending_id,
            controller(92, 1, false),
            target,
            1_250,
            false,
        ))
        .unwrap();
    assert!(
        pending
            .sample_all(
                1_500,
                10,
                &[DynamicSpeedTransport {
                    phase_reference_millis: 1_500,
                    beat_phase: 0.5,
                    ..transport
                }; 5],
                &Sources { current: 0.0 },
            )
            .is_empty()
    );
    let boundary = pending.sample_all(
        2_000,
        10,
        &[DynamicSpeedTransport {
            phase_reference_millis: 2_000,
            beat_phase: 0.0,
            ..transport
        }; 5],
        &Sources { current: 0.0 },
    );
    assert_eq!(boundary[0].value, 0.0);
}
