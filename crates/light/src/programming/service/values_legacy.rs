//! TL-552 follow-up: legacy live addresses at programming contract ≥ 1.
//!
//! The load-time validator refuses a show or a stored Programmer that holds legacy normalized
//! Position (`pan`/`tilt`), Color component (`color.red`, …) or percentage Zoom programming. So
//! that nothing a live writer accepts can trip it on the next open, every Programmer write at
//! contract ≥ 1 either routes legacy input into the semantic family representation, exactly as
//! the operator's encoders do, or refuses it before mutation with an actionable message:
//!
//! - A Color component whose encoder slot carries the same percentage on the semantic Color owner
//!   (Red, Green, Blue, Amber, UV, Saturation, White Blend) becomes that component edit of
//!   `color`: an absolute value or spread is `set`, a relative step is `relative`.
//! - Everything else is refused: a normalized `pan`/`tilt` is a percentage of an unknown travel
//!   and is never reinterpreted as degrees, and a Hue, Temperature, Duv, White emitter or
//!   CMY/lime/mint percentage has no single semantic meaning.
use super::super::{ProgrammingValueIntent, ProgrammingValueOperation};
use crate::{ActionError, ActionErrorKind};
use light_core::{
    AttributeKey, AttributeValue,
    programming::{
        ColorComponent, ComponentEdit, ProgrammingComponent, ProgrammingOwner, ScalarEdit,
        ScalarIntent,
    },
};
use std::borrow::Cow;

/// The legacy Color encoder ids whose percentage is the same component of the semantic Color
/// owner (the Color family page slots `color.red`, … in `family_encoder_pages`).
fn semantic_color_component(attribute: &str) -> Option<ColorComponent> {
    Some(match attribute {
        "color.red" => ColorComponent::Red,
        "color.green" => ColorComponent::Green,
        "color.blue" => ColorComponent::Blue,
        "color.amber" => ColorComponent::Amber,
        "color.uv" => ColorComponent::Uv,
        "color.saturation" => ColorComponent::Saturation,
        "color.white_blend" => ColorComponent::WhiteBlend,
        _ => return None,
    })
}

/// Refuses one authored fixture or Group value at a legacy address (contract ≥ 1).
pub(super) fn refuse_legacy_value(
    attribute: &AttributeKey,
    value: &AttributeValue,
    supported: u16,
) -> Result<(), ActionError> {
    if supported == 0 {
        return Ok(());
    }
    match light_show::legacy_attribute_value(&attribute.0, value) {
        Some(family) => Err(invalid(light_show::legacy_live_write_message(
            &attribute.0,
            family,
            supported,
        ))),
        None => Ok(()),
    }
}

/// The operator gesture this intent means at `supported`: unchanged unless it addresses a
/// legacy family channel, converted where the meaning is unambiguous, refused otherwise.
pub(super) fn semantic_value_intent(
    intent: &ProgrammingValueIntent,
    supported: u16,
) -> Result<Cow<'_, ProgrammingValueIntent>, ActionError> {
    if supported == 0 {
        return Ok(Cow::Borrowed(intent));
    }
    let attribute = &intent.attribute.0;
    let family = match &intent.operation {
        ProgrammingValueOperation::AbsoluteSet(value) => {
            light_show::legacy_attribute_value(attribute, value)
        }
        // A relative step of a channel is a percentage step.
        ProgrammingValueOperation::RelativeStep(_) => {
            light_show::legacy_programming_address(attribute, Some("normalized"))
        }
        // Component edits must address their complete family; intent validation refuses them.
        ProgrammingValueOperation::ComponentEdits(_) => None,
    };
    let Some(family) = family else {
        return Ok(Cow::Borrowed(intent));
    };
    let operation = match (semantic_color_component(attribute), &intent.operation) {
        (Some(_), ProgrammingValueOperation::AbsoluteSet(AttributeValue::Normalized(value))) => {
            ScalarEdit::Set(ScalarIntent::Value(*value))
        }
        (Some(_), ProgrammingValueOperation::AbsoluteSet(AttributeValue::Spread(points))) => {
            ScalarEdit::Set(ScalarIntent::Spread(points.clone()))
        }
        (Some(_), ProgrammingValueOperation::RelativeStep(delta)) => ScalarEdit::Relative(*delta),
        _ => {
            return Err(invalid(light_show::legacy_live_write_message(
                attribute, family, supported,
            )));
        }
    };
    let component = semantic_color_component(attribute).expect("matched above");
    Ok(Cow::Owned(ProgrammingValueIntent {
        attribute: ProgrammingOwner::Color.key(),
        operation: ProgrammingValueOperation::ComponentEdits(vec![ComponentEdit::Scalar {
            component: ProgrammingComponent::Color(component),
            operation,
        }]),
        ..intent.clone()
    }))
}

/// [`semantic_value_intent`] for a values command's intent (Normal and Preload lanes), against
/// the runtime contract of the action's values environment.
pub(super) fn semantic_intent<'a>(
    intent: Option<&'a ProgrammingValueIntent>,
    environment: Option<&super::super::ProgrammingValuesEnvironment>,
) -> Result<Option<Cow<'a, ProgrammingValueIntent>>, ActionError> {
    let supported = environment.map_or(0, |environment| environment.supported_programming_contract);
    intent
        .map(|intent| semantic_value_intent(intent, supported))
        .transpose()
}

/// A gesture started on a converted Color component lives on `color`; its Finish follows it.
pub(super) fn semantic_gesture_attribute(attribute: &AttributeKey) -> Cow<'_, AttributeKey> {
    if semantic_color_component(&attribute.0).is_some() {
        Cow::Owned(ProgrammingOwner::Color.key())
    } else {
        Cow::Borrowed(attribute)
    }
}

fn invalid(message: String) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_core::FixtureId;

    fn intent(attribute: &str, operation: ProgrammingValueOperation) -> ProgrammingValueIntent {
        ProgrammingValueIntent {
            fixture_ids: vec![FixtureId::new()],
            group_id: None,
            attribute: AttributeKey(attribute.into()),
            operation,
            undo_group: Some("encoder".into()),
            timing: Default::default(),
            displayed_source: None,
            color_adoption: Default::default(),
        }
    }

    #[test]
    fn legacy_color_components_become_color_edits_and_ambiguous_channels_are_refused() {
        use ProgrammingValueOperation as Op;
        let red = intent(
            "color.red",
            Op::AbsoluteSet(AttributeValue::Normalized(0.25)),
        );
        let converted = semantic_value_intent(&red, 1).unwrap();
        assert_eq!(converted.attribute, ProgrammingOwner::Color.key());
        assert_eq!(
            converted.operation,
            Op::ComponentEdits(vec![ComponentEdit::Scalar {
                component: ProgrammingComponent::Color(ColorComponent::Red),
                operation: ScalarEdit::Set(ScalarIntent::Value(0.25)),
            }])
        );
        assert_eq!(converted.undo_group.as_deref(), Some("encoder"));
        let white_blend = intent("color.white_blend", Op::RelativeStep(-0.1));
        let step = semantic_value_intent(&white_blend, 1).unwrap();
        assert!(matches!(
            &step.operation,
            Op::ComponentEdits(edits) if edits == &[ComponentEdit::Scalar {
                component: ProgrammingComponent::Color(ColorComponent::WhiteBlend),
                operation: ScalarEdit::Relative(-0.1),
            }]
        ));
        for (attribute, operation) in [
            ("pan", Op::AbsoluteSet(AttributeValue::Normalized(0.5))),
            ("tilt", Op::RelativeStep(0.01)),
            (
                "color.hue",
                Op::AbsoluteSet(AttributeValue::Normalized(0.5)),
            ),
            (
                "color.white",
                Op::AbsoluteSet(AttributeValue::Normalized(0.5)),
            ),
            ("color.cyan", Op::RelativeStep(0.01)),
            ("color.red", Op::AbsoluteSet(AttributeValue::RawDmx(12))),
            ("zoom", Op::AbsoluteSet(AttributeValue::Normalized(0.5))),
        ] {
            let error = semantic_value_intent(&intent(attribute, operation), 1).unwrap_err();
            assert_eq!(error.kind, ActionErrorKind::Invalid, "{attribute}");
            assert!(error.message.contains(attribute), "{}", error.message);
            assert!(
                error.message.contains("Nothing was changed"),
                "{}",
                error.message
            );
        }
        // Semantic, non-family and contract-0 intents pass through untouched.
        for (attribute, operation, supported) in [
            ("intensity", Op::RelativeStep(0.1), 1),
            (
                "color.wheel.1",
                Op::AbsoluteSet(AttributeValue::Normalized(0.5)),
                1,
            ),
            ("pan", Op::AbsoluteSet(AttributeValue::Normalized(0.5)), 0),
        ] {
            assert!(matches!(
                semantic_value_intent(&intent(attribute, operation), supported).unwrap(),
                Cow::Borrowed(_)
            ));
        }
        assert!(
            refuse_legacy_value(
                &AttributeKey("pan".into()),
                &AttributeValue::Normalized(0.1),
                1
            )
            .is_err()
        );
        assert!(
            refuse_legacy_value(
                &AttributeKey("pan".into()),
                &AttributeValue::Normalized(0.1),
                0
            )
            .is_ok()
        );
        assert!(
            refuse_legacy_value(
                &AttributeKey("intensity".into()),
                &AttributeValue::Normalized(0.1),
                1
            )
            .is_ok()
        );
    }
}
