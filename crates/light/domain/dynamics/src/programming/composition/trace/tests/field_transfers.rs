use super::*;
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

use light_core::{NativeColorBinding, NativeColorIdentity, NativeColorValue, PhysicalDataQuality};

type F = ProgrammingTraceField;
type S = ProgrammingFieldScope;

struct NativeModel {
    source: NativeColorIdentity,
    bindings: [NativeColorBinding; 2],
    predictions: AtomicUsize,
}

impl NativeColorEditModel for NativeModel {
    fn source(&self) -> &NativeColorIdentity {
        &self.source
    }

    fn descriptor(&self, binding: NativeColorBinding) -> Option<NativeColorComponentDescriptor> {
        self.bindings
            .contains(&binding)
            .then_some(NativeColorComponentDescriptor {
                binding,
                raw_from: 0,
                raw_to: u32::MAX,
                continuous: binding == self.bindings[0],
            })
    }

    fn predict(&self, recipe: &NativeColorRecipe) -> Result<PortableColorEstimate, IntentError> {
        assert_eq!(recipe.source, self.source);
        assert_eq!(recipe.channels.len(), 2);
        self.predictions.fetch_add(1, Ordering::Relaxed);
        Ok(PortableColorEstimate {
            model_revision: self.source.model_revision,
            visible: None,
            uv: None,
            quality: PhysicalDataQuality::Estimated,
            limitations: vec![],
        })
    }
}

struct NativeModels(Arc<NativeModel>);

impl crate::DynamicNativeModelResolver for NativeModels {
    fn resolve(
        &self,
        source: &NativeColorIdentity,
    ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, IntentError> {
        assert_eq!(source, &self.0.source);
        Ok(self.0.clone())
    }
}

fn native_value(model: &NativeModel, values: [u32; 2]) -> AttributeValue {
    AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
        recipe: NativeColorRecipe {
            source: model.source.clone(),
            channels: model
                .bindings
                .iter()
                .zip(values)
                .map(|(binding, raw)| NativeColorValue {
                    channel_id: binding.channel_id,
                    function_id: binding.function_id,
                    raw,
                })
                .collect(),
            spreads: vec![],
        },
        portable: PortableColorEstimate {
            model_revision: model.source.model_revision,
            visible: None,
            uv: None,
            quality: PhysicalDataQuality::Estimated,
            limitations: vec![],
        },
    }))
}

fn native_model() -> Arc<NativeModel> {
    Arc::new(NativeModel {
        source: NativeColorIdentity {
            profile_id: Uuid::from_u128(401),
            profile_revision: 1,
            profile_digest: "original-profile".into(),
            mode_id: Uuid::from_u128(402),
            head_id: Uuid::from_u128(403),
            path_id: Uuid::from_u128(404),
            model_revision: 1,
            native_layout_signature: "continuous-and-discrete".into(),
        },
        bindings: [
            NativeColorBinding {
                channel_id: Uuid::from_u128(405),
                function_id: Uuid::from_u128(406),
            },
            NativeColorBinding {
                channel_id: Uuid::from_u128(407),
                function_id: Uuid::from_u128(408),
            },
        ],
        predictions: AtomicUsize::new(0),
    })
}

#[test]
fn native_channels_follow_compiled_continuity_and_trace_does_not_predict_again() {
    let model = native_model();
    let from = native_value(&model, [u32::MAX - 4, 5]);
    let to = native_value(&model, [u32::MAX, 8]);
    for scale in [false, true] {
        let expression = if scale {
            Arc::new(DynamicSampleExpression::Scale {
                address: Arc::new(
                    DynamicValueAddress::whole_family(ProgrammingOwner::Color, &to).unwrap(),
                ),
                base: DynamicValue::Family(from.clone()),
                value: leaf(ProgrammingOwner::Color, to.clone(), 2),
                factor: 1.5,
                baseline_occurrence: Some(occurrence(1)),
            })
        } else {
            transition(
                leaf(ProgrammingOwner::Color, from.clone(), 1),
                leaf(ProgrammingOwner::Color, to.clone(), 2),
                0.5,
            )
        };
        let compiled = Arc::new(
            CompiledProgrammingFamilyExpression::new(
                expression,
                ProgrammingOwner::Color,
                None,
                Some(&NativeModels(model.clone())),
            )
            .unwrap(),
        );
        model.predictions.store(0, Ordering::Relaxed);
        let sample = FamilyCompositionSample::WholeExpression {
            expression: compiled,
            rank: color_sample(None, DynamicValue::Family(semantic()), 20, 1.0).rank,
            activation_mix: 1.0,
        };
        let (value, scratch) = compose_trace(ProgrammingOwner::Color, &from, &[sample], &NoFrame);
        let AttributeValue::ColorProgram(program) = value else {
            panic!()
        };
        let ColorProgram::Direct { recipe, .. } = program.as_ref() else {
            panic!()
        };
        assert_eq!(
            recipe.channels[0].raw,
            if scale { u32::MAX } else { u32::MAX - 2 }
        );
        assert_eq!(recipe.channels[1].raw, if scale { 8 } else { 5 });
        assert_eq!(
            ids(
                &scratch,
                F::NativeColorChannel(model.bindings[0].channel_id)
            ),
            vec![occurrence(1), occurrence(2)]
        );
        assert_eq!(
            ids(
                &scratch,
                F::NativeColorChannel(model.bindings[1].channel_id)
            ),
            vec![occurrence(if scale { 2 } else { 1 })]
        );
        assert_eq!(ids(&scratch, F::NativeColorIdentity), vec![occurrence(1)]);
        assert_eq!(
            ids(&scratch, F::NativePrediction),
            vec![occurrence(1), occurrence(2)]
        );
        assert_eq!(
            model.predictions.load(Ordering::Relaxed),
            1,
            "trace and all field queries must share the single value prediction"
        );
    }
}

#[test]
fn native_component_predicts_once_and_keeps_raw_channel_evidence_without_guessing_prediction_inputs()
 {
    let model = native_model();
    let base = native_value(&model, [100, 5]);
    let lower_rank = color_sample(None, DynamicValue::Family(semantic()), 1, 1.0).rank;
    let upper_rank = color_sample(None, DynamicValue::Family(semantic()), 2, 1.0).rank;
    let lower = FamilySample::new(
        Arc::new(
            CompiledDynamicValueAddress::new(
                DynamicValueAddress::whole_family(ProgrammingOwner::Color, &base).unwrap(),
                Some(model.clone()),
            )
            .unwrap(),
        ),
        DynamicValue::Family(base.clone()),
        lower_rank,
        1.0,
    )
    .unwrap();
    let upper = FamilySample::new(
        Arc::new(
            CompiledDynamicValueAddress::new(
                DynamicValueAddress {
                    representation: DynamicFamilyRepresentation::DirectColor {
                        source: model.source.clone(),
                    },
                    component: Some(ProgrammingComponent::NativeColor(model.bindings[0])),
                },
                Some(model.clone()),
            )
            .unwrap(),
        ),
        DynamicValue::Native(200),
        upper_rank,
        1.0,
    )
    .unwrap();
    model.predictions.store(0, Ordering::Relaxed);
    let (value, scratch) = compose_trace(
        ProgrammingOwner::Color,
        &base,
        &[lower.into(), upper.into()],
        &NoFrame,
    );
    let AttributeValue::ColorProgram(program) = value else {
        panic!()
    };
    let ColorProgram::Direct { recipe, .. } = program.as_ref() else {
        panic!()
    };
    assert_eq!(recipe.channels[0].raw, 200);
    assert_eq!(recipe.channels[1].raw, 5);
    assert_eq!(query(&scratch, F::NativePrediction), None);
    let edited = query(
        &scratch,
        F::NativeColorChannel(model.bindings[0].channel_id),
    )
    .unwrap();
    assert_eq!(edited.len(), 1);
    assert_eq!(edited[0].rank, upper_rank);
    let untouched = query(
        &scratch,
        F::NativeColorChannel(model.bindings[1].channel_id),
    )
    .unwrap();
    assert_eq!(untouched.len(), 1);
    assert_eq!(untouched[0].rank, lower_rank);
    assert_eq!(model.predictions.load(Ordering::Relaxed), 1);
}

fn occurrence(id: u128) -> DynamicSourceOccurrenceId {
    DynamicSourceOccurrenceId::new(Uuid::from_u128(id)).unwrap()
}

fn leaf(owner: ProgrammingOwner, value: AttributeValue, id: u128) -> Arc<DynamicSampleExpression> {
    Arc::new(DynamicSampleExpression::Programming {
        address: Arc::new(DynamicValueAddress::whole_family(owner, &value).unwrap()),
        value: DynamicValue::Family(value),
        occurrence: Some(occurrence(id)),
        dependency_occurrence: None,
    })
}

fn expression_sample(
    owner: ProgrammingOwner,
    expression: Arc<DynamicSampleExpression>,
    mix: f32,
) -> FamilyCompositionSample {
    FamilyCompositionSample::WholeExpression {
        expression: Arc::new(
            CompiledProgrammingFamilyExpression::new(expression, owner, None, None).unwrap(),
        ),
        rank: color_sample(None, DynamicValue::Family(semantic()), 20, 1.0).rank,
        activation_mix: mix,
    }
}

fn transition(
    from: Arc<DynamicSampleExpression>,
    to: Arc<DynamicSampleExpression>,
    progress: f32,
) -> Arc<DynamicSampleExpression> {
    Arc::new(DynamicSampleExpression::Transition {
        from: Some(from),
        to: Some(to),
        progress,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(9001),
        },
    })
}

fn compose_trace(
    owner: ProgrammingOwner,
    base: &AttributeValue,
    samples: &[FamilyCompositionSample],
    frame: &dyn WholeFamilyExpressionFrameResolver,
) -> (AttributeValue, RetainedFamilyCompositionScratch) {
    let mut scratch = RetainedFamilyCompositionScratch::default();
    let value = compose_retained_dynamic_family_traced(
        owner,
        base,
        samples,
        &FamilyCompositionContext {
            edit: FamilyEditContext {
                color_model: Some(&VirtualColorAuthoringV1),
                ..Default::default()
            },
            ..Default::default()
        },
        frame,
        &mut scratch,
    )
    .unwrap();
    (value, scratch)
}

fn query(scratch: &RetainedFamilyCompositionScratch, field: F) -> Option<Vec<FamilyTraceSource>> {
    let trace = scratch.family_trace();
    trace.sources_for_field(trace.root().unwrap(), field)
}

fn ids(scratch: &RetainedFamilyCompositionScratch, field: F) -> Vec<DynamicSourceOccurrenceId> {
    let mut ids = query(scratch, field)
        .unwrap()
        .into_iter()
        .map(|source| source.occurrence.unwrap())
        .collect::<Vec<_>>();
    ids.sort();
    ids
}

fn semantic_value(uv: f32, allocation: ColorAllocation) -> AttributeValue {
    AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
        intent: ColorIntent {
            uv: UvIntent { amount: uv },
            allocation,
            ..Default::default()
        },
    }))
}

#[test]
fn whole_color_transitions_keep_held_fields_outgoing_and_exact_endpoints_exclusive() {
    for equal in [false, true] {
        let from = semantic_value(0.0, ColorAllocation::PreserveRecipe);
        let to = if equal {
            from.clone()
        } else {
            semantic_value(1.0, ColorAllocation::PreferWhite)
        };
        for progress in [0.0, 0.5, 1.0] {
            let sample = expression_sample(
                ProgrammingOwner::Color,
                transition(
                    leaf(ProgrammingOwner::Color, from.clone(), 1),
                    leaf(ProgrammingOwner::Color, to.clone(), 2),
                    progress,
                ),
                1.0,
            );
            let (actual, scratch) =
                compose_trace(ProgrammingOwner::Color, &semantic(), &[sample], &NoFrame);
            assert_eq!(
                actual,
                CompiledProgrammingTransition::new(from.clone(), to.clone(), None)
                    .unwrap()
                    .sample(progress)
                    .unwrap()
            );
            let endpoint = if progress == 1.0 { 2 } else { 1 };
            for field in [F::Allocation, F::ColorWheels, F::ColorWheel(7)] {
                assert_eq!(ids(&scratch, field), vec![occurrence(endpoint)]);
            }
            assert_eq!(
                ids(&scratch, F::Uv),
                if progress == 0.5 {
                    vec![occurrence(1), occurrence(2)]
                } else {
                    vec![occurrence(endpoint)]
                }
            );
        }
    }
}

#[test]
fn whole_size_uses_baseline_held_fields_below_one_and_target_held_fields_above_one() {
    let from = semantic_value(0.25, ColorAllocation::PreserveRecipe);
    let to = semantic_value(0.5, ColorAllocation::PreferWhite);
    for factor in [0.0, 0.5, 1.0, 1.5] {
        let expression = Arc::new(DynamicSampleExpression::Scale {
            address: Arc::new(
                DynamicValueAddress::whole_family(ProgrammingOwner::Color, &to).unwrap(),
            ),
            base: DynamicValue::Family(from.clone()),
            value: leaf(ProgrammingOwner::Color, to.clone(), 2),
            factor,
            baseline_occurrence: Some(occurrence(1)),
        });
        let sample = expression_sample(ProgrammingOwner::Color, expression, 1.0);
        let (actual, scratch) =
            compose_trace(ProgrammingOwner::Color, &semantic(), &[sample], &NoFrame);
        assert_eq!(
            actual,
            CompiledProgrammingTransition::new(from.clone(), to.clone(), None)
                .unwrap()
                .scale(factor)
                .unwrap()
        );
        for field in [F::Allocation, F::ColorWheel(5)] {
            let sources = query(&scratch, field).unwrap();
            assert_eq!(sources.len(), 1);
            assert_eq!(
                sources[0].occurrence,
                Some(occurrence(if factor < 1.0 { 1 } else { 2 }))
            );
            assert_eq!(
                sources[0].role,
                if factor < 1.0 {
                    FamilyTraceRole::CalculationDependency
                } else {
                    FamilyTraceRole::Authored
                }
            );
        }
        assert_eq!(
            ids(&scratch, F::Uv),
            match factor {
                0.0 => vec![occurrence(1)],
                1.0 => vec![occurrence(2)],
                _ => vec![occurrence(1), occurrence(2)],
            }
        );
    }
}

#[test]
fn known_and_expression_activation_use_the_same_field_transfer_as_the_value() {
    let from = semantic_value(0.0, ColorAllocation::PreserveRecipe);
    let to = semantic_value(1.0, ColorAllocation::PreferWhite);
    let lower = color_sample(None, DynamicValue::Family(from.clone()), 1, 1.0);
    for incoming in [
        color_sample(None, DynamicValue::Family(to.clone()), 20, 0.5).into(),
        expression_sample(
            ProgrammingOwner::Color,
            leaf(ProgrammingOwner::Color, to.clone(), 2),
            0.5,
        ),
    ] {
        let (actual, scratch) = compose_trace(
            ProgrammingOwner::Color,
            &semantic(),
            &[lower.clone().into(), incoming],
            &NoFrame,
        );
        assert_eq!(
            actual,
            CompiledProgrammingTransition::new(from.clone(), to.clone(), None)
                .unwrap()
                .sample(0.5)
                .unwrap()
        );
        for field in [F::Allocation, F::ColorWheel(1)] {
            let sources = query(&scratch, field).unwrap();
            assert_eq!(sources.len(), 1);
            assert_eq!(sources[0].rank, lower.rank);
        }
        assert_eq!(query(&scratch, F::Uv).unwrap().len(), 2);
    }
}

#[test]
fn amber_and_rgb_history_survive_xyz_to_rgb_reconstruction_without_authored_generated_amber() {
    let amber = color_sample(
        Some(ColorComponent::Amber),
        DynamicValue::Scalar(0.7),
        1,
        1.0,
    );
    let red = color_sample(Some(ColorComponent::Red), DynamicValue::Scalar(0.2), 2, 1.0);
    let incoming = color_sample(None, DynamicValue::Family(semantic()), 20, 0.5);
    let (value, scratch) = compose_trace(
        ProgrammingOwner::Color,
        &semantic(),
        &[amber.clone().into(), red.clone().into(), incoming.into()],
        &NoFrame,
    );
    let AttributeValue::ColorProgram(program) = value else {
        panic!()
    };
    let ColorProgram::Semantic { intent } = program.as_ref() else {
        panic!()
    };
    assert_eq!(intent.recipe.amber, 0.0);
    assert_eq!(query(&scratch, F::ColorRecipeAmber), Some(vec![]));
    let trace = scratch.family_trace();
    let entries = trace
        .query_fields(
            trace.root().unwrap(),
            &S::new([F::ColorRecipeRed, F::ColorRecipeGreen]),
        )
        .unwrap();
    let prior_amber = entries
        .iter()
        .find(|entry| entry.source.rank == amber.rank)
        .expect("authored Amber contributes through XYZ");
    assert_eq!(
        prior_amber.source.footprint,
        FamilyTraceFootprint::Component(ProgrammingComponent::Color(ColorComponent::Amber))
    );
    assert_eq!(prior_amber.source.role, FamilyTraceRole::Authored);
    assert_eq!(prior_amber.fields, S::new([F::ColorRecipeAmber]));
    let prior_red = entries
        .iter()
        .find(|entry| entry.source.rank == red.rank)
        .unwrap();
    assert_eq!(
        prior_red.source.footprint,
        FamilyTraceFootprint::Component(ProgrammingComponent::Color(ColorComponent::Red))
    );
    assert_eq!(prior_red.fields, S::new([F::ColorXyz]));
}

#[test]
fn detailed_queries_keep_transformed_leaf_fields_and_rebase_mapped_nodes() {
    let mut inner = FamilyTraceArena::default();
    let base = inner.base();
    let other = inner.source(source(Uuid::new_v4(), FamilyTraceFootprint::Whole));
    let mapped = inner.mapped_blend(
        base,
        other,
        Some(ProgrammingTransitionTrace {
            from: ProgrammingFieldTransfer {
                identity: S::empty(),
                remap: vec![
                    (F::ColorXyz, F::ColorRecipeRed),
                    (F::ColorXyz, F::ColorRecipeGreen),
                ]
                .into(),
            },
            to: Default::default(),
        }),
    );
    let mut outer = FamilyTraceArena::default();
    let mut original = source(
        Uuid::new_v4(),
        FamilyTraceFootprint::Component(ProgrammingComponent::Color(ColorComponent::Amber)),
    );
    original.occurrence = Some(occurrence(33));
    let original_id = outer.source(original);
    let root = outer.append_graph_rebased(&inner, mapped, original_id);
    assert_eq!(
        outer.query_fields(root, &S::new([F::ColorRecipeRed, F::ColorRecipeGreen])),
        Some(vec![FamilyTraceContribution {
            source: original,
            fields: S::new([F::ColorXyz])
        }])
    );
    assert_eq!(
        outer.sources_for_field(root, F::ColorRecipeAmber),
        Some(vec![])
    );
}

#[test]
fn unknown_transfer_and_unrepresentable_wheel_complements_never_become_known_empty() {
    let mut arena = FamilyTraceArena::default();
    let prior = arena.source(source(Uuid::new_v4(), FamilyTraceFootprint::Whole));
    let incoming_source = source(
        Uuid::new_v4(),
        FamilyTraceFootprint::Component(ProgrammingComponent::Color(ColorComponent::Red)),
    );
    let incoming = arena.source(incoming_source);
    let unknown = arena.mapped_blend(prior, incoming, None);
    assert_eq!(arena.sources_for_field(unknown, F::ColorRecipeRed), None);
    assert_eq!(arena.query_fields(unknown, &S::empty()), Some(vec![]));
    let written = arena.write(unknown, incoming, incoming_source.footprint, false);
    assert_eq!(
        arena.sources_for_field(written, F::ColorRecipeRed),
        Some(vec![incoming_source])
    );
    assert_eq!(arena.sources_for_field(written, F::ColorRecipeGreen), None);
    let wheel = source(
        Uuid::new_v4(),
        FamilyTraceFootprint::Component(ProgrammingComponent::ColorWheel(2)),
    );
    let wheel_id = arena.source(wheel);
    let written = arena.write(prior, wheel_id, wheel.footprint, false);
    assert_eq!(arena.sources_for_field(written, F::ColorWheels), None);
    assert_eq!(
        arena.sources_for_field(written, F::ColorWheel(2)),
        Some(vec![wheel])
    );
    assert_eq!(
        arena
            .sources_for_field(written, F::ColorWheel(3))
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn hue_and_saturation_read_recipe_and_keep_prior_rgb_as_calculation_dependency() {
    let mut arena = FamilyTraceArena::default();
    let prior = source(
        Uuid::new_v4(),
        FamilyTraceFootprint::Component(ProgrammingComponent::Color(ColorComponent::Red)),
    );
    let prior_id = arena.source(prior);
    let unrelated = arena.source(source(Uuid::new_v4(), FamilyTraceFootprint::Whole));
    // This value has independently sourced XYZ and recipe. HSV reads only the recipe.
    let actual = arena.mapped_blend(
        prior_id,
        unrelated,
        Some(ProgrammingTransitionTrace {
            from: ProgrammingFieldTransfer {
                identity: S::new([F::ColorRecipeRed]),
                ..Default::default()
            },
            to: ProgrammingFieldTransfer {
                identity: S::new([F::ColorXyz]),
                ..Default::default()
            },
        }),
    );
    for component in [ColorComponent::Hue, ColorComponent::Saturation] {
        assert_eq!(
            arena.sources_for_component(actual, ProgrammingComponent::Color(component)),
            Some(vec![prior])
        );
        let incoming = source(
            Uuid::new_v4(),
            FamilyTraceFootprint::Component(ProgrammingComponent::Color(component)),
        );
        let incoming_id = arena.source(incoming);
        let written = arena.write(actual, incoming_id, incoming.footprint, false);
        let sources = arena
            .sources_for_field(written, F::ColorRecipeGreen)
            .unwrap();
        assert!(sources.contains(&incoming));
        assert!(sources.contains(&FamilyTraceSource {
            role: FamilyTraceRole::CalculationDependency,
            ..prior
        }));
        assert_eq!(sources.len(), 2);
    }
}

struct GeometryFrame {
    calls: Cell<usize>,
}
impl WholeFamilyExpressionFrameResolver for GeometryFrame {
    fn resolve(
        &self,
        requirement: TransitionRequirement,
        _: &AttributeValue,
        _: &AttributeValue,
        _: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        assert_eq!(requirement, TransitionRequirement::LiveTargetPoints);
        self.calls.set(self.calls.get() + 1);
        Ok(AttributeValue::Position(Arc::new(PositionIntent::angles(
            40.0, 20.0,
        ))))
    }
}

struct TracedGeometryFrame(GeometryFrame);
impl WholeFamilyExpressionFrameResolver for TracedGeometryFrame {
    fn resolve(
        &self,
        requirement: TransitionRequirement,
        from: &AttributeValue,
        to: &AttributeValue,
        operation: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        self.0.resolve(requirement, from, to, operation)
    }
    fn resolve_with_trace(
        &self,
        requirement: TransitionRequirement,
        from: &AttributeValue,
        to: &AttributeValue,
        operation: FamilyExpressionOperation,
    ) -> Result<(AttributeValue, Option<ProgrammingTransitionTrace>), TransitionError> {
        let value = self.resolve(requirement, from, to, operation)?;
        let transfer = ProgrammingFieldTransfer {
            identity: S::empty(),
            remap: vec![(F::TargetX, F::Pan), (F::TargetY, F::Tilt)].into(),
        };
        Ok((
            value,
            Some(ProgrammingTransitionTrace {
                from: transfer.clone(),
                to: transfer,
            }),
        ))
    }
}

#[test]
fn live_frame_conversion_solves_once_and_requires_explicit_field_transfer() {
    let target = |id| {
        AttributeValue::Position(Arc::new(PositionIntent::target(
            TargetReference::Point {
                point_id: Uuid::from_u128(id),
            },
            [0.0; 3],
        )))
    };
    let sample = expression_sample(
        ProgrammingOwner::Position,
        transition(
            leaf(ProgrammingOwner::Position, target(50), 1),
            leaf(ProgrammingOwner::Position, target(51), 2),
            0.5,
        ),
        1.0,
    );
    let base = AttributeValue::Position(Arc::new(PositionIntent::angles(0.0, 0.0)));
    let no_trace = GeometryFrame {
        calls: Cell::new(0),
    };
    let (unknown_value, unknown) = compose_trace(
        ProgrammingOwner::Position,
        &base,
        &[sample.clone()],
        &no_trace,
    );
    assert_eq!(no_trace.calls.get(), 1);
    assert_eq!(query(&unknown, F::Pan), None);
    let traced = TracedGeometryFrame(GeometryFrame {
        calls: Cell::new(0),
    });
    let (known_value, known) = compose_trace(ProgrammingOwner::Position, &base, &[sample], &traced);
    assert_eq!(traced.0.calls.get(), 1);
    assert_eq!(known_value, unknown_value);
    let trace = known.family_trace();
    let entries = trace
        .query_fields(trace.root().unwrap(), &S::new([F::Pan]))
        .unwrap();
    assert_eq!(entries.len(), 2);
    for entry in entries {
        assert_eq!(entry.fields, S::new([F::TargetX]));
        assert_eq!(entry.source.footprint, FamilyTraceFootprint::Whole);
        assert_eq!(entry.source.role, FamilyTraceRole::Authored);
    }
}

#[test]
fn component_adoption_keeps_the_value_but_unknown_fields_without_solving_again() {
    let base = AttributeValue::Position(Arc::new(PositionIntent::angles(15.0, 30.0)));
    for mix in [0.5, 1.0] {
        let calls = Cell::new(0);
        let resolver = |value: &AttributeValue, _: &DynamicValueAddress| {
            assert_eq!(value, &base);
            calls.set(calls.get() + 1);
            Ok(AttributeValue::Position(Arc::new(PositionIntent::target(
                TargetReference::Origin,
                [10.0, 20.0, 30.0],
            ))))
        };
        let mut original = source(
            Uuid::from_u128(501),
            FamilyTraceFootprint::Component(ProgrammingComponent::TargetX),
        );
        original.occurrence = Some(occurrence(502));
        let sample = FamilySample::new(
            Arc::new(
                CompiledDynamicValueAddress::new(
                    DynamicValueAddress {
                        representation: DynamicFamilyRepresentation::Target {
                            reference: Some(TargetReference::Origin),
                        },
                        component: Some(ProgrammingComponent::TargetX),
                    },
                    None,
                )
                .unwrap(),
            ),
            DynamicValue::Scalar(42.0),
            original.rank,
            mix,
        )
        .unwrap()
        .with_trace_sources(Arc::from([original]));
        let mut scratch = RetainedFamilyCompositionScratch::default();
        let value = compose_retained_dynamic_family_traced(
            ProgrammingOwner::Position,
            &base,
            &[sample.into()],
            &FamilyCompositionContext {
                resolve_adoption: Some(&resolver),
                ..Default::default()
            },
            &NoFrame,
            &mut scratch,
        )
        .unwrap();
        assert_eq!(
            value,
            AttributeValue::Position(Arc::new(PositionIntent::target(
                TargetReference::Origin,
                [if mix == 1.0 { 42.0 } else { 26.0 }, 20.0, 30.0],
            )))
        );
        assert_eq!(query(&scratch, F::TargetY), None);
        assert_eq!(query(&scratch, F::TargetReference), None);
        assert_eq!(
            query(&scratch, F::TargetX),
            if mix == 1.0 {
                Some(vec![original])
            } else {
                None
            }
        );
        assert_eq!(
            calls.get(),
            1,
            "trace must share the successful adoption solve"
        );
    }
}
