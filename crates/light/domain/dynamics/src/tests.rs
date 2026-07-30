use super::*;
use light_core::{AttributeKey, FixtureId};
use std::{cell::Cell, collections::HashMap};
use uuid::Uuid;

mod runtime_control;
mod synchronization;

struct Sources {
    current: f32,
}

impl ScalarSourceResolver for Sources {
    fn current(&self, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        Some(self.current)
    }

    fn preset(&self, _: &str, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        None
    }
}

struct CountingSources {
    current: f32,
    current_calls: Cell<usize>,
}

impl ScalarSourceResolver for CountingSources {
    fn current(&self, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        self.current_calls.set(self.current_calls.get() + 1);
        Some(self.current)
    }

    fn preset(&self, _: &str, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        None
    }
}

fn source(value: f32) -> ScalarSource {
    ScalarSource::Value { value }
}

fn lane() -> DynamicLane {
    DynamicLane {
        id: Uuid::new_v4(),
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
        },
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

#[test]
fn full_size_controller_does_not_resolve_an_unused_current_underlay() {
    let target = FixtureId::new();
    let mut dynamic_lane = lane();
    dynamic_lane.mode = DynamicLaneMode::MaxMin;
    let definition = definition(dynamic_lane);
    let definition_id = definition.id;
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition]).unwrap();
    let instance = runtime
        .start(start_request(
            definition_id,
            controller(101, 1, false),
            target,
            0,
            false,
        ))
        .unwrap();
    let sources = CountingSources {
        current: 0.25,
        current_calls: Cell::new(0),
    };

    let sample = runtime.sample(instance, 250, 1_000, 10, &sources).unwrap();

    assert_eq!(sources.current_calls.get(), 0);
    assert_eq!(sample.len(), 1);
}

fn selection_phase(offset_degrees: f32) -> PhaseDistribution {
    PhaseDistribution {
        ordering: PhaseOrdering::Selection,
        offset_degrees,
        span_degrees: 360.0,
        block_size: 1,
        repeats: 1,
        wings: false,
        anchors_degrees: vec![],
    }
}

#[test]
fn legacy_phase_spread_defaults_to_uniform_without_lane_configuration() {
    let source = definition(lane());
    let mut stored = serde_json::to_value(source).unwrap();
    stored.as_object_mut().unwrap().remove("phase_mode");
    stored["lanes"][0].as_object_mut().unwrap().remove("phase");

    let restored: DynamicDefinition = serde_json::from_value(stored).unwrap();

    assert_eq!(restored.phase_spread_mode, DynamicPhaseSpreadMode::Uniform);
    assert_eq!(restored.lanes[0].phase, None);
    assert_eq!(restored.phase_for_lane(&restored.lanes[0]), &restored.phase);
}

#[test]
fn per_lane_phase_configuration_round_trips_and_is_validated() {
    let mut configured_lane = lane();
    configured_lane.phase = Some(PhaseDistribution {
        ordering: PhaseOrdering::RandomEachLoop { seed: 42 },
        offset_degrees: 30.0,
        span_degrees: 180.0,
        block_size: 2,
        repeats: 2,
        wings: true,
        anchors_degrees: vec![0.0, 90.0, 0.0],
    });
    let mut configured = definition(configured_lane);
    configured.phase_spread_mode = DynamicPhaseSpreadMode::PerLane;

    let restored: DynamicDefinition =
        serde_json::from_value(serde_json::to_value(&configured).unwrap()).unwrap();
    assert_eq!(restored, configured);
    validate_definition(&restored).unwrap();

    configured.lanes[0].phase.as_mut().unwrap().block_size = 0;
    assert_eq!(
        validate_definition(&configured),
        Err(DynamicValidationError::Phase)
    );
}

#[test]
fn legacy_definition_without_activation_boundary_defaults_to_beat() {
    let source = definition(lane());
    let mut stored = serde_json::to_value(source).unwrap();
    stored
        .as_object_mut()
        .unwrap()
        .remove("activation_boundary");

    let restored: DynamicDefinition = serde_json::from_value(stored).unwrap();

    assert_eq!(restored.activation_boundary, ActivationBoundary::Beat);
}

#[test]
fn legacy_definition_without_run_mode_defaults_to_loop() {
    let source = definition(lane());
    let mut stored = serde_json::to_value(source).unwrap();
    stored.as_object_mut().unwrap().remove("run_mode");

    let restored: DynamicDefinition = serde_json::from_value(stored).unwrap();

    assert_eq!(restored.run_mode, DynamicRunMode::Loop);
}

#[test]
fn one_shot_completes_after_one_effective_cycle_and_does_not_reconcile_twice() {
    let target = FixtureId::new();
    let mut one_shot = definition(lane());
    one_shot.run_mode = DynamicRunMode::OneShot;
    let definition_id = one_shot.id;
    let first_controller = controller(0, 1, false);
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([one_shot.clone()]).unwrap();
    let instance = runtime
        .start(start_request(
            definition_id,
            first_controller.clone(),
            target,
            0,
            false,
        ))
        .unwrap();

    assert!(
        !runtime
            .sample(instance, 999, 1_000, 10, &Sources { current: 0.0 })
            .unwrap()
            .is_empty()
    );
    assert!(
        runtime
            .sample(instance, 1_000, 1_000, 10, &Sources { current: 0.0 })
            .unwrap()
            .is_empty()
    );
    let completed = runtime.snapshot();
    assert!(completed.instances[0].completed);
    assert!(
        runtime
            .sample(instance, 2_000, 1_000, 10, &Sources { current: 0.0 })
            .unwrap()
            .is_empty(),
        "the still-authored On value must remain completed instead of looping"
    );
    let mut restored = DynamicRuntime::default();
    restored.install_definitions([one_shot]).unwrap();
    restored.restore_snapshot(completed).unwrap();
    assert!(
        restored
            .sample(instance, 2_000, 1_000, 10, &Sources { current: 0.0 })
            .unwrap()
            .is_empty(),
        "completion survives runtime persistence and restart"
    );

    let mut retrigger = controller(0, 1, false);
    retrigger.activated_at_millis = 2_000;
    let retrigger_id = retrigger.id;
    assert_eq!(
        runtime
            .start(start_request(definition_id, retrigger, target, 2_000, true,))
            .unwrap(),
        instance,
        "a later deliberate activation reuses the terminal instance"
    );
    let retriggered = runtime.snapshot();
    assert!(!retriggered.instances[0].completed);
    assert_eq!(retriggered.instances[0].controllers.len(), 1);
    assert_eq!(retriggered.instances[0].controllers[0].id, retrigger_id);
    assert!(
        !runtime
            .sample(instance, 2_999, 1_000, 10, &Sources { current: 0.0 })
            .unwrap()
            .is_empty(),
        "the deliberate activation runs the one-shot for one fresh cycle"
    );
}

#[test]
fn aliasing_warning_reports_segments_with_fewer_than_four_output_samples() {
    let definition = definition(lane());

    let warning = aliasing_warning(&definition, 100, 20).unwrap();

    assert_eq!(warning.shortest_segment_millis, 50);
    assert_eq!(warning.output_interval_millis, 20);
    assert_eq!(warning.samples_per_segment, 2);
    assert_eq!(aliasing_warning(&definition, 100, 10), None);
}

#[test]
fn definition_structure_keeps_every_lane_scalar_and_rejects_indexed_attributes() {
    let valid = definition(lane());
    validate_definition(&valid).unwrap();

    let mut invalid = valid;
    invalid.lanes[0].attribute = AttributeKey("shutter".into());
    assert_eq!(
        validate_definition(&invalid),
        Err(DynamicValidationError::UnsupportedAttribute(
            "shutter".into()
        ))
    );
}

#[test]
fn definition_overall_speed_defaults_to_one_and_advances_runtime_phase() {
    let mut original = definition(lane());
    let mut legacy = serde_json::to_value(&original).unwrap();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("overall_speed_multiplier");
    let restored: DynamicDefinition = serde_json::from_value(legacy).unwrap();
    assert_eq!(restored.overall_speed_multiplier, Rational::ONE);

    original.overall_speed_multiplier = Rational {
        numerator: 2,
        denominator: 1,
    };
    let definition_id = original.id;
    let target = FixtureId::new();
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([original]).unwrap();
    let instance = runtime
        .start(start_request(
            definition_id,
            controller(99, 1, false),
            target,
            0,
            false,
        ))
        .unwrap();
    let value = runtime
        .sample(instance, 250, 1_000, 10, &Sources { current: 0.0 })
        .unwrap()[0]
        .value;
    assert!(
        (value - 1.0).abs() < 0.0001,
        "the definition-level x2 multiplier must sample the 500ms phase at 250ms"
    );
}

#[test]
fn keyframes_close_to_the_first_value_and_current_remains_live() {
    let mut lane = lane();
    lane.mode = DynamicLaneMode::MiddleAmplitude;
    let definition = definition(lane.clone());
    let evaluator = DynamicEvaluator::new(&definition);
    let target = FixtureId::new();
    let sample = |elapsed, current| {
        evaluator
            .sample_lane(
                &lane,
                DynamicEvaluationContext {
                    instance_id: Uuid::nil(),
                    target,
                    elapsed_millis: elapsed,
                    cycle_duration_millis: 1_000,
                    phase_degrees: 0.0,
                    output_interval_millis: 10,
                    random_envelope: None,
                    sources: &Sources { current },
                },
            )
            .unwrap()
    };
    assert!((sample(0, 0.5) - 0.5).abs() < 0.0001);
    assert!((sample(250, 0.7) - 1.0).abs() < 0.0001);
}

#[test]
fn phase_span_is_endpoint_exclusive_and_balanced_repeats_restart_the_wave() {
    let targets = (0..5).map(|_| FixtureId::new()).collect::<Vec<_>>();
    let mut distribution = definition(lane()).phase;
    distribution.repeats = 1;
    let phases = project_phase(&distribution, &targets[..4], &HashMap::new(), 0);
    assert_eq!(
        phases.iter().map(|phase| phase.degrees).collect::<Vec<_>>(),
        [0.0, 90.0, 180.0, 270.0]
    );

    distribution.repeats = 2;
    let phases = project_phase(&distribution, &targets, &HashMap::new(), 0);
    assert_eq!(
        phases.iter().map(|phase| phase.degrees).collect::<Vec<_>>(),
        [0.0, 120.0, 240.0, 0.0, 180.0]
    );
}

#[test]
fn uniform_and_per_lane_phase_modes_sample_the_expected_lane_phase() {
    let first = lane();
    let first_id = first.id;
    let mut second = lane();
    let second_id = second.id;
    second.phase = Some(selection_phase(180.0));
    let mut dynamic = definition(first);
    dynamic.lanes.push(second);
    let definition_id = dynamic.id;
    let target = FixtureId::new();

    let sample = |dynamic: DynamicDefinition| {
        let mut runtime = DynamicRuntime::default();
        runtime.install_definitions([dynamic]).unwrap();
        let instance = runtime
            .start(start_request(
                definition_id,
                controller(0, 1, false),
                target,
                0,
                false,
            ))
            .unwrap();
        runtime
            .sample(instance, 0, 1_000, 10, &Sources { current: 0.0 })
            .unwrap()
            .into_iter()
            .map(|sample| (sample.lane_id, sample.value))
            .collect::<HashMap<_, _>>()
    };

    let uniform = sample(dynamic.clone());
    assert_eq!(uniform[&first_id], 0.0);
    assert_eq!(
        uniform[&second_id], 0.0,
        "uniform mode ignores lane overrides"
    );

    dynamic.phase_spread_mode = DynamicPhaseSpreadMode::PerLane;
    let per_lane = sample(dynamic);
    assert_eq!(per_lane[&first_id], 0.0);
    assert_eq!(
        per_lane[&second_id], 1.0,
        "per-lane mode applies the lane's 180 degree offset"
    );
}

#[test]
fn per_lane_phase_snapshot_captures_stage_order_and_legacy_snapshots_expand_uniform_phase() {
    let first = lane();
    let first_id = first.id;
    let mut spatial = lane();
    let spatial_id = spatial.id;
    let mut spatial_phase = selection_phase(0.0);
    spatial_phase.ordering = PhaseOrdering::GridLinear { angle_degrees: 0.0 };
    spatial.phase = Some(spatial_phase);
    let mut dynamic = definition(first);
    dynamic.phase_spread_mode = DynamicPhaseSpreadMode::PerLane;
    dynamic.lanes.push(spatial);
    let definition_id = dynamic.id;
    let targets = [FixtureId::new(), FixtureId::new()];
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([dynamic]).unwrap();
    let instance = runtime
        .start(DynamicStartRequest {
            definition_id,
            controller: controller(0, 1, false),
            target_scope: DynamicTargetScope {
                ordered_targets: targets.to_vec(),
            },
            stage_positions: HashMap::from([
                (targets[0], SpatialPosition { x: 2.0, z: 0.0 }),
                (targets[1], SpatialPosition { x: 0.0, z: 0.0 }),
            ]),
            now_millis: 0,
            activation_policy_override: None,
            activation_delay_millis: 0,
            activation_duration_millis: 0,
            reuse_matching_targetless: false,
        })
        .unwrap();
    let snapshot = runtime.snapshot();
    let phases = snapshot.instances[0]
        .phase_by_lane_target
        .iter()
        .map(|(lane_id, target, phase)| ((*lane_id, *target), *phase))
        .collect::<HashMap<_, _>>();
    assert_eq!(phases[&(first_id, targets[0])], 0.0);
    assert_eq!(phases[&(first_id, targets[1])], 180.0);
    assert_eq!(phases[&(spatial_id, targets[0])], 180.0);
    assert_eq!(phases[&(spatial_id, targets[1])], 0.0);

    let mut legacy_dynamic = definition(lane());
    legacy_dynamic.lanes.push(lane());
    let legacy_definition_id = legacy_dynamic.id;
    let mut legacy_runtime = DynamicRuntime::default();
    legacy_runtime
        .install_definitions([legacy_dynamic.clone()])
        .unwrap();
    let legacy_instance = legacy_runtime
        .start(start_request(
            legacy_definition_id,
            controller(1, 1, false),
            targets[0],
            0,
            false,
        ))
        .unwrap();
    let expected = legacy_runtime
        .sample(legacy_instance, 250, 1_000, 10, &Sources { current: 0.0 })
        .unwrap();
    let mut legacy_snapshot = legacy_runtime.snapshot();
    legacy_snapshot.instances[0].phase_by_lane_target.clear();
    let mut restored = DynamicRuntime::default();
    restored.install_definitions([legacy_dynamic]).unwrap();
    restored.restore_snapshot(legacy_snapshot).unwrap();
    assert_eq!(
        restored
            .sample(legacy_instance, 250, 1_000, 10, &Sources { current: 0.0 },)
            .unwrap(),
        expected,
        "old phase_by_target snapshots expand across every lane"
    );

    // Keep the original instance live so the captured phase assertion exercises a valid snapshot.
    assert_eq!(runtime.snapshot().instances[0].id, instance);
}

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
    positions.insert(targets[0], SpatialPosition { x: 0.0, z: 0.0 });
    positions.insert(targets[1], SpatialPosition { x: 0.0, z: 2.0 });
    positions.insert(targets[2], SpatialPosition { x: 1.0, z: 0.0 });
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
        (targets[0], SpatialPosition { x: 1.0, z: 0.0 }),
        (targets[1], SpatialPosition { x: 3.0, z: 0.0 }),
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

fn controller(id: u128, priority: i16, paused: bool) -> DynamicController {
    DynamicController {
        id: Uuid::from_u128(id),
        source: DynamicControllerSource::Programmer {
            programmer_id: Uuid::from_u128(id + 100),
        },
        priority,
        activated_at_millis: id as u64,
        size: 1.0,
        speed_multiplier: 1.0,
        phase_offset_degrees: 0.0,
        paused,
    }
}

fn start_request(
    definition_id: Uuid,
    controller: DynamicController,
    target: FixtureId,
    now_millis: u64,
    reuse_matching_targetless: bool,
) -> DynamicStartRequest {
    DynamicStartRequest {
        definition_id,
        controller,
        target_scope: DynamicTargetScope {
            ordered_targets: vec![target],
        },
        stage_positions: HashMap::new(),
        now_millis,
        activation_delay_millis: 0,
        activation_duration_millis: 0,
        activation_policy_override: None,
        reuse_matching_targetless,
    }
}
