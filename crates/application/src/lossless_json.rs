//! Lossless updates for typed values stored as extensible JSON.
//!
//! Serde intentionally ignores fields unknown to the current build. These helpers apply the
//! changes made to a typed value to its raw JSON representation, so those extension fields are
//! not discarded when an older desk reads and writes a newer show.

use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::HashMap;

/// Applies a typed before/after delta to raw stored JSON and returns the lossless result.
///
/// Fields and keyed-array item contents unknown to `T` survive. Removing a known object or map
/// entry from `after` still removes the complete raw entry, including its unknown descendants.
pub fn merge_typed<T: Serialize>(
    stored: &Value,
    before: &T,
    after: &T,
) -> serde_json::Result<Value> {
    let before = serde_json::to_value(before)?;
    let after = serde_json::to_value(after)?;
    let mut merged = stored.clone();
    apply_delta(&mut merged, &before, &after);
    write_canonical_fields(&mut merged, &after);
    Ok(merged)
}

/// Applies a normalized typed update while merging extensions supplied by its raw request.
///
/// Known fields follow `normalized_typed`, including semantic merges and deletions. Unknown stored
/// fields survive where their typed parent survives. Fields absent from `request_typed` are
/// treated as request extensions and are added or replace an extension at the same location.
pub fn merge_typed_request<T: Serialize>(
    stored: Option<&Value>,
    stored_typed: Option<&T>,
    request: &Value,
    request_typed: &T,
    normalized_typed: &T,
) -> serde_json::Result<Value> {
    let request_canonical = serde_json::to_value(request_typed)?;
    let normalized_canonical = serde_json::to_value(normalized_typed)?;
    let mut merged = match (stored, stored_typed) {
        (Some(stored), Some(stored_typed)) => {
            let stored_canonical = serde_json::to_value(stored_typed)?;
            let mut merged = stored.clone();
            apply_delta(&mut merged, &stored_canonical, &normalized_canonical);
            merged
        }
        _ => normalized_canonical.clone(),
    };
    overlay_extensions(&mut merged, request, &request_canonical);
    write_canonical_fields(&mut merged, &normalized_canonical);
    Ok(merged)
}

/// Applies only fields changed by a typed migration. Raw fields unknown to this build survive.
pub(crate) fn apply_delta(stored: &mut Value, before: &Value, after: &Value) {
    if before == after {
        return;
    }
    match (stored, before, after) {
        (Value::Object(stored), Value::Object(before), Value::Object(after)) => {
            apply_object_delta(stored, before, after);
        }
        (Value::Array(stored), Value::Array(before), Value::Array(after)) => {
            apply_array_delta(stored, before, after);
        }
        (stored, _, after) => *stored = after.clone(),
    }
}

fn apply_object_delta(
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
            apply_delta(current, before_value, after_value);
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

fn apply_array_delta(stored: &mut Vec<Value>, before: &[Value], after: &[Value]) {
    let (Some(stored_indexes), Some(before_indexes)) =
        (keyed_indexes(stored), keyed_indexes(before))
    else {
        *stored = after.to_vec();
        return;
    };
    *stored = after
        .iter()
        .map(|after_item| {
            merge_array_item(stored, before, &stored_indexes, &before_indexes, after_item)
        })
        .collect();
}

fn merge_array_item(
    stored: &[Value],
    before: &[Value],
    stored_indexes: &HashMap<String, usize>,
    before_indexes: &HashMap<String, usize>,
    after: &Value,
) -> Value {
    let Some(key) = item_key(after) else {
        return after.clone();
    };
    let (Some(stored_index), Some(before_index)) =
        (stored_indexes.get(&key), before_indexes.get(&key))
    else {
        return after.clone();
    };
    let mut merged = stored[*stored_index].clone();
    apply_delta(&mut merged, &before[*before_index], after);
    merged
}

fn overlay_extensions(target: &mut Value, supplied: &Value, canonical: &Value) {
    match (target, supplied, canonical) {
        (Value::Object(target), Value::Object(supplied), Value::Object(canonical)) => {
            for (key, supplied_value) in supplied {
                let Some(canonical_value) = canonical.get(key) else {
                    target.insert(key.clone(), supplied_value.clone());
                    continue;
                };
                if let Some(target_value) = target.get_mut(key) {
                    overlay_extensions(target_value, supplied_value, canonical_value);
                }
            }
        }
        (Value::Array(target), Value::Array(supplied), Value::Array(canonical)) => {
            overlay_array_extensions(target, supplied, canonical);
        }
        _ => {}
    }
}

fn overlay_array_extensions(target: &mut [Value], supplied: &[Value], canonical: &[Value]) {
    if let (Some(target_indexes), Some(canonical_indexes)) =
        (keyed_indexes(target), keyed_indexes(canonical))
    {
        for supplied_item in supplied {
            let Some(key) = item_key(supplied_item) else {
                continue;
            };
            let (Some(target_index), Some(canonical_index)) =
                (target_indexes.get(&key), canonical_indexes.get(&key))
            else {
                continue;
            };
            overlay_extensions(
                &mut target[*target_index],
                supplied_item,
                &canonical[*canonical_index],
            );
        }
    } else if target.len() == supplied.len() && supplied.len() == canonical.len() {
        for ((target, supplied), canonical) in target.iter_mut().zip(supplied).zip(canonical) {
            overlay_extensions(target, supplied, canonical);
        }
    }
}

/// Writes every field understood by the current typed representation without removing extension
/// fields. Besides normalizing aliases, this retains the previous top-level behavior of
/// materializing typed defaults that were absent from legacy JSON.
fn write_canonical_fields(target: &mut Value, canonical: &Value) {
    match (target, canonical) {
        (Value::Object(target), Value::Object(canonical)) => {
            for (key, canonical_value) in canonical {
                match target.get_mut(key) {
                    Some(target_value) => write_canonical_fields(target_value, canonical_value),
                    None => {
                        target.insert(key.clone(), canonical_value.clone());
                    }
                }
            }
        }
        (Value::Array(target), Value::Array(canonical)) => {
            write_canonical_array(target, canonical);
        }
        (target, canonical) => target.clone_from(canonical),
    }
}

fn write_canonical_array(target: &mut Vec<Value>, canonical: &[Value]) {
    if let (Some(target_indexes), Some(_)) = (keyed_indexes(target), keyed_indexes(canonical)) {
        let mut normalized = Vec::with_capacity(canonical.len());
        for canonical_item in canonical {
            let key = item_key(canonical_item).expect("keyed index guarantees item keys");
            let mut item = target_indexes
                .get(&key)
                .map_or_else(|| canonical_item.clone(), |index| target[*index].clone());
            write_canonical_fields(&mut item, canonical_item);
            normalized.push(item);
        }
        *target = normalized;
    } else if target.len() == canonical.len() {
        for (target, canonical) in target.iter_mut().zip(canonical) {
            write_canonical_fields(target, canonical);
        }
    } else {
        *target = canonical.to_vec();
    }
}

fn keyed_indexes(values: &[Value]) -> Option<HashMap<String, usize>> {
    let mut indexes = HashMap::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        let key = item_key(value)?;
        if indexes.insert(key, index).is_some() {
            return None;
        }
    }
    Some(indexes)
}

fn item_key(value: &Value) -> Option<String> {
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
    use super::{apply_delta, merge_typed, merge_typed_request};
    use serde_json::json;

    #[test]
    fn typed_delta_retains_unknown_fields_on_changed_keyed_items() {
        let before = json!({"items":[{"id":"a","known":1}]});
        let after = json!({"items":[{"id":"a","known":2}]});
        let mut stored = json!({"items":[{"id":"a","known":1,"future":true}]});

        apply_delta(&mut stored, &before, &after);

        assert_eq!(
            stored,
            json!({"items":[{"id":"a","known":2,"future":true}]})
        );
    }

    #[test]
    fn keyed_items_are_matched_by_identity_instead_of_array_position() {
        let before = json!({"items":[{"id":"a","known":1},{"id":"b","known":2}]});
        let after = json!({"items":[{"id":"a","known":3},{"id":"b","known":4}]});
        let mut stored = json!({"items":[
            {"id":"b","known":2,"future":"b"},
            {"id":"a","known":1,"future":"a"}
        ]});

        apply_delta(&mut stored, &before, &after);

        assert_eq!(
            stored,
            json!({"items":[
                {"id":"a","known":3,"future":"a"},
                {"id":"b","known":4,"future":"b"}
            ]})
        );
    }

    #[test]
    fn request_extensions_merge_inside_keyed_items() {
        let stored_typed = json!({"items":[{"id":"a","known":1}]});
        let stored = json!({"items":[{"id":"a","known":1,"server_future":true}]});
        let request_typed = json!({"items":[{"id":"a","known":2}]});
        let request = json!({"items":[{
            "id":"a",
            "known":2,
            "client_future":{"accepted":true}
        }]});

        let merged = merge_typed_request(
            Some(&stored),
            Some(&stored_typed),
            &request,
            &request_typed,
            &request_typed,
        )
        .unwrap();

        assert_eq!(
            merged,
            json!({"items":[{
                "id":"a",
                "known":2,
                "server_future":true,
                "client_future":{"accepted":true}
            }]})
        );
    }

    #[test]
    fn public_typed_merge_removes_known_map_entries_without_losing_siblings() {
        let before = json!({"values":{"remove":1,"keep":2}});
        let after = json!({"values":{"keep":3}});
        let stored = json!({"values":{"remove":{"future":true},"keep":2},"future":true});

        let merged = merge_typed(&stored, &before, &after).unwrap();

        assert_eq!(merged, json!({"values":{"keep":3},"future":true}));
    }

    #[test]
    fn public_typed_merge_materializes_missing_defaults_and_normalizes_known_values() {
        let before = json!({"known":"legacy-alias","default":[]});
        let after = json!({"known":"canonical","default":[]});
        let stored = json!({"known":"legacy-alias","future":true});

        let merged = merge_typed(&stored, &before, &after).unwrap();

        assert_eq!(
            merged,
            json!({"known":"canonical","default":[],"future":true})
        );
    }
}
