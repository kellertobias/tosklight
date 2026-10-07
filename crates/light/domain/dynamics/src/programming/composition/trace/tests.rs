use super::*;
use crate::programming::*;
use light_core::programming::ProgrammingComponent;
use light_core::{AttributeValue, programming::*};
use std::sync::Arc;
use uuid::Uuid;

mod baseline_fields;
mod current_dependencies;
mod field_transfers;

fn source(lane_id: Uuid, footprint: FamilyTraceFootprint) -> FamilyTraceSource {
    FamilyTraceSource {
        rank: FamilySampleRank {
            priority: 0,
            changed_at_millis: 1,
            changed_at_submillis_nanos: 0,
            stable_order: 1,
            identity: crate::FamilySampleIdentity::Dynamic {
                instance_id: Uuid::new_v4(),
                controller_id: Uuid::new_v4(),
                lane_id,
            },
        },
        footprint,
        role: FamilyTraceRole::Authored,
        occurrence: None,
    }
}

#[test]
fn complete_component_write_cuts_only_that_field() {
    let mut arena = FamilyTraceArena::default();
    let red = source(
        Uuid::new_v4(),
        FamilyTraceFootprint::Component(ProgrammingComponent::Pan),
    );
    let blue = source(
        Uuid::new_v4(),
        FamilyTraceFootprint::Component(ProgrammingComponent::Tilt),
    );
    let replacement = source(
        Uuid::new_v4(),
        FamilyTraceFootprint::Component(ProgrammingComponent::Pan),
    );
    let base = arena.base();
    let red_id = arena.source(red);
    let after_red = arena.write(base, red_id, red.footprint, false);
    let blue_id = arena.source(blue);
    let after_blue = arena.write(after_red, blue_id, blue.footprint, false);
    let next_id = arena.source(replacement);
    let root = arena.write(after_blue, next_id, replacement.footprint, false);
    assert_eq!(
        arena
            .sources_for_component(root, ProgrammingComponent::Pan)
            .unwrap(),
        vec![replacement]
    );
    assert_eq!(
        arena
            .sources_for_component(root, ProgrammingComponent::Tilt)
            .unwrap(),
        vec![blue]
    );
}

#[test]
fn partial_write_and_bundle_keep_authored_and_current_separate() {
    let mut arena = FamilyTraceArena::default();
    let original = source(Uuid::new_v4(), FamilyTraceFootprint::Whole);
    let pan = source(
        Uuid::new_v4(),
        FamilyTraceFootprint::Component(ProgrammingComponent::Pan),
    );
    let mut current_tilt = source(
        Uuid::new_v4(),
        FamilyTraceFootprint::Component(ProgrammingComponent::Tilt),
    );
    current_tilt.role = FamilyTraceRole::CalculationDependency;
    let original_id = arena.source(original);
    let pan_id = arena.source(pan);
    let tilt_id = arena.source(current_tilt);
    let bundle = arena.bundle(vec![pan_id, tilt_id]);
    let root = arena.write(original_id, bundle, FamilyTraceFootprint::Whole, true);
    assert_eq!(
        arena
            .sources_for_component(root, ProgrammingComponent::Pan)
            .unwrap(),
        vec![pan, original]
    );
    assert_eq!(
        arena
            .sources_for_component(root, ProgrammingComponent::Tilt)
            .unwrap(),
        vec![current_tilt, original]
    );
}

struct NoFrame;
impl WholeFamilyExpressionFrameResolver for NoFrame {
    fn resolve(
        &self,
        requirement: TransitionRequirement,
        _: &AttributeValue,
        _: &AttributeValue,
        _: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        Err(TransitionError::Requires(requirement))
    }
}

fn color_sample(
    component: Option<ColorComponent>,
    level: DynamicValue,
    order: u128,
    mix: f32,
) -> FamilySample {
    FamilySample::new(
        Arc::new(
            CompiledDynamicValueAddress::new(
                DynamicValueAddress {
                    representation: DynamicFamilyRepresentation::SemanticColor {
                        basis: if component.is_none() {
                            DynamicSemanticColorBasis::Whole
                        } else if component.is_some_and(|part| {
                            matches!(
                                part,
                                ColorComponent::Red
                                    | ColorComponent::Green
                                    | ColorComponent::Blue
                                    | ColorComponent::Amber
                            )
                        }) {
                            DynamicSemanticColorBasis::Recipe
                        } else if matches!(
                            component,
                            Some(ColorComponent::Hue | ColorComponent::Saturation)
                        ) {
                            DynamicSemanticColorBasis::HueSaturation
                        } else {
                            DynamicSemanticColorBasis::Retain
                        },
                    },
                    component: component.map(ProgrammingComponent::Color),
                },
                None,
            )
            .unwrap(),
        ),
        level,
        FamilySampleRank {
            priority: 10,
            changed_at_millis: 100,
            changed_at_submillis_nanos: 0,
            stable_order: order,
            identity: crate::FamilySampleIdentity::Dynamic {
                instance_id: Uuid::from_u128(1000 + order),
                controller_id: Uuid::from_u128(2000 + order),
                lane_id: Uuid::from_u128(3000 + order),
            },
        },
        mix,
    )
    .unwrap()
}

fn semantic() -> AttributeValue {
    AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
        intent: ColorIntent::default(),
    }))
}

fn composed_sources(
    samples: &[FamilyCompositionSample],
    component: ColorComponent,
) -> Vec<FamilyTraceSource> {
    let mut scratch = RetainedFamilyCompositionScratch::default();
    let context = FamilyCompositionContext {
        edit: FamilyEditContext {
            color_model: Some(&VirtualColorAuthoringV1),
            ..Default::default()
        },
        ..Default::default()
    };
    compose_retained_dynamic_family_traced(
        ProgrammingOwner::Color,
        &semantic(),
        samples,
        &context,
        &NoFrame,
        &mut scratch,
    )
    .unwrap();
    scratch
        .family_trace()
        .sources_for_component(
            scratch.family_trace().root().expect("composed trace root"),
            ProgrammingComponent::Color(component),
        )
        .expect("known component trace")
}

#[test]
fn equal_valued_opaque_color_writer_shadows_older_edit() {
    let older = color_sample(Some(ColorComponent::Red), DynamicValue::Scalar(0.5), 1, 1.0);
    let newer = color_sample(Some(ColorComponent::Red), DynamicValue::Scalar(0.5), 2, 1.0);
    let sources = composed_sources(
        &[older.clone().into(), newer.clone().into()],
        ColorComponent::Red,
    );
    assert!(
        sources
            .iter()
            .any(|source| source.rank.dynamic_identity().unwrap().lane_id
                == newer.rank.dynamic_identity().unwrap().lane_id)
    );
    assert!(
        !sources
            .iter()
            .any(|source| source.rank.dynamic_identity().unwrap().lane_id
                == older.rank.dynamic_identity().unwrap().lane_id)
    );
}

#[test]
fn partial_activation_retains_lower_lineage_and_whole_fixat_cuts_it() {
    let older = color_sample(Some(ColorComponent::Red), DynamicValue::Scalar(0.2), 1, 1.0);
    let partial = color_sample(Some(ColorComponent::Red), DynamicValue::Scalar(0.8), 2, 0.5);
    let sources = composed_sources(
        &[older.clone().into(), partial.clone().into()],
        ColorComponent::Red,
    );
    assert!(
        sources
            .iter()
            .any(|source| source.rank.dynamic_identity().unwrap().lane_id
                == older.rank.dynamic_identity().unwrap().lane_id)
    );
    assert!(
        sources
            .iter()
            .any(|source| source.rank.dynamic_identity().unwrap().lane_id
                == partial.rank.dynamic_identity().unwrap().lane_id)
    );

    let fixed = color_sample(None, DynamicValue::Family(semantic()), 3, 1.0).into_fix_at();
    let sources = composed_sources(&[older.into(), fixed.clone().into()], ColorComponent::Red);
    assert_eq!(sources.len(), 1);
    assert_eq!(
        sources[0].rank.dynamic_identity().unwrap().lane_id,
        fixed.rank.dynamic_identity().unwrap().lane_id
    );
}

#[test]
fn component_fixat_cuts_only_its_field_and_preserves_sibling_writer() {
    let older_red = color_sample(Some(ColorComponent::Red), DynamicValue::Scalar(0.2), 1, 1.0);
    let older_green = color_sample(
        Some(ColorComponent::Green),
        DynamicValue::Scalar(0.7),
        2,
        1.0,
    );
    let fixed_red =
        color_sample(Some(ColorComponent::Red), DynamicValue::Scalar(0.6), 3, 1.0).into_fix_at();
    let samples = [
        older_red.clone().into(),
        older_green.clone().into(),
        fixed_red.clone().into(),
    ];
    let red = composed_sources(&samples, ColorComponent::Red);
    assert!(
        red.iter()
            .any(|source| source.rank.dynamic_identity().unwrap().lane_id
                == fixed_red.rank.dynamic_identity().unwrap().lane_id)
    );
    assert!(
        !red.iter()
            .any(|source| source.rank.dynamic_identity().unwrap().lane_id
                == older_red.rank.dynamic_identity().unwrap().lane_id)
    );
    let green = composed_sources(&samples, ColorComponent::Green);
    assert!(
        green
            .iter()
            .any(|source| source.rank.dynamic_identity().unwrap().lane_id
                == older_green.rank.dynamic_identity().unwrap().lane_id)
    );
    assert!(
        !green
            .iter()
            .any(|source| source.rank.dynamic_identity().unwrap().lane_id
                == fixed_red.rank.dynamic_identity().unwrap().lane_id)
    );
}

#[test]
fn retained_uv_release_keeps_branch_default_and_independent_white() {
    let lower = color_sample(Some(ColorComponent::Uv), DynamicValue::Scalar(0.2), 1, 1.0);
    let white = color_sample(
        Some(ColorComponent::WhiteBlend),
        DynamicValue::Scalar(0.4),
        2,
        1.0,
    );
    let template = color_sample(Some(ColorComponent::Uv), DynamicValue::Scalar(0.0), 3, 1.0);
    let expression = DynamicSampleExpression::Transition {
        from: Some(Arc::new(DynamicSampleExpression::Programming {
            address: Arc::new(template.address().address().clone()),
            value: DynamicValue::Scalar(0.8),
            occurrence: None,
            dependency_occurrence: None,
        })),
        to: None,
        progress: 0.5,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(901),
        },
    };
    let compiled = CompiledComponentExpression::new(
        Arc::new(expression),
        Arc::new(template.address().clone()),
    )
    .unwrap();
    let release = FamilySample::retained_component(Arc::new(compiled), template.rank, 1.0).unwrap();
    let uv = composed_sources(
        &[lower.clone().into(), white.clone().into(), release.into()],
        ColorComponent::Uv,
    );
    assert!(
        uv.iter()
            .any(|source| source.rank.dynamic_identity().unwrap().lane_id
                == lower.rank.dynamic_identity().unwrap().lane_id)
    );
    assert!(
        uv.iter()
            .any(|source| source.rank.dynamic_identity().unwrap().lane_id
                == template.rank.dynamic_identity().unwrap().lane_id)
    );
    assert!(
        !uv.iter()
            .any(|source| source.rank.dynamic_identity().unwrap().lane_id
                == white.rank.dynamic_identity().unwrap().lane_id)
    );
    let white_sources = composed_sources(
        &[lower.into(), white.clone().into()],
        ColorComponent::WhiteBlend,
    );
    assert!(
        white_sources
            .iter()
            .any(|source| source.rank.dynamic_identity().unwrap().lane_id
                == white.rank.dynamic_identity().unwrap().lane_id)
    );
}

#[test]
fn conditioned_uv_release_retains_earlier_whole_segment_as_dependency() {
    let whole = color_sample(None, DynamicValue::Family(semantic()), 1, 1.0);
    let white = color_sample(
        Some(ColorComponent::WhiteBlend),
        DynamicValue::Scalar(0.4),
        2,
        1.0,
    );
    let template = color_sample(Some(ColorComponent::Uv), DynamicValue::Scalar(0.0), 3, 1.0);
    let expression = DynamicSampleExpression::Transition {
        from: Some(Arc::new(DynamicSampleExpression::Programming {
            address: Arc::new(template.address().address().clone()),
            value: DynamicValue::Scalar(0.8),
            occurrence: None,
            dependency_occurrence: None,
        })),
        to: None,
        progress: 0.5,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(902),
        },
    };
    let compiled = CompiledComponentExpression::new(
        Arc::new(expression),
        Arc::new(template.address().clone()),
    )
    .unwrap();
    let release = FamilySample::retained_component(Arc::new(compiled), template.rank, 1.0).unwrap();
    let uv = composed_sources(
        &[whole.clone().into(), white.clone().into(), release.into()],
        ColorComponent::Uv,
    );
    assert!(
        uv.iter()
            .any(|source| source.rank.dynamic_identity().unwrap().lane_id
                == template.rank.dynamic_identity().unwrap().lane_id)
    );
    assert!(uv.iter().any(|source| {
        source.rank.dynamic_identity().unwrap().lane_id
            == whole.rank.dynamic_identity().unwrap().lane_id
            && source.role == FamilyTraceRole::CalculationDependency
    }));
    assert!(
        !uv.iter()
            .any(|source| source.rank.dynamic_identity().unwrap().lane_id
                == white.rank.dynamic_identity().unwrap().lane_id)
    );
}

#[test]
fn equal_component_values_keep_both_historical_assignment_ids_during_resume() {
    let template = color_sample(Some(ColorComponent::Uv), DynamicValue::Scalar(0.4), 4, 1.0);
    let from_id = DynamicSourceOccurrenceId::new(Uuid::from_u128(801)).unwrap();
    let to_id = DynamicSourceOccurrenceId::new(Uuid::from_u128(802)).unwrap();
    let leaf = |occurrence| {
        Arc::new(DynamicSampleExpression::Programming {
            address: Arc::new(template.address().address().clone()),
            value: DynamicValue::Scalar(0.4),
            occurrence: Some(occurrence),
            dependency_occurrence: None,
        })
    };
    let expression = DynamicSampleExpression::Transition {
        from: Some(leaf(from_id)),
        to: Some(leaf(to_id)),
        progress: 0.5,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(803),
        },
    };
    let compiled = CompiledComponentExpression::new(
        Arc::new(expression),
        Arc::new(template.address().clone()),
    )
    .unwrap();
    let sample = FamilySample::retained_component(Arc::new(compiled), template.rank, 1.0).unwrap();
    let sources = composed_sources(&[sample.into()], ColorComponent::Uv);
    assert_eq!(sources.len(), 2);
    assert!(
        sources
            .iter()
            .any(|source| source.occurrence == Some(from_id))
    );
    assert!(
        sources
            .iter()
            .any(|source| source.occurrence == Some(to_id))
    );
}

#[test]
fn whole_size_keeps_authored_value_and_distinct_static_baseline_ids() {
    let authored = DynamicSourceOccurrenceId::new(Uuid::from_u128(811)).unwrap();
    let static_base = DynamicSourceOccurrenceId::new(Uuid::from_u128(812)).unwrap();
    let value = semantic();
    let address =
        Arc::new(DynamicValueAddress::whole_family(ProgrammingOwner::Color, &value).unwrap());
    let expression = DynamicSampleExpression::Scale {
        address: address.clone(),
        base: DynamicValue::Family(value.clone()),
        value: Arc::new(DynamicSampleExpression::Programming {
            address,
            value: DynamicValue::Family(value),
            occurrence: Some(authored),
            dependency_occurrence: None,
        }),
        factor: 0.5,
        baseline_occurrence: Some(static_base),
    };
    let compiled = CompiledProgrammingFamilyExpression::new(
        Arc::new(expression),
        ProgrammingOwner::Color,
        None,
        None,
    )
    .unwrap();
    let rank = color_sample(None, DynamicValue::Family(semantic()), 5, 1.0).rank;
    let sample = FamilyCompositionSample::WholeExpression {
        expression: Arc::new(compiled),
        rank,
        activation_mix: 1.0,
    };
    let sources = composed_sources(&[sample], ColorComponent::Red);
    assert!(
        sources
            .iter()
            .any(|source| source.occurrence == Some(authored)
                && source.role == FamilyTraceRole::Authored)
    );
    assert!(
        sources
            .iter()
            .any(|source| source.occurrence == Some(static_base)
                && source.role == FamilyTraceRole::CalculationDependency)
    );
}

#[test]
fn position_forest_import_keeps_whole_leaf_and_size_baseline_ids() {
    let authored = DynamicSourceOccurrenceId::new(Uuid::from_u128(821)).unwrap();
    let baseline = DynamicSourceOccurrenceId::new(Uuid::from_u128(822)).unwrap();
    let value = AttributeValue::Position(Arc::new(PositionIntent::angles(20.0, 30.0)));
    let address =
        Arc::new(DynamicValueAddress::whole_family(ProgrammingOwner::Position, &value).unwrap());
    let leaf = DynamicSampleExpression::Programming {
        address: address.clone(),
        value: DynamicValue::Family(value.clone()),
        occurrence: Some(authored),
        dependency_occurrence: None,
    };
    let expressions = [
        leaf.clone(),
        DynamicSampleExpression::Scale {
            address,
            base: DynamicValue::Family(value.clone()),
            value: Arc::new(leaf),
            factor: 0.5,
            baseline_occurrence: Some(baseline),
        },
    ];
    for (index, expression) in expressions.into_iter().enumerate() {
        let sample = crate::DynamicRuntimeSample {
            instance_id: Uuid::from_u128(823),
            controller_id: Uuid::from_u128(824),
            target: light_core::FixtureId(Uuid::from_u128(825)),
            lane_id: Uuid::from_u128(826),
            expression,
            priority: 0,
            activated_at_millis: 1,
            activation_mix: 1.0,
            address: None,
        };
        let bundle = bundle_position_component_forest(
            &[sample],
            &crate::programming::UnavailableProgrammingSources,
        )
        .unwrap();
        let position = bundle.position.expect("whole Position forest");
        let mut scratch = RetainedFamilyCompositionScratch::default();
        compose_retained_dynamic_family_traced(
            ProgrammingOwner::Position,
            &AttributeValue::Position(Arc::new(PositionIntent::angles(0.0, 0.0))),
            &[position],
            &FamilyCompositionContext::default(),
            &NoFrame,
            &mut scratch,
        )
        .unwrap();
        let sources = scratch
            .family_trace()
            .sources_for_component(
                scratch.family_trace().root().unwrap(),
                ProgrammingComponent::Pan,
            )
            .expect("known Position trace");
        assert!(
            sources
                .iter()
                .any(|source| source.occurrence == Some(authored)
                    && source.role == FamilyTraceRole::Authored)
        );
        if index == 1 {
            assert!(
                sources
                    .iter()
                    .any(|source| source.occurrence == Some(baseline)
                        && source.role == FamilyTraceRole::CalculationDependency)
            );
        }
    }
}

#[test]
fn position_size_zero_keeps_only_static_baseline_dependency() {
    let authored = DynamicSourceOccurrenceId::new(Uuid::from_u128(831)).unwrap();
    let baseline = DynamicSourceOccurrenceId::new(Uuid::from_u128(832)).unwrap();
    let value = AttributeValue::Position(Arc::new(PositionIntent::angles(20.0, 30.0)));
    let address =
        Arc::new(DynamicValueAddress::whole_family(ProgrammingOwner::Position, &value).unwrap());
    let expression = DynamicSampleExpression::Scale {
        address: address.clone(),
        base: DynamicValue::Family(value.clone()),
        value: Arc::new(DynamicSampleExpression::Programming {
            address,
            value: DynamicValue::Family(value),
            occurrence: Some(authored),
            dependency_occurrence: None,
        }),
        factor: 0.0,
        baseline_occurrence: Some(baseline),
    };
    let sample = crate::DynamicRuntimeSample {
        instance_id: Uuid::from_u128(833),
        controller_id: Uuid::from_u128(834),
        target: light_core::FixtureId(Uuid::from_u128(835)),
        lane_id: Uuid::from_u128(836),
        expression,
        priority: 0,
        activated_at_millis: 1,
        activation_mix: 1.0,
        address: None,
    };
    let bundle = bundle_position_component_forest(
        &[sample],
        &crate::programming::UnavailableProgrammingSources,
    )
    .unwrap();
    let mut scratch = RetainedFamilyCompositionScratch::default();
    compose_retained_dynamic_family_traced(
        ProgrammingOwner::Position,
        &AttributeValue::Position(Arc::new(PositionIntent::angles(0.0, 0.0))),
        &[bundle.position.unwrap()],
        &FamilyCompositionContext::default(),
        &NoFrame,
        &mut scratch,
    )
    .unwrap();
    let sources = scratch
        .family_trace()
        .sources_for_component(
            scratch.family_trace().root().unwrap(),
            ProgrammingComponent::Pan,
        )
        .expect("known Position trace");
    assert!(
        sources
            .iter()
            .any(|source| source.occurrence == Some(baseline)
                && source.role == FamilyTraceRole::CalculationDependency)
    );
    assert!(
        !sources
            .iter()
            .any(|source| source.occurrence == Some(authored))
    );
}
