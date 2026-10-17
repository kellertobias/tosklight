//! Narrow compile-time proof; no sampled value, occurrence or frame state is cached.
use super::*;

pub(super) fn eligible(
    address: &DynamicValueAddress,
    configuration: &ProgrammingLaneConfiguration,
) -> bool {
    use DynamicSemanticColorBasis as B;
    use light_core::programming::{ColorComponent as C, ProgrammingComponent as P};
    let selected = matches!(
        (&address.representation, address.component),
        (
            DynamicFamilyRepresentation::SemanticColor { basis: B::Recipe },
            Some(P::Color(
                C::Red | C::Green | C::Blue | C::Amber | C::WhiteBlend
            ))
        ) | (
            DynamicFamilyRepresentation::SemanticColor { basis: B::Retain },
            Some(P::Color(C::Uv))
        )
    );
    selected
        && matches!(configuration, ProgrammingLaneConfiguration::MaxMin(config)
        if matches!(config.minimum, DynamicValueSource::Value { value: DynamicValue::Scalar(_) })
        && matches!(config.maximum, DynamicValueSource::Value { value: DynamicValue::Scalar(_) }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_core::programming::{ColorComponent as C, ProgrammingComponent as P};

    #[test]
    fn only_the_six_validated_literal_semantic_components_are_source_free() {
        let config = |source| {
            ProgrammingLaneConfiguration::MaxMin(MaxMinConfiguration {
                minimum: source,
                maximum: DynamicValueSource::Value {
                    value: DynamicValue::Scalar(0.9),
                },
                function: PeriodicFunction::Sinus,
                size: 0.7,
                pwm: PwmShape::default(),
            })
        };
        let literal = || DynamicValueSource::Value {
            value: DynamicValue::Scalar(0.2),
        };
        for basis in [
            DynamicSemanticColorBasis::Recipe,
            DynamicSemanticColorBasis::Retain,
            DynamicSemanticColorBasis::HueSaturation,
            DynamicSemanticColorBasis::Whole,
        ] {
            for component in [C::Red, C::Green, C::Blue, C::Amber, C::WhiteBlend, C::Uv] {
                let address = DynamicValueAddress {
                    representation: DynamicFamilyRepresentation::SemanticColor { basis },
                    component: Some(P::Color(component)),
                };
                let selected = match basis {
                    DynamicSemanticColorBasis::Recipe => component != C::Uv,
                    DynamicSemanticColorBasis::Retain => component == C::Uv,
                    _ => false,
                };
                assert_eq!(eligible(&address, &config(literal())), selected);
                assert!(!eligible(&address, &config(DynamicValueSource::Current)));
                assert!(!eligible(
                    &address,
                    &config(DynamicValueSource::Preset {
                        preset_id: "color".into(),
                        address: address.clone(),
                        last_valid_by_target: vec![],
                        retained: None,
                    })
                ));
            }
        }
        assert!(!eligible(
            &DynamicValueAddress {
                representation: DynamicFamilyRepresentation::Focus,
                component: Some(P::Focus),
            },
            &config(literal())
        ));
    }
}
