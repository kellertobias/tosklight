use super::*;
mod coupled;
mod coupled_orthogonal;
mod endpoint_output;
mod native_functions;
mod projected;
mod rank;
mod retained;
mod whole_appearance;
mod whole_retained;
use light_core::{NativeColorBinding, NativeColorIdentity, NativeColorValue, PhysicalDataQuality};
use std::sync::atomic::{AtomicUsize, Ordering};

fn sample(
    representation: DynamicFamilyRepresentation,
    component: Option<ProgrammingComponent>,
    value: DynamicValue,
    order: u128,
) -> FamilySample {
    FamilySample::new(
        Arc::new(
            CompiledDynamicValueAddress::new(
                DynamicValueAddress {
                    representation,
                    component,
                },
                None,
            )
            .unwrap(),
        ),
        value,
        FamilySampleRank {
            priority: 10,
            changed_at_millis: 100,
            changed_at_submillis_nanos: 0,
            stable_order: order,
            identity: crate::FamilySampleIdentity::Dynamic {
                instance_id: Uuid::from_u128(1000 + order),
                controller_id: Uuid::from_u128(3000 + order),
                lane_id: Uuid::from_u128(2000 + order),
            },
        },
        1.0,
    )
    .unwrap()
}
fn angle(component: ProgrammingComponent, value: f32, order: u128) -> FamilySample {
    sample(
        DynamicFamilyRepresentation::Angles,
        Some(component),
        DynamicValue::Scalar(value),
        order,
    )
}
fn color(component: ColorComponent, value: f32, order: u128) -> FamilySample {
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
    sample(
        DynamicFamilyRepresentation::SemanticColor { basis },
        Some(ProgrammingComponent::Color(component)),
        DynamicValue::Scalar(value),
        order,
    )
}
fn angles(pan: f32, tilt: f32) -> AttributeValue {
    AttributeValue::Position(Arc::new(PositionIntent::angles(pan, tilt)))
}
fn semantic(intent: ColorIntent) -> AttributeValue {
    AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic { intent }))
}
fn compose(
    owner: ProgrammingOwner,
    base: &AttributeValue,
    samples: &[FamilySample],
) -> Result<AttributeValue, TransitionError> {
    compose_dynamic_family(
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
        &mut FamilyCompositionScratch::default(),
    )
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

#[test]
fn separate_angle_dynamics_compete_as_complete_pairs() {
    let mut newer_pan = angle(ProgrammingComponent::Pan, 720.0, 2);
    newer_pan.activation_mix = 0.25;
    let partner = |axis: &FamilySample, value| {
        let mut partner = angle(ProgrammingComponent::Tilt, value, 99);
        partner.rank = axis
            .rank
            .with_dynamic_lane(Uuid::from_u128(4000 + axis.rank.stable_order))
            .unwrap();
        partner.activation_mix = axis.activation_mix;
        partner
    };
    let old_pan = angle(ProgrammingComponent::Pan, -360.0, 1);
    let mut samples = vec![
        partner(&newer_pan, 40.0),
        newer_pan,
        partner(&old_pan, -90.0),
        old_pan,
    ];
    assert_eq!(
        compose(ProgrammingOwner::Position, &angles(0.0, 0.0), &samples).unwrap(),
        angles(-90.0, -57.5)
    );
    samples.reverse();
    assert_eq!(
        compose(ProgrammingOwner::Position, &angles(0.0, 0.0), &samples).unwrap(),
        angles(-90.0, -57.5)
    );
    // Changing the static Current base leaves compatible Dynamics running.
    assert_eq!(
        compose(ProgrammingOwner::Position, &angles(999.0, 88.0), &samples).unwrap(),
        angles(-90.0, -57.5)
    );
}

#[test]
fn missing_axis_never_borrows_another_instance_or_controller_and_current_is_live() {
    let pan = angle(ProgrammingComponent::Pan, 90.0, 1);
    let mut tilt = angle(ProgrammingComponent::Tilt, -30.0, 2);
    let mut samples = vec![pan, tilt.clone()];
    let static_base = angles(5.0, 10.0);
    assert_eq!(
        compose(ProgrammingOwner::Position, &static_base, &samples).unwrap(),
        static_base
    );
    // Sharing an instance is insufficient: separate controllers still own separate bundles.
    let FamilySampleIdentity::Dynamic { instance_id, .. } = &mut tilt.rank.identity else {
        unreachable!()
    };
    *instance_id = samples[0].rank.dynamic_identity().unwrap().instance_id;
    samples[1] = tilt;
    assert_eq!(
        compose(ProgrammingOwner::Position, &static_base, &samples).unwrap(),
        static_base
    );
    samples[1].rank = samples[0]
        .rank
        .with_dynamic_lane(Uuid::from_u128(9999))
        .unwrap();
    assert_eq!(
        compose(ProgrammingOwner::Position, &static_base, &samples).unwrap(),
        angles(90.0, -30.0)
    );
    // The next frame supplies the new pre-Dynamic static Tilt without restarting Pan.
    samples[1]
        .set_component_value(DynamicValue::Scalar(75.0))
        .unwrap();
    assert_eq!(
        compose(ProgrammingOwner::Position, &angles(5.0, 75.0), &samples).unwrap(),
        angles(90.0, 75.0)
    );
    samples.push(samples[1].clone());
    assert!(compose(ProgrammingOwner::Position, &static_base, &samples).is_err());
}

#[test]
fn target_offsets_require_same_reference_and_never_fabricate_current() {
    let origin = TargetReference::Origin;
    let other = TargetReference::Point {
        point_id: Uuid::from_u128(55),
    };
    let target = |reference, offsets| {
        AttributeValue::Position(Arc::new(PositionIntent::target(reference, offsets)))
    };
    let offset = |reference, component, value, order| {
        sample(
            DynamicFamilyRepresentation::Target {
                reference: Some(reference),
            },
            Some(component),
            DynamicValue::Scalar(value),
            order,
        )
    };
    let samples = [
        offset(origin, ProgrammingComponent::TargetX, -5.0, 1),
        offset(origin, ProgrammingComponent::TargetZ, 9.0, 2),
        offset(other, ProgrammingComponent::TargetY, 42.0, 0),
    ];
    assert_eq!(
        compose(
            ProgrammingOwner::Position,
            &target(origin, [1.0, 2.0, 3.0]),
            &samples
        )
        .unwrap(),
        target(origin, [-5.0, 2.0, 9.0])
    );
    assert_eq!(
        compose(ProgrammingOwner::Position, &angles(0.0, 0.0), &samples).unwrap_err(),
        TransitionError::Requires(TransitionRequirement::LiveTargetPoints)
    );
    let result = compose_dynamic_family(
        ProgrammingOwner::Position,
        &angles(0.0, 0.0),
        &samples,
        &FamilyCompositionContext {
            adopted_base: Some(&target(origin, [7.0, 8.0, 9.0])),
            ..Default::default()
        },
        &mut FamilyCompositionScratch::default(),
    )
    .unwrap();
    assert_eq!(result, target(origin, [-5.0, 8.0, 9.0]));
}

#[test]
fn angle_takeover_requires_solved_unwrapped_joints_and_inactive_lanes_cannot_take_over() {
    let base = AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Origin,
        [1.0; 3],
    )));
    let mut samples = [sample(
        DynamicFamilyRepresentation::Angles,
        None,
        DynamicValue::Family(angles(900.0, -35.0)),
        1,
    )];
    samples[0].activation_mix = 0.5;
    assert_eq!(
        compose(ProgrammingOwner::Position, &base, &samples).unwrap_err(),
        TransitionError::Requires(TransitionRequirement::LiveJointAngles)
    );
    let result = compose_dynamic_family(
        ProgrammingOwner::Position,
        &base,
        &samples,
        &FamilyCompositionContext {
            edit: FamilyEditContext {
                solved_angles: Some(JointAngles {
                    pan_degrees: 900.0,
                    tilt_degrees: -35.0,
                }),
                ..Default::default()
            },
            ..Default::default()
        },
        &mut FamilyCompositionScratch::default(),
    )
    .unwrap();
    assert_eq!(result, angles(900.0, -35.0));
    samples[0].activation_mix = 0.0;
    assert_eq!(
        compose(ProgrammingOwner::Position, &base, &samples).unwrap(),
        base
    );
}

#[test]
fn semantic_base_conflicts_do_not_discard_orthogonal_color_or_achromatic_hue() {
    let base = semantic(ColorIntent::default());
    let samples = [
        color(ColorComponent::Red, 0.0, 1),
        color(ColorComponent::Hue, 300.0, 2),
        color(ColorComponent::Saturation, 1.0, 3),
        color(ColorComponent::WhiteBlend, 0.5, 4),
        color(ColorComponent::Uv, 0.8, 5),
        color(ColorComponent::RelativeOutput, 0.0, 6),
    ];
    let value = compose(ProgrammingOwner::Color, &base, &samples).unwrap();
    let result = intent(&value);
    assert_eq!(result.white_blend, 0.5);
    assert_eq!(result.uv.amount, 0.8);
    assert_eq!(result.relative_output, 0.0);
    assert!(
        (VirtualColorAuthoringV1
            .read_base_component(result, ColorComponent::Hue)
            .unwrap()
            - 300.0)
            .abs()
            < 0.01
    );
    assert!(
        (VirtualColorAuthoringV1
            .read_base_component(result, ColorComponent::Saturation)
            .unwrap()
            - 1.0)
            .abs()
            < 0.01
    );
}

#[test]
fn whole_color_supplies_base_and_explicit_orthogonals_override_it_in_component_rank_order() {
    let mut target = ColorIntent::default();
    target.white_blend = 0.9;
    target.uv.amount = 0.1;
    let whole = sample(
        DynamicFamilyRepresentation::SemanticColor {
            basis: DynamicSemanticColorBasis::Whole,
        },
        None,
        DynamicValue::Family(semantic(target.clone())),
        90,
    );
    let mut uv = color(ColorComponent::Uv, 1.0, 4);
    uv.activation_mix = 0.5;
    let samples = [
        whole,
        color(ColorComponent::Red, 0.0, 1),
        color(ColorComponent::WhiteBlend, 0.25, 2),
        color(ColorComponent::Uv, 0.2, 3),
        uv,
        color(ColorComponent::Temperature, 3200.0, 5),
        color(ColorComponent::Duv, -0.01, 6),
    ];
    let output = compose(
        ProgrammingOwner::Color,
        &semantic(ColorIntent::default()),
        &samples,
    )
    .unwrap();
    let actual = intent(&output);
    assert_eq!(actual.base_xyz, target.base_xyz);
    assert_eq!(actual.recipe, target.recipe);
    assert_eq!(actual.white_blend, 0.25);
    assert_eq!(actual.uv.amount, 0.6);
    assert_eq!(actual.white_target.kelvin, 3200.0);
    assert_eq!(actual.white_target.duv, -0.01);
}

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
                continuous: true,
            })
    }
    fn predict(&self, recipe: &NativeColorRecipe) -> Result<PortableColorEstimate, IntentError> {
        ensure(
            recipe.channels.len() == 2
                && recipe.source == self.source
                && self.bindings.iter().all(|binding| {
                    recipe.channels.iter().any(|channel| {
                        channel.channel_id == binding.channel_id
                            && channel.function_id == binding.function_id
                    })
                }),
            "incomplete native source recipe",
        )?;
        self.predictions.fetch_add(1, Ordering::Relaxed);
        Ok(PortableColorEstimate {
            model_revision: 1,
            visible: None,
            uv: None,
            quality: PhysicalDataQuality::Estimated,
            limitations: vec![],
        })
    }
}
fn native_model() -> Arc<NativeModel> {
    Arc::new(NativeModel {
        source: NativeColorIdentity {
            profile_id: Uuid::from_u128(1),
            profile_revision: 2,
            profile_digest: "original-profile".into(),
            mode_id: Uuid::from_u128(2),
            head_id: Uuid::from_u128(3),
            path_id: Uuid::from_u128(4),
            model_revision: 1,
            native_layout_signature: "exact-layout".into(),
        },
        bindings: [
            NativeColorBinding {
                channel_id: Uuid::from_u128(5),
                function_id: Uuid::from_u128(6),
            },
            NativeColorBinding {
                channel_id: Uuid::from_u128(7),
                function_id: Uuid::from_u128(8),
            },
        ],
        predictions: AtomicUsize::new(0),
    })
}
fn direct(model: &NativeModel) -> AttributeValue {
    semantic_to_direct(model, [0, 0])
}
fn semantic_to_direct(model: &NativeModel, values: [u32; 2]) -> AttributeValue {
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
            model_revision: 1,
            visible: None,
            uv: None,
            quality: PhysicalDataQuality::Estimated,
            limitations: vec![],
        },
    }))
}
fn native_sample(model: &Arc<NativeModel>, index: usize, raw: u32, order: u128) -> FamilySample {
    let rank = color(ColorComponent::Red, 0.0, order).rank;
    FamilySample::new(
        Arc::new(
            CompiledDynamicValueAddress::new(
                DynamicValueAddress {
                    representation: DynamicFamilyRepresentation::DirectColor {
                        source: model.source.clone(),
                    },
                    component: Some(ProgrammingComponent::NativeColor(model.bindings[index])),
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
}

#[test]
fn direct_components_keep_all_bits_and_predict_once_after_composition() {
    let model = native_model();
    let samples = [
        native_sample(&model, 0, u32::MAX - 1, 1),
        native_sample(&model, 1, u32::MAX - 3, 2),
        color(ColorComponent::Uv, 1.0, 100),
    ];
    let result = compose_dynamic_family(
        ProgrammingOwner::Color,
        &direct(&model),
        &samples,
        &FamilyCompositionContext {
            edit: FamilyEditContext {
                native_model: Some(model.as_ref()),
                ..Default::default()
            },
            ..Default::default()
        },
        &mut FamilyCompositionScratch::default(),
    )
    .unwrap();
    assert_eq!(
        result,
        semantic_to_direct(&model, [u32::MAX - 1, u32::MAX - 3])
    );
    assert_eq!(model.predictions.load(Ordering::Relaxed), 1);
}

#[test]
fn same_layout_with_different_pinned_source_does_not_merge() {
    let original = native_model();
    let mut changed = native_model();
    Arc::get_mut(&mut changed).unwrap().source.profile_revision += 1;
    let samples = [
        native_sample(&original, 0, u32::MAX, 1),
        native_sample(&changed, 1, 7, 2),
    ];
    assert_eq!(
        compose(ProgrammingOwner::Color, &direct(&original), &samples).unwrap_err(),
        TransitionError::Requires(TransitionRequirement::ColorAppearance)
    );
    let result = compose_dynamic_family(
        ProgrammingOwner::Color,
        &direct(&original),
        &samples,
        &FamilyCompositionContext {
            adopted_base: Some(&direct(&changed)),
            edit: FamilyEditContext {
                native_model: Some(changed.as_ref()),
                ..Default::default()
            },
            ..Default::default()
        },
        &mut FamilyCompositionScratch::default(),
    )
    .unwrap();
    assert_eq!(result, semantic_to_direct(&changed, [0, 7]));
    assert_eq!(original.predictions.load(Ordering::Relaxed), 0);
}

#[test]
fn stable_ties_invalid_samples_and_reused_scratch_are_deterministic() {
    let mut low = sample(
        DynamicFamilyRepresentation::Angles,
        None,
        DynamicValue::Family(angles(15.0, 10.0)),
        1,
    );
    let mut high = sample(
        DynamicFamilyRepresentation::Angles,
        None,
        DynamicValue::Family(angles(30.0, 10.0)),
        2,
    );
    low.rank.stable_order = 0;
    high.rank.stable_order = 0;
    let mut scratch = FamilyCompositionScratch::default();
    let mut samples = [high, low];
    let run = |samples: &[FamilySample], scratch: &mut FamilyCompositionScratch| {
        compose_dynamic_family(
            ProgrammingOwner::Position,
            &angles(0.0, 10.0),
            samples,
            &FamilyCompositionContext::default(),
            scratch,
        )
    };
    assert_eq!(run(&samples, &mut scratch).unwrap(), angles(30.0, 10.0));
    samples.swap(0, 1);
    assert_eq!(run(&samples, &mut scratch).unwrap(), angles(30.0, 10.0));
    samples[0].activation_mix = f32::NAN;
    assert!(run(&samples, &mut scratch).is_err());
    samples[0].activation_mix = 1.0;
    samples[1].rank = samples[0].rank;
    assert!(run(&samples, &mut scratch).is_err());
    assert_eq!(run(&[], &mut scratch).unwrap(), angles(0.0, 10.0));
}

#[test]
fn whole_family_partial_takeover_uses_the_same_coherent_adoption_as_components() {
    let mut whole = sample(
        DynamicFamilyRepresentation::Angles,
        None,
        DynamicValue::Family(angles(1080.0, 90.0)),
        1,
    );
    whole.activation_mix = 0.5;
    let base = AttributeValue::Position(Arc::new(PositionIntent::target(
        TargetReference::Origin,
        [1.0; 3],
    )));
    let result = compose_dynamic_family(
        ProgrammingOwner::Position,
        &base,
        &[whole],
        &FamilyCompositionContext {
            edit: FamilyEditContext {
                solved_angles: Some(JointAngles {
                    pan_degrees: 720.0,
                    tilt_degrees: -30.0,
                }),
                ..Default::default()
            },
            ..Default::default()
        },
        &mut FamilyCompositionScratch::default(),
    )
    .unwrap();
    assert_eq!(result, angles(900.0, 30.0));
}

#[test]
fn orthogonal_only_color_effects_cannot_force_an_active_direct_base_to_semantic() {
    let model = native_model();
    let base = direct(&model);
    let adoption = ColorIntent::default();
    let result = compose_dynamic_family(
        ProgrammingOwner::Color,
        &base,
        &[color(ColorComponent::Uv, 1.0, 1)],
        &FamilyCompositionContext {
            edit: FamilyEditContext {
                semantic_color_adoption: Some(&adoption),
                ..Default::default()
            },
            ..Default::default()
        },
        &mut FamilyCompositionScratch::default(),
    )
    .unwrap();
    assert_eq!(result, base);
}

#[test]
fn focus_and_zoom_compose_in_their_units_and_require_optical_convention_adoption() {
    use light_core::OpeningConvention;
    let mut focus = sample(
        DynamicFamilyRepresentation::Focus,
        Some(ProgrammingComponent::Focus),
        DynamicValue::Scalar(0.8),
        1,
    );
    focus.activation_mix = 0.5;
    assert_eq!(
        compose(
            ProgrammingOwner::Focus,
            &AttributeValue::Normalized(0.2),
            &[focus]
        )
        .unwrap(),
        AttributeValue::Normalized(0.5)
    );
    let zoom_value = |degrees, convention| {
        AttributeValue::Zoom(Arc::new(ZoomIntent {
            opening_degrees: ScalarIntent::Value(degrees),
            convention,
        }))
    };
    let mut zoom = sample(
        DynamicFamilyRepresentation::Zoom {
            convention: OpeningConvention::Beam,
        },
        Some(ProgrammingComponent::Zoom),
        DynamicValue::Scalar(80.0),
        1,
    );
    zoom.activation_mix = 0.25;
    let samples = [zoom];
    assert_eq!(
        compose(
            ProgrammingOwner::Zoom,
            &zoom_value(40.0, OpeningConvention::Beam),
            &samples
        )
        .unwrap(),
        zoom_value(50.0, OpeningConvention::Beam)
    );
    assert_eq!(
        compose(
            ProgrammingOwner::Zoom,
            &zoom_value(40.0, OpeningConvention::Field),
            &samples
        )
        .unwrap_err(),
        TransitionError::Requires(TransitionRequirement::ZoomConvention)
    );
}

#[test]
fn complete_native_sources_are_verified_before_composition_and_overwritten_edits_do_not_predict() {
    let model = native_model();
    let address = Arc::new(
        CompiledDynamicValueAddress::new(
            DynamicValueAddress {
                representation: DynamicFamilyRepresentation::DirectColor {
                    source: model.source.clone(),
                },
                component: None,
            },
            Some(model.clone()),
        )
        .unwrap(),
    );
    let rank = color(ColorComponent::Red, 0.0, 2).rank;
    let mut invalid = direct(&model);
    if let AttributeValue::ColorProgram(color) = &mut invalid
        && let ColorProgram::Direct { recipe, .. } = Arc::make_mut(color)
    {
        recipe.channels.pop();
    }
    assert!(FamilySample::new(address.clone(), DynamicValue::Family(invalid), rank, 1.0).is_err());
    let whole = FamilySample::new(
        address,
        DynamicValue::Family(semantic_to_direct(&model, [20, 30])),
        rank,
        1.0,
    )
    .unwrap();
    model.predictions.store(0, Ordering::Relaxed);
    let samples = [
        native_sample(&model, 0, 9, 1),
        whole,
        native_sample(&model, 1, 40, 3),
    ];
    let result = compose_dynamic_family(
        ProgrammingOwner::Color,
        &direct(&model),
        &samples,
        &FamilyCompositionContext {
            edit: FamilyEditContext {
                native_model: Some(model.as_ref()),
                ..Default::default()
            },
            ..Default::default()
        },
        &mut FamilyCompositionScratch::default(),
    )
    .unwrap();
    assert_eq!(result, semantic_to_direct(&model, [20, 40]));
    assert_eq!(model.predictions.load(Ordering::Relaxed), 1);
}

#[test]
fn direct_component_mask_roundtrips_complete_original_recipe_and_verifies_its_model() {
    let model = native_model();
    let mask = crate::ProgrammingFamilyFixAt {
        address: DynamicValueAddress {
            representation: DynamicFamilyRepresentation::DirectColor {
                source: model.source.clone(),
            },
            component: Some(ProgrammingComponent::NativeColor(model.bindings[0])),
        },
        family: semantic_to_direct(&model, [u32::MAX - 1, u32::MAX - 2]),
    };
    let restored: crate::ProgrammingFamilyFixAt =
        serde_json::from_value(serde_json::to_value(&mask).unwrap()).unwrap();
    assert_eq!(restored, mask);
    assert_eq!(
        restored.required_programming_contract(),
        PROGRAMMING_CONTRACT_VERSION
    );
    let rank = color(ColorComponent::Red, 0.0, 1).rank;
    let fixed = restored
        .compile(
            Some(model.clone()),
            &FamilyEditContext::default(),
            rank,
            1.0,
        )
        .unwrap();
    assert!(fixed.is_fix_at());
    assert_eq!(
        fixed.materialized_value(),
        Some(&DynamicValue::Native(u32::MAX - 1))
    );
    assert_eq!(model.predictions.load(Ordering::Relaxed), 1);
    let mut replacement = native_model();
    Arc::get_mut(&mut replacement)
        .unwrap()
        .source
        .profile_revision += 1;
    assert!(
        restored
            .compile(Some(replacement), &FamilyEditContext::default(), rank, 1.0)
            .is_err()
    );
    let focus = crate::ProgrammingFamilyFixAt {
        address: DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Focus,
            component: Some(ProgrammingComponent::Focus),
        },
        family: AttributeValue::Normalized(0.5),
    };
    assert_eq!(
        focus.required_programming_contract(),
        PROGRAMMING_CONTRACT_VERSION
    );
}
