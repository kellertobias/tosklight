use crate::*;

pub(crate) fn interpolate(
    from: Option<&AttributeValue>,
    to: Option<&AttributeValue>,
    progress: f32,
) -> Option<AttributeValue> {
    if progress >= 1.0 {
        return to.cloned();
    }
    match (from, to) {
        (Some(AttributeValue::Normalized(from)), Some(AttributeValue::Normalized(to))) => {
            Some(AttributeValue::Normalized(from + (to - from) * progress))
        }
        (None, Some(AttributeValue::Normalized(to))) => {
            Some(AttributeValue::Normalized(to * progress))
        }
        (Some(AttributeValue::Normalized(from)), None) => {
            Some(AttributeValue::Normalized(from * (1.0 - progress)))
        }
        (Some(AttributeValue::ColorXyz(from)), Some(AttributeValue::ColorXyz(to))) => {
            Some(AttributeValue::ColorXyz(light_core::Xyz {
                x: from.x + (to.x - from.x) * progress,
                y: from.y + (to.y - from.y) * progress,
                z: from.z + (to.z - from.z) * progress,
            }))
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
}
