use super::locations::{formatted_id, scalar_id};
use super::{
    dynamic_identity::{reidentify_automatic_angle_partners, reidentify_embedded_references},
    installed_color::rebase_installed_color_calibrations,
    native_color::{PinnedProfileMap, rewrite_native_color_sources},
};
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
    pinned_profiles: &PinnedProfileMap,
) -> Result<Value, String> {
    let mut rewritten = body.clone();
    // These exact pointers may live below fixture/group map keys. Rewrite their values before
    // renaming containing keys, then process ordinary references deepest first.
    rewrite_native_color_sources(
        &mut rewritten,
        &descriptor.native_color_references,
        pinned_profiles,
    )?;
    rebase_installed_color_calibrations(
        &mut rewritten,
        &descriptor.installed_color_references,
        pinned_profiles,
    )?;
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
    let mut references = descriptor.references.iter().collect::<Vec<_>>();
    references.sort_by_key(|reference| std::cmp::Reverse(location_depth(&reference.location)));
    for reference in references {
        let Some(destination) =
            identities.get(&(reference.target.clone(), reference.target_slot.clone()))
        else {
            if reference.allow_missing {
                continue;
            }
            return Err(format!(
                "no destination identity for {}/{} slot {}",
                reference.target.kind(),
                reference.target.id(),
                reference.target_slot
            ));
        };
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
    if owner.kind() == "dynamic"
        && let (Ok(source), Some(destination)) = (
            uuid::Uuid::parse_str(owner.id()),
            rewritten
                .get("id")
                .and_then(Value::as_str)
                .and_then(|id| uuid::Uuid::parse_str(id).ok()),
        )
    {
        reidentify_automatic_angle_partners(&mut rewritten, source, destination);
    }
    reidentify_embedded_references(body, &mut rewritten, owner.kind())?;
    Ok(rewritten)
}

fn location_depth(location: &ImportReferenceLocation) -> usize {
    match location {
        ImportReferenceLocation::Value { pointer, .. } => pointer.matches('/').count(),
        ImportReferenceLocation::ObjectKey { object_pointer, .. } => {
            object_pointer.matches('/').count() + 1
        }
    }
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
