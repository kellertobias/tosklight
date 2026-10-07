use super::*;
use light_core::{NativeColorBinding, NativeColorValue, PhysicalDataQuality};

fn rank(controller: u128, lane: u128) -> FamilySampleRank {
    FamilySampleRank {
        priority: 3,
        changed_at_millis: 17,
        changed_at_submillis_nanos: 0,
        stable_order: lane,
        identity: crate::FamilySampleIdentity::Dynamic {
            instance_id: Uuid::from_u128(1),
            controller_id: Uuid::from_u128(controller),
            lane_id: Uuid::from_u128(lane),
        },
    }
}

fn color(component: ColorComponent, value: f32) -> Arc<DynamicSampleExpression> {
    Arc::new(DynamicSampleExpression::Programming {
        address: Arc::new(DynamicValueAddress {
            representation: DynamicFamilyRepresentation::SemanticColor {
                basis: DynamicSemanticColorBasis::Retain,
            },
            component: Some(ProgrammingComponent::Color(component)),
        }),
        value: DynamicValue::Scalar(value),
        occurrence: None,
        dependency_occurrence: None,
    })
}

fn focus(value: f32) -> Arc<DynamicSampleExpression> {
    let value = AttributeValue::Normalized(value);
    Arc::new(DynamicSampleExpression::Programming {
        address: Arc::new(
            DynamicValueAddress::whole_family(ProgrammingOwner::Focus, &value).unwrap(),
        ),
        value: DynamicValue::Family(value),
        occurrence: None,
        dependency_occurrence: None,
    })
}

fn resume(
    from: Option<Arc<DynamicSampleExpression>>,
    to: Option<Arc<DynamicSampleExpression>>,
    occurrence_id: Uuid,
) -> Arc<DynamicSampleExpression> {
    Arc::new(DynamicSampleExpression::Transition {
        from,
        to,
        progress: 0.4,
        reason: DynamicTransitionReason::Resume { occurrence_id },
    })
}

fn progress(mut node: Arc<DynamicSampleExpression>, value: f32) -> Arc<DynamicSampleExpression> {
    let DynamicSampleExpression::Transition { progress, .. } = Arc::make_mut(&mut node) else {
        panic!()
    };
    *progress = value;
    node
}

fn semantic_leaf(uv: f32) -> Arc<DynamicSampleExpression> {
    let value = AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic {
        intent: ColorIntent {
            uv: UvIntent { amount: uv },
            ..Default::default()
        },
    }));
    Arc::new(DynamicSampleExpression::Programming {
        address: Arc::new(
            DynamicValueAddress::whole_family(ProgrammingOwner::Color, &value).unwrap(),
        ),
        value: DynamicValue::Family(value),
        occurrence: None,
        dependency_occurrence: None,
    })
}

fn color_whole(
    root: Arc<DynamicSampleExpression>,
    source_rank: FamilySampleRank,
) -> FamilyCompositionSample {
    FamilyCompositionSample::WholeExpression {
        expression: Arc::new(
            CompiledProgrammingFamilyExpression::new(root, ProgrammingOwner::Color, None, None)
                .unwrap(),
        ),
        rank: source_rank,
        activation_mix: 1.0,
    }
}

fn whole(root: Arc<DynamicSampleExpression>, rank: FamilySampleRank) -> FamilyCompositionSample {
    FamilyCompositionSample::WholeExpression {
        expression: Arc::new(
            CompiledProgrammingFamilyExpression::new(root, ProgrammingOwner::Focus, None, None)
                .unwrap(),
        ),
        rank,
        activation_mix: 0.7,
    }
}

#[test]
fn choices_are_idempotent_and_conflicts_do_not_change_state() {
    let occurrence = Uuid::from_u128(40);
    let mut state = BranchState::default();
    state.choose(rank(2, 3), occurrence, true).unwrap();
    state.choose(rank(2, 4), occurrence, true).unwrap();
    assert_eq!(state.decisions.len(), 1);
    assert!(state.choose(rank(2, 5), occurrence, false).is_err());
    assert!(state.decisions[0].incoming);
    state.choose(rank(9, 3), occurrence, false).unwrap();
    assert_eq!(state.decisions.len(), 2);
    state.replace(0, Some(focus(0.2)));
    state.replace(0, None);
    assert_eq!(state.overrides.len(), 1);
    assert!(state.overrides[0].1.is_none());
}

#[test]
fn synchronized_projected_fragments_keep_masks_identity_and_original_indices() {
    let occurrence = Uuid::from_u128(40);
    let root = resume(
        Some(color(ColorComponent::Uv, 0.8)),
        Some(color(ColorComponent::WhiteBlend, 0.2)),
        occurrence,
    );
    let set = CompiledComponentExpressionSet::new(root.clone(), None).unwrap();
    let samples = FamilySample::retained_components(&set, rank(2, 3), 0.7)
        .unwrap()
        .into_iter()
        .map(FamilyCompositionSample::Known)
        .collect::<Vec<_>>();
    let mut state = BranchState::default();
    state.choose(rank(2, 3), occurrence, true).unwrap();
    let result = condition_sources(&samples, &state, &AttributeValue::Normalized(0.0)).unwrap();
    assert_eq!(result.len(), 2);
    assert!(result[0].is_none());
    let FamilyCompositionSample::Known(white) = result[1].as_ref().unwrap() else {
        panic!()
    };
    assert_eq!(white.rank, rank(2, 3));
    assert_eq!(white.activation_mix, 0.7);
    assert_eq!(
        white.address.address().component,
        Some(ProgrammingComponent::Color(ColorComponent::WhiteBlend))
    );
    assert!(Arc::ptr_eq(white.projection.as_ref().unwrap(), &root));
    let FamilySampleBody::ComponentExpression(compiled) = &white.body else {
        panic!()
    };
    assert_eq!(
        compiled.evaluate(None).unwrap(),
        Some(DynamicValue::Scalar(0.2))
    );
}

#[test]
fn choices_match_instance_controller_and_occurrence_but_keep_required_progress() {
    let occurrence = Uuid::from_u128(40);
    let root = resume(Some(focus(0.2)), Some(focus(0.8)), occurrence);
    let mut other_instance = rank(2, 5);
    let FamilySampleIdentity::Dynamic { instance_id, .. } = &mut other_instance.identity else {
        unreachable!()
    };
    *instance_id = Uuid::from_u128(99);
    let required = Arc::new(DynamicSampleExpression::Transition {
        from: Some(focus(0.3)),
        to: Some(focus(0.7)),
        progress: 0.25,
        reason: DynamicTransitionReason::Required {
            requirement: TransitionRequirement::MaterializedEndpoints,
        },
    });
    let samples = [
        whole(root.clone(), rank(2, 3)),
        whole(root.clone(), rank(9, 4)),
        whole(root, other_instance),
        whole(required.clone(), rank(2, 6)),
    ];
    let mut state = BranchState::default();
    state.choose(rank(2, 3), occurrence, true).unwrap();
    let result = condition_sources(&samples, &state, &AttributeValue::Normalized(0.1)).unwrap();
    let FamilyCompositionSample::WholeExpression {
        expression,
        rank: actual_rank,
        activation_mix,
    } = result[0].as_ref().unwrap()
    else {
        panic!()
    };
    assert_eq!(expression.expression(), focus(0.8).as_ref());
    assert_eq!(*actual_rank, rank(2, 3));
    assert_eq!(*activation_mix, 0.7);
    for index in 1..=3 {
        let FamilyCompositionSample::WholeExpression {
            expression: old, ..
        } = &samples[index]
        else {
            panic!()
        };
        let FamilyCompositionSample::WholeExpression {
            expression: new, ..
        } = result[index].as_ref().unwrap()
        else {
            panic!()
        };
        assert!(Arc::ptr_eq(old, new));
    }
}

#[test]
fn override_precedes_decisions_and_whole_to_component_stays_narrow() {
    let value = AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Origin,
        [0.0; 3],
    )));
    let root = Arc::new(DynamicSampleExpression::Programming {
        address: Arc::new(
            DynamicValueAddress::whole_family(ProgrammingOwner::Position, &value).unwrap(),
        ),
        value: DynamicValue::Family(value.clone()),
        occurrence: None,
        dependency_occurrence: None,
    });
    let samples = [FamilyCompositionSample::WholeExpression {
        expression: Arc::new(
            CompiledProgrammingFamilyExpression::new(root, ProgrammingOwner::Position, None, None)
                .unwrap(),
        ),
        rank: rank(2, 3),
        activation_mix: 0.7,
    }];
    let address = Arc::new(DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Target {
            reference: Some(TargetReference::Origin),
        },
        component: Some(ProgrammingComponent::TargetX),
    });
    let point_x = Arc::new(DynamicSampleExpression::Programming {
        address: address.clone(),
        value: DynamicValue::Scalar(2.0),
        occurrence: None,
        dependency_occurrence: None,
    });
    let occurrence = Uuid::from_u128(40);
    let mut state = BranchState::default();
    state.replace(0, Some(resume(None, Some(point_x), occurrence)));
    state.choose(rank(2, 3), occurrence, true).unwrap();
    let result = condition_sources(&samples, &state, &value).unwrap();
    let FamilyCompositionSample::CoupledExpression { expression, .. } = result[0].as_ref().unwrap()
    else {
        panic!()
    };
    let crate::CoupledExpressionFootprint::Exact {
        address: actual,
        value,
    } = expression.footprint(crate::CoupledExpressionRole::Base)
    else {
        panic!()
    };
    assert_eq!(actual.address(), address.as_ref());
    assert_eq!(*value, DynamicValue::Scalar(2.0));
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

#[test]
fn released_scale_child_uses_underlay_and_keeps_real_size_baseline() {
    let occurrence = Uuid::from_u128(40);
    let baseline = AttributeValue::Normalized(0.2);
    let root = Arc::new(DynamicSampleExpression::Scale {
        address: Arc::new(
            DynamicValueAddress::whole_family(ProgrammingOwner::Focus, &baseline).unwrap(),
        ),
        base: DynamicValue::Family(baseline),
        value: resume(Some(focus(0.4)), None, occurrence),
        factor: 2.0,
        baseline_occurrence: None,
    });
    let samples = [whole(root, rank(2, 3))];
    let mut state = BranchState::default();
    state.choose(rank(2, 3), occurrence, true).unwrap();
    let underlay = AttributeValue::Normalized(0.5);
    let result = condition_sources(&samples, &state, &underlay).unwrap();
    let FamilyCompositionSample::WholeExpression { expression, .. } = result[0].as_ref().unwrap()
    else {
        panic!()
    };
    let AttributeValue::Normalized(value) = expression.evaluate(&underlay, &NoFrame).unwrap()
    else {
        panic!()
    };
    assert!((value - 0.8).abs() < 0.00001);
}

#[test]
fn scoped_progress_mismatch_is_rejected_before_a_choice_removes_resume_nodes() {
    let occurrence = Uuid::from_u128(40);
    let whole = color_whole(
        resume(
            Some(semantic_leaf(0.2)),
            Some(semantic_leaf(0.8)),
            occurrence,
        ),
        rank(2, 3),
    );
    let projected = CompiledComponentExpressionSet::new(
        progress(
            resume(
                Some(color(ColorComponent::Uv, 0.2)),
                Some(color(ColorComponent::WhiteBlend, 0.8)),
                occurrence,
            ),
            0.7,
        ),
        None,
    )
    .unwrap();
    let mut samples = vec![whole.clone()];
    samples.extend(
        FamilySample::retained_components(&projected, rank(2, 4), 1.0)
            .unwrap()
            .into_iter()
            .map(FamilyCompositionSample::Known),
    );
    let mut state = BranchState::default();
    state.choose(rank(2, 3), occurrence, true).unwrap();
    assert!(matches!(
        condition_sources(&samples, &state, &AttributeValue::Normalized(0.0)),
        Err(TransitionError::Invalid(IntentError(message))) if message.contains("different progress")
    ));

    let coupled = FamilyCompositionSample::CoupledExpression {
        expression: Arc::new(
            CompiledCoupledExpression::new(
                progress(
                    resume(
                        Some(semantic_leaf(0.5)),
                        Some(color(ColorComponent::Uv, 0.8)),
                        occurrence,
                    ),
                    0.7,
                ),
                None,
            )
            .unwrap(),
        ),
        rank: rank(2, 5),
        activation_mix: 1.0,
    };
    assert!(validate_resume_progress(&[whole, coupled]).is_err());
}

#[test]
fn resume_progress_is_independent_between_instances_controllers_and_occurrences() {
    let occurrence = Uuid::from_u128(40);
    for different_scope in 0..3 {
        let mut other_rank = rank(2, 4);
        let mut other_occurrence = occurrence;
        let FamilySampleIdentity::Dynamic {
            instance_id,
            controller_id,
            ..
        } = &mut other_rank.identity
        else {
            unreachable!()
        };
        match different_scope {
            0 => *instance_id = Uuid::from_u128(90),
            1 => *controller_id = Uuid::from_u128(91),
            _ => other_occurrence = Uuid::from_u128(92),
        }
        let samples = [
            whole(
                resume(Some(focus(0.2)), Some(focus(0.8)), occurrence),
                rank(2, 3),
            ),
            whole(
                progress(
                    resume(Some(focus(0.3)), Some(focus(0.7)), other_occurrence),
                    0.7,
                ),
                other_rank,
            ),
        ];
        validate_resume_progress(&samples).unwrap();
    }
}

#[test]
fn inactive_history_does_not_create_a_resume_progress_conflict() {
    let occurrence = Uuid::from_u128(40);
    let unused = progress(resume(Some(focus(0.3)), Some(focus(0.7)), occurrence), 0.9);
    let exact = Arc::new(DynamicSampleExpression::Transition {
        from: Some(focus(0.2)),
        to: Some(unused.clone()),
        progress: 0.0,
        reason: DynamicTransitionReason::Required {
            requirement: TransitionRequirement::MaterializedEndpoints,
        },
    });
    let zero_size = Arc::new(DynamicSampleExpression::Scale {
        address: Arc::new(
            DynamicValueAddress::whole_family(
                ProgrammingOwner::Focus,
                &AttributeValue::Normalized(0.2),
            )
            .unwrap(),
        ),
        base: DynamicValue::Family(AttributeValue::Normalized(0.2)),
        value: unused.clone(),
        factor: 0.0,
        baseline_occurrence: None,
    });
    let mut inactive = whole(unused, rank(2, 6));
    let FamilyCompositionSample::WholeExpression { activation_mix, .. } = &mut inactive else {
        panic!()
    };
    *activation_mix = 0.0;
    let samples = [
        whole(
            resume(Some(focus(0.2)), Some(focus(0.8)), occurrence),
            rank(2, 3),
        ),
        whole(exact, rank(2, 4)),
        whole(zero_size, rank(2, 5)),
        inactive,
    ];
    condition_sources(
        &samples,
        &BranchState::default(),
        &AttributeValue::Normalized(0.0),
    )
    .unwrap();
}

struct NativeModel(NativeColorIdentity);
impl NativeColorEditModel for NativeModel {
    fn source(&self) -> &NativeColorIdentity {
        &self.0
    }
    fn descriptor(&self, _: NativeColorBinding) -> Option<NativeColorComponentDescriptor> {
        None
    }
    fn predict(&self, recipe: &NativeColorRecipe) -> Result<PortableColorEstimate, IntentError> {
        ensure(recipe.source == self.0, "different original source")?;
        Ok(PortableColorEstimate {
            model_revision: self.0.model_revision,
            visible: None,
            uv: None,
            quality: PhysicalDataQuality::Unknown,
            limitations: vec![],
        })
    }
}
struct NativeModels(Arc<NativeModel>);
impl DynamicNativeModelResolver for NativeModels {
    fn resolve(
        &self,
        source: &NativeColorIdentity,
    ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, IntentError> {
        ensure(source == &self.0.0, "original source unavailable")?;
        Ok(self.0.clone())
    }
}
fn native_base() -> (NativeModels, AttributeValue) {
    let source = NativeColorIdentity {
        profile_id: Uuid::from_u128(51),
        profile_revision: 1,
        profile_digest: "pinned".into(),
        mode_id: Uuid::from_u128(52),
        head_id: Uuid::from_u128(53),
        path_id: Uuid::from_u128(54),
        model_revision: 1,
        native_layout_signature: "one-u32".into(),
    };
    let value = AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
        recipe: NativeColorRecipe {
            source: source.clone(),
            channels: vec![NativeColorValue {
                channel_id: Uuid::from_u128(55),
                function_id: Uuid::from_u128(56),
                raw: u32::MAX - 1,
            }],
            spreads: vec![],
        },
        portable: PortableColorEstimate {
            model_revision: 1,
            visible: None,
            uv: None,
            quality: PhysicalDataQuality::Unknown,
            limitations: vec![],
        },
    }));
    (NativeModels(Arc::new(NativeModel(source))), value)
}
fn nested_semantic_release(occurrence: Uuid) -> Arc<DynamicSampleExpression> {
    Arc::new(DynamicSampleExpression::Transition {
        from: Some(resume(
            Some(semantic_leaf(0.2)),
            Some(semantic_leaf(0.8)),
            occurrence,
        )),
        to: None,
        progress: 0.5,
        reason: DynamicTransitionReason::Required {
            requirement: TransitionRequirement::MaterializedEndpoints,
        },
    })
}

#[test]
fn conditioned_covered_whole_does_not_require_the_generic_direct_bases_model() {
    let occurrence = Uuid::from_u128(40);
    let (models, base) = native_base();
    let lower = color_whole(nested_semantic_release(occurrence), rank(2, 3));
    let cover = FamilySample::new(
        Arc::new(
            CompiledDynamicValueAddress::new(
                DynamicValueAddress {
                    representation: DynamicFamilyRepresentation::SemanticColor {
                        basis: DynamicSemanticColorBasis::Recipe,
                    },
                    component: Some(ProgrammingComponent::Color(ColorComponent::Red)),
                },
                None,
            )
            .unwrap(),
        ),
        DynamicValue::Scalar(0.2),
        rank(3, 4),
        1.0,
    )
    .unwrap();
    let mut state = BranchState::default();
    state.choose(rank(2, 3), occurrence, true).unwrap();
    let result = condition_sources(&[lower, cover.into()], &state, &base).unwrap();
    let FamilyCompositionSample::WholeExpression { expression, .. } = result[0].as_ref().unwrap()
    else {
        panic!()
    };
    assert!(expression.needs_underlay());
    assert!(
        expression
            .resolve_original_native_model(models.0.source())
            .is_err(),
        "a generic base must not introduce a new source-model requirement"
    );
}

#[test]
fn conditioned_whole_keeps_models_already_pinned_for_its_actual_underlay() {
    let occurrence = Uuid::from_u128(40);
    let (models, base) = native_base();
    let original = Arc::new(
        CompiledProgrammingFamilyExpression::new(
            nested_semantic_release(occurrence),
            ProgrammingOwner::Color,
            Some(&base),
            Some(&models),
        )
        .unwrap(),
    );
    let pinned = original
        .resolve_original_native_model(models.0.source())
        .unwrap();
    let samples = [FamilyCompositionSample::WholeExpression {
        expression: original,
        rank: rank(2, 3),
        activation_mix: 1.0,
    }];
    let mut state = BranchState::default();
    state.choose(rank(2, 3), occurrence, true).unwrap();
    let result = condition_sources(&samples, &state, &base).unwrap();
    let FamilyCompositionSample::WholeExpression { expression, .. } = result[0].as_ref().unwrap()
    else {
        panic!()
    };
    let retained = expression
        .resolve_original_native_model(models.0.source())
        .unwrap();
    assert!(Arc::ptr_eq(&pinned, &retained));
}
