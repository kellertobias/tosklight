use super::*;
use crate::{
    CompiledProgrammingFamilyExpression, DynamicSampleExpression as E, DynamicTransitionReason,
    FamilyCompositionSample, FamilyExpressionOperation, RetainedFamilyCompositionScratch,
    WholeFamilyExpressionFrameResolver, compose_retained_dynamic_family,
};
use std::cell::Cell;

#[derive(Default)]
struct UnavailableFrame {
    calls: Cell<usize>,
}
impl WholeFamilyExpressionFrameResolver for UnavailableFrame {
    fn resolve(
        &self,
        requirement: TransitionRequirement,
        _: &AttributeValue,
        _: &AttributeValue,
        _: FamilyExpressionOperation,
    ) -> Result<AttributeValue, TransitionError> {
        self.calls.set(self.calls.get() + 1);
        Err(TransitionError::Requires(requirement))
    }
}

fn leaf(owner: ProgrammingOwner, value: AttributeValue) -> Arc<E> {
    Arc::new(E::Programming {
        address: Arc::new(DynamicValueAddress::whole_family(owner, &value).unwrap()),
        value: DynamicValue::Family(value),
        occurrence: None,
        dependency_occurrence: None,
    })
}

fn resume(from: Option<Arc<E>>, to: Option<Arc<E>>, progress: f32) -> Arc<E> {
    Arc::new(E::Transition {
        from,
        to,
        progress,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: Uuid::from_u128(500),
        },
    })
}

fn whole(
    owner: ProgrammingOwner,
    expression: Arc<E>,
    order: u128,
    activation_mix: f32,
) -> FamilyCompositionSample {
    FamilyCompositionSample::WholeExpression {
        expression: Arc::new(
            CompiledProgrammingFamilyExpression::new(expression, owner, None, None).unwrap(),
        ),
        rank: color(ColorComponent::Red, 0.0, order).rank,
        activation_mix,
    }
}

fn rgb(rgb: [f32; 3]) -> ColorIntent {
    let mut intent = ColorIntent::default();
    for (component, value) in [
        ColorComponent::Red,
        ColorComponent::Green,
        ColorComponent::Blue,
    ]
    .into_iter()
    .zip(rgb)
    {
        VirtualColorAuthoringV1
            .set_base_component(&mut intent, component, value)
            .unwrap();
    }
    intent
}

fn run(
    owner: ProgrammingOwner,
    base: &AttributeValue,
    samples: &[FamilyCompositionSample],
    frame: &dyn WholeFamilyExpressionFrameResolver,
) -> Result<AttributeValue, TransitionError> {
    compose_retained_dynamic_family(
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
        &mut RetainedFamilyCompositionScratch::default(),
    )
}

fn unresolved_points() -> Arc<E> {
    let target = |point_id| {
        AttributeValue::Position(Arc::new(PositionIntent::target(
            TargetReference::Point { point_id },
            [0.0; 3],
        )))
    };
    Arc::new(E::Transition {
        from: Some(leaf(
            ProgrammingOwner::Position,
            target(Uuid::from_u128(51)),
        )),
        to: Some(leaf(
            ProgrammingOwner::Position,
            target(Uuid::from_u128(52)),
        )),
        progress: 0.5,
        reason: DynamicTransitionReason::Required {
            requirement: TransitionRequirement::LiveTargetPoints,
        },
    })
}

#[test]
fn whole_activation_uses_lower_hue_saturation_base_before_reclassifying_output() {
    let base = semantic(rgb([1.0, 0.0, 0.0]));
    let blue = semantic(rgb([0.0, 0.0, 1.0]));
    let lower = [
        color(ColorComponent::Hue, 120.0, 1),
        color(ColorComponent::Saturation, 1.0, 2),
    ];
    let green = compose(ProgrammingOwner::Color, &base, &lower).unwrap();
    let expected = CompiledProgrammingTransition::new(green, blue.clone(), None)
        .unwrap()
        .sample(0.5)
        .unwrap();
    let mut samples = lower
        .into_iter()
        .map(FamilyCompositionSample::from)
        .collect::<Vec<_>>();
    samples.push(whole(
        ProgrammingOwner::Color,
        leaf(ProgrammingOwner::Color, blue),
        3,
        0.5,
    ));
    let frame = UnavailableFrame::default();
    assert_eq!(
        run(ProgrammingOwner::Color, &base, &samples, &frame).unwrap(),
        expected
    );
    assert_eq!(frame.calls.get(), 0);
}

#[test]
fn lower_partial_uv_is_applied_once_after_deferred_whole_base_activation() {
    let base = semantic(ColorIntent::default());
    let mut target = rgb([0.0, 0.0, 1.0]);
    target.uv.amount = 0.5;
    target.white_blend = 0.75;
    let target = semantic(target);
    let mut lower_uv = color(ColorComponent::Uv, 1.0, 1);
    lower_uv.activation_mix = 0.5;
    let samples = [
        lower_uv.into(),
        whole(
            ProgrammingOwner::Color,
            leaf(ProgrammingOwner::Color, target.clone()),
            2,
            0.5,
        ),
    ];
    let expected = CompiledProgrammingTransition::new(base.clone(), target, None)
        .unwrap()
        .sample(0.5)
        .unwrap();
    let mut expected = intent(&expected).clone();
    expected.uv.amount = 0.625;
    let frame = UnavailableFrame::default();
    let result = run(ProgrammingOwner::Color, &base, &samples, &frame).unwrap();
    assert_eq!(result, semantic(expected));
    assert_eq!(intent(&result).white_blend, 0.375);
    assert_eq!(frame.calls.get(), 0);
}

#[test]
fn missing_whole_endpoint_and_controller_activation_use_the_same_actual_prefix_separately() {
    let owner = ProgrammingOwner::Focus;
    let lower = sample(
        DynamicFamilyRepresentation::Focus,
        Some(ProgrammingComponent::Focus),
        DynamicValue::Scalar(0.25),
        1,
    );
    let expression = resume(
        Some(leaf(owner, AttributeValue::Normalized(0.75))),
        None,
        0.5,
    );
    let samples = [lower.into(), whole(owner, expression, 2, 0.5)];
    let frame = UnavailableFrame::default();
    // Resume 0.75 -> 0.25 gives 0.5; controller activation over 0.25 gives 0.375.
    assert_eq!(
        run(owner, &AttributeValue::Normalized(0.125), &samples, &frame).unwrap(),
        AttributeValue::Normalized(0.375)
    );
    assert_eq!(frame.calls.get(), 0);
}

#[test]
fn absent_whole_endpoint_does_not_take_over_active_hue_saturation() {
    let base = semantic(rgb([1.0, 0.0, 0.0]));
    let lower = [
        color(ColorComponent::Hue, 120.0, 1),
        color(ColorComponent::Saturation, 1.0, 2),
        color(ColorComponent::WhiteBlend, 0.25, 3),
    ];
    let expected = compose(ProgrammingOwner::Color, &base, &lower).unwrap();
    let blue = leaf(ProgrammingOwner::Color, semantic(rgb([0.0, 0.0, 1.0])));
    for expression in [
        resume(Some(blue.clone()), None, 1.0),
        resume(None, Some(blue), 0.0),
    ] {
        let mut samples = lower
            .iter()
            .cloned()
            .map(FamilyCompositionSample::from)
            .collect::<Vec<_>>();
        samples.push(whole(ProgrammingOwner::Color, expression, 90, 1.0));
        let frame = UnavailableFrame::default();
        assert_eq!(
            run(ProgrammingOwner::Color, &base, &samples, &frame).unwrap(),
            expected
        );
        assert_eq!(frame.calls.get(), 0);
    }
}

#[test]
fn higher_opaque_whole_or_whole_fix_at_covers_unresolved_point_geometry() {
    let owner = ProgrammingOwner::Position;
    let base = angles(0.0, 0.0);
    let frame = UnavailableFrame::default();
    assert_eq!(
        run(
            owner,
            &base,
            &[whole(owner, unresolved_points(), 1, 1.0)],
            &frame
        )
        .unwrap_err(),
        TransitionError::Requires(TransitionRequirement::LiveTargetPoints)
    );
    assert_eq!(frame.calls.get(), 1);
    let expected = angles(720.0, 20.0);
    for fixed in [false, true] {
        let cover = if fixed {
            sample(
                DynamicFamilyRepresentation::Angles,
                None,
                DynamicValue::Family(expected.clone()),
                2,
            )
            .into_fix_at()
            .into()
        } else {
            whole(owner, leaf(owner, expected.clone()), 2, 1.0)
        };
        let samples = [whole(owner, unresolved_points(), 1, 1.0), cover];
        frame.calls.set(0);
        assert_eq!(run(owner, &base, &samples, &frame).unwrap(), expected);
        assert_eq!(
            frame.calls.get(),
            0,
            "covered Point expressions must not resolve"
        );
    }
}

#[test]
fn resolved_position_is_classified_as_actual_angles_before_a_higher_pan_mask() {
    #[derive(Default)]
    struct AnglesFrame(Cell<usize>);
    impl WholeFamilyExpressionFrameResolver for AnglesFrame {
        fn resolve(
            &self,
            requirement: TransitionRequirement,
            _: &AttributeValue,
            _: &AttributeValue,
            operation: FamilyExpressionOperation,
        ) -> Result<AttributeValue, TransitionError> {
            assert_eq!(requirement, TransitionRequirement::LiveTargetPoints);
            assert_eq!(
                operation,
                FamilyExpressionOperation::Transition { progress: 0.5 }
            );
            self.0.set(self.0.get() + 1);
            Ok(angles(720.0, 40.0))
        }
    }
    let frame = AnglesFrame::default();
    let samples = [
        whole(ProgrammingOwner::Position, unresolved_points(), 1, 1.0),
        angle(ProgrammingComponent::Pan, 900.0, 2)
            .into_fix_at()
            .into(),
    ];
    assert_eq!(
        run(
            ProgrammingOwner::Position,
            &angles(0.0, 0.0),
            &samples,
            &frame
        )
        .unwrap(),
        angles(900.0, 40.0)
    );
    assert_eq!(frame.0.get(), 1);
}

#[test]
fn newer_recipe_base_excludes_older_whole_expression_without_resolving_its_appearance() {
    let model = native_model();
    let base = direct(&model);
    let adoption = ColorIntent::default();
    let context = FamilyCompositionContext {
        edit: FamilyEditContext {
            color_model: Some(&VirtualColorAuthoringV1),
            semantic_color_adoption: Some(&adoption),
            ..Default::default()
        },
        ..Default::default()
    };
    let recipe = color(ColorComponent::Red, 0.25, 2);
    let expected = compose_dynamic_family(
        ProgrammingOwner::Color,
        &base,
        &[recipe.clone()],
        &context,
        &mut FamilyCompositionScratch::default(),
    )
    .unwrap();
    // If evaluated, this older missing endpoint needs the Direct underlay's unknown appearance.
    // Its expression contains no native source leaf, so compiling it does not need a model.
    let older = resume(
        None,
        Some(leaf(
            ProgrammingOwner::Color,
            semantic(rgb([0.0, 0.0, 1.0])),
        )),
        0.5,
    );
    let samples = [whole(ProgrammingOwner::Color, older, 1, 1.0), recipe.into()];
    let frame = UnavailableFrame::default();
    let result = compose_retained_dynamic_family(
        ProgrammingOwner::Color,
        &base,
        &samples,
        &context,
        &frame,
        &mut RetainedFamilyCompositionScratch::default(),
    )
    .unwrap();
    assert_eq!(result, expected);
    assert_eq!(frame.calls.get(), 0);
    assert_eq!(model.predictions.load(Ordering::Relaxed), 0);
}

#[test]
fn missing_endpoint_above_partial_whole_mask_includes_the_masks_lower_orthogonals_once() {
    let base = semantic(ColorIntent::default());
    let mut mask = sample(
        DynamicFamilyRepresentation::SemanticColor {
            basis: DynamicSemanticColorBasis::Whole,
        },
        None,
        DynamicValue::Family(base.clone()),
        2,
    )
    .into_fix_at();
    mask.activation_mix = 0.5;
    let target = semantic(ColorIntent {
        uv: UvIntent { amount: 1.0 },
        ..Default::default()
    });
    let samples = [
        color(ColorComponent::Uv, 1.0, 1).into(),
        mask.into(),
        whole(
            ProgrammingOwner::Color,
            resume(None, Some(leaf(ProgrammingOwner::Color, target)), 0.5),
            3,
            0.5,
        ),
    ];
    let frame = UnavailableFrame::default();
    let result = run(ProgrammingOwner::Color, &base, &samples, &frame).unwrap();
    let expected = ColorIntent {
        // Below mask: 1; mask: 0.5; resume: 0.75; activation over masked 0.5: 0.625.
        uv: UvIntent { amount: 0.625 },
        ..Default::default()
    };
    assert_eq!(result, semantic(expected));
    assert_eq!(frame.calls.get(), 0);
}

#[test]
fn retained_scratch_recovers_after_an_error_and_re_resolves_cached_lower_sources_next_frame() {
    struct RecoveringFrame {
        fail: Cell<bool>,
        lower_pan: Cell<f32>,
        target_calls: Cell<usize>,
        joint_calls: Cell<usize>,
    }
    impl WholeFamilyExpressionFrameResolver for RecoveringFrame {
        fn resolve(
            &self,
            requirement: TransitionRequirement,
            from: &AttributeValue,
            _: &AttributeValue,
            _: FamilyExpressionOperation,
        ) -> Result<AttributeValue, TransitionError> {
            match requirement {
                TransitionRequirement::LiveTargetPoints => {
                    self.target_calls.set(self.target_calls.get() + 1);
                    Ok(angles(self.lower_pan.get(), 0.0))
                }
                TransitionRequirement::LiveJointAngles => {
                    self.joint_calls.set(self.joint_calls.get() + 1);
                    assert_eq!(from, &angles(self.lower_pan.get(), 0.0));
                    if self.fail.get() {
                        Err(TransitionError::Requires(requirement))
                    } else {
                        Ok(angles(self.lower_pan.get() + 100.0, 0.0))
                    }
                }
                _ => Err(TransitionError::Requires(requirement)),
            }
        }
    }
    let owner = ProgrammingOwner::Position;
    let aim = AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Point {
            point_id: Uuid::from_u128(99),
        },
        [0.0; 3],
    )));
    let samples = [
        whole(owner, unresolved_points(), 1, 1.0),
        whole(owner, resume(None, Some(leaf(owner, aim)), 0.5), 2, 1.0),
    ];
    let frame = RecoveringFrame {
        fail: Cell::new(true),
        lower_pan: Cell::new(10.0),
        target_calls: Cell::new(0),
        joint_calls: Cell::new(0),
    };
    let base = angles(0.0, 0.0);
    let context = FamilyCompositionContext::default();
    let mut scratch = RetainedFamilyCompositionScratch::default();
    assert_eq!(
        compose_retained_dynamic_family(owner, &base, &samples, &context, &frame, &mut scratch)
            .unwrap_err(),
        TransitionError::Requires(TransitionRequirement::LiveJointAngles)
    );
    assert_eq!((frame.target_calls.get(), frame.joint_calls.get()), (1, 1));
    frame.fail.set(false);
    frame.lower_pan.set(30.0);
    assert_eq!(
        compose_retained_dynamic_family(owner, &base, &samples, &context, &frame, &mut scratch)
            .unwrap(),
        angles(130.0, 0.0)
    );
    assert_eq!((frame.target_calls.get(), frame.joint_calls.get()), (2, 2));
    let empty_base = angles(4.0, 5.0);
    assert_eq!(
        compose_retained_dynamic_family(owner, &empty_base, &[], &context, &frame, &mut scratch)
            .unwrap(),
        empty_base
    );
    assert_eq!((frame.target_calls.get(), frame.joint_calls.get()), (2, 2));
}

struct PinnedNativeModel {
    model: Arc<NativeModel>,
    lookups: AtomicUsize,
}
impl crate::DynamicNativeModelResolver for PinnedNativeModel {
    fn resolve(
        &self,
        source: &NativeColorIdentity,
    ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, IntentError> {
        self.lookups.fetch_add(1, Ordering::Relaxed);
        if source != &self.model.source {
            return Err(IntentError(
                "requested source is not the pinned native model".into(),
            ));
        }
        Ok(self.model.clone())
    }
}

fn native_whole(
    model: &PinnedNativeModel,
    values: [u32; 2],
    order: u128,
) -> FamilyCompositionSample {
    FamilyCompositionSample::WholeExpression {
        expression: Arc::new(
            CompiledProgrammingFamilyExpression::new(
                leaf(
                    ProgrammingOwner::Color,
                    semantic_to_direct(&model.model, values),
                ),
                ProgrammingOwner::Color,
                None,
                Some(model),
            )
            .unwrap(),
        ),
        rank: color(ColorComponent::Red, 0.0, order).rank,
        activation_mix: 1.0,
    }
}

#[test]
fn a_verified_native_whole_leaf_is_not_predicted_again_on_each_composed_frame() {
    let old = native_model();
    let mut replacement = native_model();
    Arc::get_mut(&mut replacement)
        .unwrap()
        .source
        .profile_revision += 1;
    let pinned = PinnedNativeModel {
        model: replacement.clone(),
        lookups: AtomicUsize::new(0),
    };
    let values = [u32::MAX - 1, u32::MAX - 3];
    let samples = [native_whole(&pinned, values, 1)];
    assert_eq!(replacement.predictions.load(Ordering::Relaxed), 1);
    let frame = UnavailableFrame::default();
    let context = FamilyCompositionContext {
        edit: FamilyEditContext {
            native_model: Some(old.as_ref()),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut scratch = RetainedFamilyCompositionScratch::default();
    for _ in 0..3 {
        assert_eq!(
            compose_retained_dynamic_family(
                ProgrammingOwner::Color,
                &direct(&old),
                &samples,
                &context,
                &frame,
                &mut scratch,
            )
            .unwrap(),
            semantic_to_direct(&replacement, values)
        );
    }
    assert_eq!(replacement.predictions.load(Ordering::Relaxed), 1);
    assert_eq!(old.predictions.load(Ordering::Relaxed), 0);
    assert_eq!(pinned.lookups.load(Ordering::Relaxed), 1);
    assert_eq!(frame.calls.get(), 0);
}

#[test]
fn native_edits_above_a_new_whole_source_use_their_pinned_model_despite_missing_or_stale_context() {
    let old = native_model();
    let mut replacement = native_model();
    Arc::get_mut(&mut replacement)
        .unwrap()
        .source
        .profile_revision += 1;
    let pinned = PinnedNativeModel {
        model: replacement.clone(),
        lookups: AtomicUsize::new(0),
    };
    let samples = [
        native_whole(&pinned, [u32::MAX - 1, 17], 1),
        native_sample(&replacement, 1, u32::MAX - 3, 2).into(),
    ];
    assert_eq!(replacement.predictions.load(Ordering::Relaxed), 1);
    let frame = UnavailableFrame::default();
    let mut scratch = RetainedFamilyCompositionScratch::default();
    for stale_context in [false, true] {
        let context = FamilyCompositionContext {
            edit: FamilyEditContext {
                native_model: stale_context.then_some(old.as_ref() as &dyn NativeColorEditModel),
                ..Default::default()
            },
            ..Default::default()
        };
        for _ in 0..2 {
            assert_eq!(
                compose_retained_dynamic_family(
                    ProgrammingOwner::Color,
                    &direct(&old),
                    &samples,
                    &context,
                    &frame,
                    &mut scratch,
                )
                .unwrap(),
                semantic_to_direct(&replacement, [u32::MAX - 1, u32::MAX - 3])
            );
        }
    }
    // One cold leaf verification plus exactly one complete native edit prediction per frame.
    assert_eq!(replacement.predictions.load(Ordering::Relaxed), 5);
    assert_eq!(old.predictions.load(Ordering::Relaxed), 0);
    assert_eq!(pinned.lookups.load(Ordering::Relaxed), 1);
    assert_eq!(frame.calls.get(), 0);
}

#[test]
fn known_and_deferred_whole_activation_have_identical_hue_saturation_underlays() {
    let base = semantic(rgb([1.0, 0.0, 0.0]));
    let blue = semantic(rgb([0.0, 0.0, 1.0]));
    let lower = [
        color(ColorComponent::Hue, 120.0, 1),
        color(ColorComponent::Saturation, 1.0, 2),
    ];
    let green = compose(ProgrammingOwner::Color, &base, &lower).unwrap();
    let expected = CompiledProgrammingTransition::new(green, blue.clone(), None)
        .unwrap()
        .sample(0.5)
        .unwrap();
    let mut known = sample(
        DynamicFamilyRepresentation::SemanticColor {
            basis: DynamicSemanticColorBasis::Whole,
        },
        None,
        DynamicValue::Family(blue.clone()),
        3,
    );
    known.activation_mix = 0.5;
    let deferred = whole(
        ProgrammingOwner::Color,
        leaf(ProgrammingOwner::Color, blue),
        3,
        0.5,
    );
    let frame = UnavailableFrame::default();
    for upper in [FamilyCompositionSample::Known(known), deferred] {
        let mut samples = lower
            .iter()
            .cloned()
            .map(FamilyCompositionSample::from)
            .collect::<Vec<_>>();
        samples.push(upper);
        assert_eq!(
            run(ProgrammingOwner::Color, &base, &samples, &frame).unwrap(),
            expected
        );
    }
    assert_eq!(frame.calls.get(), 0);
}

#[test]
fn long_partial_whole_chain_uses_bounded_call_stack_and_reuses_scratch_deterministically() {
    let mut expected = 0.0_f32;
    let samples = (0..1537)
        .map(|index| {
            let target = if index % 2 == 0 { 0.25_f32 } else { 0.75_f32 };
            expected = ((f64::from(expected) + f64::from(target)) * 0.5) as f32;
            let order = index as u128 + 1;
            if index % 2 == 0 {
                let mut known = sample(
                    DynamicFamilyRepresentation::Focus,
                    None,
                    DynamicValue::Family(AttributeValue::Normalized(target)),
                    order,
                );
                known.activation_mix = 0.5;
                FamilyCompositionSample::Known(known)
            } else {
                whole(
                    ProgrammingOwner::Focus,
                    leaf(ProgrammingOwner::Focus, AttributeValue::Normalized(target)),
                    order,
                    0.5,
                )
            }
        })
        .collect::<Vec<_>>();
    // Build the independent, shallow expressions before creating the small-stack output worker.
    let result = std::thread::Builder::new()
        .name("retained-family-small-stack".into())
        .stack_size(128 * 1024)
        .spawn(move || {
            let mut scratch = RetainedFamilyCompositionScratch::default();
            let frame = UnavailableFrame::default();
            let context = FamilyCompositionContext::default();
            let mut first = None;
            for _ in 0..2 {
                let result = compose_retained_dynamic_family(
                    ProgrammingOwner::Focus,
                    &AttributeValue::Normalized(0.0),
                    &samples,
                    &context,
                    &frame,
                    &mut scratch,
                )
                .unwrap();
                let AttributeValue::Normalized(value) = result else {
                    panic!("Focus composition must retain its normalized owner value")
                };
                if let Some(previous) = first {
                    assert_eq!(
                        value, previous,
                        "the reused frame scratch must be deterministic"
                    );
                } else {
                    first = Some(value);
                }
            }
            assert_eq!(frame.calls.get(), 0);
            first.unwrap()
        })
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(result, expected);
}

#[test]
fn known_partial_target_is_reclassified_from_resolved_angles_before_a_higher_pan_mask() {
    struct JointFrame(Cell<usize>);
    impl WholeFamilyExpressionFrameResolver for JointFrame {
        fn resolve(
            &self,
            requirement: TransitionRequirement,
            from: &AttributeValue,
            to: &AttributeValue,
            operation: FamilyExpressionOperation,
        ) -> Result<AttributeValue, TransitionError> {
            assert_eq!(requirement, TransitionRequirement::LiveJointAngles);
            assert_eq!(from, &angles(0.0, 0.0));
            assert!(matches!(to, AttributeValue::Position(position)
                if matches!(position.as_ref(), PositionIntent::Target { .. })));
            assert_eq!(
                operation,
                FamilyExpressionOperation::Transition { progress: 0.5 }
            );
            self.0.set(self.0.get() + 1);
            Ok(angles(720.0, 40.0))
        }
    }
    let target = AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Point {
            point_id: Uuid::from_u128(99),
        },
        [0.0; 3],
    )));
    let mut known = sample(
        DynamicFamilyRepresentation::Target { reference: None },
        None,
        DynamicValue::Family(target),
        1,
    );
    known.activation_mix = 0.5;
    let samples = [
        known.into(),
        angle(ProgrammingComponent::Pan, 900.0, 2)
            .into_fix_at()
            .into(),
    ];
    let frame = JointFrame(Cell::new(0));
    assert_eq!(
        run(
            ProgrammingOwner::Position,
            &angles(0.0, 0.0),
            &samples,
            &frame
        )
        .unwrap(),
        angles(900.0, 40.0)
    );
    assert_eq!(frame.0.get(), 1);
}
