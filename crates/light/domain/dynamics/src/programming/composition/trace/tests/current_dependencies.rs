use super::*;
use light_core::FixtureId;
use std::cell::Cell;

type F = ProgrammingTraceField;
type S = ProgrammingFieldScope;

fn id(value: u128) -> DynamicSourceOccurrenceId {
    DynamicSourceOccurrenceId::new(Uuid::from_u128(value)).unwrap()
}

fn compose_color(
    expression: DynamicSampleExpression,
    component: ColorComponent,
) -> RetainedFamilyCompositionScratch {
    let template = color_sample(Some(component), DynamicValue::Scalar(0.4), 1, 1.0);
    let compiled =
        CompiledComponentExpression::new(Arc::new(expression), template.address.clone()).unwrap();
    let sample = FamilySample::retained_component(Arc::new(compiled), template.rank, 1.0).unwrap();
    let mut scratch = RetainedFamilyCompositionScratch::default();
    compose_retained_dynamic_family_traced(
        ProgrammingOwner::Color,
        &semantic(),
        &[sample.into()],
        &FamilyCompositionContext {
            edit: FamilyEditContext {
                color_model: Some(&VirtualColorAuthoringV1),
                ..Default::default()
            },
            ..Default::default()
        },
        &NoFrame,
        &mut scratch,
    )
    .unwrap();
    scratch
}

fn color_leaf(
    component: ColorComponent,
    dependency: DynamicSourceDependency,
) -> DynamicSampleExpression {
    let template = color_sample(Some(component), DynamicValue::Scalar(0.4), 1, 1.0);
    DynamicSampleExpression::Programming {
        address: Arc::new(template.address.address().clone()),
        value: DynamicValue::Scalar(0.4),
        occurrence: Some(id(801)),
        dependency_occurrence: Some(dependency),
    }
}

#[test]
fn compatible_red_and_hsv_current_reads_report_original_recipe_inputs_for_xyz_output() {
    for component in [
        ColorComponent::Red,
        ColorComponent::Hue,
        ColorComponent::Saturation,
    ] {
        let template = color_sample(Some(component), DynamicValue::Scalar(0.4), 1, 1.0);
        let dependency =
            DynamicSourceDependency::compatible(Some(id(802)), template.address.address());
        let tree = color_leaf(component, dependency);
        let tape = RetainedExpressionTape::from_roots(&[Arc::new(tree.clone())]).unwrap();
        let restored: RetainedExpressionTape =
            serde_json::from_value(serde_json::to_value(&tape).unwrap()).unwrap();
        let held = DynamicSampleExpression::Retained {
            root: restored.roots()[0],
            tape: Arc::new(restored),
        };
        for expression in [tree, held] {
            let scratch = compose_color(expression, component);
            let trace = scratch.family_trace();
            let contributions = trace
                .query_fields(trace.root().unwrap(), &S::new([F::ColorXyz]))
                .unwrap();
            let dependency = contributions
                .iter()
                .find(|entry| entry.source.occurrence == Some(id(802)))
                .unwrap();
            assert_eq!(
                dependency.source.role,
                FamilyTraceRole::CalculationDependency
            );
            assert_eq!(
                dependency.source.footprint,
                FamilyTraceFootprint::Component(ProgrammingComponent::Color(component))
            );
            assert_eq!(
                dependency.fields,
                if component == ColorComponent::Red {
                    S::new([F::ColorRecipeRed])
                } else {
                    S::new([F::ColorRecipeRed, F::ColorRecipeGreen, F::ColorRecipeBlue])
                }
            );
            assert_eq!(
                contributions
                    .iter()
                    .filter(|entry| entry.source.occurrence == Some(id(801)))
                    .count(),
                1
            );
        }
    }
}

#[test]
fn used_current_with_unknown_identity_keeps_a_dependency_and_unknown_transfer_stays_unknown() {
    for transfer in [
        DynamicSourceTransfer::Identity,
        DynamicSourceTransfer::Unknown,
    ] {
        let scratch = compose_color(
            color_leaf(
                ColorComponent::Uv,
                DynamicSourceDependency {
                    occurrence: None,
                    transfer: transfer.clone(),
                },
            ),
            ColorComponent::Uv,
        );
        let trace = scratch.family_trace();
        let result = trace.query_fields(trace.root().unwrap(), &S::new([F::Uv]));
        if transfer == DynamicSourceTransfer::Unknown {
            assert_eq!(result, None);
        } else {
            let sources = result.unwrap();
            assert_eq!(sources.len(), 2);
            assert!(sources.iter().any(|entry| entry.source.occurrence.is_none()
                && entry.source.role == FamilyTraceRole::CalculationDependency
                && entry.fields == S::new([F::Uv])));
        }
        // An unknown Current for UV does not taint unrelated fields.
        assert_eq!(
            trace.sources_for_field(trace.root().unwrap(), F::WhiteBlend),
            Some(vec![])
        );
    }
}

#[test]
fn exact_coupled_leaf_and_whole_family_leaf_keep_current_transfer_metadata() {
    let unknown = DynamicSourceDependency::unknown(Some(id(804)));
    let coupled = FamilyCompositionSample::CoupledExpression {
        expression: Arc::new(
            CompiledCoupledExpression::new(
                Arc::new(color_leaf(ColorComponent::Uv, unknown.clone())),
                None,
            )
            .unwrap(),
        ),
        rank: color_sample(Some(ColorComponent::Uv), DynamicValue::Scalar(0.4), 1, 1.0).rank,
        activation_mix: 1.0,
    };
    let value = semantic();
    let whole = DynamicSampleExpression::Programming {
        address: Arc::new(
            DynamicValueAddress::whole_family(ProgrammingOwner::Color, &value).unwrap(),
        ),
        value: DynamicValue::Family(value),
        occurrence: Some(id(801)),
        dependency_occurrence: Some(unknown),
    };
    let whole = FamilyCompositionSample::WholeExpression {
        expression: Arc::new(
            CompiledProgrammingFamilyExpression::new(
                Arc::new(whole),
                ProgrammingOwner::Color,
                None,
                None,
            )
            .unwrap(),
        ),
        rank: color_sample(None, DynamicValue::Family(semantic()), 2, 1.0).rank,
        activation_mix: 1.0,
    };
    for sample in [coupled, whole] {
        let mut scratch = RetainedFamilyCompositionScratch::default();
        compose_retained_dynamic_family_traced(
            ProgrammingOwner::Color,
            &semantic(),
            &[sample],
            &FamilyCompositionContext {
                edit: FamilyEditContext {
                    color_model: Some(&VirtualColorAuthoringV1),
                    ..Default::default()
                },
                ..Default::default()
            },
            &NoFrame,
            &mut scratch,
        )
        .unwrap();
        let trace = scratch.family_trace();
        assert_eq!(trace.sources_for_field(trace.root().unwrap(), F::Uv), None);
    }
}

struct AdoptedAngles {
    dependency: DynamicSourceDependency,
    calls: Cell<usize>,
}
impl DynamicValueSourceResolver for AdoptedAngles {
    fn current(&self, _: FixtureId, address: &DynamicValueAddress) -> Option<DynamicValue> {
        assert_eq!(address.component, Some(ProgrammingComponent::Tilt));
        self.calls.set(self.calls.get() + 1);
        Some(DynamicValue::Scalar(30.0))
    }
    fn current_dependency(&self, _: FixtureId, _: &DynamicValueAddress) -> DynamicSourceDependency {
        self.dependency.clone()
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
fn adopted_current_angle_partner_uses_exact_transfer_or_unknown_and_solves_only_once() {
    let transfer = ProgrammingFieldTransfer {
        identity: S::empty(),
        remap: vec![(F::TargetY, F::Tilt)].into(),
    };
    for dependency in [
        DynamicSourceDependency::unknown(Some(id(803))),
        DynamicSourceDependency::mapped(Some(id(803)), transfer),
    ] {
        let sources = AdoptedAngles {
            dependency: dependency.clone(),
            calls: Cell::new(0),
        };
        let address = |component| {
            Arc::new(DynamicValueAddress {
                representation: DynamicFamilyRepresentation::Angles,
                component: Some(component),
            })
        };
        let expressions = [
            DynamicSampleExpression::Programming {
                address: address(ProgrammingComponent::Pan),
                value: DynamicValue::Scalar(90.0),
                occurrence: Some(id(801)),
                dependency_occurrence: None,
            },
            DynamicSampleExpression::AngleCurrent {
                address: address(ProgrammingComponent::Tilt),
            },
        ];
        let samples = expressions
            .into_iter()
            .enumerate()
            .map(|(lane, expression)| crate::DynamicRuntimeSample {
                instance_id: Uuid::from_u128(1),
                controller_id: Uuid::from_u128(2),
                target: FixtureId(Uuid::from_u128(3)),
                lane_id: Uuid::from_u128(10 + lane as u128),
                expression,
                priority: 1,
                activated_at_millis: 100,
                activation_mix: 1.0,
                address: None,
            })
            .collect::<Vec<_>>();
        let mut preparation = DynamicFamilyPreparationScratch::default();
        let prepared =
            prepare_dynamic_family_samples(&samples, &sources, None, &mut preparation).unwrap();
        let mut scratch = RetainedFamilyCompositionScratch::default();
        let output = compose_retained_dynamic_family_traced(
            ProgrammingOwner::Position,
            &AttributeValue::Position(Arc::new(PositionIntent::angles(10.0, 20.0))),
            &prepared.families[0].samples,
            &FamilyCompositionContext::default(),
            &NoFrame,
            &mut scratch,
        )
        .unwrap();
        assert_eq!(
            output,
            AttributeValue::Position(Arc::new(PositionIntent::angles(90.0, 30.0)))
        );
        let trace = scratch.family_trace();
        let tilt = trace.query_fields(trace.root().unwrap(), &S::new([F::Tilt]));
        if matches!(dependency.transfer, DynamicSourceTransfer::Unknown) {
            assert_eq!(tilt, None);
        } else {
            let tilt = tilt.unwrap();
            assert_eq!(tilt.len(), 1);
            assert_eq!(tilt[0].source.occurrence, Some(id(803)));
            assert_eq!(
                tilt[0].source.footprint,
                FamilyTraceFootprint::Component(ProgrammingComponent::Tilt)
            );
            assert_eq!(tilt[0].source.role, FamilyTraceRole::CalculationDependency);
            assert_eq!(tilt[0].fields, S::new([F::TargetY]));
        }
        assert_eq!(
            trace
                .sources_for_field(trace.root().unwrap(), F::Pan)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(sources.calls.get(), 1);
    }
}
