use crate::ActionError;
use light_show::PortableShowCandidateObject;
use serde_json::{Map, Value};
use std::collections::HashSet;

const LEGACY_STROBE: &str = "strobe";
const CANONICAL_SHUTTER: &str = "shutter";

pub(super) fn migrate(
    object: PortableShowCandidateObject<'_>,
    body: &mut Value,
) -> Result<(), ActionError> {
    match object.key().kind() {
        "cue_list" => migrate_cue_list(object, body)?,
        "dynamic" => migrate_dynamic_definition(object, body, "/")?,
        "group" => migrate_attribute_map(object, body, "/programming")?,
        "preset" => migrate_preset(object, body)?,
        _ => {}
    }
    migrate_embedded_dynamic_definitions(object, body, "/")
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
    let mut legacy_addresses = HashSet::new();
    let mut canonical_addresses = HashSet::new();
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
        let address = address.to_owned();
        if attribute == LEGACY_STROBE {
            legacy_addresses.insert(address.clone());
        } else if attribute == CANONICAL_SHUTTER {
            canonical_addresses.insert(address.clone());
        }
        if legacy_addresses.contains(&address) && canonical_addresses.contains(&address) {
            return Err(conflict(
                object,
                &format!("{path}/{index}"),
                &address,
                CANONICAL_SHUTTER,
            ));
        }
    }
    for record in records {
        migrate_attribute_field(record, "attribute");
    }
    Ok(())
}

fn migrate_dynamic_definition(
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
        has_legacy |= attribute == LEGACY_STROBE;
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
        migrate_attribute_field(lane, "attribute");
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
                migrate_attribute_field_in_map(fields, "attribute");
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
    if values.contains_key(LEGACY_STROBE) && values.contains_key(CANONICAL_SHUTTER) {
        return Err(conflict(object, path, "stored values", CANONICAL_SHUTTER));
    }
    if let Some(value) = values.remove(LEGACY_STROBE) {
        values.insert(CANONICAL_SHUTTER.into(), value);
    }
    Ok(())
}

fn migrate_attribute_field(value: &mut Value, field: &str) {
    if let Some(body) = value.as_object_mut() {
        migrate_attribute_field_in_map(body, field);
    }
}

fn migrate_attribute_field_in_map(body: &mut Map<String, Value>, field: &str) {
    if body.get(field).and_then(Value::as_str) == Some(LEGACY_STROBE) {
        body.insert(field.into(), Value::String(CANONICAL_SHUTTER.into()));
    }
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
