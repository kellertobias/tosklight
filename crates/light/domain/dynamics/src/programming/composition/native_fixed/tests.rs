use super::*;
use crate::{
    FamilyCompositionSample, FamilyExpressionOperation, FamilyFixedSampleSource,
    FamilySampleIdentity, ProgrammingFamilyFixAt, WholeFamilyExpressionFrameResolver,
    compose_retained_dynamic_family_traced,
};
use light_core::{NativeColorIdentity, PhysicalDataQuality};
use std::sync::atomic::{AtomicUsize, Ordering};

const EMITTER: u128 = 11;
const WHEEL: u128 = 12;
const UV: u128 = 13;
fn binding(channel: u128, function: u128) -> NativeColorBinding {
    NativeColorBinding {
        channel_id: Uuid::from_u128(channel),
        function_id: Uuid::from_u128(function),
    }
}
struct Model {
    source: NativeColorIdentity,
    predictions: AtomicUsize,
}
impl NativeColorEditModel for Model {
    fn source(&self) -> &NativeColorIdentity {
        &self.source
    }
    fn descriptor(&self, binding: NativeColorBinding) -> Option<NativeColorComponentDescriptor> {
        let channel = binding.channel_id.as_u128();
        let function = binding.function_id.as_u128();
        match (channel, function) {
            (EMITTER, 111) => Some((u32::MAX, true)),
            (WHEEL, 121 | 122) => Some((255, false)),
            (WHEEL, 123 | 124) => Some((255, true)),
            (UV, 131) => Some((1000, true)),
            _ => None,
        }
        .map(|(max, continuous)| NativeColorComponentDescriptor {
            binding,
            raw_from: 0,
            raw_to: max,
            continuous,
        })
    }
    fn predict(&self, recipe: &NativeColorRecipe) -> Result<PortableColorEstimate, IntentError> {
        self.predictions.fetch_add(1, Ordering::Relaxed);
        ensure(
            recipe.source == self.source && recipe.channels.len() == 3,
            "invalid complete native test recipe",
        )?;
        for channel in &recipe.channels {
            let descriptor = self
                .descriptor(NativeColorBinding {
                    channel_id: channel.channel_id,
                    function_id: channel.function_id,
                })
                .ok_or_else(|| IntentError("unknown native test function".into()))?;
            ensure(
                channel.raw <= descriptor.raw_to,
                "native test value outside function",
            )?;
        }
        let uv = recipe
            .channels
            .iter()
            .find(|value| value.channel_id == Uuid::from_u128(UV))
            .unwrap()
            .raw as f32
            / 1000.0;
        Ok(PortableColorEstimate {
            model_revision: 1,
            visible: None,
            uv: Some(PortableUv {
                amount: uv,
                quality: PhysicalDataQuality::Estimated,
            }),
            quality: PhysicalDataQuality::Estimated,
            limitations: vec![],
        })
    }
}
fn model() -> Arc<Model> {
    Arc::new(Model {
        source: NativeColorIdentity {
            profile_id: Uuid::from_u128(1),
            profile_revision: 2,
            profile_digest: "original".into(),
            mode_id: Uuid::from_u128(2),
            head_id: Uuid::from_u128(3),
            path_id: Uuid::from_u128(4),
            model_revision: 1,
            native_layout_signature: "emitter-wheel-uv".into(),
        },
        predictions: AtomicUsize::new(0),
    })
}
fn family(model: &Model, emitter: u32, function: u128, wheel: u32, uv: u32) -> AttributeValue {
    let recipe = NativeColorRecipe {
        source: model.source.clone(),
        channels: vec![
            NativeColorValue {
                channel_id: Uuid::from_u128(EMITTER),
                function_id: Uuid::from_u128(111),
                raw: emitter,
            },
            NativeColorValue {
                channel_id: Uuid::from_u128(WHEEL),
                function_id: Uuid::from_u128(function),
                raw: wheel,
            },
            NativeColorValue {
                channel_id: Uuid::from_u128(UV),
                function_id: Uuid::from_u128(131),
                raw: uv,
            },
        ],
        spreads: vec![],
    };
    let portable = model.predict(&recipe).unwrap();
    AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct { recipe, portable }))
}
fn direct(value: &AttributeValue) -> (&NativeColorRecipe, &PortableColorEstimate) {
    let AttributeValue::ColorProgram(value) = value else {
        panic!()
    };
    let ColorProgram::Direct { recipe, portable } = value.as_ref() else {
        panic!()
    };
    (recipe, portable)
}
fn rank(index: usize) -> FamilySampleRank {
    FamilySampleRank {
        priority: 100,
        changed_at_millis: 1000,
        changed_at_submillis_nanos: 0,
        stable_order: index as u128,
        identity: FamilySampleIdentity::Fixed {
            source: FamilyFixedSampleSource::Programmer,
            row_index: index,
        },
    }
}
fn fixed(model: &Arc<Model>, function: u128, raw: u32, mix: f32, index: usize) -> FamilySample {
    let mask = ProgrammingFamilyFixAt::from_family(
        ProgrammingOwner::Color,
        Some(ProgrammingComponent::NativeColor(binding(WHEEL, function))),
        family(model, 900, function, raw, 200),
    )
    .unwrap();
    let rank = rank(index);
    mask.compile(
        Some(model.clone()),
        &FamilyEditContext::default(),
        rank,
        mix,
    )
    .unwrap()
    .with_trace_sources(Arc::from([FamilyTraceSource {
        rank,
        footprint: FamilyTraceFootprint::Component(ProgrammingComponent::NativeColor(binding(
            WHEEL, function,
        ))),
        role: FamilyTraceRole::Authored,
        occurrence: Some(
            crate::DynamicSourceOccurrenceId::new(Uuid::from_u128(index as u128 + 100)).unwrap(),
        ),
    }]))
}
fn dynamic(model: &Arc<Model>, native: NativeColorBinding, raw: u32, index: usize) -> FamilySample {
    let mut rank = rank(index);
    rank.identity = FamilySampleIdentity::Dynamic {
        instance_id: Uuid::from_u128(200),
        controller_id: Uuid::from_u128(201),
        lane_id: Uuid::from_u128(index as u128 + 300),
    };
    FamilySample::new(
        Arc::new(
            CompiledDynamicValueAddress::new(
                DynamicValueAddress {
                    representation: DynamicFamilyRepresentation::DirectColor {
                        source: model.source.clone(),
                    },
                    component: Some(ProgrammingComponent::NativeColor(native)),
                },
                Some(model.clone()),
            )
            .unwrap(),
        ),
        DynamicValue::Native(raw),
        rank,
        1.0,
    )
    .unwrap()
    .with_trace_sources(Arc::from([FamilyTraceSource {
        rank,
        footprint: FamilyTraceFootprint::Component(ProgrammingComponent::NativeColor(native)),
        role: FamilyTraceRole::Authored,
        occurrence: Some(
            crate::DynamicSourceOccurrenceId::new(Uuid::from_u128(index as u128 + 100)).unwrap(),
        ),
    }]))
}
struct NoFrame;
impl WholeFamilyExpressionFrameResolver for NoFrame {
    fn resolve(
        &self,
        _: TransitionRequirement,
        _: &AttributeValue,
        _: &AttributeValue,
        _: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        panic!("same-source native Fixed steps need no appearance adoption")
    }
}
fn compose(
    base: &AttributeValue,
    samples: &[FamilySample],
    scratch: &mut RetainedFamilyCompositionScratch,
) -> Result<AttributeValue, TransitionError> {
    compose_retained_dynamic_family_traced(
        ProgrammingOwner::Color,
        base,
        &samples
            .iter()
            .cloned()
            .map(FamilyCompositionSample::Known)
            .collect::<Vec<_>>(),
        &FamilyCompositionContext::default(),
        &NoFrame,
        scratch,
    )
}

#[test]
fn discrete_native_fixed_step_preserves_other_channels_uv_and_appearance_until_endpoint() {
    let model = model();
    let base = family(&model, 20, 121, 10, 800);
    let mut sample = fixed(&model, 122, 33, 0.5, 1);
    let mut scratch = RetainedFamilyCompositionScratch::default();
    let compiled_predictions = model.predictions.load(Ordering::Relaxed);
    for progress in [0.0, 0.25, 0.999_999] {
        sample.activation_mix = progress;
        let result = compose(&base, &[sample.clone()], &mut scratch).unwrap();
        assert_eq!(result, base);
        let trace = scratch.family_trace();
        let root = trace.root().unwrap();
        let fields = ProgrammingFieldScope::new([ProgrammingTraceField::NativeColorChannel(
            Uuid::from_u128(WHEEL),
        )]);
        let query = trace.query_fields_with_base(root, &fields).unwrap();
        assert!(query.sources.is_empty());
        assert_eq!(query.base_fields, fields);
        let prediction = trace
            .query_fields_with_base(
                root,
                &ProgrammingFieldScope::new([ProgrammingTraceField::NativePrediction]),
            )
            .unwrap();
        assert!(
            prediction
                .base_fields
                .contains(ProgrammingTraceField::NativePrediction)
        );
        let controls = trace.control_sources_for_fields(root, &fields).unwrap();
        assert_eq!(
            controls.iter().any(|control| control.rank == sample.rank),
            progress > 0.0
        );
    }
    assert_eq!(
        model.predictions.load(Ordering::Relaxed),
        compiled_predictions,
        "held step preserves exact underlay without re-prediction"
    );
    sample.activation_mix = 1.0;
    let value = compose(&base, &[sample.clone()], &mut scratch).unwrap();
    let (recipe, portable) = direct(&value);
    assert_eq!(recipe.channels[0], direct(&base).0.channels[0]);
    assert_eq!(recipe.channels[1].function_id, Uuid::from_u128(122));
    assert_eq!(recipe.channels[1].raw, 33);
    assert_eq!(recipe.channels[2], direct(&base).0.channels[2]);
    assert_eq!(portable.uv, direct(&base).1.uv);
    let trace = scratch.family_trace();
    let query = trace
        .query_fields_with_base(
            trace.root().unwrap(),
            &ProgrammingFieldScope::new([ProgrammingTraceField::NativeColorChannel(
                Uuid::from_u128(WHEEL),
            )]),
        )
        .unwrap();
    assert_eq!(query.sources.len(), 1);
    assert_eq!(
        query.sources[0].source.occurrence,
        Some(crate::DynamicSourceOccurrenceId::new(Uuid::from_u128(101)).unwrap())
    );
    assert_eq!(
        query.sources[0].source.footprint,
        FamilyTraceFootprint::Component(ProgrammingComponent::NativeColor(binding(WHEEL, 122)))
    );
    assert!(query.base_fields.is_empty());
}

#[test]
fn same_function_discrete_raw_and_continuous_function_switch_both_step() {
    let model = model();
    let mut scratch = RetainedFamilyCompositionScratch::default();
    for (old_function, new_function) in [(121, 121), (123, 124)] {
        let base = family(&model, 20, old_function, 10, 800);
        let mut mask = fixed(&model, new_function, 200, 0.75, 1);
        assert_eq!(compose(&base, &[mask.clone()], &mut scratch).unwrap(), base);
        mask.activation_mix = 1.0;
        let result = compose(&base, &[mask], &mut scratch).unwrap();
        assert_eq!(
            direct(&result).0.channels[1].function_id,
            Uuid::from_u128(new_function)
        );
        assert_eq!(direct(&result).0.channels[1].raw, 200);
    }
}

#[test]
fn same_function_continuous_fixed_mask_keeps_numeric_interpolation() {
    let model = model();
    let base = family(&model, 20, 123, 10, 800);
    let sample = fixed(&model, 123, 110, 0.5, 1);
    let value = compose(
        &base,
        &[sample],
        &mut RetainedFamilyCompositionScratch::default(),
    )
    .unwrap();
    assert_eq!(direct(&value).0.channels[1].raw, 60);
    assert_eq!(direct(&value).0.channels[0], direct(&base).0.channels[0]);
    assert_eq!(direct(&value).0.channels[2], direct(&base).0.channels[2]);
}

#[test]
fn partial_function_mask_keeps_lower_dynamic_value_and_separate_control_lineage() {
    let model = model();
    let base = family(&model, 20, 123, 10, 800);
    let lower = dynamic(&model, binding(WHEEL, 123), 55, 1);
    let mut mask = fixed(&model, 124, 88, 0.5, 2);
    let other = dynamic(&model, binding(EMITTER, 111), 404, 3);
    let mut scratch = RetainedFamilyCompositionScratch::default();
    for progress in [0.5, 1.0] {
        mask.activation_mix = progress;
        let result = compose(
            &base,
            &[lower.clone(), mask.clone(), other.clone()],
            &mut scratch,
        )
        .unwrap();
        let channel = &direct(&result).0.channels[1];
        assert_eq!(
            channel.function_id,
            Uuid::from_u128(if progress < 1.0 { 123 } else { 124 })
        );
        assert_eq!(channel.raw, if progress < 1.0 { 55 } else { 88 });
        assert_eq!(direct(&result).0.channels[0].raw, 404);
        assert_eq!(direct(&result).0.channels[2].raw, 800);
        let trace = scratch.family_trace();
        let root = trace.root().unwrap();
        let fields = ProgrammingFieldScope::new([ProgrammingTraceField::NativeColorChannel(
            Uuid::from_u128(WHEEL),
        )]);
        let sources = trace.query_fields_with_base(root, &fields).unwrap().sources;
        assert_eq!(sources.len(), 1);
        assert_eq!(
            sources[0].source.rank,
            if progress < 1.0 {
                lower.rank
            } else {
                mask.rank
            }
        );
        let controls = trace.control_sources_for_fields(root, &fields).unwrap();
        assert!(controls.iter().any(|control| control.rank == mask.rank));
        assert_eq!(
            controls.iter().any(|control| control.rank == lower.rank),
            progress < 1.0
        );
    }
}

#[test]
fn opaque_whole_mask_covers_foreign_native_step_without_adoption() {
    let model = model();
    let base = AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
        intent: ColorIntent::default(),
    }));
    let narrow = fixed(&model, 122, 33, 0.5, 1);
    let cover = ProgrammingFamilyFixAt::from_family(ProgrammingOwner::Color, None, base.clone())
        .unwrap()
        .compile(None, &FamilyEditContext::default(), rank(2), 1.0)
        .unwrap();
    assert_eq!(
        compose(
            &base,
            &[narrow.clone(), cover],
            &mut RetainedFamilyCompositionScratch::default()
        )
        .unwrap(),
        base
    );
    assert!(matches!(
        compose(
            &base,
            &[narrow],
            &mut RetainedFamilyCompositionScratch::default()
        ),
        Err(TransitionError::Requires(
            TransitionRequirement::ColorAppearance
        ))
    ));
}

#[test]
fn discrete_fixed_domain_does_not_enable_continuous_dynamic_arithmetic() {
    let model = model();
    let mask = fixed(&model, 122, 33, 1.0, 1);
    let address = mask.address();
    assert!(
        CompiledDynamicValueAddress::new(address.address().clone(), Some(model.clone())).is_err()
    );
    assert!(
        address
            .wave_between(
                &DynamicValue::Native(0),
                &DynamicValue::Native(100),
                0.5,
                1.0
            )
            .is_err()
    );
    assert!(
        address
            .scale_from(&DynamicValue::Native(0), &DynamicValue::Native(100), 0.5)
            .is_err()
    );
    assert!(
        address
            .transition(DynamicValue::Native(0), DynamicValue::Native(100))
            .is_err()
    );
    let mut invalid = ProgrammingFamilyFixAt::from_family(
        ProgrammingOwner::Color,
        Some(ProgrammingComponent::NativeColor(binding(WHEEL, 122))),
        family(&model, 20, 122, 33, 800),
    )
    .unwrap();
    let AttributeValue::ColorProgram(program) = &mut invalid.family else {
        panic!()
    };
    let ColorProgram::Direct { recipe, .. } = Arc::make_mut(program) else {
        panic!()
    };
    recipe.channels[1].raw = 300;
    assert!(
        invalid
            .compile(Some(model), &FamilyEditContext::default(), rank(2), 1.0)
            .is_err()
    );
}

#[test]
fn fixed_step_uses_the_pending_lower_function_even_when_base_matches_its_target() {
    let model = model();
    let base = family(&model, 20, 123, 10, 800);
    let lower = dynamic(&model, binding(WHEEL, 124), 55, 1);
    let mask = fixed(&model, 123, 88, 0.5, 2);
    let adoptions = AtomicUsize::new(0);
    let adopt_function = |value: &AttributeValue, address: &DynamicValueAddress| {
        adoptions.fetch_add(1, Ordering::Relaxed);
        let Some(ProgrammingComponent::NativeColor(binding)) = address.component else {
            panic!()
        };
        let mut recipe = direct(value).0.clone();
        let channel = recipe
            .channels
            .iter_mut()
            .find(|channel| channel.channel_id == binding.channel_id)
            .unwrap();
        channel.function_id = binding.function_id;
        channel.raw = 0;
        let portable = model.predict(&recipe)?;
        Ok(AttributeValue::ColorProgram(Arc::new(
            ColorProgram::Direct { recipe, portable },
        )))
    };
    let context = FamilyCompositionContext {
        resolve_adoption: Some(&adopt_function),
        ..Default::default()
    };
    let samples = [
        FamilyCompositionSample::Known(lower.clone()),
        FamilyCompositionSample::Known(mask),
    ];
    let mut scratch = RetainedFamilyCompositionScratch::default();
    let result = compose_retained_dynamic_family_traced(
        ProgrammingOwner::Color,
        &base,
        &samples,
        &context,
        &NoFrame,
        &mut scratch,
    )
    .unwrap();
    assert_eq!(adoptions.load(Ordering::Relaxed), 1);
    let (recipe, portable) = direct(&result);
    assert_eq!(recipe.channels[1].function_id, Uuid::from_u128(124));
    assert_eq!(recipe.channels[1].raw, 55);
    assert_eq!(recipe.channels[0], direct(&base).0.channels[0]);
    assert_eq!(recipe.channels[2], direct(&base).0.channels[2]);
    assert_eq!(portable.uv, direct(&base).1.uv);
    let trace = scratch.family_trace();
    let query = trace
        .query_fields_with_base(
            trace.root().unwrap(),
            &ProgrammingFieldScope::new([ProgrammingTraceField::NativeColorChannel(
                Uuid::from_u128(WHEEL),
            )]),
        )
        .unwrap();
    assert_eq!(query.sources.len(), 1);
    assert_eq!(query.sources[0].source.rank, lower.rank);
}
