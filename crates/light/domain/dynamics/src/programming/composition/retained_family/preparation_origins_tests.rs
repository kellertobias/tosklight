//! Original slots are explicit preparation metadata, independent of ranks and winning values.
use super::*;
use crate::programming::expression_coupled::PositionForestNode;
use crate::{
    CoupledCohortEndpoint, CoupledComponentEndpoint, CoupledLeafRole, DynamicPresetSourceBinding,
    DynamicValueSourceResolver,
};
use light_core::FixtureId;

fn rank(lane: u128) -> FamilySampleRank {
    FamilySampleRank {
        priority: 3,
        changed_at_millis: 100,
        changed_at_submillis_nanos: 0,
        stable_order: 0,
        identity: FamilySampleIdentity::Dynamic {
            instance_id: Uuid::from_u128(1),
            controller_id: Uuid::from_u128(2),
            lane_id: Uuid::from_u128(lane),
        },
    }
}
fn base() -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Origin,
        [0.; 3],
    )))
}
fn address(component: Option<ProgrammingComponent>) -> Arc<CompiledDynamicValueAddress> {
    Arc::new(
        CompiledDynamicValueAddress::new(
            DynamicValueAddress {
                representation: DynamicFamilyRepresentation::Target {
                    reference: Some(TargetReference::Origin),
                },
                component,
            },
            None,
        )
        .unwrap(),
    )
}
fn known(lane: u128, component: Option<ProgrammingComponent>, mix: f32) -> FamilyCompositionSample {
    FamilySample::new(
        address(component),
        if component.is_some() {
            DynamicValue::Scalar(lane as f32)
        } else {
            DynamicValue::Family(base())
        },
        rank(lane),
        mix,
    )
    .unwrap()
    .into()
}
fn whole_expression() -> Arc<CompiledProgrammingFamilyExpression> {
    Arc::new(
        CompiledProgrammingFamilyExpression::new(
            Arc::new(DynamicSampleExpression::Programming {
                address: Arc::new(address(None).address().clone()),
                value: DynamicValue::Family(base()),
                occurrence: None,
                dependency_occurrence: None,
            }),
            ProgrammingOwner::Position,
            None,
            None,
        )
        .unwrap(),
    )
}
fn whole(lane: u128, mix: f32) -> FamilyCompositionSample {
    FamilyCompositionSample::WholeExpression {
        expression: whole_expression(),
        rank: rank(lane),
        activation_mix: mix,
    }
}
fn prepare(
    inputs: Vec<(FamilyCompositionSample, Vec<usize>)>,
    scratch: &mut RetainedFamilyCompositionScratch,
) {
    prepare_family_inputs_with_origins(
        ProgrammingOwner::Position,
        &base(),
        inputs.into_iter(),
        &FamilyCompositionContext::default(),
        scratch,
        false,
    )
    .unwrap();
    assert_eq!(scratch.sources.len(), scratch.source_origins.len());
    assert_eq!(scratch.known.len(), scratch.known_origins.len());
}
fn original_slots(scratch: &RetainedFamilyCompositionScratch) -> Vec<Vec<usize>> {
    scratch
        .source_origins
        .iter()
        .map(|origins| origins.iter().map(|origin| origin.original_index).collect())
        .collect()
}
fn ordered_origins(scratch: &RetainedFamilyCompositionScratch) -> Vec<Vec<usize>> {
    scratch
        .ordered
        .iter()
        .map(|&index| {
            scratch.source_origins[index]
                .iter()
                .map(|origin| origin.original_index)
                .collect()
        })
        .collect()
}
fn member_paths(scratch: &RetainedFamilyCompositionScratch) -> Vec<Vec<Option<usize>>> {
    scratch
        .source_origins
        .iter()
        .map(|origins| origins.iter().map(|origin| origin.member).collect())
        .collect()
}

#[test]
fn original_slots_survive_reversed_inputs_known_buffering_filtering_and_source_order() {
    let inputs = vec![
        (whole(30, 1.), vec![7]),
        (known(20, Some(ProgrammingComponent::TargetX), 1.), vec![3]),
        (known(5, None, 0.), vec![1]),
        (whole(10, 0.), vec![2]),
    ];
    let mut forward = RetainedFamilyCompositionScratch::default();
    prepare(inputs.clone(), &mut forward);
    assert_eq!(original_slots(&forward), vec![vec![7], vec![3]]);
    assert_eq!(forward.ordered, vec![1, 0]);
    let mut reverse = RetainedFamilyCompositionScratch::default();
    prepare(inputs.into_iter().rev().collect(), &mut reverse);
    assert_eq!(ordered_origins(&forward), vec![vec![3], vec![7]]);
    assert_eq!(ordered_origins(&reverse), ordered_origins(&forward));
    assert_eq!(
        forward.sources.len(),
        2,
        "zero-activation input slots are filtered without renumbering survivors"
    );
}
fn axis(lane: u128, component: ProgrammingComponent, instance: u128) -> FamilyCompositionSample {
    let mut source_rank = rank(lane);
    source_rank.identity = FamilySampleIdentity::Dynamic {
        instance_id: Uuid::from_u128(instance),
        controller_id: Uuid::from_u128(2),
        lane_id: Uuid::from_u128(lane),
    };
    FamilySample::new(
        Arc::new(
            CompiledDynamicValueAddress::new(
                DynamicValueAddress {
                    representation: DynamicFamilyRepresentation::Angles,
                    component: Some(component),
                },
                None,
            )
            .unwrap(),
        ),
        DynamicValue::Scalar(lane as f32),
        source_rank,
        1.,
    )
    .unwrap()
    .into()
}
#[test]
fn synthetic_angle_pair_records_both_actual_input_slots_and_drops_incomplete_bundle() {
    let mut scratch = RetainedFamilyCompositionScratch::default();
    prepare(
        vec![
            (axis(12, ProgrammingComponent::Tilt, 1), vec![9]),
            (known(30, None, 1.), vec![80]),
            (axis(11, ProgrammingComponent::Pan, 1), vec![4, 6]),
            (axis(40, ProgrammingComponent::Pan, 3), vec![100]),
        ],
        &mut scratch,
    );
    assert_eq!(scratch.bundled_membership, vec![vec![1], vec![2, 0]]);
    assert_eq!(original_slots(&scratch), vec![vec![80], vec![4, 6, 9]]);
    assert_eq!(
        scratch.sources[1].rank(),
        rank(12),
        "representative Tilt rank alone does not identify Pan membership"
    );
    let FamilyCompositionSample::Known(pair) = &scratch.sources[1] else {
        panic!("Angle bundle")
    };
    assert_eq!(
        pair.materialized_value(),
        Some(&DynamicValue::Family(AttributeValue::Position(Arc::new(
            PositionIntent::angles(11., 12.)
        ))))
    );
    assert!(
        !scratch
            .source_origins
            .iter()
            .flatten()
            .any(|origin| origin.original_index == 100)
    );
}
fn component(lane: u128, part: ProgrammingComponent) -> CoupledComponentEndpoint {
    CoupledComponentEndpoint {
        lane_id: Uuid::from_u128(lane),
        address: address(Some(part)),
        value: DynamicValue::Scalar(lane as f32),
        role: CoupledLeafRole::Authored,
        occurrence: None,
        dependency_occurrence: None,
    }
}
fn forest(node: PositionForestNode) -> FamilyCompositionSample {
    FamilyCompositionSample::CoupledExpression {
        expression: Arc::new(
            CompiledCoupledExpression::from_position_forest(Arc::from([]), &[node], 0).unwrap(),
        ),
        rank: rank(90),
        activation_mix: 1.,
    }
}
#[test]
fn exact_coupled_cohorts_replicate_original_aggregate_across_all_emitted_members() {
    let mut scratch = RetainedFamilyCompositionScratch::default();
    prepare(
        vec![(
            forest(PositionForestNode::Cohort(
                vec![
                    component(20, ProgrammingComponent::TargetX),
                    component(21, ProgrammingComponent::TargetY),
                ]
                .into(),
            )),
            vec![10, 30],
        )],
        &mut scratch,
    );
    assert_eq!(original_slots(&scratch), vec![vec![10, 30], vec![10, 30]]);
    assert_eq!(
        member_paths(&scratch),
        vec![vec![Some(0), Some(0)], vec![Some(1), Some(1)]]
    );
    assert_eq!(
        scratch
            .sources
            .iter()
            .map(FamilyCompositionSample::rank)
            .collect::<Vec<_>>(),
        vec![rank(20), rank(21)]
    );
    let sources = vec![
        CoupledCohortEndpoint::WholeExpression {
            lane_id: Uuid::from_u128(25),
            expression: whole_expression(),
        },
        CoupledCohortEndpoint::Materialized(component(26, ProgrammingComponent::TargetZ)),
    ];
    prepare(
        vec![(
            forest(PositionForestNode::SourceCohort(sources.into())),
            vec![70],
        )],
        &mut scratch,
    );
    assert!(matches!(
        &scratch.sources[0],
        FamilyCompositionSample::WholeExpression { .. }
    ));
    assert!(matches!(
        &scratch.sources[1],
        FamilyCompositionSample::Known(_)
    ));
    assert_eq!(original_slots(&scratch), vec![vec![70], vec![70]]);
    assert_eq!(member_paths(&scratch), vec![vec![Some(0)], vec![Some(1)]]);
    assert_eq!(
        scratch
            .sources
            .iter()
            .map(FamilyCompositionSample::rank)
            .collect::<Vec<_>>(),
        vec![rank(25), rank(26)]
    );
}
#[test]
fn released_original_slots_remain_holes_and_scratch_reuse_clears_bound_membership() {
    let slots = vec![None, Some(known(20, None, 1.)), None, Some(whole(30, 1.))];
    let mut scratch = RetainedFamilyCompositionScratch::default();
    prepare(
        slots
            .into_iter()
            .enumerate()
            .filter_map(|(slot, sample)| sample.map(|sample| (sample, vec![slot])))
            .collect(),
        &mut scratch,
    );
    assert_eq!(ordered_origins(&scratch), vec![vec![1], vec![3]]);
    prepare_family_inputs(
        ProgrammingOwner::Position,
        &base(),
        vec![known(40, None, 1.)].into_iter(),
        &FamilyCompositionContext::default(),
        &mut scratch,
        false,
    )
    .unwrap();
    assert_eq!(original_slots(&scratch), vec![Vec::<usize>::new()]);
    assert!(scratch.known_origins.iter().all(Vec::is_empty));
    assert!(scratch.bundled_membership.is_empty());
    prepare(Vec::new(), &mut scratch);
    assert!(scratch.sources.is_empty());
    assert!(scratch.source_origins.is_empty());
    assert!(scratch.known_origins.is_empty());
    assert!(scratch.bundled_membership.is_empty());
}
struct AbsentCurrent;
impl DynamicValueSourceResolver for AbsentCurrent {
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
fn suppressed_output_keeps_source_index_and_origin_alignment_but_leaves_ordered_execution() {
    let current = AbsentCurrent;
    let control = |rank: FamilySampleRank| {
        if rank == self::rank(20) {
            FamilyEndpointOutputControl::Suppressed
        } else {
            FamilyEndpointOutputControl::Unchanged
        }
    };
    let context = FamilyCompositionContext {
        endpoint_output: Some(FamilyEndpointOutputContext {
            control: &control,
            target: FixtureId(Uuid::from_u128(3)),
            current: &current,
            native_models: None,
        }),
        ..Default::default()
    };
    let mut scratch = RetainedFamilyCompositionScratch::default();
    prepare_family_inputs_with_origins(
        ProgrammingOwner::Position,
        &base(),
        vec![
            (known(20, None, 1.), vec![4]),
            (known(30, None, 1.), vec![8]),
        ]
        .into_iter(),
        &context,
        &mut scratch,
        false,
    )
    .unwrap();
    assert_eq!(original_slots(&scratch), vec![vec![4], vec![8]]);
    assert_eq!(scratch.ordered, vec![1]);
}

#[test]
fn synthetic_pair_keeps_distinct_member_paths_of_the_same_exact_cohort_slot() {
    let axis_component = |lane, part| {
        let FamilyCompositionSample::Known(sample) = axis(lane, part, 1) else {
            unreachable!()
        };
        CoupledComponentEndpoint {
            lane_id: Uuid::from_u128(lane),
            address: sample.address.clone(),
            value: sample.materialized_value().unwrap().clone(),
            role: CoupledLeafRole::Authored,
            occurrence: None,
            dependency_occurrence: None,
        }
    };
    let mut scratch = RetainedFamilyCompositionScratch::default();
    prepare(
        vec![(
            forest(PositionForestNode::SourceCohort(
                vec![
                    CoupledCohortEndpoint::Materialized(axis_component(
                        12,
                        ProgrammingComponent::Tilt,
                    )),
                    CoupledCohortEndpoint::Materialized(axis_component(
                        11,
                        ProgrammingComponent::Pan,
                    )),
                ]
                .into(),
            )),
            vec![4],
        )],
        &mut scratch,
    );
    assert_eq!(scratch.bundled_membership, vec![vec![1, 0]]);
    assert_eq!(
        scratch.source_origins,
        vec![vec![
            PreparedSourceOrigin {
                original_index: 4,
                member: Some(1)
            },
            PreparedSourceOrigin {
                original_index: 4,
                member: Some(0)
            },
        ]]
    );
}
