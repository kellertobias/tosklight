use super::*;
use light_core::{AttributeKey, FixtureId};
use std::{cell::Cell, collections::HashMap, sync::Arc};
use uuid::Uuid;

mod runtime_control;
mod runtime_spatial;
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
            invert_waveform: false,
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
fn legacy_spatial_position_without_y_defaults_to_stage_zero() {
    let restored: SpatialPosition =
        serde_json::from_value(serde_json::json!({"x": 1.25, "z": -3.5})).unwrap();

    assert_eq!(
        restored,
        SpatialPosition {
            x: 1.25,
            y: 0.0,
            z: -3.5,
        }
    );
    assert_eq!(
        serde_json::to_value(restored).unwrap(),
        serde_json::json!({"x": 1.25, "y": 0.0, "z": -3.5})
    );
}

#[test]
fn spatial_mapping_defaults_to_inherit_and_canonical_serialization_omits_it() {
    let source = definition(lane());
    let mut absent = serde_json::to_value(&source).unwrap();
    assert!(absent.get("spatial_mapping").is_none());

    let restored: DynamicDefinition = serde_json::from_value(absent.clone()).unwrap();
    assert_eq!(
        restored.spatial_mapping,
        DynamicSpatialMappingOverride::default()
    );

    absent["spatial_mapping"] = serde_json::json!({
        "projection": {"type": "inherit"},
        "shape": {"type": "inherit"}
    });
    let explicit_inherit: DynamicDefinition = serde_json::from_value(absent).unwrap();
    assert_eq!(explicit_inherit.spatial_mapping, restored.spatial_mapping);
    assert!(
        serde_json::to_value(explicit_inherit)
            .unwrap()
            .get("spatial_mapping")
            .is_none()
    );
}

#[test]
fn explicit_projection_and_random_shape_replacements_round_trip() {
    let mut configured = definition(lane());
    configured.spatial_mapping = DynamicSpatialMappingOverride {
        projection: OverrideStage::Replace(SpatialProjection::from_preset(
            ProjectionPreset::Front,
            Position3d {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            },
        )),
        shape: OverrideStage::Replace(DynamicSelectionShape::Random { seed: 0x5eed }),
    };

    let stored = serde_json::to_value(&configured).unwrap();
    assert_eq!(stored["spatial_mapping"]["projection"]["type"], "replace");
    assert_eq!(stored["spatial_mapping"]["shape"]["type"], "replace");
    assert_eq!(
        stored["spatial_mapping"]["shape"]["value"],
        serde_json::json!({"type": "random", "seed": 0x5eed})
    );

    let restored: DynamicDefinition = serde_json::from_value(stored).unwrap();
    assert_eq!(restored, configured);
}

#[test]
fn legacy_uniform_phase_orderings_infer_definition_local_spatial_mappings() {
    let cases = [
        PhaseOrdering::GridLinear {
            angle_degrees: 37.5,
        },
        PhaseOrdering::RadialOut {
            center_x: 0.25,
            center_z: 0.75,
        },
        PhaseOrdering::RadialIn {
            center_x: -1.25,
            center_z: 3.5,
        },
        PhaseOrdering::Axial {
            center_x: 2.0,
            center_z: -4.0,
        },
        PhaseOrdering::RandomEachLoop { seed: 42 },
    ];

    for ordering in cases {
        let mut legacy = definition(lane());
        legacy.phase.ordering = ordering.clone();
        let stored = serde_json::to_value(&legacy).unwrap();
        assert!(stored.get("spatial_mapping").is_none());

        let restored: DynamicDefinition = serde_json::from_value(stored).unwrap();
        assert_eq!(restored.phase.ordering, ordering);
        assert!(
            !restored.spatial_mapping.is_inherit(),
            "legacy ordering must infer a local mapping"
        );
        assert!(
            serde_json::to_value(restored).unwrap()["spatial_mapping"].is_object(),
            "the first typed write must persist the inferred mapping"
        );
    }
}

#[test]
fn explicit_spatial_mapping_is_never_replaced_by_legacy_ordering_inference() {
    let mut legacy = definition(lane());
    legacy.phase.ordering = PhaseOrdering::GridLinear {
        angle_degrees: 37.5,
    };
    let mut stored = serde_json::to_value(legacy).unwrap();
    stored["spatial_mapping"] = serde_json::json!({
        "projection": {"type": "inherit"},
        "shape": {"type": "replace", "value": {"type": "random", "seed": 99}}
    });

    let restored: DynamicDefinition = serde_json::from_value(stored).unwrap();
    assert_eq!(
        restored.spatial_mapping,
        DynamicSpatialMappingOverride {
            projection: OverrideStage::Inherit,
            shape: OverrideStage::Replace(DynamicSelectionShape::Random { seed: 99 }),
        }
    );
}

#[test]
fn legacy_lane_consistent_phase_ordering_infers_mapping_without_rewriting_ordering() {
    let mut configured_lane = lane();
    configured_lane.phase = Some(PhaseDistribution {
        ordering: PhaseOrdering::Axial {
            center_x: 0.25,
            center_z: 0.75,
        },
        offset_degrees: 15.0,
        span_degrees: 270.0,
        block_size: 2,
        repeats: 3,
        wings: true,
        anchors_degrees: vec![0.0, 120.0, 240.0],
    });
    let mut legacy = definition(configured_lane);
    legacy.phase_spread_mode = DynamicPhaseSpreadMode::PerLane;
    let stored = serde_json::to_value(legacy).unwrap();
    assert!(stored.get("spatial_mapping").is_none());

    let restored: DynamicDefinition = serde_json::from_value(stored.clone()).unwrap();
    assert_eq!(
        restored.lanes[0].phase.as_ref().unwrap().ordering,
        PhaseOrdering::Axial {
            center_x: 0.25,
            center_z: 0.75,
        }
    );
    assert_eq!(restored.phase_spread_mode, DynamicPhaseSpreadMode::PerLane);
    assert!(matches!(
        restored.spatial_mapping.shape,
        OverrideStage::Replace(DynamicSelectionShape::Radar { .. })
    ));
    let migrated = serde_json::to_value(restored).unwrap();
    assert_eq!(migrated["lanes"], stored["lanes"]);
    assert!(migrated["spatial_mapping"].is_object());
}

#[test]
fn legacy_different_per_lane_orderings_remain_on_the_compatibility_runtime_path() {
    let mut axial = lane();
    axial.phase = Some(PhaseDistribution {
        ordering: PhaseOrdering::Axial {
            center_x: 0.25,
            center_z: 0.75,
        },
        ..definition(lane()).phase
    });
    let mut radial = lane();
    radial.phase = Some(PhaseDistribution {
        ordering: PhaseOrdering::RadialOut {
            center_x: 0.5,
            center_z: 0.5,
        },
        ..definition(lane()).phase
    });
    let mut legacy = definition(axial);
    legacy.lanes.push(radial);
    legacy.phase_spread_mode = DynamicPhaseSpreadMode::PerLane;
    let stored = serde_json::to_value(legacy).unwrap();

    let restored: DynamicDefinition = serde_json::from_value(stored.clone()).unwrap();
    assert_eq!(
        restored.spatial_mapping,
        DynamicSpatialMappingOverride::default()
    );
    assert_eq!(serde_json::to_value(restored).unwrap(), stored);
}

#[test]
fn embedded_fallback_uses_the_same_legacy_phase_migration_boundary() {
    let mut legacy = definition(lane());
    legacy.phase.ordering = PhaseOrdering::RandomEachLoop { seed: 0x5eed };
    let reference = DynamicReference {
        dynamic_id: None,
        last_known_pool_number: legacy.pool_number,
        embedded_fallback: DynamicDefinitionSnapshot {
            definition: Arc::new(legacy),
        },
    };
    let mut stored = serde_json::to_value(reference).unwrap();
    stored["embedded_fallback"]["definition"]
        .as_object_mut()
        .unwrap()
        .remove("spatial_mapping");

    let restored: DynamicReference = serde_json::from_value(stored).unwrap();
    assert_eq!(
        restored.embedded_fallback.definition.phase.ordering,
        PhaseOrdering::RandomEachLoop { seed: 0x5eed }
    );
    assert!(matches!(
        restored.embedded_fallback.definition.spatial_mapping.shape,
        OverrideStage::Replace(DynamicSelectionShape::Random { seed: 0x5eed })
    ));
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
    configured.spatial_mapping = DynamicSpatialMappingOverride {
        projection: OverrideStage::Inherit,
        shape: OverrideStage::Replace(DynamicSelectionShape::Random { seed: 42 }),
    };

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
fn ranked_phase_counts_spatial_ranks_and_keeps_equal_ranks_parallel() {
    let targets = (1..=8)
        .map(|value| FixtureId(Uuid::from_u128(value)))
        .collect::<Vec<_>>();
    let spatial_ranks = [0, 0, 1, 2, 2, 3, 4, 5];
    let ranked = RankedSelection {
        ordered_fixture_ids: targets.clone(),
        rank_by_fixture: targets.iter().copied().zip(spatial_ranks).collect(),
        rank_count: 6,
        warnings: Vec::new(),
    };
    let mut distribution = selection_phase(10.0);
    distribution.block_size = 2;

    let phases = project_ranked_phase(&distribution, &ranked);
    assert_eq!(
        phases.iter().map(|phase| phase.degrees).collect::<Vec<_>>(),
        [10.0, 10.0, 10.0, 130.0, 130.0, 130.0, 250.0, 250.0],
        "two spatial ranks per block are counted independently of tied fixture cardinality"
    );

    distribution.block_size = 1;
    distribution.repeats = 2;
    distribution.wings = true;
    distribution.anchors_degrees = vec![20.0, 80.0];
    let complex = project_ranked_phase(&distribution, &ranked);
    assert_eq!(
        complex
            .iter()
            .map(|phase| phase.degrees)
            .collect::<Vec<_>>(),
        [60.0, 60.0, 90.0, 60.0, 60.0, 60.0, 90.0, 60.0],
        "repeat, wing, and anchor lengths count six ranks rather than eight fixtures"
    );
    let degrees_by_rank = complex
        .iter()
        .map(|phase| (ranked.rank_by_fixture[&phase.target], phase.degrees))
        .collect::<HashMap<_, _>>();
    assert_eq!(degrees_by_rank.len(), ranked.rank_count);
    for phase in complex {
        assert_eq!(
            phase.degrees, degrees_by_rank[&ranked.rank_by_fixture[&phase.target]],
            "repeat, wing, and anchor projection stays a function of rank"
        );
    }
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
                (
                    targets[0],
                    SpatialPosition {
                        x: 2.0,
                        y: 0.0,
                        z: 0.0,
                    },
                ),
                (
                    targets[1],
                    SpatialPosition {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                ),
            ]),
            inherited_spatial_mapping: None,
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
fn runtime_selection_phase_uses_inherited_spatial_ranks_once_for_all_parallel_targets() {
    let targets = (1..=3)
        .map(|value| FixtureId(Uuid::from_u128(value)))
        .collect::<Vec<_>>();
    let dynamic = definition(lane());
    let definition_id = dynamic.id;
    let lane_id = dynamic.lanes[0].id;
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([dynamic]).unwrap();

    let instance = runtime
        .start(DynamicStartRequest {
            definition_id,
            controller: controller(0, 1, false),
            target_scope: DynamicTargetScope {
                ordered_targets: targets.clone(),
            },
            stage_positions: HashMap::from([
                (
                    targets[0],
                    SpatialPosition {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                ),
                (
                    targets[1],
                    SpatialPosition {
                        x: 0.0,
                        y: 4.0,
                        z: 0.0,
                    },
                ),
                (
                    targets[2],
                    SpatialPosition {
                        x: 10.0,
                        y: 0.0,
                        z: 0.0,
                    },
                ),
            ]),
            inherited_spatial_mapping: Some(SpatialSelectionMapping {
                projection: SpatialProjection::from_preset(
                    ProjectionPreset::Top,
                    Position3d::default(),
                ),
                shape: SpatialSelectionShape::Grid {
                    angle_degrees: 0.0,
                    direction: RankDirection::Ascending,
                },
            }),
            now_millis: 0,
            activation_delay_millis: 0,
            activation_duration_millis: 0,
            activation_policy_override: None,
            reuse_matching_targetless: false,
        })
        .unwrap();

    let snapshot = runtime.snapshot();
    let phases = snapshot
        .instances
        .iter()
        .find(|candidate| candidate.id == instance)
        .unwrap()
        .phase_by_lane_target
        .iter()
        .filter(|(candidate_lane, _, _)| *candidate_lane == lane_id)
        .map(|(_, target, phase)| (*target, *phase))
        .collect::<HashMap<_, _>>();
    assert_eq!(phases[&targets[0]], 0.0);
    assert_eq!(phases[&targets[1]], 0.0);
    assert_eq!(phases[&targets[2]], 180.0);
}

#[test]
fn runtime_reconciles_live_targets_positions_and_mapping_without_restarting() {
    let targets = (1..=3)
        .map(|value| FixtureId(Uuid::from_u128(value)))
        .collect::<Vec<_>>();
    let dynamic = definition(lane());
    let definition_id = dynamic.id;
    let lane_id = dynamic.lanes[0].id;
    let mapping = SpatialSelectionMapping {
        projection: SpatialProjection::from_preset(ProjectionPreset::Top, Position3d::default()),
        shape: SpatialSelectionShape::Grid {
            angle_degrees: 0.0,
            direction: RankDirection::Ascending,
        },
    };
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([dynamic]).unwrap();
    let controller = controller(42, 1, false);
    let controller_id = controller.id;
    let instance = runtime
        .start(DynamicStartRequest {
            definition_id,
            controller,
            target_scope: DynamicTargetScope {
                ordered_targets: targets[..2].to_vec(),
            },
            stage_positions: HashMap::from([
                (
                    targets[0],
                    SpatialPosition {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                ),
                (
                    targets[1],
                    SpatialPosition {
                        x: 10.0,
                        y: 0.0,
                        z: 0.0,
                    },
                ),
            ]),
            inherited_spatial_mapping: Some(mapping.clone()),
            now_millis: 123,
            activation_delay_millis: 0,
            activation_duration_millis: 0,
            activation_policy_override: None,
            reuse_matching_targetless: false,
        })
        .unwrap();

    let changed = runtime
        .reconcile_instance_targets(
            instance,
            DynamicTargetScope {
                ordered_targets: vec![targets[1], targets[2]],
            },
            &HashMap::from([
                (
                    targets[1],
                    SpatialPosition {
                        x: 10.0,
                        y: 0.0,
                        z: 0.0,
                    },
                ),
                (
                    targets[2],
                    SpatialPosition {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                ),
            ]),
            Some(&mapping),
        )
        .unwrap();
    assert!(changed);

    let snapshot = runtime.snapshot();
    let live = snapshot
        .instances
        .iter()
        .find(|candidate| candidate.id == instance)
        .unwrap();
    assert_eq!(live.started_at_millis, 123);
    assert_eq!(live.targets, vec![targets[1], targets[2]]);
    assert_eq!(live.controllers[0].id, controller_id);
    let phases = live
        .phase_by_lane_target
        .iter()
        .filter(|(candidate_lane, _, _)| *candidate_lane == lane_id)
        .map(|(_, target, phase)| (*target, *phase))
        .collect::<HashMap<_, _>>();
    assert!(!phases.contains_key(&targets[0]));
    assert_eq!(phases[&targets[2]], 0.0);
    assert_eq!(phases[&targets[1]], 180.0);
}

#[test]
fn runtime_reconciliation_rejects_invalid_mapping_atomically() {
    let targets = [FixtureId::new(), FixtureId::new()];
    let mut dynamic = definition(lane());
    dynamic.spatial_mapping.shape = OverrideStage::Replace(DynamicSelectionShape::Grid {
        angle_degrees: 0.0,
        direction: RankDirection::Ascending,
    });
    let definition_id = dynamic.id;
    let inherited = SpatialSelectionMapping {
        projection: SpatialProjection::from_preset(ProjectionPreset::Top, Position3d::default()),
        shape: SpatialSelectionShape::Grid {
            angle_degrees: 0.0,
            direction: RankDirection::Ascending,
        },
    };
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([dynamic]).unwrap();
    let instance = runtime
        .start(DynamicStartRequest {
            definition_id,
            controller: controller(0, 1, false),
            target_scope: DynamicTargetScope {
                ordered_targets: targets.to_vec(),
            },
            stage_positions: HashMap::new(),
            inherited_spatial_mapping: Some(inherited),
            now_millis: 0,
            activation_delay_millis: 0,
            activation_duration_millis: 0,
            activation_policy_override: None,
            reuse_matching_targetless: false,
        })
        .unwrap();
    let before = runtime.snapshot();

    assert!(matches!(
        runtime.reconcile_instance_targets(
            instance,
            DynamicTargetScope {
                ordered_targets: targets.into_iter().rev().collect(),
            },
            &HashMap::new(),
            None,
        ),
        Err(DynamicRuntimeError::InvalidSpatialMapping(_))
    ));
    assert_eq!(runtime.snapshot(), before);
}

#[test]
fn runtime_non_selection_lane_preserves_legacy_projection_without_resolving_new_mapping() {
    let targets = [FixtureId::new(), FixtureId::new()];
    let mut dynamic = definition(lane());
    dynamic.phase.ordering = PhaseOrdering::GridLinear { angle_degrees: 0.0 };
    dynamic.spatial_mapping = DynamicSpatialMappingOverride {
        projection: OverrideStage::Inherit,
        shape: OverrideStage::Replace(DynamicSelectionShape::Grid {
            angle_degrees: 90.0,
            direction: RankDirection::Descending,
        }),
    };
    let definition_id = dynamic.id;
    let lane_id = dynamic.lanes[0].id;
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
                (
                    targets[0],
                    SpatialPosition {
                        x: 2.0,
                        y: 0.0,
                        z: 0.0,
                    },
                ),
                (
                    targets[1],
                    SpatialPosition {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    },
                ),
            ]),
            inherited_spatial_mapping: None,
            now_millis: 0,
            activation_delay_millis: 0,
            activation_duration_millis: 0,
            activation_policy_override: None,
            reuse_matching_targetless: false,
        })
        .unwrap();

    let phases = runtime.snapshot().instances[0]
        .phase_by_lane_target
        .iter()
        .filter(|(candidate_lane, _, _)| *candidate_lane == lane_id)
        .map(|(_, target, phase)| (*target, *phase))
        .collect::<HashMap<_, _>>();
    assert_eq!(phases[&targets[1]], 0.0);
    assert_eq!(phases[&targets[0]], 180.0);
    assert_eq!(runtime.snapshot().instances[0].id, instance);
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
        inherited_spatial_mapping: None,
        now_millis,
        activation_delay_millis: 0,
        activation_duration_millis: 0,
        activation_policy_override: None,
        reuse_matching_targetless,
    }
}
