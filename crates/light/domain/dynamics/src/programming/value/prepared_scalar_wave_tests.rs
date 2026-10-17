use super::*;
use crate::{DynamicFamilyRepresentation as R, DynamicSemanticColorBasis as B};

fn compiled(component: ColorComponent) -> CompiledDynamicValueAddress {
    CompiledDynamicValueAddress::new(
        DynamicValueAddress {
            representation: R::SemanticColor {
                basis: if component == ColorComponent::Uv {
                    B::Retain
                } else {
                    B::Recipe
                },
            },
            component: Some(ProgrammingComponent::Color(component)),
        },
        None,
    )
    .unwrap()
}
fn bits(value: Result<DynamicValue, IntentError>) -> Result<u32, String> {
    value
        .map(|value| match value {
            DynamicValue::Scalar(value) => value.to_bits(),
            _ => panic!("scalar kernel"),
        })
        .map_err(|error| error.to_string())
}
#[test]
fn sealed_scalar_wave_matches_existing_arithmetic_bits_and_errors_for_all_six_slots() {
    for component in [
        ColorComponent::Red,
        ColorComponent::Green,
        ColorComponent::Blue,
        ColorComponent::Amber,
        ColorComponent::WhiteBlend,
        ColorComponent::Uv,
    ] {
        let address = compiled(component);
        for (low, high) in [
            (0., 1.),
            (1., 0.),
            (0.3, 0.3),
            (-0.0, 0.0),
            (0.0, -0.0),
            (f32::from_bits(1), f32::EPSILON),
        ] {
            for size in [0., -0.0, f32::EPSILON, 0.3, 1., 2., f32::MAX] {
                let prepared = address.prepare_scalar_wave(low, high, size).unwrap();
                for amount in [
                    -f32::MAX,
                    -1.,
                    -0.0,
                    0.,
                    f32::from_bits(1),
                    0.3,
                    0.5,
                    1.,
                    f32::MAX,
                    f32::NAN,
                    f32::INFINITY,
                    f32::NEG_INFINITY,
                ] {
                    assert_eq!(
                        bits(prepared.sample(amount)),
                        bits(address.wave_between(
                            &DynamicValue::Scalar(low),
                            &DynamicValue::Scalar(high),
                            amount,
                            size
                        )),
                        "{component:?} low={low} high={high} size={size} amount={amount}"
                    );
                }
            }
        }
    }
}
#[test]
fn sealed_scalar_wave_constructor_rejects_invalid_operands_and_unsupported_addresses() {
    let address = compiled(ColorComponent::Red);
    for (low, high, size) in [
        (f32::NAN, 1., 1.),
        (0., f32::INFINITY, 1.),
        (-0.1, 1., 1.),
        (0., 1.1, 1.),
        (0., 1., -1.),
        (0., 1., f32::NAN),
    ] {
        assert!(address.prepare_scalar_wave(low, high, size).is_none());
    }
    for address in [
        DynamicValueAddress {
            representation: R::Focus,
            component: Some(ProgrammingComponent::Focus),
        },
        DynamicValueAddress {
            representation: R::Angles,
            component: Some(ProgrammingComponent::Pan),
        },
        DynamicValueAddress {
            representation: R::SemanticColor {
                basis: B::HueSaturation,
            },
            component: Some(ProgrammingComponent::Color(ColorComponent::Hue)),
        },
    ] {
        assert!(
            CompiledDynamicValueAddress::new(address, None)
                .unwrap()
                .prepare_scalar_wave(0., 1., 1.)
                .is_none()
        );
    }
}
