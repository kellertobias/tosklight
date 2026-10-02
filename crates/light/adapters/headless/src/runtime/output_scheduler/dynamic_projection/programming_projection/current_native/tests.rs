use super::*;
use light_core::{NativeColorBinding, NativeColorValue, PhysicalDataQuality, programming::*};
use light_dynamics::{
    DynamicFamilyRepresentation, DynamicNativeModelResolver, DynamicValueSourceResolver,
    NativeColorModelUnavailable, NativeColorUnavailableReason, ScalarSourceResolver,
};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Model {
    source: NativeColorIdentity,
    predicts: AtomicUsize,
}
fn binding(index: u128) -> NativeColorBinding {
    NativeColorBinding {
        channel_id: Uuid::from_u128(10 + index),
        function_id: Uuid::from_u128(20 + index),
    }
}
impl NativeColorEditModel for Model {
    fn source(&self) -> &NativeColorIdentity {
        &self.source
    }
    fn descriptor(&self, requested: NativeColorBinding) -> Option<NativeColorComponentDescriptor> {
        [binding(0), binding(1)]
            .contains(&requested)
            .then_some(NativeColorComponentDescriptor {
                binding: requested,
                raw_from: 0,
                raw_to: 255,
                continuous: true,
            })
    }
    fn predict(&self, recipe: &NativeColorRecipe) -> Result<PortableColorEstimate, IntentError> {
        self.predicts.fetch_add(1, Ordering::Relaxed);
        if recipe.source != self.source
            || recipe.channels.len() != 2
            || !(0..2).all(|i| {
                recipe.channels.iter().any(|c| {
                    c.channel_id == binding(i).channel_id
                        && c.function_id == binding(i).function_id
                        && c.raw <= 255
                })
            })
        {
            return Err(IntentError(
                "incomplete or out-of-function original recipe".into(),
            ));
        }
        Ok(portable())
    }
}
fn model() -> Arc<Model> {
    Arc::new(Model {
        source: NativeColorIdentity {
            profile_id: Uuid::from_u128(1),
            profile_revision: 1,
            profile_digest: "original".into(),
            mode_id: Uuid::from_u128(2),
            head_id: Uuid::from_u128(3),
            path_id: Uuid::from_u128(4),
            model_revision: 1,
            native_layout_signature: "two-original-channels".into(),
        },
        predicts: AtomicUsize::new(0),
    })
}
fn portable() -> PortableColorEstimate {
    PortableColorEstimate {
        model_revision: 1,
        visible: None,
        uv: None,
        quality: PhysicalDataQuality::Unknown,
        limitations: vec![],
    }
}
fn value(model: &Model, raws: &[u32]) -> AttributeValue {
    AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
        recipe: NativeColorRecipe {
            source: model.source.clone(),
            channels: raws
                .iter()
                .enumerate()
                .map(|(i, raw)| NativeColorValue {
                    channel_id: binding(i as u128).channel_id,
                    function_id: binding(i as u128).function_id,
                    raw: *raw,
                })
                .collect(),
            spreads: vec![],
        },
        portable: portable(),
    }))
}
struct Models {
    model: Arc<Model>,
    additional: Vec<Arc<Model>>,
    unavailable: bool,
    lookups: AtomicUsize,
}
impl Models {
    fn new(model: Arc<Model>) -> Self {
        Self {
            model,
            additional: vec![],
            unavailable: false,
            lookups: AtomicUsize::new(0),
        }
    }
}
impl DynamicNativeModelResolver for Models {
    fn resolve(
        &self,
        _: &NativeColorIdentity,
    ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, IntentError> {
        Ok(self.model.clone())
    }
    fn resolve_capability(
        &self,
        source: &NativeColorIdentity,
    ) -> Result<NativeColorModelCapability, IntentError> {
        self.lookups.fetch_add(1, Ordering::Relaxed);
        if let Some(model) = self.additional.iter().find(|model| &model.source == source) {
            return Ok(NativeColorModelCapability::Available(model.clone()));
        }
        Ok(if self.unavailable {
            NativeColorModelCapability::Unavailable(NativeColorModelUnavailable {
                source: source.clone(),
                reason: NativeColorUnavailableReason::MissingRevision,
                detail: "not available in this capture".into(),
            })
        } else {
            NativeColorModelCapability::Available(self.model.clone())
        })
    }
}
struct Source {
    target: FixtureId,
    value: AttributeValue,
}
impl ScalarSourceResolver for Source {
    fn current(&self, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        None
    }
    fn preset(&self, _: &str, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        None
    }
}
impl DynamicTickSource for Source {
    fn value(&self, target: FixtureId, key: &AttributeKey) -> Option<&AttributeValue> {
        (target == self.target && *key == ProgrammingOwner::Color.key()).then_some(&self.value)
    }
}
fn address(model: &Model, index: Option<u128>) -> DynamicValueAddress {
    DynamicValueAddress {
        representation: DynamicFamilyRepresentation::DirectColor {
            source: model.source.clone(),
        },
        component: index.map(|i| ProgrammingComponent::NativeColor(binding(i))),
    }
}
fn no_adoption(
    _: FixtureId,
    _: &AttributeValue,
    _: &DynamicValueAddress,
) -> Result<AttributeValue, TransitionError> {
    panic!("native Current must be verified before adoption")
}
fn semantic() -> AttributeValue {
    AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
        intent: ColorIntent::default(),
    }))
}
fn semantic_address() -> DynamicValueAddress {
    DynamicValueAddress {
        representation: DynamicFamilyRepresentation::SemanticColor {
            basis: light_dynamics::DynamicSemanticColorBasis::Whole,
        },
        component: None,
    }
}
fn other_model() -> Arc<Model> {
    let mut result = model();
    Arc::get_mut(&mut result).unwrap().source.profile_id = Uuid::from_u128(100);
    result
}
fn read_frame(
    source: &Source,
    models: &Models,
    cache: &RefCell<CurrentNativeVerificationCache>,
) -> Result<(), TransitionError> {
    cache.borrow_mut().begin_frame();
    let typed = CapturedProgrammingSources::new(source, &no_adoption, None)
        .with_native_current_validation(models, cache);
    let result = (|| {
        for index in [0, 1, 0] {
            typed.try_current(source.target, &address(&models.model, Some(index)))?;
        }
        typed.try_current_family_base(source.target, &address(&models.model, None))?;
        typed.check()
    })();
    drop(typed);
    cache.borrow_mut().finish_frame();
    result
}

#[test]
fn paused_reads_share_one_complete_proof_until_value_or_model_arc_changes() {
    let first = model();
    let mut models = Models::new(first.clone());
    let mut source = Source {
        target: FixtureId::new(),
        value: value(&first, &[21, 37]),
    };
    let cache = RefCell::new(CurrentNativeVerificationCache::default());
    for _ in 0..3 {
        read_frame(&source, &models, &cache).unwrap();
    }
    assert_eq!(first.predicts.load(Ordering::Relaxed), 1);
    assert_eq!(
        models.lookups.load(Ordering::Relaxed),
        3,
        "capabilities refresh per capture, not per address"
    );
    // Equal numeric data in a new immutable occurrence must be verified, never value-matched.
    source.value = value(&first, &[21, 37]);
    read_frame(&source, &models, &cache).unwrap();
    assert_eq!(first.predicts.load(Ordering::Relaxed), 2);
    let replacement = model();
    models.model = replacement.clone();
    read_frame(&source, &models, &cache).unwrap();
    assert_eq!(
        replacement.predicts.load(Ordering::Relaxed),
        1,
        "same identity with a new verified model object needs a new proof"
    );
}

#[test]
fn incomplete_and_out_of_function_current_are_invalid_before_extraction_or_adoption() {
    for raws in [vec![21], vec![21, 256]] {
        let model = model();
        let models = Models::new(model.clone());
        let source = Source {
            target: FixtureId::new(),
            value: value(&model, &raws),
        };
        let cache = RefCell::new(CurrentNativeVerificationCache::default());
        cache.borrow_mut().begin_frame();
        let typed = CapturedProgrammingSources::new(&source, &no_adoption, None)
            .with_native_current_validation(&models, &cache);
        let current = typed.try_current(source.target, &address(&model, Some(0)));
        assert!(matches!(current, Err(TransitionError::Invalid(_))));
        assert!(matches!(
            typed.try_current_family_base(source.target, &address(&model, None)),
            Err(TransitionError::Invalid(_))
        ));
        let semantic = DynamicValueAddress {
            representation: DynamicFamilyRepresentation::SemanticColor {
                basis: light_dynamics::DynamicSemanticColorBasis::Whole,
            },
            component: None,
        };
        assert!(matches!(
            typed.try_current(source.target, &semantic),
            Err(TransitionError::Invalid(_))
        ));
        assert!(typed.check().is_err());
        assert_eq!(
            model.predicts.load(Ordering::Relaxed),
            1,
            "different reads share the same invalid immutable proof"
        );
        cache.borrow_mut().finish_frame();
    }
}

#[test]
fn whole_master_zero_current_is_verified_without_a_component_read() {
    let model = model();
    let models = Models::new(model.clone());
    let source = Source {
        target: FixtureId::new(),
        value: value(&model, &[21]),
    };
    let cache = RefCell::new(CurrentNativeVerificationCache::default());
    cache.borrow_mut().begin_frame();
    let typed = CapturedProgrammingSources::new(&source, &no_adoption, None)
        .with_native_current_validation(&models, &cache);
    assert!(matches!(
        typed.try_current_family_base(source.target, &address(&model, None)),
        Err(TransitionError::Invalid(_))
    ));
    assert!(typed.check().is_err());
    assert_eq!(model.predicts.load(Ordering::Relaxed), 1);
    cache.borrow_mut().finish_frame();
}

#[test]
fn unavailable_current_preserves_payload_and_recovers_in_a_later_capture() {
    let model = model();
    let mut models = Models::new(model.clone());
    let source = Source {
        target: FixtureId::new(),
        value: value(&model, &[21, 37]),
    };
    let saved = source.value.clone();
    let cache = RefCell::new(CurrentNativeVerificationCache::default());
    read_frame(&source, &models, &cache).unwrap();
    models.unavailable = true;
    cache.borrow_mut().begin_frame();
    let typed = CapturedProgrammingSources::new(&source, &no_adoption, None)
        .with_native_current_validation(&models, &cache);
    assert!(matches!(
        typed.try_current(source.target, &address(&model, Some(0))),
        Err(TransitionError::Requires(
            TransitionRequirement::NativeColorModel
        ))
    ));
    assert_eq!(
        typed
            .try_current_family_base(source.target, &address(&model, None))
            .unwrap(),
        Some(saved.clone()),
        "opaque whole/Size Current carries intent without claiming a native proof"
    );
    assert_eq!(
        typed
            .try_current(source.target, &address(&model, None))
            .unwrap(),
        Some(DynamicValue::Family(saved.clone()))
    );
    typed.check().unwrap();
    assert!(
        typed
            .requirements()
            .iter()
            .any(|r| r.target == source.target
                && r.requirement == TransitionRequirement::NativeColorModel)
    );
    drop(typed);
    cache.borrow_mut().finish_frame();
    assert!(
        cache.borrow().entries.is_empty(),
        "old available proof cannot mask a missing current capability"
    );
    assert_eq!(source.value, saved);
    models.unavailable = false;
    read_frame(&source, &models, &cache).unwrap();
    assert_eq!(model.predicts.load(Ordering::Relaxed), 2);
}

#[test]
fn incomplete_adopted_direct_is_rejected_from_semantic_and_other_native_sources() {
    for original_is_native in [false, true] {
        for raws in [vec![21], vec![21, 256]] {
            let original = model();
            let adopted_model = other_model();
            let mut models = Models::new(original.clone());
            models.additional.push(adopted_model.clone());
            let source = Source {
                target: FixtureId::new(),
                value: if original_is_native {
                    value(&original, &[10, 20])
                } else {
                    semantic()
                },
            };
            let adopted = value(&adopted_model, &raws);
            let adopt =
                |_: FixtureId, _: &AttributeValue, _: &DynamicValueAddress| Ok(adopted.clone());
            let cache = RefCell::new(CurrentNativeVerificationCache::default());
            cache.borrow_mut().begin_frame();
            let typed = CapturedProgrammingSources::new(&source, &adopt, None)
                .with_native_current_validation(&models, &cache);
            assert!(
                matches!(
                    typed.try_current(source.target, &address(&adopted_model, Some(0))),
                    Err(TransitionError::Invalid(_))
                ),
                "a valid requested channel cannot certify incomplete or invalid adopted siblings"
            );
            assert!(typed.check().is_err());
            assert_eq!(adopted_model.predicts.load(Ordering::Relaxed), 1);
            assert_eq!(
                original.predicts.load(Ordering::Relaxed),
                usize::from(original_is_native)
            );
            cache.borrow_mut().finish_frame();
        }
    }
}

#[test]
fn original_and_adopted_direct_proofs_stay_warm_and_retire_independently() {
    let original = model();
    let adopted_model = other_model();
    let mut models = Models::new(original.clone());
    models.additional.push(adopted_model.clone());
    let source = Source {
        target: FixtureId::new(),
        value: value(&original, &[10, 20]),
    };
    let adopted = value(&adopted_model, &[30, 40]);
    let adopt = |_: FixtureId, _: &AttributeValue, _: &DynamicValueAddress| Ok(adopted.clone());
    let cache = RefCell::new(CurrentNativeVerificationCache::default());
    for _ in 0..3 {
        cache.borrow_mut().begin_frame();
        let typed = CapturedProgrammingSources::new(&source, &adopt, None)
            .with_native_current_validation(&models, &cache);
        assert_eq!(
            typed
                .try_current(source.target, &address(&adopted_model, Some(0)))
                .unwrap(),
            Some(DynamicValue::Native(30))
        );
        assert_eq!(
            typed
                .try_current(source.target, &address(&adopted_model, Some(1)))
                .unwrap(),
            Some(DynamicValue::Native(40))
        );
        assert_eq!(
            typed
                .try_current(source.target, &address(&original, Some(0)))
                .unwrap(),
            Some(DynamicValue::Native(10))
        );
        typed.check().unwrap();
        drop(typed);
        cache.borrow_mut().finish_frame();
        assert_eq!(
            cache.borrow().entries[&(source.target, ProgrammingOwner::Color)].len(),
            2
        );
    }
    assert_eq!(original.predicts.load(Ordering::Relaxed), 1);
    assert_eq!(adopted_model.predicts.load(Ordering::Relaxed), 1);
    read_frame(&source, &models, &cache).unwrap();
    assert_eq!(
        cache.borrow().entries[&(source.target, ProgrammingOwner::Color)].len(),
        1,
        "an unqueried adopted projection cannot grow history"
    );
}

#[test]
fn missing_original_model_allows_portable_adoption_and_a_verified_native_destination() {
    let original = model();
    let adopted_model = other_model();
    let mut models = Models::new(original.clone());
    models.unavailable = true;
    models.additional.push(adopted_model.clone());
    let source = Source {
        target: FixtureId::new(),
        value: value(&original, &[10, 20]),
    };
    let portable = semantic();
    let adopted = value(&adopted_model, &[30, 40]);
    let adopt = |_: FixtureId, _: &AttributeValue, address: &DynamicValueAddress| {
        Ok(
            if matches!(
                address.representation,
                DynamicFamilyRepresentation::SemanticColor { .. }
            ) {
                portable.clone()
            } else {
                adopted.clone()
            },
        )
    };
    let cache = RefCell::new(CurrentNativeVerificationCache::default());
    cache.borrow_mut().begin_frame();
    let typed = CapturedProgrammingSources::new(&source, &adopt, None)
        .with_native_current_validation(&models, &cache);
    assert_eq!(
        typed
            .try_current(source.target, &semantic_address())
            .unwrap(),
        Some(DynamicValue::Family(portable.clone()))
    );
    assert_eq!(
        typed
            .try_current(source.target, &address(&adopted_model, Some(0)))
            .unwrap(),
        Some(DynamicValue::Native(30))
    );
    assert!(
        typed.requirements().is_empty(),
        "unused original native capability is not a conversion requirement"
    );
    typed.check().unwrap();
    assert_eq!(original.predicts.load(Ordering::Relaxed), 0);
    assert_eq!(adopted_model.predicts.load(Ordering::Relaxed), 1);
    drop(typed);
    cache.borrow_mut().finish_frame();
    assert_eq!(
        cache.borrow().entries[&(source.target, ProgrammingOwner::Color)].len(),
        1,
        "no proof claimed for the unavailable original"
    );
}

#[test]
fn proofs_are_branch_local_bounded_and_do_not_keep_source_or_model_alive() {
    let model = model();
    let models = Models::new(model.clone());
    let source = Source {
        target: FixtureId::new(),
        value: value(&model, &[21, 37]),
    };
    let left = RefCell::new(CurrentNativeVerificationCache::default());
    let right = RefCell::new(CurrentNativeVerificationCache::default());
    read_frame(&source, &models, &left).unwrap();
    read_frame(&source, &models, &right).unwrap();
    assert_eq!(model.predicts.load(Ordering::Relaxed), 2);
    left.borrow_mut().begin_frame();
    left.borrow_mut().finish_frame();
    assert!(left.borrow().entries.is_empty());
    assert_eq!(right.borrow().entries.len(), 1);
    read_frame(&source, &models, &left).unwrap();
    left.borrow_mut().clear();
    assert!(left.borrow().entries.is_empty());
    let AttributeValue::ColorProgram(program) = &source.value else {
        panic!()
    };
    let weak_value = Arc::downgrade(program);
    let weak_model = Arc::downgrade(&model);
    drop(source);
    drop(models);
    drop(model);
    assert!(weak_value.upgrade().is_none());
    assert!(weak_model.upgrade().is_none());
    assert_eq!(
        right.borrow().entries.len(),
        1,
        "only weak immutable identities remain until next frame retirement"
    );
}

#[test]
fn wrong_original_model_identity_is_invalid_and_default_adapter_remains_explicitly_opt_in() {
    let original = model();
    let mut wrong = model();
    Arc::get_mut(&mut wrong).unwrap().source.profile_revision += 1;
    let models = Models::new(wrong);
    let source = Source {
        target: FixtureId::new(),
        value: value(&original, &[21, 37]),
    };
    let cache = RefCell::new(CurrentNativeVerificationCache::default());
    assert!(matches!(
        read_frame(&source, &models, &cache),
        Err(TransitionError::Invalid(_))
    ));
    let malformed = Source {
        target: source.target,
        value: value(&original, &[21]),
    };
    let old = CapturedProgrammingSources::new(&malformed, &no_adoption, None);
    assert!(
        old.try_current_family_base(malformed.target, &address(&original, None))
            .unwrap()
            .is_some(),
        "legacy adapter paths do not silently claim complete recipe verification"
    );
    assert_eq!(original.predicts.load(Ordering::Relaxed), 0);
}
