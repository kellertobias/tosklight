use super::fixtures::FixtureIdentityCatalog;
use crate::selective_import::{
    ImportIdentityFormat, ImportObjectDescriptor, ImportObjectReference, ImportOwnedIdentity,
    ImportReferenceLocation,
};
use light_show::{PortableShowObject, PortableShowObjectKey};
use serde_json::Value;

pub(super) fn key_only_descriptor(object: &PortableShowObject) -> ImportObjectDescriptor {
    ImportObjectDescriptor {
        identities: vec![ImportOwnedIdentity {
            slot: "object".into(),
            value: object.key().id().into(),
            location: None,
        }],
        ..ImportObjectDescriptor::default()
    }
}

pub(super) fn id_descriptor(object: &PortableShowObject) -> Result<ImportObjectDescriptor, String> {
    Ok(ImportObjectDescriptor {
        identities: vec![primary_identity(object, "/id", ImportIdentityFormat::Full)?],
        ..ImportObjectDescriptor::default()
    })
}

pub(super) fn add_fixture_array(
    body: &Value,
    pointer: &str,
    source: &FixtureIdentityCatalog,
    target: &FixtureIdentityCatalog,
    descriptor: &mut ImportObjectDescriptor,
) -> Result<(), String> {
    add_fixture_array_at(body, pointer, pointer, source, target, descriptor)
}

pub(super) fn add_fixture_array_at(
    body: &Value,
    local_pointer: &str,
    body_pointer: &str,
    source: &FixtureIdentityCatalog,
    target: &FixtureIdentityCatalog,
    descriptor: &mut ImportObjectDescriptor,
) -> Result<(), String> {
    let Some(values) = array_at(body, local_pointer) else {
        return Ok(());
    };
    for (index, value) in values.iter().enumerate() {
        let id = scalar_id(value)
            .ok_or_else(|| format!("fixture reference {body_pointer}/{index} is invalid"))?;
        descriptor.references.push(fixture_reference(
            id,
            value_location(
                format!("{body_pointer}/{index}"),
                ImportIdentityFormat::Full,
            ),
            source,
            target,
        )?);
    }
    Ok(())
}

pub(super) fn add_direct_array_at(
    body: &Value,
    local_pointer: &str,
    body_pointer: &str,
    kind: &str,
    descriptor: &mut ImportObjectDescriptor,
) -> Result<(), String> {
    let Some(values) = array_at(body, local_pointer) else {
        return Ok(());
    };
    for (index, value) in values.iter().enumerate() {
        let id = scalar_id(value)
            .ok_or_else(|| format!("reference {body_pointer}/{index} is invalid"))?;
        descriptor.references.push(direct_reference(
            kind,
            id,
            value_location(
                format!("{body_pointer}/{index}"),
                ImportIdentityFormat::Full,
            ),
        ));
    }
    Ok(())
}

pub(super) fn add_fixture_map_keys(
    body: &Value,
    pointer: &str,
    source: &FixtureIdentityCatalog,
    target: &FixtureIdentityCatalog,
    descriptor: &mut ImportObjectDescriptor,
) -> Result<(), String> {
    let Some(values) = body.pointer(pointer).and_then(Value::as_object) else {
        return Ok(());
    };
    for id in values.keys() {
        descriptor.references.push(fixture_reference(
            id.clone(),
            ImportReferenceLocation::ObjectKey {
                object_pointer: pointer.into(),
                key: id.clone(),
            },
            source,
            target,
        )?);
    }
    Ok(())
}

pub(super) fn add_direct_map_keys(
    body: &Value,
    pointer: &str,
    kind: &str,
    descriptor: &mut ImportObjectDescriptor,
) -> Result<(), String> {
    let Some(values) = body.pointer(pointer).and_then(Value::as_object) else {
        return Ok(());
    };
    descriptor.references.extend(values.keys().map(|id| {
        direct_reference(
            kind,
            id.clone(),
            ImportReferenceLocation::ObjectKey {
                object_pointer: pointer.into(),
                key: id.clone(),
            },
        )
    }));
    Ok(())
}

pub(super) fn add_fixture_value(
    value: &Value,
    local_pointer: &str,
    body_pointer: String,
    source: &FixtureIdentityCatalog,
    target: &FixtureIdentityCatalog,
    descriptor: &mut ImportObjectDescriptor,
) -> Result<(), String> {
    let id = scalar_at(value, local_pointer)?;
    descriptor.references.push(fixture_reference(
        id,
        value_location(body_pointer, ImportIdentityFormat::Full),
        source,
        target,
    )?);
    Ok(())
}

pub(super) fn add_direct_value(
    value: &Value,
    local_pointer: &str,
    body_pointer: String,
    kind: &str,
    descriptor: &mut ImportObjectDescriptor,
) -> Result<(), String> {
    let id = scalar_at(value, local_pointer)?;
    descriptor.references.push(direct_reference(
        kind,
        id,
        value_location(body_pointer, ImportIdentityFormat::Full),
    ));
    Ok(())
}

pub(super) fn add_optional_direct_reference(
    body: &Value,
    pointer: &str,
    kind: &str,
    descriptor: &mut ImportObjectDescriptor,
) -> Result<(), String> {
    let Some(value) = body.pointer(pointer) else {
        return Ok(());
    };
    if value.is_null() {
        return Ok(());
    }
    let id = scalar_id(value).ok_or_else(|| format!("reference {pointer} is invalid"))?;
    descriptor.references.push(direct_reference(
        kind,
        id,
        value_location(pointer, ImportIdentityFormat::Full),
    ));
    Ok(())
}

fn fixture_reference(
    source_identity: String,
    location: ImportReferenceLocation,
    source: &FixtureIdentityCatalog,
    target: &FixtureIdentityCatalog,
) -> Result<ImportObjectReference, String> {
    let owner = source
        .resolve(&source_identity)?
        .or(target.resolve(&source_identity)?);
    Ok(match owner {
        Some(owner) => ImportObjectReference {
            target: owner.object.clone(),
            target_slot: owner.slot.clone(),
            source_identity,
            location,
        },
        None => ImportObjectReference {
            target: PortableShowObjectKey::new("patched_fixture", &source_identity),
            target_slot: "object".into(),
            source_identity,
            location,
        },
    })
}

pub(super) fn direct_reference(
    kind: impl Into<String>,
    source_identity: String,
    location: ImportReferenceLocation,
) -> ImportObjectReference {
    ImportObjectReference {
        target: PortableShowObjectKey::new(kind, &source_identity),
        target_slot: "object".into(),
        source_identity,
        location,
    }
}

pub(super) fn primary_identity(
    object: &PortableShowObject,
    pointer: &str,
    format: ImportIdentityFormat,
) -> Result<ImportOwnedIdentity, String> {
    let value = scalar_at(object.body(), pointer)?;
    let expected = formatted_id(object.key().id(), format)?;
    if value != expected {
        return Err(format!(
            "{} identity {pointer} is {value}, expected {expected}",
            object.key().kind()
        ));
    }
    Ok(ImportOwnedIdentity {
        slot: "object".into(),
        value: object.key().id().into(),
        location: Some(value_location(pointer, format)),
    })
}

pub(super) fn identity_at(
    value: &Value,
    body_pointer: &str,
    local_pointer: &str,
    slot: String,
) -> Result<ImportOwnedIdentity, String> {
    Ok(ImportOwnedIdentity {
        slot,
        value: scalar_at(value, local_pointer)?,
        location: Some(value_location(body_pointer, ImportIdentityFormat::Full)),
    })
}

pub(super) fn formatted_id(id: &str, format: ImportIdentityFormat) -> Result<String, String> {
    match format {
        ImportIdentityFormat::Full => Ok(id.to_owned()),
        ImportIdentityFormat::NumericSuffix => id
            .rsplit_once('.')
            .map(|(_, suffix)| suffix.to_owned())
            .ok_or_else(|| format!("identity {id} has no numeric suffix")),
    }
}

pub(super) fn scalar_at(value: &Value, pointer: &str) -> Result<String, String> {
    value
        .pointer(pointer)
        .and_then(scalar_id)
        .ok_or_else(|| format!("{pointer} is not a string or integer identity"))
}

pub(super) fn scalar_id(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

pub(super) fn array_at<'a>(value: &'a Value, pointer: &str) -> Option<&'a Vec<Value>> {
    value.pointer(pointer).and_then(Value::as_array)
}

pub(super) fn value_location(
    pointer: impl Into<String>,
    format: ImportIdentityFormat,
) -> ImportReferenceLocation {
    ImportReferenceLocation::Value {
        pointer: pointer.into(),
        format,
    }
}
