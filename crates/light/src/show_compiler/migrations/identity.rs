//! Stable instance identities for operator-numbered objects (TL-858).
//!
//! Groups and Presets are keyed by the number the operator sees, so two offline copies of one
//! show can both create "Group 7" for different things. A merge tells those apart by an instance
//! UUID stored in the body (`uid` for Groups, `instance_id` for Presets) that never changes while
//! the object exists, however its body is rewritten.
//!
//! Writers that rebuild a body from a typed value may drop the field, so every staged write gets
//! it back from the stored object before anything else migrates. A new object gets a fresh
//! random identity, and a copy that duplicates another object's identity in the same show (for
//! example a Partial Load duplicate) is given its own.

use super::{ObjectUpdate, candidate};
use crate::ActionError;
use light_show::{
    PortableShowCandidateObject, PortableShowDocument, PortableShowObjectKey,
    PortableShowTransaction,
};
use serde_json::Value;
use std::collections::BTreeSet;
use uuid::Uuid;

/// Object kinds with an instance identity, and the body field that holds it.
pub(crate) const INSTANCE_IDENTITY_FIELDS: &[(&str, &str)] =
    &[("group", "uid"), ("preset", "instance_id")];

/// The instance identity field of `kind`, when it has one.
pub(crate) fn instance_identity_field(kind: &str) -> Option<&'static str> {
    INSTANCE_IDENTITY_FIELDS
        .iter()
        .find_map(|(candidate, field)| (*candidate == kind).then_some(*field))
}

/// The instance identity stored in a body, when it is a valid UUID.
pub(crate) fn instance_identity(kind: &str, body: &Value) -> Option<Uuid> {
    let field = instance_identity_field(kind)?;
    body.get(field)
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
}

/// Assigns or restores instance identities. With `scope`, only those objects are considered;
/// otherwise every identity-bearing object in the candidate is.
pub(super) fn collect(
    document: &PortableShowDocument,
    transaction: &PortableShowTransaction,
    scope: Option<&BTreeSet<PortableShowObjectKey>>,
) -> Result<Vec<ObjectUpdate>, ActionError> {
    let candidate = candidate(document, transaction)?;
    let mut updates = Vec::new();
    for (kind, field) in INSTANCE_IDENTITY_FIELDS {
        let objects = candidate.objects_of_kind(kind).collect::<Vec<_>>();
        // Identities held by objects outside the scope are fixed; the scope adapts around them.
        let mut taken = objects
            .iter()
            .filter(|object| scope.is_some_and(|keys| !keys.contains(object.key())))
            .filter_map(|object| instance_identity(kind, object.body()))
            .collect::<BTreeSet<_>>();
        for object in objects
            .iter()
            .filter(|object| scope.is_none_or(|keys| keys.contains(object.key())))
        {
            let stored = document
                .object(kind, object.key().id())
                .and_then(|stored| instance_identity(kind, stored.body()));
            let identity = [instance_identity(kind, object.body()), stored]
                .into_iter()
                .flatten()
                .find(|identity| !taken.contains(identity))
                .unwrap_or_else(Uuid::new_v4);
            taken.insert(identity);
            if instance_identity(kind, object.body()) != Some(identity) {
                updates.push(with_identity(*object, field, identity)?);
            }
        }
    }
    Ok(updates)
}

fn with_identity(
    object: PortableShowCandidateObject<'_>,
    field: &str,
    identity: Uuid,
) -> Result<ObjectUpdate, ActionError> {
    let mut body = object.body().clone();
    body.as_object_mut()
        .ok_or_else(|| super::invalid_object(object, "body must be a JSON object"))?
        .insert(field.into(), Value::String(identity.to_string()));
    Ok(ObjectUpdate::from_object(object, body))
}
