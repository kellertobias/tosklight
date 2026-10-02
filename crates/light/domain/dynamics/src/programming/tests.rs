use super::*;
use light_core::{AttributeValue, programming::*};
use std::sync::Arc;

fn scalar(
    representation: DynamicFamilyRepresentation,
    component: ProgrammingComponent,
) -> CompiledDynamicValueAddress {
    CompiledDynamicValueAddress::new(
        DynamicValueAddress {
            representation,
            component: Some(component),
        },
        None,
    )
    .unwrap()
}

#[test]
fn physical_dynamic_units_keep_signed_turns_metres_and_descriptor_interpolation() {
    let pan = scalar(
        DynamicFamilyRepresentation::Angles,
        ProgrammingComponent::Pan,
    );
    assert_eq!(
        pan.transition(DynamicValue::Scalar(-720.0), DynamicValue::Scalar(720.0))
            .unwrap()
            .sample(0.75)
            .unwrap(),
        DynamicValue::Scalar(360.0)
    );
    assert_eq!(
        pan.scale_from(
            &DynamicValue::Scalar(-90.0),
            &DynamicValue::Scalar(270.0),
            2.0
        )
        .unwrap(),
        DynamicValue::Scalar(630.0)
    );
    let target = scalar(
        DynamicFamilyRepresentation::Target {
            reference: Some(TargetReference::Origin),
        },
        ProgrammingComponent::TargetZ,
    );
    assert_eq!(
        target
            .around(
                &DynamicValue::Scalar(2.0),
                &DynamicValue::Scalar(10.0),
                -0.5
            )
            .unwrap(),
        DynamicValue::Scalar(-3.0)
    );
    let cct = scalar(
        DynamicFamilyRepresentation::SemanticColor {
            basis: DynamicSemanticColorBasis::Retain,
        },
        ProgrammingComponent::Color(ColorComponent::Temperature),
    );
    let DynamicValue::Scalar(kelvin) = cct
        .transition(DynamicValue::Scalar(2000.0), DynamicValue::Scalar(10000.0))
        .unwrap()
        .sample(0.5)
        .unwrap()
    else {
        panic!()
    };
    assert!((kelvin - 3333.3333).abs() < 0.001);
    let focus = scalar(
        DynamicFamilyRepresentation::Focus,
        ProgrammingComponent::Focus,
    );
    assert_eq!(
        focus
            .around(&DynamicValue::Scalar(0.5), &DynamicValue::Scalar(2.0), -1.0)
            .unwrap(),
        DynamicValue::Scalar(0.0)
    );
    assert!(
        pan.around(
            &DynamicValue::Scalar(f32::MAX),
            &DynamicValue::Scalar(f32::MAX),
            1.0
        )
        .is_err()
    );
}

#[test]
fn typed_dynamic_addresses_reject_conflicting_representations_and_discrete_controls() {
    for address in [
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Angles,
            component: Some(ProgrammingComponent::TargetX),
        },
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Target { reference: None },
            component: Some(ProgrammingComponent::TargetX),
        },
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Target {
                reference: Some(TargetReference::Origin),
            },
            component: Some(ProgrammingComponent::TargetReference),
        },
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::SemanticColor {
                basis: DynamicSemanticColorBasis::Recipe,
            },
            component: Some(ProgrammingComponent::Color(ColorComponent::Hue)),
        },
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::SemanticColor {
                basis: DynamicSemanticColorBasis::Retain,
            },
            component: None,
        },
    ] {
        assert!(address.validate().is_err(), "{address:?}");
    }
    let pan = scalar(
        DynamicFamilyRepresentation::Angles,
        ProgrammingComponent::Pan,
    );
    assert!(pan.validate_value(&DynamicValue::Native(1)).is_err());
}

#[test]
fn typed_source_extraction_preserves_units_uv_and_native_identity_without_implicit_adoption() {
    let context = FamilyEditContext {
        color_model: Some(&VirtualColorAuthoringV1),
        ..Default::default()
    };
    let position = AttributeValue::Position(Arc::new(PositionIntent::angles(720.0, -45.0)));
    let pan = scalar(
        DynamicFamilyRepresentation::Angles,
        ProgrammingComponent::Pan,
    );
    assert_eq!(
        extract_compatible_dynamic_value(&position, pan.address(), &context).unwrap(),
        Some(DynamicValue::Scalar(720.0))
    );
    let reference = TargetReference::Point {
        point_id: uuid::Uuid::from_u128(7),
    };
    let target = AttributeValue::Position(Arc::new(PositionIntent::target(
        reference,
        [-3.0, 2.0, 1.0],
    )));
    assert_eq!(
        extract_compatible_dynamic_value(&target, pan.address(), &context).unwrap(),
        None
    );
    let x = scalar(
        DynamicFamilyRepresentation::Target {
            reference: Some(reference),
        },
        ProgrammingComponent::TargetX,
    );
    assert_eq!(
        extract_compatible_dynamic_value(&target, x.address(), &context).unwrap(),
        Some(DynamicValue::Scalar(-3.0))
    );
    let different_reference = scalar(
        DynamicFamilyRepresentation::Target {
            reference: Some(TargetReference::Origin),
        },
        ProgrammingComponent::TargetX,
    );
    assert_eq!(
        extract_compatible_dynamic_value(&target, different_reference.address(), &context).unwrap(),
        None
    );
    let mut intent = ColorIntent::default();
    intent.recipe.rgb = [0.0; 3];
    intent.base_xyz = light_core::Xyz {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    intent.white_blend = 0.0;
    intent.relative_output = 0.0;
    intent.uv.amount = 0.75;
    let color = AttributeValue::ColorProgram(Arc::new(ColorProgram::Semantic { intent }));
    let uv = scalar(
        DynamicFamilyRepresentation::SemanticColor {
            basis: DynamicSemanticColorBasis::Retain,
        },
        ProgrammingComponent::Color(ColorComponent::Uv),
    );
    assert_eq!(
        extract_compatible_dynamic_value(&color, uv.address(), &context).unwrap(),
        Some(DynamicValue::Scalar(0.75))
    );
    let native = native_domain();
    let DynamicFamilyRepresentation::DirectColor { source } = &native.address().representation
    else {
        panic!()
    };
    let Some(ProgrammingComponent::NativeColor(binding)) = native.address().component else {
        panic!()
    };
    let direct = AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
        recipe: NativeColorRecipe {
            source: source.clone(),
            channels: vec![light_core::NativeColorValue {
                channel_id: binding.channel_id,
                function_id: binding.function_id,
                raw: u32::MAX - 1,
            }],
            spreads: vec![],
        },
        portable: PortableColorEstimate {
            model_revision: 1,
            visible: None,
            uv: None,
            quality: light_core::PhysicalDataQuality::Estimated,
            limitations: vec![],
        },
    }));
    assert_eq!(
        extract_compatible_dynamic_value(&direct, native.address(), &context).unwrap(),
        Some(DynamicValue::Native(u32::MAX - 1))
    );
    assert_eq!(
        extract_compatible_dynamic_value(&direct, uv.address(), &context).unwrap(),
        None
    );
    let mut wrong_model = native.address().clone();
    if let DynamicFamilyRepresentation::DirectColor { source } = &mut wrong_model.representation {
        source.profile_revision += 1;
    }
    assert_eq!(
        extract_compatible_dynamic_value(&direct, &wrong_model, &context).unwrap(),
        None
    );
    let unmaterialized = AttributeValue::Position(Arc::new(PositionIntent::Angles {
        pan_degrees: ScalarIntent::Spread(vec![0.0, 720.0]),
        tilt_degrees: ScalarIntent::Value(0.0),
    }));
    assert!(extract_compatible_dynamic_value(&unmaterialized, pan.address(), &context).is_err());
}

#[test]
fn whole_dynamic_values_preserve_uv_and_require_live_context_for_target_changes() {
    let color = CompiledDynamicValueAddress::new(
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::SemanticColor {
                basis: DynamicSemanticColorBasis::Whole,
            },
            component: None,
        },
        None,
    )
    .unwrap();
    let mut intent = ColorIntent::default();
    VirtualColorAuthoringV1
        .set_coordinates(
            &mut intent,
            light_core::Xyz {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
        )
        .unwrap();
    let mut target = intent.clone();
    target.uv.amount = 1.0;
    let value = |intent| {
        DynamicValue::Family(AttributeValue::ColorProgram(Arc::new(
            ColorProgram::Semantic { intent },
        )))
    };
    let transition = color
        .transition(value(intent.clone()), value(target))
        .unwrap();
    let DynamicValue::Family(AttributeValue::ColorProgram(midpoint)) =
        transition.sample(0.5).unwrap()
    else {
        panic!()
    };
    let ColorProgram::Semantic { intent: midpoint } = midpoint.as_ref() else {
        panic!()
    };
    assert_eq!(midpoint.base_xyz, intent.base_xyz);
    assert_eq!(midpoint.recipe, intent.recipe);
    assert_eq!(midpoint.uv.amount, 0.5);

    let target = CompiledDynamicValueAddress::new(
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Target { reference: None },
            component: None,
        },
        None,
    )
    .unwrap();
    let value = |reference| {
        DynamicValue::Family(AttributeValue::Position(Arc::new(PositionIntent::target(
            reference, [0.0; 3],
        ))))
    };
    let transition = target
        .transition(
            value(TargetReference::Origin),
            value(TargetReference::Point {
                point_id: uuid::Uuid::new_v4(),
            }),
        )
        .unwrap();
    assert_eq!(
        transition.sample(0.5).unwrap_err(),
        TransitionError::Requires(TransitionRequirement::LiveTargetPoints)
    );
    assert_eq!(&transition.sample(1.0).unwrap(), transition.endpoints().1);
}

struct NativeModel {
    source: light_core::NativeColorIdentity,
    descriptor: NativeColorComponentDescriptor,
}
impl NativeColorEditModel for NativeModel {
    fn source(&self) -> &light_core::NativeColorIdentity {
        &self.source
    }
    fn descriptor(
        &self,
        binding: light_core::NativeColorBinding,
    ) -> Option<NativeColorComponentDescriptor> {
        (binding == self.descriptor.binding).then_some(self.descriptor)
    }
    fn predict(&self, _: &NativeColorRecipe) -> Result<PortableColorEstimate, IntentError> {
        panic!("individual native lanes must not predict color before family composition")
    }
}
fn native_domain() -> CompiledDynamicValueAddress {
    let model = Arc::new(NativeModel {
        source: light_core::NativeColorIdentity {
            profile_id: uuid::Uuid::from_u128(1),
            profile_revision: 2,
            profile_digest: "pinned-source".into(),
            mode_id: uuid::Uuid::from_u128(2),
            head_id: uuid::Uuid::from_u128(3),
            path_id: uuid::Uuid::from_u128(4),
            model_revision: 1,
            native_layout_signature: "native-layout".into(),
        },
        descriptor: NativeColorComponentDescriptor {
            binding: light_core::NativeColorBinding {
                channel_id: uuid::Uuid::from_u128(5),
                function_id: uuid::Uuid::from_u128(6),
            },
            raw_from: u32::MAX,
            raw_to: 0,
            continuous: true,
        },
    });
    let address = DynamicValueAddress {
        representation: DynamicFamilyRepresentation::DirectColor {
            source: model.source.clone(),
        },
        component: Some(ProgrammingComponent::NativeColor(model.descriptor.binding)),
    };
    assert!(CompiledDynamicValueAddress::new(address.clone(), None).is_err());
    CompiledDynamicValueAddress::new(address, Some(model)).unwrap()
}

#[test]
fn native_dynamic_interpolation_size_amplitude_and_storage_keep_low_bits() {
    let domain = native_domain();
    let amount = (f64::from(0.2_f32) * 2.0 - 1.0) * f64::from(0.1_f32);
    let expected = (f64::from(u32::MAX) + f64::from(u32::MAX) * amount).round() as u32;
    assert_eq!(
        domain
            .around_wide(
                &DynamicValue::Native(u32::MAX),
                &DynamicValue::Native(u32::MAX),
                amount
            )
            .unwrap(),
        DynamicValue::Native(expected)
    );
    assert_eq!(
        domain
            .wave_between(
                &DynamicValue::Native(0),
                &DynamicValue::Native(u32::MAX),
                0.0,
                0.1
            )
            .unwrap(),
        DynamicValue::Native(1_932_735_280)
    );
    let transition = domain
        .transition(DynamicValue::Native(u32::MAX), DynamicValue::Native(0))
        .unwrap();
    assert_eq!(
        transition.sample(0.4999999403953552).unwrap(),
        DynamicValue::Native(2_147_483_903)
    );
    assert_eq!(
        domain
            .scale_from(
                &DynamicValue::Native(2_000_000_000),
                &DynamicValue::Native(2_000_000_001),
                2.0
            )
            .unwrap(),
        DynamicValue::Native(2_000_000_002)
    );
    assert_eq!(
        domain
            .around(
                &DynamicValue::Native(2_000_000_001),
                &DynamicValue::Native(1),
                1.0
            )
            .unwrap(),
        DynamicValue::Native(2_000_000_002)
    );
    assert_eq!(
        domain
            .scale_from(
                &DynamicValue::Native(0),
                &DynamicValue::Native(u32::MAX),
                f32::MAX
            )
            .unwrap(),
        DynamicValue::Native(u32::MAX)
    );
    let value = DynamicValue::Native(u32::MAX - 1);
    assert_eq!(
        serde_json::from_str::<DynamicValue>(&serde_json::to_string(&value).unwrap()).unwrap(),
        value
    );
    let DynamicFamilyRepresentation::DirectColor { source } = &domain.address().representation
    else {
        panic!()
    };
    let mut other = source.clone();
    other.profile_revision += 1;
    let address = DynamicValueAddress {
        representation: DynamicFamilyRepresentation::DirectColor { source: other },
        component: domain.address().component,
    };
    assert!(validate_dynamic_value_addresses([domain.address(), &address]).is_err());
}

struct Sources {
    current: Option<DynamicValue>,
    preset: Option<DynamicValue>,
}
impl DynamicValueSourceResolver for Sources {
    fn current(&self, _: light_core::FixtureId, _: &DynamicValueAddress) -> Option<DynamicValue> {
        self.current.clone()
    }
    fn preset(
        &self,
        _: &DynamicPresetSourceBinding,
        _: uuid::Uuid,
        _: light_core::FixtureId,
    ) -> Option<DynamicValue> {
        self.preset.clone()
    }
}

#[test]
fn typed_preset_fallbacks_are_target_specific_and_live_values_win() {
    let domain = native_domain();
    let target = light_core::FixtureId::new();
    let fallback = DynamicValue::Native(u32::MAX - 1);
    let source = DynamicValueSource::Preset {
        retained: None,
        preset_id: "2.7".into(),
        address: domain.address().clone(),
        last_valid_by_target: vec![DynamicValueFallback {
            target,
            value: fallback.clone(),
        }],
    };
    let stored = serde_json::to_string(&source).unwrap();
    let source: DynamicValueSource = serde_json::from_str(&stored).unwrap();
    let compiled = source.compile(&domain).unwrap();
    let missing = Sources {
        current: None,
        preset: None,
    };
    let instance = uuid::Uuid::new_v4();
    assert_eq!(compiled.resolve(instance, target, &missing), Some(fallback));
    assert_eq!(
        compiled.resolve(instance, light_core::FixtureId::new(), &missing),
        None
    );
    let live = Sources {
        current: Some(DynamicValue::Native(2_000_000_001)),
        preset: Some(DynamicValue::Native(2_000_000_003)),
    };
    assert_eq!(compiled.resolve(instance, target, &live), live.preset);
    assert_eq!(
        DynamicValueSource::Current
            .compile(&domain)
            .unwrap()
            .resolve(instance, target, &live),
        live.current
    );
    assert_eq!(
        DynamicValueSource::Current
            .compile(&domain)
            .unwrap()
            .resolve(instance, target, &missing),
        None
    );
    let mut incompatible = source;
    let DynamicValueSource::Preset { address, .. } = &mut incompatible else {
        panic!()
    };
    let DynamicFamilyRepresentation::DirectColor { source } = &mut address.representation else {
        panic!()
    };
    source.profile_digest = "different-profile".into();
    assert!(incompatible.compile(&domain).is_err());
}

#[test]
fn contribution_validation_composes_orthogonal_color_but_never_mixed_base_representations() {
    let address = |basis, component| DynamicValueAddress {
        representation: DynamicFamilyRepresentation::SemanticColor { basis },
        component: Some(ProgrammingComponent::Color(component)),
    };
    let uv = address(DynamicSemanticColorBasis::Retain, ColorComponent::Uv);
    let hue = address(
        DynamicSemanticColorBasis::HueSaturation,
        ColorComponent::Hue,
    );
    let red = address(DynamicSemanticColorBasis::Recipe, ColorComponent::Red);
    assert!(validate_dynamic_value_addresses([&uv, &hue]).is_ok());
    assert!(validate_dynamic_value_addresses([&uv, &red]).is_ok());
    // Starting with an orthogonal lane must not hide a later contradictory writer.
    assert!(validate_dynamic_value_addresses([&uv, &hue, &red]).is_err());
    let whole = DynamicValueAddress {
        representation: DynamicFamilyRepresentation::SemanticColor {
            basis: DynamicSemanticColorBasis::Whole,
        },
        component: None,
    };
    assert!(validate_dynamic_value_addresses([&whole, &uv]).is_ok());
    assert!(validate_dynamic_value_addresses([&whole, &red]).is_err());
    let pan = DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Angles,
        component: Some(ProgrammingComponent::Pan),
    };
    let tilt = DynamicValueAddress {
        component: Some(ProgrammingComponent::Tilt),
        ..pan.clone()
    };
    assert!(validate_dynamic_value_addresses([&pan, &tilt]).is_ok());
    let target = DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Target {
            reference: Some(TargetReference::Origin),
        },
        component: Some(ProgrammingComponent::TargetX),
    };
    assert!(validate_dynamic_value_addresses([&pan, &target]).is_err());
}
