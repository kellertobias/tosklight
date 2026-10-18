use super::*;
use crate::{
    DynamicNativeModelResolver, DynamicRuntimeSample, DynamicValueSourceResolver,
    WholeFamilyExpressionFrameResolver,
};
use light_core::{FixtureId, NativeColorBinding, NativeColorIdentity};
use std::{cell::Cell, sync::OnceLock};

fn rank() -> FamilySampleRank {
    FamilySampleRank {
        priority: 10,
        changed_at_millis: 100,
        changed_at_submillis_nanos: 0,
        stable_order: 1,
        identity: FamilySampleIdentity::Dynamic {
            instance_id: Uuid::from_u128(1),
            controller_id: Uuid::from_u128(2),
            lane_id: Uuid::from_u128(3),
        },
    }
}
fn address(component: ColorComponent) -> DynamicValueAddress {
    DynamicValueAddress {
        representation: DynamicFamilyRepresentation::SemanticColor {
            basis: if component == ColorComponent::Uv {
                DynamicSemanticColorBasis::Retain
            } else {
                DynamicSemanticColorBasis::Recipe
            },
        },
        component: Some(ProgrammingComponent::Color(component)),
    }
}
fn unique_static() -> &'static Arc<CompiledDynamicValueAddress> {
    // This OnceLock belongs only to the reference-count test, never to parallel production slots.
    static ADDRESS: OnceLock<Arc<CompiledDynamicValueAddress>> = OnceLock::new();
    ADDRESS.get_or_init(|| {
        Arc::new(CompiledDynamicValueAddress::new(address(ColorComponent::Red), None).unwrap())
    })
}
// Frozen original source-boundary validation order; storage wrapping is the only adaptation.
fn old_new(
    address: Arc<CompiledDynamicValueAddress>,
    value: DynamicValue,
    rank: FamilySampleRank,
    activation_mix: f32,
) -> Result<FamilySample, IntentError> {
    rank.validate()?;
    address.validate_source_value(&value)?;
    ensure(
        activation_mix.is_finite() && (0.0..=1.0).contains(&activation_mix),
        "Dynamic activation influence must be between zero and one",
    )?;
    Ok(FamilySample {
        address: Cow::Owned(address),
        body: FamilySampleBody::Materialized(value),
        projection: None,
        fix_at: false,
        endpoint_output_exempt: false,
        trace_sources: None,
        trace_node: None,
        rank,
        activation_mix,
    })
}

#[test]
fn borrowed_clone_and_drop_do_not_touch_static_arc_but_required_owned_conversion_does() {
    let address = unique_static();
    assert_eq!(Arc::strong_count(address), 1);
    let sample = FamilySample::with_address(
        Cow::Borrowed(address),
        DynamicValue::Scalar(0.4),
        rank(),
        1.0,
    )
    .unwrap();
    let cloned = sample.clone();
    assert_eq!(Arc::strong_count(address), 1);
    assert!(std::ptr::eq(sample.address(), address.as_ref()));
    assert!(matches!(sample.address, Cow::Borrowed(_)));
    let expression = Arc::new(DynamicSampleExpression::Programming {
        address: Arc::new(address.address().clone()),
        value: DynamicValue::Scalar(0.4),
        occurrence: None,
        dependency_occurrence: None,
    });
    let compiled = Arc::new(
        CompiledComponentExpression::new(expression, Arc::clone(sample.address.as_ref())).unwrap(),
    );
    assert!(Arc::ptr_eq(compiled.address(), address));
    let retained = FamilySample::retained_component(compiled.clone(), rank(), 1.0).unwrap();
    assert!(matches!(retained.address, Cow::Owned(_)));
    assert!(Arc::ptr_eq(retained.address.as_ref(), address));
    drop((retained, compiled));
    assert_eq!(Arc::strong_count(address), 1);
    drop((cloned, sample));
    assert_eq!(Arc::strong_count(address), 1);
    let owned =
        FamilySample::new(Arc::clone(address), DynamicValue::Scalar(0.4), rank(), 1.0).unwrap();
    let cloned = owned.clone();
    assert!(matches!(owned.address, Cow::Owned(_)));
    assert_eq!(Arc::strong_count(address), 3);
    drop(cloned);
    assert_eq!(Arc::strong_count(address), 2);
    drop(owned);
    assert_eq!(Arc::strong_count(address), 1);
}

struct Sources;
impl DynamicValueSourceResolver for Sources {
    fn current(&self, _: FixtureId, _: &DynamicValueAddress) -> Option<DynamicValue> {
        panic!("literal leaf read Current")
    }
    fn preset(
        &self,
        _: &crate::DynamicPresetSourceBinding,
        _: Uuid,
        _: FixtureId,
    ) -> Option<DynamicValue> {
        panic!("literal leaf read Preset")
    }
}
fn input(component: ColorComponent, value: f32) -> DynamicRuntimeSample {
    let address = Arc::new(address(component));
    let occurrence = crate::DynamicSourceOccurrenceId::new(Uuid::from_u128(4)).unwrap();
    DynamicRuntimeSample {
        instance_id: Uuid::from_u128(1),
        controller_id: Uuid::from_u128(2),
        target: FixtureId(Uuid::from_u128(5)),
        lane_id: Uuid::from_u128(3),
        priority: 10,
        activated_at_millis: 100,
        activation_mix: 1.0,
        address: None,
        expression: DynamicSampleExpression::Programming {
            address: address.clone(),
            value: DynamicValue::Scalar(value),
            occurrence: Some(occurrence),
            dependency_occurrence: Some(crate::DynamicSourceDependency::compatible(
                Some(occurrence),
                &address,
            )),
        },
    }
}
fn prepare(
    input: DynamicRuntimeSample,
    scratch: &mut crate::DynamicFamilyPreparationScratch,
) -> FamilySample {
    let prepared = crate::prepare_dynamic_family_samples(
        &[input],
        &Sources,
        None::<&dyn DynamicNativeModelResolver>,
        scratch,
    )
    .unwrap();
    assert!(prepared.requirements.is_empty());
    assert_eq!(prepared.families.len(), 1);
    assert_eq!(prepared.families[0].samples.len(), 1);
    let FamilyCompositionSample::Known(sample) = &prepared.families[0].samples[0] else {
        panic!("expected component sample")
    };
    sample.clone()
}

#[test]
fn six_actual_plain_slots_borrow_and_general_wrapped_paths_keep_ownership() {
    let mut fast = crate::DynamicFamilyPreparationScratch::default();
    let mut general = crate::DynamicFamilyPreparationScratch::default();
    for component in [
        ColorComponent::Red,
        ColorComponent::Green,
        ColorComponent::Blue,
        ColorComponent::Amber,
        ColorComponent::WhiteBlend,
        ColorComponent::Uv,
    ] {
        let mut previous = None;
        for value in [0.2, 0.8, -0.0] {
            let input = input(component, value);
            let sample = prepare(input.clone(), &mut fast);
            assert!(matches!(sample.address, Cow::Borrowed(_)));
            assert!(sample.address.native_model().is_none());
            let pointer = sample.address() as *const _;
            if let Some(previous) = previous {
                assert_eq!(pointer, previous);
            }
            previous = Some(pointer);
            let mut wrapped = input;
            wrapped.expression = DynamicSampleExpression::Operation {
                origin: None,
                value: Arc::new(wrapped.expression),
            };
            let reference = prepare(wrapped, &mut general);
            assert!(matches!(reference.address, Cow::Owned(_)));
            assert_eq!(sample.materialized_value(), reference.materialized_value());
            assert_eq!(sample.rank, reference.rank);
        }
    }
}

struct Frame(Cell<usize>);
impl WholeFamilyExpressionFrameResolver for Frame {
    fn resolve(
        &self,
        requirement: TransitionRequirement,
        _: &AttributeValue,
        _: &AttributeValue,
        _: crate::FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        self.0.set(self.0.get() + 1);
        Err(TransitionError::Requires(requirement))
    }
}
fn compare_traces(a: &RetainedFamilyCompositionScratch, b: &RetainedFamilyCompositionScratch) {
    use light_core::programming::ProgrammingTraceField as F;
    let fields = [
        F::ColorXyz,
        F::ColorRecipeRed,
        F::ColorRecipeGreen,
        F::ColorRecipeBlue,
        F::ColorRecipeAmber,
        F::WhiteBlend,
        F::Uv,
        F::Temperature,
        F::Duv,
    ];
    let a = a.family_trace();
    let b = b.family_trace();
    assert_eq!(a.root(), b.root());
    let root = a.root().expect("nonempty traced composition");
    for scope in fields
        .iter()
        .map(|field| ProgrammingFieldScope::new([*field]))
        .chain(std::iter::once(ProgrammingFieldScope::new(fields)))
    {
        assert_eq!(
            a.query_fields_with_base(root, &scope),
            b.query_fields_with_base(root, &scope)
        );
        assert_eq!(
            a.control_sources_for_fields(root, &scope),
            b.control_sources_for_fields(root, &scope)
        );
    }
}

#[test]
fn borrowed_and_frozen_owned_constructor_preserve_trace_errors_order_and_scratch_reuse() {
    let mut prep = crate::DynamicFamilyPreparationScratch::default();
    let mut fast = RetainedFamilyCompositionScratch::default();
    let mut old = RetainedFamilyCompositionScratch::default();
    let context = FamilyCompositionContext {
        edit: FamilyEditContext {
            color_model: Some(&VirtualColorAuthoringV1),
            ..Default::default()
        },
        ..Default::default()
    };
    let base = AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
        intent: ColorIntent::default(),
    }));
    let frames = (Frame(Cell::new(0)), Frame(Cell::new(0)));
    for (index, value) in [0.2, 0.9, 0.4].into_iter().enumerate() {
        let mut samples = Vec::new();
        let mut reference = Vec::new();
        for component in [
            ColorComponent::Uv,
            ColorComponent::Red,
            ColorComponent::WhiteBlend,
            ColorComponent::Green,
        ] {
            let mut sample = prepare(input(component, value), &mut prep);
            sample.activation_mix = 1.0 - index as f32 * 0.2;
            sample.rank.stable_order += component as u128;
            let mut old = old_new(
                Arc::clone(sample.address.as_ref()),
                sample.materialized_value().unwrap().clone(),
                sample.rank,
                sample.activation_mix,
            )
            .unwrap();
            old.trace_sources = sample.trace_sources.clone();
            reference.push(FamilyCompositionSample::Known(old));
            samples.push(FamilyCompositionSample::Known(sample));
        }
        if index == 1 {
            samples.reverse();
            reference.reverse();
        }
        let a = compose_retained_dynamic_family_traced(
            ProgrammingOwner::Color,
            &base,
            &samples,
            &context,
            &frames.0,
            &mut fast,
        )
        .unwrap();
        let b = compose_retained_dynamic_family_traced(
            ProgrammingOwner::Color,
            &base,
            &reference,
            &context,
            &frames.1,
            &mut old,
        )
        .unwrap();
        assert_eq!(a, b);
        compare_traces(&fast, &old);
        // Invalid late rank must fail identically, then the same scratch must recover.
        let FamilyCompositionSample::Known(last) = samples.last_mut().unwrap() else {
            unreachable!()
        };
        last.rank.changed_at_submillis_nanos = 1_000_000;
        let FamilyCompositionSample::Known(last) = reference.last_mut().unwrap() else {
            unreachable!()
        };
        last.rank.changed_at_submillis_nanos = 1_000_000;
        let a = compose_retained_dynamic_family_traced(
            ProgrammingOwner::Color,
            &base,
            &samples,
            &context,
            &frames.0,
            &mut fast,
        );
        let b = compose_retained_dynamic_family_traced(
            ProgrammingOwner::Color,
            &base,
            &reference,
            &context,
            &frames.1,
            &mut old,
        );
        assert!(a.is_err());
        assert_eq!(a, b);
    }
    assert_eq!(frames.0.0.get(), 0);
    assert_eq!(frames.1.0.get(), 0);
    static ERROR_ADDRESS: OnceLock<Arc<CompiledDynamicValueAddress>> = OnceLock::new();
    let address = ERROR_ADDRESS.get_or_init(|| {
        Arc::new(CompiledDynamicValueAddress::new(address(ColorComponent::Red), None).unwrap())
    });
    for (bad_rank, value, activation) in [
        (true, f32::NAN, f32::NAN),
        (false, f32::NAN, f32::NAN),
        (false, 0.4, f32::NAN),
        (false, 0.4, -0.1),
    ] {
        let mut rank = rank();
        if bad_rank {
            rank.changed_at_submillis_nanos = 1_000_000;
        }
        let a = FamilySample::with_address(
            Cow::Borrowed(address),
            DynamicValue::Scalar(value),
            rank,
            activation,
        );
        let b = old_new(
            address.clone(),
            DynamicValue::Scalar(value),
            rank,
            activation,
        );
        assert!(a.is_err());
        assert_eq!(a.err(), b.err());
    }
}

struct Native {
    source: NativeColorIdentity,
    binding: NativeColorBinding,
}
impl NativeColorEditModel for Native {
    fn source(&self) -> &NativeColorIdentity {
        &self.source
    }
    fn descriptor(&self, binding: NativeColorBinding) -> Option<NativeColorComponentDescriptor> {
        (binding == self.binding).then_some(NativeColorComponentDescriptor {
            binding,
            raw_from: 0,
            raw_to: u32::MAX,
            continuous: true,
        })
    }
    fn predict(&self, _: &NativeColorRecipe) -> Result<PortableColorEstimate, IntentError> {
        panic!("native scalar construction must not predict a recipe")
    }
}
#[test]
fn public_native_construction_retains_original_owned_model_and_allocation_identity() {
    let model = Arc::new(Native {
        source: NativeColorIdentity {
            profile_id: Uuid::from_u128(1),
            profile_revision: 1,
            profile_digest: "original".into(),
            mode_id: Uuid::from_u128(2),
            head_id: Uuid::from_u128(3),
            path_id: Uuid::from_u128(4),
            model_revision: 1,
            native_layout_signature: "layout".into(),
        },
        binding: NativeColorBinding {
            channel_id: Uuid::from_u128(5),
            function_id: Uuid::from_u128(6),
        },
    });
    let weak_model = Arc::downgrade(&model);
    let address = Arc::new(
        CompiledDynamicValueAddress::new(
            DynamicValueAddress {
                representation: DynamicFamilyRepresentation::DirectColor {
                    source: model.source.clone(),
                },
                component: Some(ProgrammingComponent::NativeColor(model.binding)),
            },
            Some(model.clone()),
        )
        .unwrap(),
    );
    let weak_address = Arc::downgrade(&address);
    let sample =
        FamilySample::new(address.clone(), DynamicValue::Native(u32::MAX), rank(), 1.0).unwrap();
    assert!(matches!(sample.address, Cow::Owned(_)));
    assert!(Arc::ptr_eq(sample.address.as_ref(), &address));
    let cloned = sample.clone();
    drop((model, address, sample));
    assert!(weak_model.upgrade().is_some());
    assert!(weak_address.upgrade().is_some());
    drop(cloned);
    assert!(weak_model.upgrade().is_none());
    assert!(weak_address.upgrade().is_none());
}
