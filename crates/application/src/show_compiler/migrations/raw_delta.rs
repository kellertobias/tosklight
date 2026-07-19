use serde_json::{Map, Value};
use std::collections::HashMap;

/// Applies only fields changed by a typed migration. Raw fields unknown to this build survive.
pub(super) fn merge(stored: &mut Value, before: &Value, after: &Value) {
    if before == after {
        return;
    }
    match (stored, before, after) {
        (Value::Object(stored), Value::Object(before), Value::Object(after)) => {
            merge_object(stored, before, after);
        }
        (Value::Array(stored), Value::Array(before), Value::Array(after)) => {
            merge_array(stored, before, after);
        }
        (stored, _, after) => *stored = after.clone(),
    }
}

fn merge_object(
    stored: &mut Map<String, Value>,
    before: &Map<String, Value>,
    after: &Map<String, Value>,
) {
    for (key, before_value) in before {
        let Some(after_value) = after.get(key) else {
            stored.remove(key);
            continue;
        };
        if before_value == after_value {
            continue;
        }
        if let Some(current) = stored.get_mut(key) {
            merge(current, before_value, after_value);
        } else {
            stored.insert(key.clone(), after_value.clone());
        }
    }
    for (key, after_value) in after {
        if !before.contains_key(key) {
            stored.insert(key.clone(), after_value.clone());
        }
    }
}

fn merge_array(stored: &mut Vec<Value>, before: &[Value], after: &[Value]) {
    let Some(indexes) = keyed_indexes(before).filter(|_| stored.len() == before.len()) else {
        *stored = after.to_vec();
        return;
    };
    *stored = after
        .iter()
        .map(|after_item| merge_array_item(stored, before, &indexes, after_item))
        .collect();
}

fn merge_array_item(
    stored: &[Value],
    before: &[Value],
    indexes: &HashMap<String, usize>,
    after: &Value,
) -> Value {
    let Some(index) = array_item_key(after).and_then(|key| indexes.get(&key).copied()) else {
        return after.clone();
    };
    let mut merged = stored[index].clone();
    merge(&mut merged, &before[index], after);
    merged
}

fn keyed_indexes(values: &[Value]) -> Option<HashMap<String, usize>> {
    let mut indexes = HashMap::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        let key = array_item_key(value)?;
        if indexes.insert(key, index).is_some() {
            return None;
        }
    }
    Some(indexes)
}

fn array_item_key(value: &Value) -> Option<String> {
    let object = value.as_object()?;
    ["id", "fixture_id", "channel_id", "split", "head_index"]
        .into_iter()
        .find_map(|field| scalar_key(field, object.get(field)?))
}

fn scalar_key(field: &str, value: &Value) -> Option<String> {
    if !matches!(value, Value::String(_) | Value::Number(_) | Value::Bool(_)) {
        return None;
    }
    Some(format!("{field}:{}", serde_json::to_string(value).ok()?))
}

#[cfg(test)]
mod tests {
    use super::merge;
    use serde_json::json;

    #[test]
    fn typed_delta_retains_unknown_fields_on_changed_keyed_items() {
        let before = json!({"items":[{"id":"a","known":1}]});
        let after = json!({"items":[{"id":"a","known":2}]});
        let mut stored = json!({"items":[{"id":"a","known":1,"future":true}]});

        merge(&mut stored, &before, &after);

        assert_eq!(
            stored,
            json!({"items":[{"id":"a","known":2,"future":true}]})
        );
    }
}
