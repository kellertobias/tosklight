//! Frozen old owning-root snapshot algorithm is the differential oracle.
use super::*;
use crate::*;
use light_core::{
    AttributeValue,
    programming::{PositionIntent, ProgrammingComponent, TargetReference},
};

fn old_snapshot(
    last: &SampleValueMap,
    held: &SampleValueMap,
) -> (
    Option<Arc<crate::RetainedExpressionTape>>,
    Vec<DynamicHeldSampleSnapshot>,
    Vec<DynamicHeldSampleSnapshot>,
) {
    let sorted = |values: &SampleValueMap| {
        let mut values = values
            .iter()
            .map(|(key, value)| (*key, Arc::new(value.clone())))
            .collect::<Vec<_>>();
        values.sort_by_key(|((controller, target, lane), _)| (*controller, target.0, *lane));
        values
    };
    let mut values = sorted(last);
    let last_len = values.len();
    values.extend(sorted(held));
    if values.is_empty() {
        return (None, Vec::new(), Vec::new());
    }
    let roots = values
        .iter()
        .map(|(_, expression)| expression.clone())
        .collect::<Vec<_>>();
    let tape = Arc::new(
        crate::RetainedExpressionTape::from_roots(&roots)
            .expect("validated runtime samples form an acyclic retained graph"),
    );
    let mut rows = values
        .into_iter()
        .zip(&tape.roots)
        .map(
            |(((controller_id, target, lane_id), _), root)| DynamicHeldSampleSnapshot {
                controller_id,
                target,
                lane_id,
                payload: DynamicHeldPayload::TapeRoot { tape_root: *root },
            },
        )
        .collect::<Vec<_>>();
    let held = rows.split_off(last_len);
    (Some(tape), rows, held)
}

fn key(n: u128) -> (Uuid, FixtureId, Uuid) {
    (
        Uuid::from_u128(n),
        FixtureId(Uuid::from_u128(n + 100)),
        Uuid::from_u128(n + 200),
    )
}
fn leaf(value: f32) -> DynamicSampleExpression {
    DynamicSampleExpression::LegacyScalar {
        attribute: AttributeKey::intensity(),
        value,
        occurrence: None,
        dependency_occurrence: None,
    }
}
fn compare(last: &SampleValueMap, held: &SampleValueMap) {
    let old = old_snapshot(last, held);
    let new = sample_values_snapshot(last, held);
    assert_eq!(new, old);
    assert_eq!(
        serde_json::to_value(&new).unwrap(),
        serde_json::to_value(&old).unwrap()
    );
    if let (Some(a), Some(b)) = (&new.0, &old.0) {
        for (a, b) in a.operation_emissions().iter().zip(b.operation_emissions()) {
            assert!(Arc::ptr_eq(a, b));
        }
    }
}
#[test]
fn borrowed_snapshot_preserves_sorted_distinct_rows_and_last_then_held_roots() {
    compare(&Default::default(), &Default::default());
    for count in [1, 8, 9, 64] {
        let last = (1..=count)
            .rev()
            .map(|n| (key(n), leaf(0.5)))
            .collect::<SampleValueMap>();
        let held = (1..=count)
            .map(|n| (key(n), leaf(0.5)))
            .collect::<SampleValueMap>();
        compare(&last, &held);
        compare(&last, &last);
        let (tape, last, held) = sample_values_snapshot(&last, &held);
        assert_eq!(tape.unwrap().nodes.len(), count as usize * 2);
        assert!(
            last.windows(2)
                .all(|r| r[0].controller_id < r[1].controller_id)
        );
        assert_eq!(last.len(), held.len());
    }
}
#[test]
fn borrowed_snapshot_keeps_shared_arc_tape_and_current_history() {
    let current = Arc::new(DynamicSampleExpression::AngleCurrent {
        address: Arc::new(DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Angles,
            component: Some(ProgrammingComponent::Pan),
        }),
    });
    let branch = DynamicSampleExpression::Transition {
        from: Some(current.clone()),
        to: Some(current),
        progress: 0.4,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(77),
        },
    };
    let tape = Arc::new(RetainedExpressionTape::from_roots(&[Arc::new(branch.clone())]).unwrap());
    let retained = DynamicSampleExpression::Retained {
        root: tape.roots[0],
        tape,
    };
    let last = [(key(3), branch), (key(1), retained.clone())]
        .into_iter()
        .collect();
    let held = [(key(1), retained)].into_iter().collect();
    compare(&last, &held);
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

struct NoSources;
impl ScalarSourceResolver for NoSources {
    fn current(&self, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        None
    }
    fn preset(&self, _: &str, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        None
    }
}
impl DynamicValueSourceResolver for NoSources {
    fn current(&self, _: FixtureId, _: &DynamicValueAddress) -> Option<DynamicValue> {
        None
    }
    fn preset(
        &self,
        _: &DynamicPresetSourceBinding,
        _: Uuid,
        _: FixtureId,
    ) -> Option<DynamicValue> {
        None
    }
}
#[test]
fn borrowed_snapshot_preserves_real_sampler_operation_witnesses_and_restored_history() {
    let family = |reference| DynamicValueSource::Value {
        value: DynamicValue::Family(AttributeValue::Position(Arc::new(PositionIntent::target(
            reference,
            [1., 2., 3.],
        )))),
    };
    let lane = DynamicLane {
        id: Uuid::from_u128(401),
        body: DynamicLaneBody::Programming(ProgrammingLaneBody {
            address: DynamicValueAddress {
                representation: DynamicFamilyRepresentation::Target { reference: None },
                component: None,
            },
            configuration: ProgrammingLaneConfiguration::Keyframes(KeyframeConfiguration {
                points: vec![
                    DynamicKeyframe {
                        position: 0.,
                        source: family(TargetReference::Origin),
                        interpolation: ScalarInterpolation::Linear,
                    },
                    DynamicKeyframe {
                        position: 0.5,
                        source: family(TargetReference::Point {
                            point_id: Uuid::from_u128(402),
                        }),
                        interpolation: ScalarInterpolation::Linear,
                    },
                ],
                size: 1.,
            }),
        }),
        speed_multiplier: Rational::ONE,
        width: 1.,
        phase: None,
        random_group_id: None,
    };
    let definition = definition(lane);
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    let controller = DynamicController {
        id: Uuid::from_u128(403),
        source: DynamicControllerSource::Programmer {
            programmer_id: Uuid::from_u128(404),
            instance_link: None,
        },
        priority: 12,
        activated_at_millis: 0,
        size: 1.,
        speed_multiplier: 1.,
        phase_offset_degrees: 0.,
        paused: false,
    };
    let instance = runtime
        .start(DynamicStartRequest {
            definition_id: definition.id,
            controller: controller.clone(),
            target_scope: DynamicTargetScope {
                ordered_targets: vec![
                    FixtureId(Uuid::from_u128(405)),
                    FixtureId(Uuid::from_u128(406)),
                ],
            },
            stage_positions: Default::default(),
            inherited_spatial_mapping: None,
            now_millis: 0,
            activation_delay_millis: 0,
            activation_duration_millis: 0,
            activation_policy_override: Some(ActivationPolicy::JoinSyncNow),
            reuse_matching_targetless: false,
        })
        .unwrap();
    runtime
        .sample_programming(instance, 125, 1000, 25, &NoSources, &NoSources)
        .unwrap();
    runtime
        .set_controller_paused(instance, controller.id, true, 125)
        .unwrap();
    let stored = &runtime.instances[&instance];
    compare(&stored.last_sample_values, &stored.synchronized_hold_values);
    let old = old_snapshot(&stored.last_sample_values, &stored.synchronized_hold_values);
    assert!(
        !old.0.as_ref().unwrap().operation_emissions().is_empty(),
        "real producer origins must be admitted"
    );
    let mut old_runtime = runtime.snapshot();
    let new_runtime = old_runtime.clone();
    let row = &mut old_runtime.instances[0];
    row.expression_tape = old.0;
    row.last_sample_values = old.1;
    row.synchronized_hold_values = old.2;
    assert_eq!(old_runtime, new_runtime);
    let mut old = DynamicRuntime::default();
    let mut new = DynamicRuntime::default();
    old.restore_snapshot(old_runtime).unwrap();
    new.restore_snapshot(new_runtime).unwrap();
    assert_eq!(old.snapshot(), new.snapshot());
    assert_eq!(
        old.sample_programming(instance, 250, 1000, 25, &NoSources, &NoSources)
            .unwrap(),
        new.sample_programming(instance, 250, 1000, 25, &NoSources, &NoSources)
            .unwrap()
    );
}
