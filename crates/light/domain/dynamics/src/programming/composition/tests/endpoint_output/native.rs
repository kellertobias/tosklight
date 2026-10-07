use super::*;
use crate::{
    DynamicNativeModelResolver, NativeColorModelCapability, NativeColorModelUnavailable,
    NativeColorUnavailableReason,
};

struct Models {
    model: Arc<NativeModel>,
    lookups: AtomicUsize,
    unavailable: bool,
    wrong_identity: bool,
}
impl Models {
    fn new(model: Arc<NativeModel>) -> Self {
        Self {
            model,
            lookups: AtomicUsize::new(0),
            unavailable: false,
            wrong_identity: false,
        }
    }
}
impl DynamicNativeModelResolver for Models {
    fn resolve(
        &self,
        source: &NativeColorIdentity,
    ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, IntentError> {
        self.lookups.fetch_add(1, Ordering::Relaxed);
        if !self.wrong_identity {
            ensure(source == &self.model.source, "unexpected source requested")?;
        }
        Ok(self.model.clone())
    }
    fn resolve_capability(
        &self,
        source: &NativeColorIdentity,
    ) -> Result<NativeColorModelCapability, IntentError> {
        if self.unavailable {
            return Ok(NativeColorModelCapability::Unavailable(
                NativeColorModelUnavailable {
                    source: source.clone(),
                    reason: NativeColorUnavailableReason::MissingRevision,
                    detail: "test missing revision".into(),
                },
            ));
        }
        self.resolve(source)
            .map(NativeColorModelCapability::Available)
    }
}
fn compose_native(
    current: &Current,
    samples: &[FamilyCompositionSample],
    mix: f32,
    models: Option<&dyn DynamicNativeModelResolver>,
    frame: &dyn WholeFamilyExpressionFrameResolver,
) -> Result<AttributeValue, TransitionError> {
    let control = at(3, mix);
    compose_retained_dynamic_family_traced(
        ProgrammingOwner::Color,
        &current.value,
        samples,
        &FamilyCompositionContext {
            edit: FamilyEditContext {
                color_model: Some(&VirtualColorAuthoringV1),
                ..Default::default()
            },
            endpoint_output: Some(FamilyEndpointOutputContext {
                control: &control,
                target: FixtureId::new(),
                current,
                native_models: models,
            }),
            ..Default::default()
        },
        frame,
        &mut RetainedFamilyCompositionScratch::default(),
    )
}
fn materialized(model: &Arc<NativeModel>, values: [u32; 2], order: u128) -> FamilySample {
    let value = semantic_to_direct(model, values);
    FamilySample::new(
        Arc::new(
            CompiledDynamicValueAddress::new(
                DynamicValueAddress::whole_family(ProgrammingOwner::Color, &value).unwrap(),
                Some(model.clone()),
            )
            .unwrap(),
        ),
        DynamicValue::Family(value),
        color(ColorComponent::Uv, 0.0, order).rank,
        1.0,
    )
    .unwrap()
}
fn semantic_endpoint(activation_mix: f32, retained: bool) -> FamilyCompositionSample {
    let sample = sample(
        DynamicFamilyRepresentation::SemanticColor {
            basis: DynamicSemanticColorBasis::Whole,
        },
        None,
        DynamicValue::Family(uv(0.8)),
        3,
    );
    if retained {
        FamilyCompositionSample::WholeExpression {
            expression: Arc::new(
                crate::CompiledProgrammingFamilyExpression::new(
                    leaf(&sample),
                    ProgrammingOwner::Color,
                    None,
                    None,
                )
                .unwrap(),
            ),
            rank: sample.rank,
            activation_mix,
        }
    } else {
        let mut sample = sample;
        sample.activation_mix = activation_mix;
        sample.into()
    }
}
fn channels(value: &AttributeValue) -> Vec<u32> {
    let AttributeValue::ColorProgram(program) = value else {
        panic!()
    };
    let ColorProgram::Direct { recipe, .. } = program.as_ref() else {
        panic!()
    };
    recipe.channels.iter().map(|channel| channel.raw).collect()
}

#[test]
fn zero_master_uses_captured_current_model_for_different_original_source_without_predicting() {
    let endpoint_model = native_model();
    let mut current_model = native_model();
    Arc::get_mut(&mut current_model)
        .unwrap()
        .source
        .profile_revision += 1;
    let current = Current::new(semantic_to_direct(&current_model, [100, 200]));
    let models = Models::new(current_model.clone());
    let samples = [materialized(&endpoint_model, [300, 400], 3).into()];
    assert!(matches!(
        compose_native(&current, &samples, 0.0, None, &NoFrame),
        Err(TransitionError::Requires(
            TransitionRequirement::NativeColorModel
        ))
    ));
    let result = compose_native(&current, &samples, 0.0, Some(&models), &NoFrame).unwrap();
    assert_eq!(result, current.value);
    assert_eq!(
        current_model.predictions.load(Ordering::Relaxed),
        0,
        "unchanged captured Current must not be predicted again"
    );
    assert_eq!(models.lookups.load(Ordering::Relaxed), 1);
    let reads = current.calls.get();
    assert_eq!(
        channels(&compose_native(&current, &samples, 1.0, Some(&models), &NoFrame).unwrap()),
        vec![300, 400]
    );
    assert_eq!(current.calls.get(), reads);
    assert_eq!(models.lookups.load(Ordering::Relaxed), 1);
}

#[test]
fn captured_current_model_also_supplies_partial_activation_after_whole_envelope() {
    let model = native_model();
    let current = Current::new(semantic_to_direct(&model, [100, 200]));
    let models = Models::new(model.clone());
    for retained in [false, true] {
        let samples = [
            materialized(&model, [300, 400], 1).into(),
            semantic_endpoint(0.5, retained),
        ];
        let before = model.predictions.load(Ordering::Relaxed);
        let result = compose_native(&current, &samples, 0.0, Some(&models), &NoFrame).unwrap();
        assert_eq!(channels(&result), vec![200, 300]);
        assert_eq!(
            model.predictions.load(Ordering::Relaxed) - before,
            1,
            "one native activation, no duplicate prediction"
        );
    }
}

struct SemanticAppearance {
    calls: Cell<usize>,
}
impl WholeFamilyExpressionFrameResolver for SemanticAppearance {
    fn resolve(
        &self,
        requirement: TransitionRequirement,
        from: &AttributeValue,
        to: &AttributeValue,
        operation: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        assert_eq!(requirement, TransitionRequirement::ColorAppearance);
        assert!(
            matches!(from,AttributeValue::ColorProgram(program) if matches!(program.as_ref(),ColorProgram::Direct { .. }))
        );
        assert!(
            matches!(to,AttributeValue::ColorProgram(program) if matches!(program.as_ref(),ColorProgram::Semantic { .. }))
        );
        assert_eq!(
            operation,
            FamilyExpressionOperation::Transition { progress: 0.5 }
        );
        self.calls.set(self.calls.get() + 1);
        Ok(uv(0.45))
    }
}
#[test]
fn portable_direct_to_semantic_envelope_needs_no_original_model() {
    let model = native_model();
    let current = Current::new(semantic_to_direct(&model, [100, 200]));
    let models = Models::new(model);
    let frame = SemanticAppearance {
        calls: Cell::new(0),
    };
    for retained in [false, true] {
        let result = compose_native(
            &current,
            &[semantic_endpoint(1.0, retained)],
            0.5,
            Some(&models),
            &frame,
        )
        .unwrap();
        assert_eq!(result, uv(0.45));
    }
    assert_eq!(frame.calls.get(), 2);
    assert_eq!(
        models.lookups.load(Ordering::Relaxed),
        0,
        "portable appearance conversion must not demand a source model"
    );
}

#[test]
fn captured_unavailable_model_is_passive_but_wrong_source_model_is_invalid() {
    let model = native_model();
    let current = Current::new(semantic_to_direct(&model, [100, 200]));
    let samples = [semantic_endpoint(1.0, false)];
    let mut missing = Models::new(model);
    missing.unavailable = true;
    assert!(matches!(
        compose_native(&current, &samples, 0.0, Some(&missing), &NoFrame),
        Err(TransitionError::Requires(
            TransitionRequirement::NativeColorModel
        ))
    ));
    let mut other = native_model();
    Arc::get_mut(&mut other).unwrap().source.profile_revision += 1;
    let mut wrong = Models::new(other);
    wrong.wrong_identity = true;
    assert!(matches!(
        compose_native(&current, &samples, 0.0, Some(&wrong), &NoFrame),
        Err(TransitionError::Invalid(_))
    ));
}
