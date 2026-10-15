use super::*;
use serde_json::{Value, json};

fn ordering(value: Value) -> PhaseOrdering {
    serde_json::from_value(value).expect("operator phase ordering must decode")
}

fn spatial(shape: Value) -> PhaseOrdering {
    ordering(json!({"type": "spatial", "shape": shape}))
}

fn setup(
    orderings: Vec<PhaseOrdering>,
) -> (
    DynamicDefinition,
    Vec<FixtureId>,
    HashMap<FixtureId, SpatialPosition>,
    SpatialSelectionMapping,
) {
    let mut dynamic = definition(lane());
    dynamic.phase_spread_mode = DynamicPhaseSpreadMode::PerLane;
    dynamic.phase.span_degrees = 240.0;
    dynamic.lanes = orderings
        .into_iter()
        .map(|ordering| {
            let mut lane = lane();
            lane.phase = Some(PhaseDistribution {
                ordering,
                ..dynamic.phase.clone()
            });
            lane
        })
        .collect();
    let targets = (1..=3)
        .map(|id| FixtureId(Uuid::from_u128(id)))
        .collect::<Vec<_>>();
    let positions = targets
        .iter()
        .copied()
        .zip([
            SpatialPosition {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            SpatialPosition {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            SpatialPosition {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
        ])
        .collect();
    let projection = SpatialProjection::from_preset(ProjectionPreset::Top, Position3d::default());
    dynamic.spatial_mapping = DynamicSpatialMappingOverride {
        projection: OverrideStage::Replace(projection.clone()),
        shape: OverrideStage::Replace(DynamicSelectionShape::Grid {
            angle_degrees: 0.0,
            direction: RankDirection::Descending,
        }),
    };
    let inherited = SpatialSelectionMapping {
        projection,
        shape: SpatialSelectionShape::Grid {
            angle_degrees: 0.0,
            direction: RankDirection::Ascending,
        },
    };
    (dynamic, targets, positions, inherited)
}

fn start(
    dynamic: DynamicDefinition,
    targets: Vec<FixtureId>,
    positions: HashMap<FixtureId, SpatialPosition>,
    inherited: SpatialSelectionMapping,
) -> (DynamicRuntime, Uuid) {
    let id = dynamic.id;
    let mut runtime = DynamicRuntime::with_programming_contract_support(
        light_core::programming::DYNAMIC_PHASE_ORDERING_CONTRACT,
    );
    runtime.install_definitions([dynamic]).unwrap();
    let instance = runtime
        .start(DynamicStartRequest {
            definition_id: id,
            controller: controller(0, 1, false),
            target_scope: DynamicTargetScope {
                ordered_targets: targets,
            },
            stage_positions: positions,
            inherited_spatial_mapping: Some(inherited),
            now_millis: 0,
            activation_policy_override: None,
            activation_delay_millis: 0,
            activation_duration_millis: 0,
            reuse_matching_targetless: false,
        })
        .unwrap();
    (runtime, instance)
}

#[test]
fn phase_authority_distinguishes_linear_group_and_lane_spatial_shapes() {
    let choices = vec![
        ordering(json!({"type": "source_order"})),
        spatial(json!({"type": "inherit"})),
        spatial(
            json!({"type": "replace", "value": {"type": "grid", "angle_degrees": 90.0, "direction": "ascending"}}),
        ),
        PhaseOrdering::Selection,
    ];
    let (dynamic, targets, positions, inherited) = setup(choices);
    let lane_ids = dynamic.lanes.iter().map(|lane| lane.id).collect::<Vec<_>>();
    let original = serde_json::to_value(&dynamic).unwrap();
    let restored: DynamicDefinition = serde_json::from_value(original.clone()).unwrap();
    assert_eq!(serde_json::to_value(&restored).unwrap(), original);
    let (runtime, _) = start(restored, targets.clone(), positions, inherited);
    let snapshot = runtime.snapshot();
    let phases = |lane| {
        targets
            .iter()
            .map(|target| {
                snapshot.instances[0]
                    .phase_by_lane_target
                    .iter()
                    .find(|(id, fixture, _)| *id == lane && fixture == target)
                    .unwrap()
                    .2
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        phases(lane_ids[0]),
        vec![0.0, 80.0, 160.0],
        "Linear is source order"
    );
    assert_eq!(
        phases(lane_ids[1]),
        vec![0.0, 120.0, 0.0],
        "Inherit ignores legacy global descending shape"
    );
    assert_eq!(
        phases(lane_ids[2]),
        vec![0.0, 0.0, 120.0],
        "lane Grid uses Top XY projection, not legacy XZ"
    );
    assert_eq!(
        phases(lane_ids[3]),
        vec![120.0, 0.0, 120.0],
        "legacy Selection still uses stored global shape"
    );
}

#[test]
fn phase_authority_linear_does_not_need_physical_positions() {
    let (dynamic, targets, _, inherited) = setup(vec![ordering(json!({"type": "source_order"}))]);
    let (runtime, _) = start(dynamic, targets, HashMap::new(), inherited);
    assert_eq!(
        runtime.snapshot().instances[0].phase_by_lane_target.len(),
        3
    );
}

#[test]
fn phase_authority_rejects_nonfinite_spatial_phase_parameters() {
    let (mut dynamic, _, _, _) = setup(vec![spatial(json!({"type": "inherit"}))]);
    dynamic.lanes[0].phase.as_mut().unwrap().ordering = PhaseOrdering::Spatial {
        shape: OverrideStage::Replace(DynamicSelectionShape::Radial {
            center_u: f64::NAN,
            center_v: 0.0,
            direction: RadialDirection::Outward,
        }),
    };
    assert_eq!(
        validate_definition(&dynamic),
        Err(DynamicValidationError::Phase)
    );
}

#[test]
fn phase_authority_random_obeys_each_lane_loop_clock() {
    let random = || spatial(json!({"type":"replace","value":{"type":"random","seed":91}}));
    let (mut dynamic, targets, positions, inherited) = setup(vec![
        random(),
        random(),
        ordering(json!({"type":"source_order"})),
    ]);
    dynamic.lanes[1].speed_multiplier = Rational {
        numerator: 2,
        denominator: 1,
    };
    let lanes = dynamic.lanes.clone();
    let phase = dynamic.phase.clone();
    let (mut runtime, instance) = start(dynamic, targets.clone(), positions, inherited);
    for now in [0, 499, 500, 999, 1000, 1499, 1500] {
        let samples = runtime
            .sample(instance, now, 1000, 10, &Sources { current: 0.0 })
            .unwrap();
        for lane in &lanes {
            let speed = lane.speed_multiplier.factor();
            let elapsed_cycles = now as f64 * speed / 1000.0;
            let distribution = lane.phase.as_ref().unwrap_or(&phase);
            let spatial_targets = targets
                .iter()
                .map(|id| SpatialTarget {
                    fixture_id: *id,
                    position: None,
                })
                .collect::<Vec<_>>();
            let ranked = evaluate_phase_ordering(
                &distribution.ordering,
                None,
                &DynamicSpatialMappingOverride::default(),
                &spatial_targets,
                elapsed_cycles.floor() as u64,
            )
            .unwrap();
            for projected in project_ranked_phase(distribution, &ranked) {
                let fraction =
                    (elapsed_cycles + f64::from(projected.degrees) / 360.0).fract() as f32;
                let expected = if fraction <= 0.5 {
                    fraction * 2.0
                } else {
                    (1.0 - fraction) * 2.0
                };
                let sample = samples
                    .iter()
                    .find(|sample| sample.lane_id == lane.id && sample.target == projected.target)
                    .unwrap();
                assert!(
                    (sample.legacy().unwrap().value - expected).abs() < 0.0001,
                    "lane {} at {now}: {} != {expected}",
                    lane.id,
                    sample.legacy().unwrap().value
                );
            }
        }
    }
}

#[test]
fn phase_authority_uniform_random_uses_one_global_loop_clock() {
    let (mut dynamic, targets, positions, inherited) =
        setup(vec![PhaseOrdering::Selection, PhaseOrdering::Selection]);
    dynamic.phase_spread_mode = DynamicPhaseSpreadMode::Uniform;
    dynamic.phase.ordering = spatial(json!({"type":"replace","value":{"type":"random","seed":91}}));
    dynamic.lanes[1].speed_multiplier = Rational {
        numerator: 2,
        denominator: 1,
    };
    let ids = dynamic.lanes.iter().map(|lane| lane.id).collect::<Vec<_>>();
    let (mut runtime, instance) = start(dynamic, targets, positions, inherited);
    let mut permutations = Vec::new();
    for loop_index in 0..=8 {
        let samples = runtime
            .sample(
                instance,
                loop_index * 1000,
                1000,
                10,
                &Sources { current: 0.0 },
            )
            .unwrap();
        let values = |lane_id| {
            samples
                .iter()
                .filter(|sample| sample.lane_id == lane_id)
                .map(|sample| (sample.target, sample.legacy().unwrap().value))
                .collect::<HashMap<_, _>>()
        };
        assert_eq!(
            values(ids[0]),
            values(ids[1]),
            "uniform phase uses global loop {loop_index}"
        );
        permutations.push(values(ids[0]));
    }
    assert!(
        permutations
            .iter()
            .skip(1)
            .any(|values| values != &permutations[0])
    );
}

#[test]
fn phase_authority_reader_four_refuses_new_modes_without_changing_legacy_support() {
    let (dynamic, _, _, _) = setup(vec![ordering(json!({"type": "source_order"}))]);
    let mut old_reader = DynamicRuntime::with_programming_contract_support(4);
    let error = old_reader.install_definitions([dynamic]).unwrap_err();
    assert!(error.to_string().contains("supports 4"), "{error}");
    assert_eq!(old_reader.instance_count(), 0);

    let legacy = definition(lane());
    assert!(matches!(legacy.phase.ordering, PhaseOrdering::Selection));
    DynamicRuntime::default()
        .install_definitions([legacy])
        .unwrap();
}
