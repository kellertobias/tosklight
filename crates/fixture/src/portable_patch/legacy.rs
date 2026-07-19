use super::{PortablePatchError, RETAINED_LEGACY_DEFINITION_FIELDS};
use crate::PatchedFixture;
use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Serialize)]
pub(super) struct RetainedUnknownField {
    json_pointer: String,
    value: Value,
}

pub(super) fn retained_definition_fields(
    body: &Value,
    fixture: &PatchedFixture,
) -> Result<Vec<RetainedUnknownField>, PortablePatchError> {
    let raw = body.get("definition").ok_or_else(|| {
        PortablePatchError::InvalidRecord("legacy record has no definition".into())
    })?;
    let known = serde_json::to_value(&fixture.definition)?;
    let mut retained = Vec::new();
    collect_unknown_fields(raw, &known, "", true, &mut retained);
    Ok(retained)
}

pub(super) fn write_retained_extensions(
    body: &mut Map<String, Value>,
    retained: Vec<RetainedUnknownField>,
) -> Result<(), PortablePatchError> {
    if !retained.is_empty() {
        body.insert(
            RETAINED_LEGACY_DEFINITION_FIELDS.into(),
            serde_json::to_value(retained)?,
        );
    }
    Ok(())
}

fn collect_unknown_fields(
    raw: &Value,
    known: &Value,
    pointer: &str,
    definition_root: bool,
    retained: &mut Vec<RetainedUnknownField>,
) {
    match (raw, known) {
        (Value::Object(raw), Value::Object(known)) => {
            collect_unknown_object(raw, known, pointer, definition_root, retained);
        }
        (Value::Array(raw), Value::Array(known)) => {
            collect_unknown_array(raw, known, pointer, retained);
        }
        _ => {}
    }
}

fn collect_unknown_object(
    raw: &Map<String, Value>,
    known: &Map<String, Value>,
    pointer: &str,
    definition_root: bool,
    retained: &mut Vec<RetainedUnknownField>,
) {
    for (key, value) in raw {
        if definition_root && key == "profile_snapshot" {
            continue;
        }
        collect_unknown_value(known, pointer, key, value, retained);
    }
}

fn collect_unknown_value(
    known: &Map<String, Value>,
    pointer: &str,
    key: &str,
    value: &Value,
    retained: &mut Vec<RetainedUnknownField>,
) {
    let child = format!("{pointer}/{}", escape_pointer(key));
    match known.get(key) {
        Some(known) => collect_unknown_fields(value, known, &child, false, retained),
        None => retained.push(RetainedUnknownField {
            json_pointer: child,
            value: value.clone(),
        }),
    }
}

fn collect_unknown_array(
    raw: &[Value],
    known: &[Value],
    pointer: &str,
    retained: &mut Vec<RetainedUnknownField>,
) {
    for (index, value) in raw.iter().enumerate() {
        let child = format!("{pointer}/{index}");
        match known.get(index) {
            Some(known) => collect_unknown_fields(value, known, &child, false, retained),
            None => retained.push(RetainedUnknownField {
                json_pointer: child,
                value: value.clone(),
            }),
        }
    }
}

fn escape_pointer(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}
