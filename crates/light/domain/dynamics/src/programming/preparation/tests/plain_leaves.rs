//! TL-639: plain Programming and legacy scalar leaves skip the generic traversals. Each shortcut
//! must give exactly what the traversal gives; an `Operation` wrapper without an origin is
//! transparent to every view and forces the generic path.
use super::*;

fn wrapped(expression: &E) -> E {
    E::Operation {
        origin: None,
        value: Arc::new(expression.clone()),
    }
}

fn legacy(attribute: &str, value: f32) -> E {
    E::LegacyScalar {
        attribute: AttributeKey(attribute.into()),
        value,
        occurrence: None,
        dependency_occurrence: None,
    }
}

fn leaves() -> Vec<E> {
    vec![
        angle(ProgrammingComponent::Pan, 12.0),
        angle(ProgrammingComponent::Tilt, f32::NAN),
        color(ColorComponent::Red, 0.4),
        color(ColorComponent::Uv, 0.8),
        E::Programming {
            address: angle_address(ProgrammingComponent::Pan),
            value: DynamicValue::Family(angles(1.0, 2.0)),
            occurrence: None,
            dependency_occurrence: None,
        },
        E::Programming {
            address: Arc::new(DynamicValueAddress {
                representation: DynamicFamilyRepresentation::Angles,
                component: None,
            }),
            value: DynamicValue::Family(angles(1.0, 2.0)),
            occurrence: None,
            dependency_occurrence: None,
        },
        legacy("intensity", 0.5),
        legacy("intensity", f32::INFINITY),
        legacy("not-a-dynamic-attribute", 0.5),
    ]
}

#[test]
fn leaf_validation_equals_retained_tape_validation() {
    for leaf in leaves() {
        let direct = leaf.validate().map_err(|error| error.0);
        let imported = RetainedExpressionTape::from_roots(&[Arc::new(leaf.clone())])
            .map(|_| ())
            .map_err(|error| error.0);
        assert_eq!(direct, imported, "{leaf:?}");
    }
}

#[test]
fn leaf_owner_split_shape_and_angles_equal_the_generic_traversal() {
    for leaf in leaves() {
        let generic = wrapped(&leaf);
        assert_eq!(
            leaf.contains_angles(),
            generic.contains_angles(),
            "{leaf:?}"
        );
        let owners = |expression: E| {
            split_owners(Arc::new(expression))
                .map(|parts| {
                    parts
                        .into_iter()
                        .map(|(owner, _)| owner)
                        .collect::<Vec<_>>()
                })
                .map_err(|error| format!("{error:?}"))
        };
        assert_eq!(owners(leaf.clone()), owners(generic.clone()), "{leaf:?}");
        let shape = |expression: &E| {
            classify(expression)
                .map(|shape| (shape.native, shape.components, shape.whole))
                .map_err(|error| format!("{error:?}"))
        };
        assert_eq!(shape(&leaf), shape(&generic), "{leaf:?}");
    }
}
