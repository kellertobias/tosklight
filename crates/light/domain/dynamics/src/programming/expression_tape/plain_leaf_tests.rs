//! Borrowed leaf checks must agree with the owned retained-node validation boundary.
use super::*;
use crate::{DynamicSemanticColorBasis, DynamicSourceDependency};

fn leaf(component: ProgrammingComponent, value: DynamicValue) -> DynamicSampleExpression {
    DynamicSampleExpression::Programming {
        address: Arc::new(DynamicValueAddress {
            representation: DynamicFamilyRepresentation::SemanticColor {
                basis: DynamicSemanticColorBasis::Recipe,
            },
            component: Some(component),
        }),
        value,
        occurrence: Some(DynamicSourceOccurrenceId::new(Uuid::from_u128(81)).unwrap()),
        dependency_occurrence: None,
    }
}

fn result(expression: &DynamicSampleExpression) -> Result<(), String> {
    RetainedExpressionTape::validate_plain_leaf(expression)
        .expect("plain leaf")
        .map_err(|error| error.to_string())
}

fn owned_oracle(expression: &DynamicSampleExpression) -> Result<(), String> {
    // Force the complete graph importer through an Operation wrapper, independently of the
    // borrowed fast path. The original leaf is copied and checked by node_summary.
    let wrapped = Arc::new(DynamicSampleExpression::Operation {
        origin: None,
        value: Arc::new(expression.clone()),
    });
    RetainedExpressionTape::from_roots(&[wrapped])
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn assert_oracle(expression: &DynamicSampleExpression) {
    assert_eq!(result(expression), owned_oracle(expression));
}

fn invalid_dependency() -> DynamicSourceDependency {
    DynamicSourceDependency::mapped(
        None,
        ProgrammingFieldTransfer {
            identity: ProgrammingFieldScope::from_component(ProgrammingComponent::Focus),
            remap: Arc::from([]),
        },
    )
}

#[test]
fn borrowed_leaf_matches_owned_checks_for_fresh_values_and_dependencies() {
    for component in [
        ColorComponent::Red,
        ColorComponent::Green,
        ColorComponent::Blue,
        ColorComponent::Amber,
        ColorComponent::WhiteBlend,
    ] {
        for value in [
            0.0,
            0.2,
            1.0,
            -0.1,
            1.1,
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
        ] {
            let mut expression = leaf(
                ProgrammingComponent::Color(component),
                DynamicValue::Scalar(value),
            );
            let DynamicSampleExpression::Programming { address, .. } = &expression else {
                unreachable!()
            };
            let compatible = DynamicSourceDependency::compatible(None, address);
            for dependency in [
                None,
                Some(compatible),
                Some(DynamicSourceDependency::identity(None)),
                Some(DynamicSourceDependency::unknown(None)),
                Some(invalid_dependency()),
            ] {
                let DynamicSampleExpression::Programming {
                    dependency_occurrence,
                    ..
                } = &mut expression
                else {
                    unreachable!()
                };
                *dependency_occurrence = dependency;
                assert_oracle(&expression);
                if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                    assert!(result(&expression).is_err());
                }
            }
        }
    }
}

#[test]
fn borrowed_leaf_preserves_address_then_value_then_dependency_error_order() {
    let mut expression = leaf(ProgrammingComponent::Focus, DynamicValue::Scalar(f32::NAN));
    let DynamicSampleExpression::Programming {
        dependency_occurrence,
        ..
    } = &mut expression
    else {
        unreachable!()
    };
    *dependency_occurrence = Some(invalid_dependency());
    assert_oracle(&expression);
    assert_eq!(
        result(&expression).unwrap_err(),
        "Dynamic component belongs to a different owner"
    );
    let DynamicSampleExpression::Programming { address, .. } = &mut expression else {
        unreachable!()
    };
    Arc::make_mut(address).component = Some(ProgrammingComponent::Color(ColorComponent::Red));
    assert_oracle(&expression);
    assert_eq!(
        result(&expression).unwrap_err(),
        "Dynamic scalar is outside its declared component domain"
    );
    let DynamicSampleExpression::Programming { value, .. } = &mut expression else {
        unreachable!()
    };
    *value = DynamicValue::Scalar(0.5);
    assert_oracle(&expression);
    assert!(
        result(&expression).is_err(),
        "the fresh dependency is still validated"
    );
}

#[test]
fn borrowed_leaf_rejects_wrong_value_shape_and_hot_address_edits() {
    let mut expression = leaf(
        ProgrammingComponent::Color(ColorComponent::Red),
        DynamicValue::Native(7),
    );
    assert_oracle(&expression);
    assert!(result(&expression).is_err());
    let DynamicSampleExpression::Programming { value, address, .. } = &mut expression else {
        unreachable!()
    };
    *value = DynamicValue::Scalar(0.5);
    Arc::make_mut(address).representation = DynamicFamilyRepresentation::SemanticColor {
        basis: DynamicSemanticColorBasis::Retain,
    };
    assert_oracle(&expression);
    assert!(
        result(&expression).is_err(),
        "an old valid component cannot qualify an edited address"
    );
    let DynamicSampleExpression::Programming { address, .. } = &mut expression else {
        unreachable!()
    };
    Arc::make_mut(address).representation = DynamicFamilyRepresentation::SemanticColor {
        basis: DynamicSemanticColorBasis::Recipe,
    };
    assert_oracle(&expression);
    assert!(result(&expression).is_ok());
    let wrapped = DynamicSampleExpression::Operation {
        origin: None,
        value: Arc::new(expression),
    };
    assert!(RetainedExpressionTape::validate_plain_leaf(&wrapped).is_none());
}
