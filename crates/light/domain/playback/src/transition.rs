use crate::*;

/// Whether a Cue transition must snap instead of interpolate.
///
/// Cue timing follows the canonical attribute registry. Fixture-profile channel flags still
/// govern profile-local behavior such as Programmer fades and signal-loss handling, but they do
/// not redefine the operator-facing value type stored in a Cue.
pub fn attribute_uses_snap_transition(attribute: &AttributeKey) -> bool {
    matches!(
        light_core::attribute_descriptor(attribute).value_type,
        light_core::AttributeValueType::Indexed | light_core::AttributeValueType::Control
    )
}

pub(crate) fn interpolate(
    from: Option<&AttributeValue>,
    to: Option<&AttributeValue>,
    progress: f32,
) -> Option<AttributeValue> {
    if progress >= 1.0 {
        return to.cloned();
    }
    match (from, to) {
        (Some(from), Some(to)) => {
            match light_core::programming::interpolate_programming_value(from, to, progress) {
                Ok(value) => Some(value),
                // Pre-activation compatibility hold. TL-548 must carry the endpoint pair, progress
                // and lane identity through arbitration; the held value alone cannot recover them.
                Err(light_core::programming::TransitionError::Requires(_)) => Some(from.clone()),
                Err(light_core::programming::TransitionError::Invalid(_)) => None,
            }
        }
        (None, Some(AttributeValue::Normalized(to))) => {
            Some(AttributeValue::Normalized(to * progress))
        }
        (Some(AttributeValue::Normalized(from)), None) => {
            Some(AttributeValue::Normalized(from * (1.0 - progress)))
        }
        (None, Some(AttributeValue::ColorXyz(to))) => {
            Some(AttributeValue::ColorXyz(light_core::Xyz {
                x: to.x * progress,
                y: to.y * progress,
                z: to.z * progress,
            }))
        }
        (Some(AttributeValue::ColorXyz(from)), None) => {
            Some(AttributeValue::ColorXyz(light_core::Xyz {
                x: from.x * (1.0 - progress),
                y: from.y * (1.0 - progress),
                z: from.z * (1.0 - progress),
            }))
        }
        (Some(from), _) => Some(from.clone()),
        (None, Some(to)) if progress >= 1.0 => Some(to.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_cue_transitions_share_physical_units_and_never_invent_a_zero_owner() {
        let position = |pan| {
            AttributeValue::Position(std::sync::Arc::new(
                light_core::programming::PositionIntent::angles(pan, 0.0),
            ))
        };
        assert_eq!(
            interpolate(Some(&position(0.0)), Some(&position(720.0)), 0.5),
            Some(position(360.0))
        );
        assert_eq!(interpolate(None, Some(&position(720.0)), 0.5), None);
        assert_eq!(
            interpolate(Some(&position(720.0)), None, 0.5),
            Some(position(720.0))
        );
        assert_eq!(interpolate(Some(&position(720.0)), None, 1.0), None);
        let from = AttributeValue::ColorProgram(std::sync::Arc::new(
            light_core::programming::ColorProgram::Semantic {
                intent: light_core::programming::ColorIntent::default(),
            },
        ));
        let to = AttributeValue::ColorProgram(std::sync::Arc::new(
            light_core::programming::ColorProgram::Semantic {
                intent: light_core::programming::ColorIntent {
                    uv: light_core::programming::UvIntent { amount: 1.0 },
                    white_blend: 1.0,
                    ..Default::default()
                },
            },
        ));
        let Some(AttributeValue::ColorProgram(midpoint)) = interpolate(Some(&from), Some(&to), 0.5)
        else {
            panic!()
        };
        let light_core::programming::ColorProgram::Semantic { intent } = midpoint.as_ref() else {
            panic!()
        };
        assert_eq!(intent.uv.amount, 0.5);
        assert_eq!(intent.white_blend, 0.5);
    }

    #[test]
    fn canonical_colors_interpolate_during_cue_transitions() {
        let from = AttributeValue::ColorXyz(light_core::Xyz {
            x: 0.2,
            y: 0.4,
            z: 0.6,
        });
        let to = AttributeValue::ColorXyz(light_core::Xyz {
            x: 0.6,
            y: 0.2,
            z: 1.0,
        });
        let Some(AttributeValue::ColorXyz(midpoint)) = interpolate(Some(&from), Some(&to), 0.5)
        else {
            panic!("color transition midpoint is missing")
        };
        assert!((midpoint.x - 0.4).abs() < 0.000_001);
        assert!((midpoint.y - 0.3).abs() < 0.000_001);
        assert!((midpoint.z - 0.8).abs() < 0.000_001);
        assert_eq!(interpolate(Some(&from), Some(&to), 1.0), Some(to));
    }

    #[test]
    fn registry_value_type_is_the_cue_transition_boundary() {
        for attribute in [
            "focus",
            "media.mask.opacity",
            "media.playback.blur",
            "color",
        ] {
            assert!(
                !attribute_uses_snap_transition(&AttributeKey(attribute.into())),
                "{attribute} must interpolate"
            );
        }
        for attribute in ["media.play_mode", "color.wheel.1", "control"] {
            assert!(
                attribute_uses_snap_transition(&AttributeKey(attribute.into())),
                "{attribute} must snap"
            );
        }
    }
}
