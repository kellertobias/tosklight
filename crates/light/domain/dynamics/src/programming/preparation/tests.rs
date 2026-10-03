use super::*;
mod deferred_angle_pair;
mod leaf_provenance;
mod passive_requirements;
mod plain_leaves;
mod position_forest;
use crate::DynamicSampleExpression as E;
use light_core::{AttributeKey, AttributeValue, NativeColorBinding, NativeColorIdentity};
use std::{
    cell::Cell,
    sync::atomic::{AtomicUsize, Ordering},
};

#[derive(Default)]
struct Sources {
    pan: Option<f32>,
    tilt: Option<f32>,
    reads: Cell<usize>,
}
impl DynamicValueSourceResolver for Sources {
    fn current(&self, _: FixtureId, address: &DynamicValueAddress) -> Option<DynamicValue> {
        self.reads.set(self.reads.get() + 1);
        match address.component {
            Some(ProgrammingComponent::Pan) => self.pan,
            Some(ProgrammingComponent::Tilt) => self.tilt,
            _ => None,
        }
        .map(DynamicValue::Scalar)
    }
    fn preset(
        &self,
        _: &DynamicPresetSourceBinding,
        _: Uuid,
        _: FixtureId,
    ) -> Option<DynamicValue> {
        None
    }
    fn current_dependency(
        &self,
        _: FixtureId,
        address: &DynamicValueAddress,
    ) -> DynamicSourceDependency {
        // These mock values are already compatible angles; no geometry adoption occurs.
        DynamicSourceDependency::compatible(None, address)
    }
}

struct Frame;
impl WholeFamilyExpressionFrameResolver for Frame {
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

fn sample(lane: u128, expression: E) -> DynamicRuntimeSample {
    DynamicRuntimeSample {
        instance_id: Uuid::from_u128(1),
        controller_id: Uuid::from_u128(2),
        target: FixtureId(Uuid::from_u128(3)),
        lane_id: Uuid::from_u128(lane),
        expression,
        priority: 10,
        activated_at_millis: 100,
        activation_mix: 1.0,
        address: None,
    }
}
fn angle_address(component: ProgrammingComponent) -> Arc<DynamicValueAddress> {
    Arc::new(DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Angles,
        component: Some(component),
    })
}
fn angle(component: ProgrammingComponent, value: f32) -> E {
    E::Programming {
        address: angle_address(component),
        value: DynamicValue::Scalar(value),
        occurrence: None,
        dependency_occurrence: None,
    }
}
fn current(component: ProgrammingComponent) -> E {
    E::AngleCurrent {
        address: angle_address(component),
    }
}
fn angles(pan: f32, tilt: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(pan, tilt)))
}
fn whole(owner: ProgrammingOwner, value: AttributeValue) -> E {
    E::Programming {
        address: Arc::new(DynamicValueAddress::whole_family(owner, &value).unwrap()),
        value: DynamicValue::Family(value),
        occurrence: None,
        dependency_occurrence: None,
    }
}
fn color(component: ColorComponent, value: f32) -> E {
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
    E::Programming {
        address: Arc::new(DynamicValueAddress {
            representation: DynamicFamilyRepresentation::SemanticColor { basis },
            component: Some(ProgrammingComponent::Color(component)),
        }),
        value: DynamicValue::Scalar(value),
        occurrence: None,
        dependency_occurrence: None,
    }
}
fn resume(from: Option<E>, to: Option<E>, progress: f32) -> E {
    E::Transition {
        from: from.map(Arc::new),
        to: to.map(Arc::new),
        progress,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(500),
        },
    }
}
fn semantic(intent: ColorIntent) -> AttributeValue {
    AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic { intent }))
}
fn compose(
    group: &DynamicFamilySampleGroup,
    base: &AttributeValue,
) -> Result<AttributeValue, TransitionError> {
    compose_retained_dynamic_family(
        group.owner,
        base,
        &group.samples,
        &FamilyCompositionContext {
            edit: FamilyEditContext {
                color_model: Some(&VirtualColorAuthoringV1),
                ..Default::default()
            },
            ..Default::default()
        },
        &Frame,
        &mut RetainedFamilyCompositionScratch::default(),
    )
}

#[test]
fn pan_and_explicit_current_tilt_form_one_complete_pair_and_refresh_current() {
    let mut samples = [
        sample(10, angle(ProgrammingComponent::Pan, 90.0)),
        sample(11, current(ProgrammingComponent::Tilt)),
    ];
    for sample in &mut samples {
        sample.activation_mix = 0.5;
    }
    let mut scratch = DynamicFamilyPreparationScratch::default();
    for tilt in [30.0, 50.0] {
        let sources = Sources {
            tilt: Some(tilt),
            ..Default::default()
        };
        let prepared =
            prepare_dynamic_family_samples(&samples, &sources, None, &mut scratch).unwrap();
        assert_eq!(prepared.families.len(), 1);
        assert!(prepared.legacy.is_empty());
        let group = &prepared.families[0];
        assert_eq!(group.owner, ProgrammingOwner::Position);
        assert_eq!(group.target, samples[0].target);
        assert_eq!(group.samples.len(), 1);
        let FamilyCompositionSample::CoupledExpression {
            expression,
            rank,
            activation_mix,
        } = &group.samples[0]
        else {
            panic!("complete leaf")
        };
        assert!(
            matches!(expression.footprint(CoupledExpressionRole::Base), CoupledExpressionFootprint::Exact { value, .. } if value == &DynamicValue::Family(angles(90.0, tilt)))
        );
        assert_eq!(
            rank.dynamic_identity().unwrap().controller_id,
            samples[0].controller_id
        );
        assert_eq!(rank.dynamic_identity().unwrap().lane_id, samples[1].lane_id);
        assert_eq!(*activation_mix, 0.5);
        assert_eq!(
            compose(group, &angles(0.0, 0.0)).unwrap(),
            angles(45.0, tilt * 0.5)
        );
        assert_eq!(sources.reads.get(), 1);
    }
}

#[test]
fn incomplete_controllers_cannot_supply_each_others_missing_axes() {
    let first = sample(10, angle(ProgrammingComponent::Pan, 90.0));
    let mut second = sample(11, angle(ProgrammingComponent::Tilt, 60.0));
    second.controller_id = Uuid::from_u128(20);
    let sources = Sources {
        pan: Some(5.0),
        tilt: Some(6.0),
        ..Default::default()
    };
    let mut scratch = DynamicFamilyPreparationScratch::default();
    for samples in [[first.clone(), second.clone()], [second, first]] {
        let prepared =
            prepare_dynamic_family_samples(&samples, &sources, None, &mut scratch).unwrap();
        assert!(prepared.families.is_empty());
    }
    assert_eq!(
        sources.reads.get(),
        0,
        "only explicit Current tokens read the static frame"
    );
}

#[test]
fn angle_to_target_resume_keeps_geometry_deferred_and_exact_target_endpoint() {
    let target = AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Point {
            point_id: Uuid::from_u128(90),
        },
        [1.0, 2.0, 3.0],
    )));
    let mut scratch = DynamicFamilyPreparationScratch::default();
    let sources = Sources {
        tilt: Some(30.0),
        ..Default::default()
    };
    for progress in [0.25, 1.0] {
        let samples = [
            sample(
                10,
                resume(
                    Some(angle(ProgrammingComponent::Pan, 90.0)),
                    Some(whole(ProgrammingOwner::Position, target.clone())),
                    progress,
                ),
            ),
            sample(
                11,
                resume(Some(current(ProgrammingComponent::Tilt)), None, progress),
            ),
        ];
        let prepared =
            prepare_dynamic_family_samples(&samples, &sources, None, &mut scratch).unwrap();
        assert_eq!(prepared.families[0].samples.len(), 1);
        if progress == 1.0 {
            assert_eq!(
                compose(&prepared.families[0], &angles(0.0, 0.0)).unwrap(),
                target
            );
        } else {
            assert!(matches!(
                &prepared.families[0].samples[0],
                FamilyCompositionSample::CoupledExpression { .. }
            ));
            assert!(matches!(
                compose(&prepared.families[0], &angles(0.0, 0.0)),
                Err(TransitionError::Requires(_))
            ));
        }
    }
}

#[test]
fn uv_release_retains_its_component_mask_and_activation_separate_from_visible_base() {
    let initial = ColorIntent {
        uv: UvIntent { amount: 0.2 },
        white_blend: 0.3,
        ..Default::default()
    };
    let mut uv = sample(11, resume(Some(color(ColorComponent::Uv, 0.8)), None, 0.5));
    uv.activation_mix = 0.5;
    let samples = [
        sample(
            10,
            whole(ProgrammingOwner::Color, semantic(initial.clone())),
        ),
        uv,
    ];
    let mut scratch = DynamicFamilyPreparationScratch::default();
    let prepared =
        prepare_dynamic_family_samples(&samples, &Sources::default(), None, &mut scratch).unwrap();
    assert_eq!(prepared.families.len(), 1);
    let group = &prepared.families[0];
    let FamilyCompositionSample::Known(component) = &group.samples[1] else {
        panic!("component projection")
    };
    assert_eq!(
        component.address().address().component,
        Some(ProgrammingComponent::Color(ColorComponent::Uv))
    );
    let mut expected = initial.clone();
    expected.uv.amount = 0.35;
    let result = compose(group, &semantic(initial)).unwrap();
    let AttributeValue::ColorProgram(result) = result else {
        panic!()
    };
    let ColorProgram::Semantic { intent } = result.as_ref() else {
        panic!()
    };
    assert!((intent.uv.amount - expected.uv.amount).abs() < 0.00001);
    expected.uv.amount = intent.uv.amount;
    assert_eq!(intent, &expected);
}

#[test]
fn incompatible_component_bases_use_coupled_cohorts_without_promoting_plain_target_offsets() {
    let mut scratch = DynamicFamilyPreparationScratch::default();
    let inputs = [sample(
        10,
        resume(
            Some(color(ColorComponent::Red, 0.8)),
            Some(color(ColorComponent::Hue, 240.0)),
            0.5,
        ),
    )];
    let prepared =
        prepare_dynamic_family_samples(&inputs, &Sources::default(), None, &mut scratch).unwrap();
    assert!(matches!(
        prepared.families[0].samples[0],
        FamilyCompositionSample::CoupledExpression { .. }
    ));
    let inputs = [sample(
        11,
        E::Programming {
            address: Arc::new(DynamicValueAddress {
                representation: DynamicFamilyRepresentation::Target {
                    reference: Some(TargetReference::Origin),
                },
                component: Some(ProgrammingComponent::TargetX),
            }),
            value: DynamicValue::Scalar(4.0),
            occurrence: None,
            dependency_occurrence: None,
        },
    )];
    let prepared =
        prepare_dynamic_family_samples(&inputs, &Sources::default(), None, &mut scratch).unwrap();
    let FamilyCompositionSample::Known(component) = &prepared.families[0].samples[0] else {
        panic!()
    };
    assert_eq!(
        component.address().address().component,
        Some(ProgrammingComponent::TargetX)
    );
}

#[test]
fn completed_angle_to_target_component_uses_target_mask_and_interior_stays_deferred() {
    let target_x = || E::Programming {
        address: Arc::new(DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Target {
                reference: Some(TargetReference::Origin),
            },
            component: Some(ProgrammingComponent::TargetX),
        }),
        value: DynamicValue::Scalar(4.0),
        occurrence: None,
        dependency_occurrence: None,
    };
    let sources = Sources {
        tilt: Some(20.0),
        ..Default::default()
    };
    let mut scratch = DynamicFamilyPreparationScratch::default();
    for progress in [1.0, 0.5] {
        let inputs = [
            sample(
                10,
                resume(
                    Some(angle(ProgrammingComponent::Pan, 90.0)),
                    Some(target_x()),
                    progress,
                ),
            ),
            sample(
                11,
                resume(Some(current(ProgrammingComponent::Tilt)), None, progress),
            ),
        ];
        if progress == 1.0 {
            let prepared =
                prepare_dynamic_family_samples(&inputs, &sources, None, &mut scratch).unwrap();
            assert_eq!(prepared.families[0].samples.len(), 1);
            let FamilyCompositionSample::Known(value) = &prepared.families[0].samples[0] else {
                panic!()
            };
            assert_eq!(
                value.address().address().component,
                Some(ProgrammingComponent::TargetX)
            );
            assert_eq!(
                sources.reads.get(),
                0,
                "inactive Angle partner does not sample Current"
            );
        } else {
            let prepared =
                prepare_dynamic_family_samples(&inputs, &sources, None, &mut scratch).unwrap();
            assert!(matches!(
                &prepared.families[0].samples[0],
                FamilyCompositionSample::CoupledExpression { .. }
            ));
        }
    }
}

#[test]
fn position_bundle_preserves_non_position_and_legacy_remainders() {
    let inputs = [
        sample(
            10,
            resume(
                Some(angle(ProgrammingComponent::Pan, 90.0)),
                Some(color(ColorComponent::Uv, 0.8)),
                0.25,
            ),
        ),
        sample(
            11,
            resume(
                Some(current(ProgrammingComponent::Tilt)),
                Some(E::LegacyScalar {
                    attribute: AttributeKey::intensity(),
                    value: 0.6,
                    occurrence: None,
                    dependency_occurrence: None,
                }),
                0.25,
            ),
        ),
    ];
    let mut scratch = DynamicFamilyPreparationScratch::default();
    let prepared = prepare_dynamic_family_samples(
        &inputs,
        &Sources {
            tilt: Some(30.0),
            ..Default::default()
        },
        None,
        &mut scratch,
    )
    .unwrap();
    assert_eq!(prepared.families.len(), 2);
    assert!(
        prepared
            .families
            .iter()
            .any(|group| group.owner == ProgrammingOwner::Position)
    );
    assert!(
        prepared
            .families
            .iter()
            .any(|group| group.owner == ProgrammingOwner::Color)
    );
    assert_eq!(prepared.legacy.len(), 1);
    let mut contributions = Vec::new();
    assert!(prepared.legacy[0].expression.visit_legacy_contributions(
        |attribute, value, influence| contributions.push((attribute.clone(), value, influence))
    ));
    assert_eq!(contributions, vec![(AttributeKey::intensity(), 0.6, 0.25)]);
    assert_eq!(prepared.legacy[0].activation_mix, 1.0);
    assert_eq!(prepared.legacy[0].lane_id, inputs[1].lane_id);
}

#[test]
fn cross_owner_hot_edit_splits_retained_roots_and_keeps_legacy_influence() {
    let input = sample(
        10,
        resume(
            Some(color(ColorComponent::Uv, 0.8)),
            Some(E::LegacyScalar {
                attribute: AttributeKey::intensity(),
                value: 0.4,
                occurrence: None,
                dependency_occurrence: None,
            }),
            0.25,
        ),
    );
    let mut scratch = DynamicFamilyPreparationScratch::default();
    let prepared =
        prepare_dynamic_family_samples(&[input], &Sources::default(), None, &mut scratch).unwrap();
    assert_eq!(prepared.families.len(), 1);
    assert_eq!(prepared.families[0].owner, ProgrammingOwner::Color);
    assert_eq!(prepared.legacy.len(), 1);
    let mut influence = None;
    assert!(
        prepared.legacy[0]
            .expression
            .visit_legacy_contributions(|_, _, weight| influence = Some(weight))
    );
    assert_eq!(influence, Some(0.25));
}

struct Model {
    source: NativeColorIdentity,
    binding: NativeColorBinding,
}
impl NativeColorEditModel for Model {
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
        panic!("preparing a native component does not predict a whole recipe")
    }
}
struct Models {
    model: Arc<Model>,
    calls: AtomicUsize,
    fail: bool,
}
impl DynamicNativeModelResolver for Models {
    fn resolve(
        &self,
        source: &NativeColorIdentity,
    ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, IntentError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        assert_eq!(source, &self.model.source);
        if self.fail {
            return Err(IntentError("original source unavailable".into()));
        }
        Ok(self.model.clone())
    }
}
fn models() -> Models {
    Models {
        model: Arc::new(Model {
            source: NativeColorIdentity {
                profile_id: Uuid::from_u128(101),
                profile_revision: 2,
                profile_digest: "original".into(),
                mode_id: Uuid::from_u128(102),
                head_id: Uuid::from_u128(103),
                path_id: Uuid::from_u128(104),
                model_revision: 1,
                native_layout_signature: "original-layout".into(),
            },
            binding: NativeColorBinding {
                channel_id: Uuid::from_u128(105),
                function_id: Uuid::from_u128(106),
            },
        }),
        calls: AtomicUsize::new(0),
        fail: false,
    }
}

#[test]
fn native_leaf_preserves_u32_and_resolves_only_its_original_identity_without_stale_cache() {
    let mut models = models();
    let inputs = [sample(
        10,
        E::Programming {
            address: Arc::new(DynamicValueAddress {
                representation: DynamicFamilyRepresentation::DirectColor {
                    source: models.model.source.clone(),
                },
                component: Some(ProgrammingComponent::NativeColor(models.model.binding)),
            }),
            value: DynamicValue::Native(u32::MAX - 2),
            occurrence: None,
            dependency_occurrence: None,
        },
    )];
    let mut scratch = DynamicFamilyPreparationScratch::default();
    for _ in 0..2 {
        let prepared = prepare_dynamic_family_samples(
            &inputs,
            &Sources::default(),
            Some(&models),
            &mut scratch,
        )
        .unwrap();
        let FamilyCompositionSample::Known(value) = &prepared.families[0].samples[0] else {
            panic!()
        };
        assert_eq!(
            value.materialized_value(),
            Some(&DynamicValue::Native(u32::MAX - 2))
        );
    }
    assert_eq!(models.calls.load(Ordering::Relaxed), 2);
    models.fail = true;
    assert!(matches!(
        prepare_dynamic_family_samples(&inputs, &Sources::default(), Some(&models), &mut scratch),
        Err(TransitionError::Invalid(_))
    ));
    assert!(scratch.families.is_empty());
    assert!(scratch.cache.is_empty());
    let unavailable =
        prepare_dynamic_family_samples(&inputs, &Sources::default(), None, &mut scratch).unwrap();
    assert!(unavailable.families.is_empty());
    assert_eq!(unavailable.requirements.len(), 1);
    assert!(matches!(
        &unavailable.requirements[0].reason,
        DynamicFamilyPreparationRequirementReason::NativeColorModelUnavailable(reason)
            if reason.reason == crate::NativeColorUnavailableReason::MissingResolver
    ));
}

#[test]
fn unavailable_foreign_direct_size_base_is_scoped_and_exact_full_size_prunes_it() {
    let model = models();
    let direct = AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
        recipe: NativeColorRecipe {
            source: model.model.source.clone(),
            channels: vec![light_core::NativeColorValue {
                channel_id: model.model.binding.channel_id,
                function_id: model.model.binding.function_id,
                raw: u32::MAX - 1,
            }],
            spreads: vec![],
        },
        portable: PortableColorEstimate {
            model_revision: 1,
            visible: None,
            uv: None,
            quality: light_core::PhysicalDataQuality::Unknown,
            limitations: vec![],
        },
    }));
    let wanted = AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
        intent: ColorIntent {
            uv: UvIntent { amount: 0.6 },
            ..Default::default()
        },
    }));
    let address =
        Arc::new(DynamicValueAddress::whole_family(ProgrammingOwner::Color, &wanted).unwrap());
    let expression = |factor| E::Scale {
        address: address.clone(),
        base: DynamicValue::Family(direct.clone()),
        value: Arc::new(whole(ProgrammingOwner::Color, wanted.clone())),
        factor,
        baseline_occurrence: None,
    };
    let focus = sample(
        11,
        whole(ProgrammingOwner::Focus, AttributeValue::Normalized(0.35)),
    );
    let mut scratch = DynamicFamilyPreparationScratch::default();
    for factor in [0.0, 0.5, 1.0] {
        let inputs = [sample(10, expression(factor)), focus.clone()];
        let prepared =
            prepare_dynamic_family_samples(&inputs, &Sources::default(), None, &mut scratch)
                .unwrap();
        assert!(
            prepared
                .families
                .iter()
                .any(|group| group.owner == ProgrammingOwner::Focus)
        );
        if factor < 1.0 {
            assert_eq!(prepared.families.len(), 1);
            assert_eq!(
                prepared.requirements,
                &[DynamicFamilyPreparationRequirement {
                    target: inputs[0].target,
                    owner: ProgrammingOwner::Color,
                    rank: rank(&inputs[0]),
                    reason: DynamicFamilyPreparationRequirementReason::NativeColorModelUnavailable(
                        crate::NativeColorModelUnavailable {
                            source: model.model.source.clone(),
                            reason: crate::NativeColorUnavailableReason::MissingResolver,
                            detail: "Original native Color model is not available".into(),
                        }
                    ),
                }]
            );
        } else {
            assert!(prepared.requirements.is_empty());
            assert_eq!(prepared.families.len(), 2);
        }
    }
    // Pruning removes model work, never structural validation of saved payloads.
    let mut invalid = expression(1.0);
    let E::Scale {
        base: DynamicValue::Family(AttributeValue::ColorProgram(program)),
        ..
    } = &mut invalid
    else {
        panic!()
    };
    let ColorProgram::Direct { recipe, .. } = Arc::make_mut(program) else {
        panic!()
    };
    recipe.source.profile_id = Uuid::nil();
    assert!(matches!(
        prepare_dynamic_family_samples(
            &[sample(10, invalid)],
            &Sources::default(),
            None,
            &mut scratch
        ),
        Err(TransitionError::Invalid(_))
    ));
}

#[test]
fn unchanged_tree_reuses_owned_cache_but_activation_progress_and_membership_remain_current() {
    let mut input = sample(10, resume(Some(color(ColorComponent::Uv, 0.8)), None, 0.25));
    let mut scratch = DynamicFamilyPreparationScratch::default();
    prepare_dynamic_family_samples(&[input.clone()], &Sources::default(), None, &mut scratch)
        .unwrap();
    let original = scratch.cache.values().next().unwrap().expression.clone();
    input.activation_mix = 0.2;
    {
        let prepared = prepare_dynamic_family_samples(
            &[input.clone()],
            &Sources::default(),
            None,
            &mut scratch,
        )
        .unwrap();
        let FamilyCompositionSample::Known(value) = &prepared.families[0].samples[0] else {
            panic!()
        };
        assert_eq!(value.activation_mix, 0.2);
    }
    assert!(Arc::ptr_eq(
        &original,
        &scratch.cache.values().next().unwrap().expression
    ));
    input.expression = resume(Some(color(ColorComponent::Uv, 0.8)), None, 0.5);
    prepare_dynamic_family_samples(&[input], &Sources::default(), None, &mut scratch).unwrap();
    assert!(!Arc::ptr_eq(
        &original,
        &scratch.cache.values().next().unwrap().expression
    ));
    prepare_dynamic_family_samples(&[], &Sources::default(), None, &mut scratch).unwrap();
    assert!(scratch.cache.is_empty());
    assert!(scratch.families.is_empty());
}
