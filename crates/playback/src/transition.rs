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
        (Some(from), _) => Some(from.clone()),
        (None, Some(to)) if progress >= 1.0 => Some(to.clone()),
        _ => None,
    }
}
