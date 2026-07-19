use super::locations::{formatted_id, scalar_id};
use crate::selective_import::{ImportObjectDescriptor, ImportProfileKey, ImportReferenceLocation};
use light_show::PortableShowObjectKey;
use serde_json::{Number, Value};
use std::collections::BTreeMap;

pub(crate) type IdentityMap = BTreeMap<(PortableShowObjectKey, String), String>;
pub(crate) type ProfileMap = BTreeMap<ImportProfileKey, ImportProfileKey>;

pub(crate) fn rewrite_body(
    body: &Value,
    owner: &PortableShowObjectKey,
    descriptor: &ImportObjectDescriptor,
    identities: &IdentityMap,
    profiles: &ProfileMap,
) -> Result<Value, String> {
    let mut rewritten = body.clone();
    for identity in &descriptor.identities {
        let Some(location) = &identity.location else {
            continue;
        };
        let destination = identities
            .get(&(owner.clone(), identity.slot.clone()))
            .ok_or_else(|| format!("no destination identity for slot {}", identity.slot))?;
        if destination != &identity.value {
            rewrite_location(&mut rewritten, location, &identity.value, destination)?;
        }
    }
    for reference in &descriptor.references {
        let destination = identities
            .get(&(reference.target.clone(), reference.target_slot.clone()))
            .ok_or_else(|| {
                format!(
                    "no destination identity for {}/{} slot {}",
                    reference.target.kind(),
                    reference.target.id(),
                    reference.target_slot
                )
            })?;
        if destination != &reference.source_identity {
            rewrite_location(
                &mut rewritten,
                &reference.location,
                &reference.source_identity,
                destination,
            )?;
        }
    }
    for reference in &descriptor.profile_references {
        let Some(destination) = profiles.get(&reference.key) else {
            continue;
        };
        if destination.profile_id != reference.key.profile_id {
            for location in &reference.id_locations {
                rewrite_location(
                    &mut rewritten,
                    location,
                    &reference.key.profile_id.0.to_string(),
                    &destination.profile_id.0.to_string(),
                )?;
            }
        }
    }
    Ok(rewritten)
}

fn rewrite_location(
    body: &mut Value,
    location: &ImportReferenceLocation,
    expected: &str,
    replacement: &str,
) -> Result<(), String> {
    match location {
        ImportReferenceLocation::Value { pointer, format } => {
            let current = body
                .pointer_mut(pointer)
                .ok_or_else(|| format!("reference location {pointer} no longer exists"))?;
            let expected = formatted_id(expected, *format)?;
            let replacement = formatted_id(replacement, *format)?;
            if scalar_id(current).as_deref() != Some(expected.as_str()) {
                return Err(format!(
                    "reference location {pointer} changed before rewrite"
                ));
            }
            *current = scalar_replacement(current, &replacement)?;
            Ok(())
        }
        ImportReferenceLocation::ObjectKey {
            object_pointer,
            key,
        } => {
            if key != expected {
                return Err(format!(
                    "object-key descriptor {key} does not match {expected}"
                ));
            }
            let object = body
                .pointer_mut(object_pointer)
                .and_then(Value::as_object_mut)
                .ok_or_else(|| format!("reference map {object_pointer} no longer exists"))?;
            if replacement != key && object.contains_key(replacement) {
                return Err(format!(
                    "reference map {object_pointer} already contains {replacement}"
                ));
            }
            let value = object.remove(key).ok_or_else(|| {
                format!("reference map {object_pointer} no longer contains {key}")
            })?;
            object.insert(replacement.to_owned(), value);
            Ok(())
        }
    }
}

fn scalar_replacement(current: &Value, replacement: &str) -> Result<Value, String> {
    match current {
        Value::String(_) => Ok(Value::String(replacement.to_owned())),
        Value::Number(number) if number.is_u64() => replacement
            .parse::<u64>()
            .map(Number::from)
            .map(Value::Number)
            .map_err(|_| format!("identity {replacement} is not an unsigned integer")),
        Value::Number(number) if number.is_i64() => replacement
            .parse::<i64>()
            .map(Number::from)
            .map(Value::Number)
            .map_err(|_| format!("identity {replacement} is not an integer")),
        _ => Err("only string and integer references can be rewritten".into()),
    }
}
