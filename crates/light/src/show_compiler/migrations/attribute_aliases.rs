use crate::ActionError;
use light_show::PortableShowCandidateObject;
use serde_json::{Map, Value};
use std::collections::HashMap;

const CANONICAL_SHUTTER: &str = "shutter";

pub(super) fn migrate(
    object: PortableShowCandidateObject<'_>,
    body: &mut Value,
) -> Result<(), ActionError> {
    match object.key().kind() {
        "attribute_configuration" => migrate_attribute_configuration(object, body)?,
        "cue_list" => migrate_cue_list(object, body)?,
        "dynamic" => migrate_dynamic_definition(object, body, "/")?,
        "group" => migrate_attribute_map(object, body, "/programming")?,
        "preset" => migrate_preset(object, body)?,
        _ => {}
    }
    migrate_embedded_dynamic_definitions(object, body, "/")
}

fn migrate_attribute_configuration(
    object: PortableShowCandidateObject<'_>,
    body: &mut Value,
) -> Result<(), ActionError> {
    let stored = body.clone();
    let mut configuration =
        serde_json::from_value::<light_core::AttributeConfiguration>(body.clone())
            .map_err(|error| invalid_value(object, "/", &error.to_string()))?;
    let before = serde_json::to_value(&configuration)
        .map_err(|error| invalid_value(object, "/", &error.to_string()))?;
    configuration = configuration
        .migrate_canonical_attributes()
        .map_err(|error| invalid_value(object, "/", &error.to_string()))?;
    let after = serde_json::to_value(configuration)
        .map_err(|error| invalid_value(object, "/", &error.to_string()))?;
    crate::lossless_json::apply_delta(body, &before, &after);
    for (field, identity) in [("placements", "attribute"), ("activation_groups", "id")] {
        body[field] = reconcile_configuration_array(&stored, &before, &after, field, identity);
    }
    Ok(())
}

fn reconcile_configuration_array(
    stored: &Value,
    before: &Value,
    after: &Value,
    field: &str,
    identity: &str,
) -> Value {
    let stored_items = stored[field].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let before_items = before[field].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let after_items = after[field].as_array().map(Vec::as_slice).unwrap_or(&[]);
    Value::Array(
        after_items
            .iter()
            .map(|after_item| {
                let after_id = after_item.get(identity).and_then(Value::as_str);
                let exact_before = after_id.and_then(|after_id| {
                    before_items
                        .iter()
                        .find(|item| item.get(identity).and_then(Value::as_str) == Some(after_id))
                });
                let migrated_before = if field == "placements" && exact_before.is_none() {
                    after_id.and_then(|after_id| {
                        before_items.iter().find(|item| {
                            item.get(identity)
                                .and_then(Value::as_str)
                                .and_then(migration)
                                .is_some_and(|(target, _)| target == after_id)
                        })
                    })
                } else {
                    None
                };
                let before_item = exact_before.or(migrated_before);
                let stored_item = before_item
                    .and_then(|before_item| before_item.get(identity).and_then(Value::as_str))
                    .and_then(|before_id| {
                        stored_items.iter().find(|item| {
                            item.get(identity).and_then(Value::as_str) == Some(before_id)
                        })
                    });
                let mut merged = stored_item.cloned().unwrap_or_else(|| after_item.clone());
                if let Some(before_item) = before_item {
                    crate::lossless_json::apply_delta(&mut merged, before_item, after_item);
                }
                merged
            })
            .collect(),
    )
}

fn migrate_preset(
    object: PortableShowCandidateObject<'_>,
    body: &mut Value,
) -> Result<(), ActionError> {
    for field in ["values", "group_values"] {
        let Some(targets) = body.get_mut(field).and_then(Value::as_object_mut) else {
            continue;
        };
        for (target, values) in targets {
            migrate_attribute_map_value(object, values, &format!("/{field}/{target}"))?;
        }
    }
    Ok(())
}

fn migrate_cue_list(
    object: PortableShowCandidateObject<'_>,
    body: &mut Value,
) -> Result<(), ActionError> {
    let Some(cues) = body.get_mut("cues").and_then(Value::as_array_mut) else {
        return Ok(());
    };
    for (cue_index, cue) in cues.iter_mut().enumerate() {
        for (field, address_field) in [
            ("changes", "fixture_id"),
            ("group_changes", "group_id"),
            ("dynamic_changes", "fixture_id"),
        ] {
            let path = format!("/cues/{cue_index}/{field}");
            if let Some(changes) = cue.get_mut(field).and_then(Value::as_array_mut) {
                for (index, change) in changes.iter_mut().enumerate() {
                    migrate_cue_change_value(object, change, field, &format!("{path}/{index}"))?;
                }
                migrate_attributed_records(object, changes, address_field, &path)?;
            }
        }
    }
    Ok(())
}

fn migrate_attributed_records(
    object: PortableShowCandidateObject<'_>,
    records: &mut [Value],
    address_field: &str,
    path: &str,
) -> Result<(), ActionError> {
    let mut addresses = HashMap::new();
    for (index, record) in records.iter().enumerate() {
        let Some(record) = record.as_object() else {
            continue;
        };
        let Some(address) = record.get(address_field).and_then(Value::as_str) else {
            continue;
        };
        let Some(attribute) = record.get("attribute").and_then(Value::as_str) else {
            continue;
        };
        let canonical = migration(attribute).map_or(attribute, |migration| migration.0);
        let address = address.to_owned();
        let key = (address.clone(), canonical.to_owned());
        if let Some(previous) = addresses.insert(key, attribute.to_owned())
            && previous != attribute
        {
            return Err(conflict(
                object,
                &format!("{path}/{index}"),
                &address,
                canonical,
            ));
        }
    }
    for record in records {
        migrate_attribute_field(object, record, "attribute", path)?;
    }
    Ok(())
}

fn migrate_cue_change_value(
    object: PortableShowCandidateObject<'_>,
    change: &mut Value,
    field: &str,
    path: &str,
) -> Result<(), ActionError> {
    let Some(attribute) = change.get("attribute").and_then(Value::as_str) else {
        return Ok(());
    };
    let Some((_, transform)) = migration(attribute) else {
        return Ok(());
    };
    let Some(value) = change.get_mut("value") else {
        return Ok(());
    };
    if field == "dynamic_changes" {
        migrate_dynamic_semantic_value(object, value, transform, path)
    } else if value.is_null() {
        Ok(())
    } else {
        migrate_attribute_value(object, value, transform, &format!("{path}/value"))
    }
}

fn migrate_dynamic_semantic_value(
    object: PortableShowCandidateObject<'_>,
    value: &mut Value,
    transform: light_core::CanonicalAttributeTransform,
    path: &str,
) -> Result<(), ActionError> {
    match value.get("type").and_then(Value::as_str) {
        Some("static") => {
            let stored = value
                .get_mut("value")
                .ok_or_else(|| invalid_value(object, path, "static Dynamic value is missing"))?;
            migrate_attribute_value(object, stored, transform, &format!("{path}/value/value"))
        }
        Some("fix_at")
            if transform == light_core::CanonicalAttributeTransform::InvertNormalized =>
        {
            let stored = value.get("value").and_then(Value::as_f64).ok_or_else(|| {
                invalid_value(object, path, "Dynamic Fix At value must be a number")
            })?;
            value["value"] = transformed_number(object, path, stored, transform)?;
            Ok(())
        }
        _ => Ok(()),
    }
}

fn migrate_dynamic_definition(
    object: PortableShowCandidateObject<'_>,
    definition: &mut Value,
    path: &str,
) -> Result<(), ActionError> {
    let Ok(mut model) =
        serde_json::from_value::<light_dynamics::DynamicDefinition>(definition.clone())
    else {
        migrate_strobe_dynamic_definition(object, definition, path)?;
        return Ok(());
    };
    if light_dynamics::validate_definition(&model).is_err() {
        migrate_strobe_dynamic_definition(object, definition, path)?;
        return Ok(());
    }
    let before = serde_json::to_value(&model)
        .map_err(|error| invalid_value(object, path, &error.to_string()))?;
    light_dynamics::migrate_canonical_attributes(&mut model)
        .map_err(|error| invalid_value(object, path, &error))?;
    let after = serde_json::to_value(model)
        .map_err(|error| invalid_value(object, path, &error.to_string()))?;
    crate::lossless_json::apply_delta(definition, &before, &after);
    Ok(())
}

fn migrate_strobe_dynamic_definition(
    object: PortableShowCandidateObject<'_>,
    definition: &mut Value,
    path: &str,
) -> Result<(), ActionError> {
    let Some(lanes) = definition.get_mut("lanes").and_then(Value::as_array_mut) else {
        return Ok(());
    };
    let mut has_legacy = false;
    let mut has_canonical = false;
    for (lane_index, lane) in lanes.iter().enumerate() {
        let Some(attribute) = lane.get("attribute").and_then(Value::as_str) else {
            continue;
        };
        has_legacy |= is_legacy_strobe(attribute);
        has_canonical |= attribute == CANONICAL_SHUTTER;
        if has_legacy && has_canonical {
            return Err(conflict(
                object,
                &format!("{path}lanes/{lane_index}"),
                "Dynamic lane",
                CANONICAL_SHUTTER,
            ));
        }
    }
    for lane in lanes {
        migrate_identity_attribute_field(lane, "attribute");
        migrate_scalar_source_attributes(lane);
    }
    Ok(())
}

fn migrate_scalar_source_attributes(value: &mut Value) {
    match value {
        Value::Array(values) => {
            for value in values {
                migrate_scalar_source_attributes(value);
            }
        }
        Value::Object(fields) => {
            if fields.get("type").and_then(Value::as_str) == Some("preset") {
                migrate_identity_attribute_field_in_map(fields, "attribute");
            }
            for value in fields.values_mut() {
                migrate_scalar_source_attributes(value);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn migrate_embedded_dynamic_definitions(
    object: PortableShowCandidateObject<'_>,
    value: &mut Value,
    path: &str,
) -> Result<(), ActionError> {
    if looks_like_dynamic_definition(value) {
        migrate_dynamic_definition(object, value, path)?;
    }
    match value {
        Value::Array(values) => {
            for (index, value) in values.iter_mut().enumerate() {
                migrate_embedded_dynamic_definitions(object, value, &format!("{path}{index}/"))?;
            }
        }
        Value::Object(fields) => {
            for (field, value) in fields {
                migrate_embedded_dynamic_definitions(object, value, &format!("{path}{field}/"))?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}

fn looks_like_dynamic_definition(value: &Value) -> bool {
    let Some(body) = value.as_object() else {
        return false;
    };
    ["target_binding", "lanes", "phase", "speed"]
        .iter()
        .all(|field| body.contains_key(*field))
}

fn migrate_attribute_map(
    object: PortableShowCandidateObject<'_>,
    body: &mut Value,
    path: &str,
) -> Result<(), ActionError> {
    let Some(values) = body.pointer_mut(path) else {
        return Ok(());
    };
    migrate_attribute_map_value(object, values, path)
}

fn migrate_attribute_map_value(
    object: PortableShowCandidateObject<'_>,
    values: &mut Value,
    path: &str,
) -> Result<(), ActionError> {
    let Some(values) = values.as_object_mut() else {
        return Ok(());
    };
    let migrations = values
        .keys()
        .filter_map(|source| {
            migration(source).map(|(target, transform)| (source.clone(), target, transform))
        })
        .collect::<Vec<_>>();
    let mut migrated_targets = HashMap::new();
    for (source, target, _) in &migrations {
        if source != target && values.contains_key(*target) {
            return Err(conflict(object, path, "stored values", target));
        }
        if let Some(previous) = migrated_targets.insert(*target, source.as_str())
            && previous != source
        {
            return Err(conflict(object, path, "stored values", target));
        }
    }
    for (source, target, transform) in migrations {
        let mut value = values
            .remove(&source)
            .expect("collected migration source remains present");
        migrate_attribute_value(object, &mut value, transform, &format!("{path}/{source}"))?;
        values.insert(target.into(), value);
    }
    Ok(())
}

fn migrate_attribute_field(
    object: PortableShowCandidateObject<'_>,
    value: &mut Value,
    field: &str,
    path: &str,
) -> Result<(), ActionError> {
    if let Some(body) = value.as_object_mut() {
        migrate_attribute_field_in_map(object, body, field, path)?;
    }
    Ok(())
}

fn migrate_attribute_field_in_map(
    _object: PortableShowCandidateObject<'_>,
    body: &mut Map<String, Value>,
    field: &str,
    _path: &str,
) -> Result<(), ActionError> {
    if let Some(attribute) = body.get(field).and_then(Value::as_str)
        && let Some((canonical, _)) = migration(attribute)
    {
        body.insert(field.into(), Value::String(canonical.into()));
    }
    Ok(())
}

fn migrate_identity_attribute_field(value: &mut Value, field: &str) {
    if let Some(body) = value.as_object_mut() {
        migrate_identity_attribute_field_in_map(body, field);
    }
}

fn migrate_identity_attribute_field_in_map(body: &mut Map<String, Value>, field: &str) {
    if let Some(attribute) = body.get(field).and_then(Value::as_str)
        && let Some((canonical, light_core::CanonicalAttributeTransform::Identity)) =
            migration(attribute)
    {
        body.insert(field.into(), Value::String(canonical.into()));
    }
}

fn migrate_attribute_value(
    object: PortableShowCandidateObject<'_>,
    value: &mut Value,
    transform: light_core::CanonicalAttributeTransform,
    path: &str,
) -> Result<(), ActionError> {
    if transform == light_core::CanonicalAttributeTransform::Identity {
        return Ok(());
    }
    let kind = value.get("kind").and_then(Value::as_str).map(str::to_owned);
    let stored = value
        .get_mut("value")
        .ok_or_else(|| invalid_value(object, path, "attribute value payload is missing"))?;
    match kind.as_deref() {
        Some("normalized") => {
            let number = stored.as_f64().ok_or_else(|| {
                invalid_value(object, path, "normalized attribute value must be a number")
            })?;
            *stored = transformed_number(object, path, number, transform)?;
        }
        Some("spread") => {
            let points = stored.as_array_mut().ok_or_else(|| {
                invalid_value(object, path, "spread attribute value must be an array")
            })?;
            for point in points {
                let number = point
                    .as_f64()
                    .ok_or_else(|| invalid_value(object, path, "spread points must be numbers"))?;
                *point = transformed_number(object, path, number, transform)?;
            }
        }
        _ => {
            return Err(invalid_value(
                object,
                path,
                "inverse canonical migration requires a normalized or spread value",
            ));
        }
    }
    Ok(())
}

fn migration(attribute: &str) -> Option<(&'static str, light_core::CanonicalAttributeTransform)> {
    light_core::canonical_attribute_migration_id(attribute)
}

fn transformed_number(
    object: PortableShowCandidateObject<'_>,
    path: &str,
    value: f64,
    transform: light_core::CanonicalAttributeTransform,
) -> Result<Value, ActionError> {
    serde_json::to_value(light_core::transform_canonical_normalized(
        value as f32,
        transform,
    ))
    .map_err(|error| invalid_value(object, path, &error.to_string()))
}

fn is_legacy_strobe(attribute: &str) -> bool {
    matches!(
        light_core::canonical_attribute_migration_id(attribute),
        Some((
            CANONICAL_SHUTTER,
            light_core::CanonicalAttributeTransform::Identity
        ))
    )
}

fn invalid_value(
    object: PortableShowCandidateObject<'_>,
    path: &str,
    message: &str,
) -> ActionError {
    super::invalid_object(
        object,
        format!("attribute migration failed at {path}: {message}"),
    )
}

fn conflict(
    object: PortableShowCandidateObject<'_>,
    path: &str,
    address: &str,
    attribute: &str,
) -> ActionError {
    super::invalid_object(
        object,
        format!(
            "attribute migration conflict at {path}: {address} stores both legacy and canonical {attribute} values"
        ),
    )
}
