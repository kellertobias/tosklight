use serde_json::{Map, Value};
use std::collections::HashMap;

pub(super) fn merge_typed_delta(stored: &mut Value, before: &Value, after: &Value) {
    if before == after {
        return;
    }
    match (stored, before, after) {
        (Value::Object(stored), Value::Object(before), Value::Object(after)) => {
            merge_object_delta(stored, before, after);
        }
        (Value::Array(stored), Value::Array(before), Value::Array(after)) => {
            merge_array_delta(stored, before, after);
        }
        (stored, _, after) => *stored = after.clone(),
    }
}

fn merge_array_delta(stored: &mut Vec<Value>, before: &[Value], after: &[Value]) {
    let Some(merged) = merge_keyed_array(stored, before, after) else {
        *stored = after.to_vec();
        return;
    };
    *stored = merged;
}

fn merge_keyed_array(stored: &[Value], before: &[Value], after: &[Value]) -> Option<Vec<Value>> {
    if stored.len() != before.len() {
        return None;
    }
    let indexes = keyed_indexes(before)?;
    after
        .iter()
        .map(|after| merge_keyed_item(stored, before, &indexes, after))
        .collect()
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

fn merge_keyed_item(
    stored: &[Value],
    before: &[Value],
    indexes: &HashMap<String, usize>,
    after: &Value,
) -> Option<Value> {
    let key = array_item_key(after)?;
    let Some(index) = indexes.get(&key).copied() else {
        return Some(after.clone());
    };
    let mut merged = stored[index].clone();
    merge_typed_delta(&mut merged, &before[index], after);
    Some(merged)
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

fn merge_object_delta(
    stored: &mut Map<String, Value>,
    before: &Map<String, Value>,
    after: &Map<String, Value>,
) {
    for (key, before_value) in before {
        let Some(after_value) = after.get(key) else {
            stored.remove(key);
            continue;
        };
        if before_value != after_value {
            merge_changed_field(stored, key, before_value, after_value);
        }
    }
    for (key, after_value) in after {
        if !before.contains_key(key) {
            stored.insert(key.clone(), after_value.clone());
        }
    }
}

fn merge_changed_field(stored: &mut Map<String, Value>, key: &str, before: &Value, after: &Value) {
    if let Some(current) = stored.get_mut(key) {
        merge_typed_delta(current, before, after);
    } else {
        stored.insert(key.into(), after.clone());
    }
}
