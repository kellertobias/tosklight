use light_core::{AttributeKey, AttributeValue};
use std::collections::{BTreeMap, HashMap};

pub(crate) fn apply_safe_values(
    values: &mut HashMap<AttributeKey, AttributeValue>,
    safe: &BTreeMap<AttributeKey, AttributeValue>,
    progress: f32,
) {
    apply_safe_values_with_snap(values, safe, progress, |_| false);
}

pub(crate) fn apply_safe_values_with_snap(
    values: &mut HashMap<AttributeKey, AttributeValue>,
    safe: &BTreeMap<AttributeKey, AttributeValue>,
    progress: f32,
    is_snap: impl Fn(&AttributeKey) -> bool,
) {
    for (attribute, target) in safe {
        let progress = if is_snap(attribute) { 1.0 } else { progress };
        let value = match (values.get(attribute), target) {
            (Some(AttributeValue::Normalized(current)), AttributeValue::Normalized(target)) => {
                AttributeValue::Normalized(current + (target - current) * progress)
            }
            _ if progress >= 1.0 => target.clone(),
            (Some(current), _) => current.clone(),
            (None, AttributeValue::Normalized(target)) => {
                AttributeValue::Normalized(target * progress)
            }
            _ => continue,
        };
        values.insert(attribute.clone(), value);
    }
}
