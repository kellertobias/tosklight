use super::*;
use crate::DynamicTransitionReason;
use light_core::{NativeColorBinding, NativeColorValue, PhysicalDataQuality};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    sync::atomic::{AtomicUsize, Ordering},
};
use uuid::Uuid;

fn retained_history(
    expression: Arc<DynamicSampleExpression>,
    steps: usize,
) -> Arc<DynamicSampleExpression> {
    let mut tape = RetainedExpressionTape::from_roots(&[expression]).unwrap();
    let mut root = tape.roots()[0];
    for step in 0..steps {
        root = tape
            .append_resume(
                Some(root),
                Some(root),
                0.5,
                Uuid::from_u128(20_000 + step as u128),
            )
            .unwrap();
    }
    tape.replace_root(0, root).unwrap();
    Arc::new(DynamicSampleExpression::Retained {
        tape: Arc::new(tape),
        root,
    })
}

#[test]
fn deep_coupled_history_keeps_branch_local_uv_and_exact_component_footprints() {
    let expression = retained_history(
        resume(
            Some(whole(ProgrammingOwner::Color, semantic(0.8))),
            Some(component(ColorComponent::Uv, 0.2)),
            0.5,
        ),
        128,
    );
    let compiled = CompiledCoupledExpression::new(expression.clone(), None).unwrap();
    assert!(Arc::ptr_eq(compiled.expression().unwrap(), &expression));
    let mut context = color_context();
    let uv = compiled
        .evaluate_orthogonal(ColorComponent::Uv, &context, &UnavailableFrame)
        .unwrap()
        .unwrap();
    assert!((uv - 0.5).abs() < 0.00001);
    context.older_uv = Some(0.6);
    let uv = compiled
        .evaluate_orthogonal(ColorComponent::Uv, &context, &UnavailableFrame)
        .unwrap()
        .unwrap();
    assert!((uv - 0.4).abs() < 0.00001);
    let exact = CompiledCoupledExpression::new(
        resume(
            Some(expression),
            Some(component(ColorComponent::Red, 0.2)),
            1.0,
        ),
        None,
    )
    .unwrap();
    let CoupledExpressionFootprint::Exact { address, .. } =
        exact.footprint(CoupledExpressionRole::Base)
    else {
        panic!()
    };
    assert_eq!(
        address.address().component,
        Some(ProgrammingComponent::Color(ColorComponent::Red))
    );
    assert!(matches!(
        exact.footprint(CoupledExpressionRole::ColorOrthogonal(ColorComponent::Uv)),
        CoupledExpressionFootprint::Inactive
    ));
}

#[test]
fn deep_coupled_point_history_materializes_each_original_cohort_once_per_frame() {
    let refs = [Uuid::from_u128(101), Uuid::from_u128(102)];
    let endpoint = |index: usize, offset| {
        leaf(
            DynamicValueAddress {
                representation: DynamicFamilyRepresentation::Target {
                    reference: Some(TargetReference::Point {
                        point_id: refs[index],
                    }),
                },
                component: Some(ProgrammingComponent::TargetX),
            },
            DynamicValue::Scalar(offset),
        )
    };
    let compiled = CompiledCoupledExpression::new(
        retained_history(
            resume(Some(endpoint(0, 1.0)), Some(endpoint(1, 2.0)), 0.5),
            128,
        ),
        None,
    )
    .unwrap();
    assert_eq!(compiled.base_endpoints().len(), 2);
    let context = TargetContext {
        calls: RefCell::new(vec![]),
    };
    let frame = TargetFrame {
        points: RefCell::new(HashMap::from([(refs[0], 10.0), (refs[1], 100.0)])),
        calls: Cell::new(0),
    };
    assert_eq!(
        compiled.evaluate_base(&context, &frame).unwrap(),
        Some(AttributeValue::Position(Arc::new(PositionIntent::angles(
            56.5, 0.0
        ))))
    );
    frame.points.borrow_mut().insert(refs[1], 200.0);
    assert_eq!(
        compiled.evaluate_base(&context, &frame).unwrap(),
        Some(AttributeValue::Position(Arc::new(PositionIntent::angles(
            106.5, 0.0
        ))))
    );
    assert_eq!(frame.calls.get(), 2);
    assert_eq!(
        context.calls.borrow().len(),
        4,
        "shared histories do not duplicate endpoint cohort evaluation"
    );
}

#[test]
fn deep_coupled_native_history_keeps_bits_source_requirements_and_pruning() {
    let first = model(301);
    let second = model(302);
    let models = Models {
        entries: vec![first.clone(), second.clone()],
        calls: AtomicUsize::new(0),
    };
    let expression = retained_history(
        resume(
            Some(native_leaf(&first, 10, u32::MAX - 2)),
            Some(native_leaf(&first, 10, u32::MAX)),
            0.5,
        ),
        128,
    );
    let compiled = CompiledCoupledExpression::new(expression, Some(&models)).unwrap();
    assert_eq!(models.calls.load(Ordering::Relaxed), 1);
    let Some(AttributeValue::ColorProgram(value)) = compiled
        .evaluate_base(&NativeContext, &UnavailableFrame)
        .unwrap()
    else {
        panic!()
    };
    let ColorProgram::Direct { recipe, portable } = value.as_ref() else {
        panic!()
    };
    assert_eq!(recipe.channels[0].raw, u32::MAX - 1);
    assert_eq!(recipe.source, first.source);
    assert!(
        portable.uv.is_none(),
        "unknown UV must not become measured zero"
    );
    let pending = retained_history(
        resume(
            Some(native_leaf(&first, 10, 3)),
            Some(native_leaf(&second, 10, 7)),
            0.5,
        ),
        128,
    );
    let compiled = CompiledCoupledExpression::new(pending.clone(), Some(&models)).unwrap();
    assert!(matches!(
        compiled.evaluate_base(&NativeContext, &UnavailableFrame),
        Err(TransitionError::Requires(
            TransitionRequirement::ColorAppearance
        ))
    ));
    let pruned = CompiledCoupledExpression::new(
        resume(
            Some(pending),
            Some(component(ColorComponent::Red, 0.2)),
            1.0,
        ),
        None,
    )
    .unwrap();
    assert_eq!(pruned.base_endpoints().len(), 1);
    assert!(matches!(
        pruned.footprint(CoupledExpressionRole::Base),
        CoupledExpressionFootprint::Exact { .. }
    ));
}

fn leaf(address: DynamicValueAddress, value: DynamicValue) -> Arc<DynamicSampleExpression> {
    Arc::new(DynamicSampleExpression::Programming {
        address: Arc::new(address),
        value,
        occurrence: None,
        dependency_occurrence: None,
    })
}
fn whole(owner: ProgrammingOwner, value: AttributeValue) -> Arc<DynamicSampleExpression> {
    leaf(
        DynamicValueAddress::whole_family(owner, &value).unwrap(),
        DynamicValue::Family(value),
    )
}
fn component(component: ColorComponent, value: f32) -> Arc<DynamicSampleExpression> {
    let basis = match component {
        ColorComponent::Red
        | ColorComponent::Green
        | ColorComponent::Blue
        | ColorComponent::Amber => DynamicSemanticColorBasis::Recipe,
        ColorComponent::Hue | ColorComponent::Saturation => {
            DynamicSemanticColorBasis::HueSaturation
        }
        _ => DynamicSemanticColorBasis::Retain,
    };
    leaf(
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::SemanticColor { basis },
            component: Some(ProgrammingComponent::Color(component)),
        },
        DynamicValue::Scalar(value),
    )
}
fn resume(
    from: Option<Arc<DynamicSampleExpression>>,
    to: Option<Arc<DynamicSampleExpression>>,
    progress: f32,
) -> Arc<DynamicSampleExpression> {
    Arc::new(DynamicSampleExpression::Transition {
        from,
        to,
        progress,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::new_v4(),
        },
    })
}
fn semantic(uv: f32) -> AttributeValue {
    let mut intent = ColorIntent::default();
    intent.uv.amount = uv;
    AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic { intent }))
}
fn intent(value: &AttributeValue) -> &ColorIntent {
    let AttributeValue::ColorProgram(color) = value else {
        panic!()
    };
    let ColorProgram::Semantic { intent } = color.as_ref() else {
        panic!()
    };
    intent
}
fn edit(value: &AttributeValue, component: ProgrammingComponent, sample: f32) -> AttributeValue {
    edit_family(
        value,
        &[ComponentEdit::Scalar {
            component,
            operation: ScalarEdit::Set(ScalarIntent::Value(sample)),
        }],
        &FamilyEditContext {
            color_model: Some(&VirtualColorAuthoringV1),
            ..Default::default()
        },
    )
    .unwrap()
}
struct UnavailableFrame;
impl WholeFamilyExpressionFrameResolver for UnavailableFrame {
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
struct ColorContext {
    recipe: AttributeValue,
    hue: AttributeValue,
    global: AttributeValue,
    older_uv: Option<f32>,
    calls: RefCell<Vec<Option<DynamicValueAddress>>>,
}
impl CoupledExpressionContext for ColorContext {
    fn materialize_base(
        &self,
        endpoint: Option<(&CompiledDynamicValueAddress, &DynamicValue)>,
    ) -> Result<AttributeValue, TransitionError> {
        self.calls
            .borrow_mut()
            .push(endpoint.map(|(address, _)| address.address().clone()));
        let Some((address, DynamicValue::Scalar(value))) = endpoint else {
            return Ok(self.global.clone());
        };
        let base = match address.address().representation {
            DynamicFamilyRepresentation::SemanticColor {
                basis: DynamicSemanticColorBasis::Recipe,
            } => &self.recipe,
            DynamicFamilyRepresentation::SemanticColor {
                basis: DynamicSemanticColorBasis::HueSaturation,
            } => &self.hue,
            _ => panic!("endpoint must select its own compatible cohort"),
        };
        Ok(edit(base, address.address().component.unwrap(), *value))
    }
    fn orthogonal_underlay(
        &self,
        component: ColorComponent,
        branch: &AttributeValue,
    ) -> Result<f32, TransitionError> {
        if component == ColorComponent::Uv
            && let Some(value) = self.older_uv
        {
            return Ok(value);
        }
        Ok(read_color_component(
            intent(branch),
            component,
            &FamilyEditContext::default(),
        )?)
    }
}
fn color_context() -> ColorContext {
    ColorContext {
        recipe: semantic(0.0),
        hue: semantic(0.0),
        global: semantic(0.0),
        older_uv: None,
        calls: RefCell::new(vec![]),
    }
}

#[test]
fn recipe_to_hue_asks_for_each_compatible_cohort_before_edit_and_interpolates_the_results() {
    let expression = resume(
        Some(component(ColorComponent::Red, 0.2)),
        Some(component(ColorComponent::Hue, 120.0)),
        0.5,
    );
    let compiled = CompiledCoupledExpression::new(expression.clone(), None).unwrap();
    let mut context = color_context();
    context.recipe = edit(
        &context.recipe,
        ProgrammingComponent::Color(ColorComponent::Green),
        0.7,
    );
    context.hue = edit(
        &context.hue,
        ProgrammingComponent::Color(ColorComponent::Saturation),
        0.8,
    );
    context.global = edit(
        &context.global,
        ProgrammingComponent::Color(ColorComponent::Green),
        0.01,
    );
    let from = edit(
        &context.recipe,
        ProgrammingComponent::Color(ColorComponent::Red),
        0.2,
    );
    let to = edit(
        &context.hue,
        ProgrammingComponent::Color(ColorComponent::Hue),
        120.0,
    );
    let expected = CompiledProgrammingTransition::new(from, to, None)
        .unwrap()
        .sample(0.5)
        .unwrap();
    assert_eq!(
        compiled.evaluate_base(&context, &UnavailableFrame).unwrap(),
        Some(expected)
    );
    assert!(Arc::ptr_eq(compiled.expression().unwrap(), &expression));
    assert_eq!(compiled.base_endpoints().len(), 2);
    assert!(!compiled.needs_base_underlay());
    assert_eq!(compiled.roles(), &[CoupledExpressionRole::Base]);
    assert!(matches!(
        compiled.footprint(CoupledExpressionRole::Base),
        CoupledExpressionFootprint::Coupled
    ));
    let calls = context.calls.borrow();
    assert_eq!(calls.len(), 2);
    assert!(
        calls.iter().all(Option::is_some),
        "global winner must not replace endpoint-specific cohorts"
    );
}

#[test]
fn whole_to_uv_uses_branch_local_defaults_and_older_explicit_uv_without_double_fading() {
    let compiled = CompiledCoupledExpression::new(
        resume(
            Some(whole(ProgrammingOwner::Color, semantic(0.8))),
            Some(component(ColorComponent::Uv, 0.2)),
            0.5,
        ),
        None,
    )
    .unwrap();
    let mut context = color_context();
    assert!(compiled.needs_base_underlay());
    assert_eq!(
        compiled.roles(),
        &[
            CoupledExpressionRole::Base,
            CoupledExpressionRole::ColorOrthogonal(ColorComponent::Uv)
        ]
    );
    let base = compiled
        .evaluate_base(&context, &UnavailableFrame)
        .unwrap()
        .unwrap();
    assert!(
        (intent(&base).uv.amount - 0.4).abs() < 0.00001,
        "base carries implicit defaults only"
    );
    let uv = compiled
        .evaluate_orthogonal(ColorComponent::Uv, &context, &UnavailableFrame)
        .unwrap()
        .unwrap();
    assert!(
        (uv - 0.5).abs() < 0.00001,
        "do not mix explicit UV against already blended implicit .4"
    );
    context.older_uv = Some(0.6);
    assert!(
        (compiled
            .evaluate_orthogonal(ColorComponent::Uv, &context, &UnavailableFrame)
            .unwrap()
            .unwrap()
            - 0.4)
            .abs()
            < 0.00001
    );
    assert_eq!(
        compiled
            .evaluate_orthogonal(ColorComponent::WhiteBlend, &context, &UnavailableFrame)
            .unwrap(),
        None
    );
}

#[test]
fn exact_whole_to_component_retains_the_narrow_address_and_exact_release_has_no_roles() {
    let old = whole(ProgrammingOwner::Color, semantic(0.8));
    let new = component(ColorComponent::Red, 0.2);
    let expression = resume(Some(old.clone()), Some(new.clone()), 1.0);
    let compiled = CompiledCoupledExpression::new(expression, None).unwrap();
    let CoupledExpressionFootprint::Exact { address, value } =
        compiled.footprint(CoupledExpressionRole::Base)
    else {
        panic!("exact component mask")
    };
    assert_eq!(
        address.address().component,
        Some(ProgrammingComponent::Color(ColorComponent::Red))
    );
    assert_eq!(value, &DynamicValue::Scalar(0.2));
    assert_eq!(compiled.base_endpoints().len(), 1);
    let explicit = CompiledCoupledExpression::new(
        resume(
            Some(old.clone()),
            Some(component(ColorComponent::Uv, 0.2)),
            1.0,
        ),
        None,
    )
    .unwrap();
    assert!(matches!(
        explicit.footprint(CoupledExpressionRole::Base),
        CoupledExpressionFootprint::Inactive
    ));
    assert!(matches!(
        explicit.footprint(CoupledExpressionRole::ColorOrthogonal(ColorComponent::Uv)),
        CoupledExpressionFootprint::Exact { .. }
    ));
    let released = CompiledCoupledExpression::new(resume(Some(old), None, 1.0), None).unwrap();
    assert!(released.roles().is_empty());
    assert!(released.base_endpoints().is_empty());
    assert!(!released.needs_base_underlay());
    let context = color_context();
    assert_eq!(
        released.evaluate_base(&context, &UnavailableFrame).unwrap(),
        None
    );
    assert!(context.calls.borrow().is_empty());
}

#[test]
fn orthogonal_temperature_uses_reciprocal_units_over_its_branch_default() {
    let mut start = semantic(0.0);
    start = edit(
        &start,
        ProgrammingComponent::Color(ColorComponent::Temperature),
        2000.0,
    );
    let compiled = CompiledCoupledExpression::new(
        resume(
            Some(whole(ProgrammingOwner::Color, start)),
            Some(component(ColorComponent::Temperature, 10000.0)),
            0.5,
        ),
        None,
    )
    .unwrap();
    let result = compiled
        .evaluate_orthogonal(
            ColorComponent::Temperature,
            &color_context(),
            &UnavailableFrame,
        )
        .unwrap()
        .unwrap();
    assert!((result - 3333.3333).abs() < 0.001);
}

struct TargetContext {
    calls: RefCell<Vec<TargetReference>>,
}
impl CoupledExpressionContext for TargetContext {
    fn materialize_base(
        &self,
        endpoint: Option<(&CompiledDynamicValueAddress, &DynamicValue)>,
    ) -> Result<AttributeValue, TransitionError> {
        let Some((address, DynamicValue::Scalar(value))) = endpoint else {
            panic!()
        };
        let DynamicFamilyRepresentation::Target {
            reference: Some(reference),
        } = address.address().representation
        else {
            panic!()
        };
        self.calls.borrow_mut().push(reference);
        Ok(AttributeValue::Position(Arc::new(PositionIntent::target(
            reference,
            [*value, 0.0, 0.0],
        ))))
    }
    fn orthogonal_underlay(
        &self,
        _: ColorComponent,
        _: &AttributeValue,
    ) -> Result<f32, TransitionError> {
        panic!()
    }
}
struct TargetFrame {
    points: RefCell<HashMap<Uuid, f32>>,
    calls: Cell<usize>,
}
impl WholeFamilyExpressionFrameResolver for TargetFrame {
    fn resolve(
        &self,
        requirement: TransitionRequirement,
        from: &AttributeValue,
        to: &AttributeValue,
        operation: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        assert_eq!(requirement, TransitionRequirement::LiveTargetPoints);
        self.calls.set(self.calls.get() + 1);
        let position = |value: &AttributeValue| {
            let AttributeValue::Position(value) = value else {
                panic!()
            };
            let PositionIntent::Target {
                reference: TargetReference::Point { point_id },
                offset_metres,
            } = value.as_ref()
            else {
                panic!()
            };
            let ScalarIntent::Value(offset) = offset_metres[0] else {
                panic!()
            };
            self.points.borrow()[point_id] + offset
        };
        let FamilyExpressionOperation::Transition { progress } = operation else {
            panic!()
        };
        let (a, b) = (position(from), position(to));
        Ok(AttributeValue::Position(Arc::new(PositionIntent::angles(
            a + (b - a) * progress,
            0.0,
        ))))
    }
}

#[test]
fn target_component_reference_changes_materialize_both_frames_and_resolve_live_geometry() {
    let refs = [Uuid::from_u128(1), Uuid::from_u128(2)];
    let endpoint = |index: usize, offset| {
        leaf(
            DynamicValueAddress {
                representation: DynamicFamilyRepresentation::Target {
                    reference: Some(TargetReference::Point {
                        point_id: refs[index],
                    }),
                },
                component: Some(ProgrammingComponent::TargetX),
            },
            DynamicValue::Scalar(offset),
        )
    };
    let compiled = CompiledCoupledExpression::new(
        resume(Some(endpoint(0, 1.0)), Some(endpoint(1, 2.0)), 0.5),
        None,
    )
    .unwrap();
    let context = TargetContext {
        calls: RefCell::new(vec![]),
    };
    let frame = TargetFrame {
        points: RefCell::new(HashMap::from([(refs[0], 10.0), (refs[1], 100.0)])),
        calls: Cell::new(0),
    };
    assert_eq!(
        compiled.evaluate_base(&context, &frame).unwrap(),
        Some(AttributeValue::Position(Arc::new(PositionIntent::angles(
            56.5, 0.0
        ))))
    );
    frame.points.borrow_mut().insert(refs[1], 200.0);
    assert_eq!(
        compiled.evaluate_base(&context, &frame).unwrap(),
        Some(AttributeValue::Position(Arc::new(PositionIntent::angles(
            106.5, 0.0
        ))))
    );
    assert_eq!(frame.calls.get(), 2);
    assert_eq!(
        &context.calls.borrow()[..2],
        &refs.map(|point_id| TargetReference::Point { point_id })
    );
}

struct Model {
    source: NativeColorIdentity,
    channel: Uuid,
}
impl NativeColorEditModel for Model {
    fn source(&self) -> &NativeColorIdentity {
        &self.source
    }
    fn descriptor(&self, binding: NativeColorBinding) -> Option<NativeColorComponentDescriptor> {
        (binding.channel_id == self.channel
            && [Uuid::from_u128(10), Uuid::from_u128(11)].contains(&binding.function_id))
        .then_some(NativeColorComponentDescriptor {
            binding,
            raw_from: if binding.function_id == Uuid::from_u128(11) {
                100
            } else {
                0
            },
            raw_to: if binding.function_id == Uuid::from_u128(11) {
                200
            } else {
                u32::MAX
            },
            continuous: true,
        })
    }
    fn predict(&self, recipe: &NativeColorRecipe) -> Result<PortableColorEstimate, IntentError> {
        if recipe.source != self.source
            || recipe.channels.len() != 1
            || recipe.channels[0].channel_id != self.channel
        {
            return Err(IntentError("incomplete or foreign native recipe".into()));
        }
        let value = &recipe.channels[0];
        let descriptor = self
            .descriptor(NativeColorBinding {
                channel_id: value.channel_id,
                function_id: value.function_id,
            })
            .ok_or_else(|| IntentError("native function is absent".into()))?;
        if !(descriptor.raw_from..=descriptor.raw_to).contains(&value.raw) {
            return Err(IntentError("native function value is out of bounds".into()));
        }
        Ok(PortableColorEstimate {
            model_revision: self.source.model_revision,
            visible: None,
            uv: None,
            quality: PhysicalDataQuality::Unknown,
            limitations: vec![],
        })
    }
}
struct Models {
    entries: Vec<Arc<Model>>,
    calls: AtomicUsize,
}
impl DynamicNativeModelResolver for Models {
    fn resolve(
        &self,
        source: &NativeColorIdentity,
    ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, IntentError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.entries
            .iter()
            .find(|model| &model.source == source)
            .cloned()
            .map(|model| model as Arc<dyn NativeColorEditModel + Send + Sync>)
            .ok_or_else(|| IntentError("original model unavailable".into()))
    }
}
fn model(id: u128) -> Arc<Model> {
    Arc::new(Model {
        source: NativeColorIdentity {
            profile_id: Uuid::from_u128(id),
            profile_revision: 1,
            profile_digest: format!("source-{id}"),
            mode_id: Uuid::from_u128(2),
            head_id: Uuid::from_u128(3),
            path_id: Uuid::from_u128(4),
            model_revision: 1,
            native_layout_signature: "one-channel".into(),
        },
        channel: Uuid::from_u128(5),
    })
}
fn native_leaf(model: &Model, function: u128, raw: u32) -> Arc<DynamicSampleExpression> {
    leaf(
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::DirectColor {
                source: model.source.clone(),
            },
            component: Some(ProgrammingComponent::NativeColor(NativeColorBinding {
                channel_id: model.channel,
                function_id: Uuid::from_u128(function),
            })),
        },
        DynamicValue::Native(raw),
    )
}
struct NativeContext;
impl CoupledExpressionContext for NativeContext {
    fn materialize_base(
        &self,
        endpoint: Option<(&CompiledDynamicValueAddress, &DynamicValue)>,
    ) -> Result<AttributeValue, TransitionError> {
        let Some((address, DynamicValue::Native(raw))) = endpoint else {
            panic!()
        };
        let DynamicFamilyRepresentation::DirectColor { source } = &address.address().representation
        else {
            panic!()
        };
        let Some(ProgrammingComponent::NativeColor(binding)) = address.address().component else {
            panic!()
        };
        Ok(AttributeValue::ColorProgram(Arc::new(
            ColorProgram::Direct {
                recipe: NativeColorRecipe {
                    source: source.clone(),
                    channels: vec![NativeColorValue {
                        channel_id: binding.channel_id,
                        function_id: binding.function_id,
                        raw: *raw,
                    }],
                    spreads: vec![],
                },
                portable: PortableColorEstimate {
                    model_revision: source.model_revision,
                    visible: None,
                    uv: None,
                    quality: PhysicalDataQuality::Unknown,
                    limitations: vec![],
                },
            },
        )))
    }
    fn orthogonal_underlay(
        &self,
        _: ColorComponent,
        _: &AttributeValue,
    ) -> Result<f32, TransitionError> {
        panic!()
    }
}

#[test]
fn native_function_changes_hold_exact_source_function_and_source_changes_require_appearance() {
    let first = model(1);
    let second = model(2);
    let models = Models {
        entries: vec![first.clone(), second.clone()],
        calls: AtomicUsize::new(0),
    };
    let old = native_leaf(&first, 10, u32::MAX - 2);
    let new = native_leaf(&first, 11, 150);
    let compiled =
        CompiledCoupledExpression::new(resume(Some(old.clone()), Some(new), 0.5), Some(&models))
            .unwrap();
    assert_eq!(models.calls.load(Ordering::Relaxed), 1);
    let value = compiled
        .evaluate_base(&NativeContext, &UnavailableFrame)
        .unwrap()
        .unwrap();
    let AttributeValue::ColorProgram(program) = value else {
        panic!()
    };
    let ColorProgram::Direct { recipe, .. } = program.as_ref() else {
        panic!()
    };
    assert_eq!(recipe.channels[0].function_id, Uuid::from_u128(10));
    assert_eq!(recipe.channels[0].raw, u32::MAX - 2);
    let compiled = CompiledCoupledExpression::new(
        resume(Some(old), Some(native_leaf(&second, 10, 7)), 0.5),
        Some(&models),
    )
    .unwrap();
    assert_eq!(
        models.calls.load(Ordering::Relaxed),
        3,
        "one cold lookup per distinct source in each compiled expression"
    );
    assert!(matches!(
        compiled.evaluate_base(&NativeContext, &UnavailableFrame),
        Err(TransitionError::Requires(
            TransitionRequirement::ColorAppearance
        ))
    ));
    assert!(
        CompiledCoupledExpression::new(native_leaf(&first, 11, 201), Some(&models)).is_err(),
        "active components obey the original function's bounds"
    );
}

#[test]
fn exact_pruning_avoids_unavailable_native_models_and_rejects_foreign_callback_identity() {
    let first = model(1);
    let expression = resume(
        Some(native_leaf(&first, 10, 9)),
        Some(component(ColorComponent::Red, 0.2)),
        1.0,
    );
    assert!(CompiledCoupledExpression::new(expression, None).is_ok());
    let released =
        CompiledCoupledExpression::new(resume(Some(native_leaf(&first, 10, 9)), None, 1.0), None)
            .unwrap();
    assert!(released.roles().is_empty());
    struct ForeignContext;
    impl CoupledExpressionContext for ForeignContext {
        fn materialize_base(
            &self,
            _: Option<(&CompiledDynamicValueAddress, &DynamicValue)>,
        ) -> Result<AttributeValue, TransitionError> {
            Ok(semantic(0.0))
        }
        fn orthogonal_underlay(
            &self,
            _: ColorComponent,
            _: &AttributeValue,
        ) -> Result<f32, TransitionError> {
            panic!()
        }
    }
    let models = Models {
        entries: vec![first.clone()],
        calls: AtomicUsize::new(0),
    };
    let compiled =
        CompiledCoupledExpression::new(native_leaf(&first, 10, 9), Some(&models)).unwrap();
    assert!(matches!(
        compiled.evaluate_base(&ForeignContext, &UnavailableFrame),
        Err(TransitionError::Requires(
            TransitionRequirement::CompatibleOwners
        ))
    ));
    struct WrongFunction;
    impl CoupledExpressionContext for WrongFunction {
        fn materialize_base(
            &self,
            endpoint: Option<(&CompiledDynamicValueAddress, &DynamicValue)>,
        ) -> Result<AttributeValue, TransitionError> {
            let mut value = NativeContext.materialize_base(endpoint)?;
            let AttributeValue::ColorProgram(program) = &mut value else {
                panic!()
            };
            let ColorProgram::Direct { recipe, .. } = Arc::make_mut(program) else {
                panic!()
            };
            recipe.channels[0].function_id = Uuid::from_u128(11);
            recipe.channels[0].raw = 150;
            Ok(value)
        }
        fn orthogonal_underlay(
            &self,
            _: ColorComponent,
            _: &AttributeValue,
        ) -> Result<f32, TransitionError> {
            panic!()
        }
    }
    assert!(
        matches!(
            compiled.evaluate_base(&WrongFunction, &UnavailableFrame),
            Err(TransitionError::Requires(
                TransitionRequirement::CompatibleOwners
            ))
        ),
        "a valid same-source recipe must still use the requested function"
    );
}
