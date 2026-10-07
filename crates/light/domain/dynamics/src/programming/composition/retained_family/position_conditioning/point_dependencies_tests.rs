use super::*;
use crate::programming::expression_coupled::{CapturedPositionCurrent, PositionAnglePairEndpoint};
use crate::{CompiledCoupledExpression, DynamicRuntimeSample};
use light_core::FixtureId;
use std::cell::Cell;

fn angles() -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(0., 0.)))
}
fn target(id: u128) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Point {
            point_id: Uuid::from_u128(id),
        },
        [1., 2., 3.],
    )))
}
fn rank(lane: u128) -> FamilySampleRank {
    FamilySampleRank {
        priority: 3,
        changed_at_millis: 100,
        changed_at_submillis_nanos: 0,
        stable_order: lane,
        identity: FamilySampleIdentity::Dynamic {
            instance_id: Uuid::from_u128(1),
            controller_id: Uuid::from_u128(2),
            lane_id: Uuid::from_u128(lane),
        },
    }
}
fn leaf(value: AttributeValue) -> Arc<DynamicSampleExpression> {
    Arc::new(DynamicSampleExpression::Programming {
        address: Arc::new(
            DynamicValueAddress::whole_family(ProgrammingOwner::Position, &value).unwrap(),
        ),
        value: DynamicValue::Family(value),
        occurrence: None,
        dependency_occurrence: None,
    })
}
fn resume(
    from: Arc<DynamicSampleExpression>,
    to: Option<Arc<DynamicSampleExpression>>,
    id: u128,
    progress: f32,
) -> Arc<DynamicSampleExpression> {
    Arc::new(DynamicSampleExpression::Transition {
        from: Some(from),
        to,
        progress,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(id),
        },
    })
}
fn compiled(expression: Arc<DynamicSampleExpression>) -> Arc<CompiledProgrammingFamilyExpression> {
    Arc::new(
        CompiledProgrammingFamilyExpression::new(
            expression,
            ProgrammingOwner::Position,
            None,
            None,
        )
        .unwrap(),
    )
}
fn whole(expression: Arc<DynamicSampleExpression>, lane: u128) -> FamilyCompositionSample {
    FamilyCompositionSample::WholeExpression {
        expression: compiled(expression),
        rank: rank(lane),
        activation_mix: 1.,
    }
}
fn ids(census: &PositionPointDependencies) -> Vec<u128> {
    census.point_ids().iter().map(Uuid::as_u128).collect()
}

#[test]
fn nested_retained_hidden_endpoints_and_size_baseline_are_cached_sorted_and_unique() {
    let scaled = Arc::new(DynamicSampleExpression::Scale {
        address: Arc::new(
            DynamicValueAddress::whole_family(ProgrammingOwner::Position, &target(30)).unwrap(),
        ),
        base: DynamicValue::Family(target(30)),
        value: resume(leaf(target(10)), Some(leaf(target(20))), 40, 0.),
        factor: 1.5,
        baseline_occurrence: None,
    });
    let root = resume(scaled, Some(leaf(target(20))), 41, 0.5);
    let tape = Arc::new(RetainedExpressionTape::from_roots(&[root]).unwrap());
    let retained = Arc::new(DynamicSampleExpression::Retained {
        root: tape.roots()[0],
        tape,
    });
    // The later complete Angle source can hide all lower Target work during evaluation.
    let registry = CapturedPositionProgram::new(
        Uuid::new_v4(),
        &target(5),
        &[whole(retained, 10), whole(leaf(angles()), 99)],
    )
    .unwrap();
    assert_eq!(ids(registry.point_dependencies()), vec![5, 10, 20, 30]);
    assert!(!registry.point_dependencies().incomplete());
    let clone = registry.clone();
    assert!(std::ptr::eq(
        registry.point_dependencies(),
        clone.point_dependencies()
    ));
    let mut branch = registry.branch();
    branch
        .choose_resume(
            PositionResumeScope {
                instance_id: Uuid::from_u128(1),
                controller_id: Uuid::from_u128(2),
                occurrence_id: Uuid::from_u128(41),
            },
            PositionResumeEndpoint::Incoming,
        )
        .unwrap();
    assert_eq!(
        ids(registry.point_dependencies()),
        vec![5, 10, 20, 30],
        "conditioning cannot erase original subscriptions"
    );
}

#[test]
fn target_component_addresses_and_equal_capture_foreign_registries_keep_original_membership() {
    let address = Arc::new(
        CompiledDynamicValueAddress::new(
            DynamicValueAddress {
                representation: DynamicFamilyRepresentation::Target {
                    reference: Some(TargetReference::Point {
                        point_id: Uuid::from_u128(77),
                    }),
                },
                component: Some(ProgrammingComponent::TargetX),
            },
            None,
        )
        .unwrap(),
    );
    let source = FamilySample::new(address, DynamicValue::Scalar(4.), rank(10), 1.).unwrap();
    let capture = Uuid::new_v4();
    let registry = CapturedPositionProgram::new(capture, &angles(), &[source.into()]).unwrap();
    assert_eq!(ids(registry.point_dependencies()), vec![77]);
    assert!(!registry.point_dependencies().incomplete());
    // No Point registry is consulted: a missing/recreated Point remains an exact UUID dependency.
    let foreign = CapturedPositionProgram::new(capture, &target(88), &[]).unwrap();
    assert_eq!(ids(foreign.point_dependencies()), vec![88]);
    assert_eq!(ids(registry.point_dependencies()), vec![77]);
}

struct Originals {
    value: AttributeValue,
    reads: Cell<usize>,
}
impl crate::DynamicValueSourceResolver for Originals {
    fn current(&self, _: FixtureId, _: &DynamicValueAddress) -> Option<DynamicValue> {
        panic!("the census must not sample scalar Current")
    }
    fn try_position_current_family(
        &self,
        _: FixtureId,
        _: &DynamicValueAddress,
    ) -> Result<Option<AttributeValue>, TransitionError> {
        self.reads.set(self.reads.get() + 1);
        Ok(Some(self.value.clone()))
    }
    fn preset(
        &self,
        _: &crate::DynamicPresetSourceBinding,
        _: Uuid,
        _: FixtureId,
    ) -> Option<DynamicValue> {
        None
    }
}
#[test]
fn actual_numeric_forest_enumerates_captured_target_current_without_another_read() {
    let address = |component| DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Angles,
        component: Some(component),
    };
    let numeric = Arc::new(AngleNumericProgram {
        address: address(ProgrammingComponent::Pan),
        occurrence: None,
        nodes: vec![
            crate::AngleNumericNode::Current,
            crate::AngleNumericNode::Around {
                middle: 0,
                amplitude: DynamicValue::Scalar(20.),
                amount: 0.5,
            },
        ],
        root: 1,
        operations: Default::default(),
    });
    let sample = |lane, expression| DynamicRuntimeSample {
        instance_id: Uuid::from_u128(1),
        controller_id: Uuid::from_u128(2),
        target: FixtureId(Uuid::from_u128(3)),
        lane_id: Uuid::from_u128(lane),
        expression,
        priority: 3,
        activated_at_millis: 100,
        activation_mix: 1.,
        address: None,
    };
    let originals = Originals {
        value: target(45),
        reads: Cell::new(0),
    };
    let source = crate::bundle_position_component_forest(
        &[
            sample(
                10,
                DynamicSampleExpression::AngleNumeric { program: numeric },
            ),
            sample(
                11,
                DynamicSampleExpression::AngleCurrent {
                    address: Arc::new(address(ProgrammingComponent::Tilt)),
                },
            ),
        ],
        &originals,
    )
    .unwrap()
    .position
    .unwrap();
    assert_eq!(originals.reads.get(), 1);
    let registry = CapturedPositionProgram::new(Uuid::new_v4(), &angles(), &[source]).unwrap();
    assert_eq!(ids(registry.point_dependencies()), vec![45]);
    assert!(!registry.point_dependencies().incomplete());
    assert_eq!(
        originals.reads.get(),
        1,
        "census reads only the already captured original Current"
    );
}

#[test]
fn nested_source_cohort_whole_tapes_and_original_current_are_all_enumerated() {
    let original = Arc::new(CapturedPositionCurrent {
        value: target(60),
        occurrence: None,
    });
    let pair = Arc::new(PositionAnglePairEndpoint {
        axes: [ProgrammingComponent::Pan, ProgrammingComponent::Tilt].map(|component| {
            PositionAngleAxis::Current {
                lane_id: Uuid::new_v4(),
                address: Arc::new(
                    CompiledDynamicValueAddress::new(
                        DynamicValueAddress {
                            representation: DynamicFamilyRepresentation::Angles,
                            component: Some(component),
                        },
                        None,
                    )
                    .unwrap(),
                ),
                original: Arc::clone(&original),
            }
        }),
    });
    let nodes = vec![
        PositionForestNode::SourceCohort(
            vec![CoupledCohortEndpoint::WholeExpression {
                lane_id: Uuid::from_u128(20),
                expression: compiled(resume(leaf(target(50)), Some(leaf(target(55))), 70, 0.5)),
            }]
            .into(),
        ),
        PositionForestNode::AnglePair(pair),
        PositionForestNode::Transition {
            from: Some(0),
            to: Some(1),
            progress: 0.5,
            reason: DynamicTransitionReason::Resume {
                occurrence_id: Uuid::from_u128(71),
            },
        },
    ];
    let source = FamilyCompositionSample::CoupledExpression {
        expression: Arc::new(
            CompiledCoupledExpression::from_position_forest(Arc::from([]), &nodes, 2).unwrap(),
        ),
        rank: rank(90),
        activation_mix: 1.,
    };
    let registry = CapturedPositionProgram::new(Uuid::new_v4(), &angles(), &[source]).unwrap();
    assert_eq!(ids(registry.point_dependencies()), vec![50, 55, 60]);
    assert!(!registry.point_dependencies().incomplete());
}

#[test]
fn trace_only_scalar_current_explicitly_marks_the_census_incomplete() {
    let address = Arc::new(
        CompiledDynamicValueAddress::new(
            DynamicValueAddress {
                representation: DynamicFamilyRepresentation::Angles,
                component: Some(ProgrammingComponent::Pan),
            },
            None,
        )
        .unwrap(),
    );
    let mut sample = FamilySample::new(address, DynamicValue::Scalar(10.), rank(10), 1.).unwrap();
    sample.trace_sources = Some(
        vec![trace::FamilyTraceLeaf::Current(
            FamilyTraceSource {
                rank: rank(10),
                footprint: FamilyTraceFootprint::Component(ProgrammingComponent::Pan),
                role: FamilyTraceRole::CalculationDependency,
                occurrence: None,
            },
            crate::DynamicSourceDependency::unknown(None),
        )]
        .into(),
    );
    let registry =
        CapturedPositionProgram::new(Uuid::new_v4(), &target(80), &[sample.into()]).unwrap();
    assert_eq!(ids(registry.point_dependencies()), vec![80]);
    assert!(
        registry.point_dependencies().incomplete(),
        "scalar trace cannot reveal its original family Point"
    );
}

#[test]
fn shared_retained_dag_is_traversed_without_expanding_interrupted_history() {
    let mut expression = leaf(target(90));
    for index in 0..40 {
        expression = resume(Arc::clone(&expression), Some(expression), 100 + index, 0.5);
    }
    let tape = Arc::new(RetainedExpressionTape::from_roots(&[expression]).unwrap());
    assert_eq!(
        tape.nodes.len(),
        41,
        "shared branches remain one flat history"
    );
    let source = whole(
        Arc::new(DynamicSampleExpression::Retained {
            root: tape.roots()[0],
            tape,
        }),
        10,
    );
    let registry = CapturedPositionProgram::new(Uuid::new_v4(), &angles(), &[source]).unwrap();
    assert_eq!(ids(registry.point_dependencies()), vec![90]);
    assert!(!registry.point_dependencies().incomplete());
}
