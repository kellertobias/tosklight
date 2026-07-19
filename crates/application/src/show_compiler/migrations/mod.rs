mod objects;
mod patch;
mod patch_heads;
mod records;

use super::invalid_candidate;
use crate::ActionError;
use light_show::{
    PortableShowCandidate, PortableShowCandidateObject, PortableShowDocument,
    PortableShowObjectKey, PortableShowTransaction,
};
use serde_json::Value;

/// Stages compatibility migrations into an existing candidate transaction without touching its
/// backing store. A failed migration leaves the supplied transaction exactly as it was.
pub(crate) fn stage_candidate_migrations(
    document: &PortableShowDocument,
    transaction: &mut PortableShowTransaction,
) -> Result<(), ActionError> {
    stage_candidate_migrations_preserving(document, transaction, None)
}

pub(crate) fn stage_candidate_migrations_preserving_object(
    document: &PortableShowDocument,
    transaction: &mut PortableShowTransaction,
    kind: &str,
    object_id: &str,
) -> Result<(), ActionError> {
    let preserved = PortableShowObjectKey::new(kind, object_id);
    stage_candidate_migrations_preserving(document, transaction, Some(&preserved))
}

fn stage_candidate_migrations_preserving(
    document: &PortableShowDocument,
    transaction: &mut PortableShowTransaction,
    preserved: Option<&PortableShowObjectKey>,
) -> Result<(), ActionError> {
    let mut staged = transaction.clone();
    stage_object_migrations(document, &mut staged, preserved)?;
    patch::stage_inline_migrations(document, &mut staged, preserved)?;
    patch::stage_lean_migrations(document, &mut staged, preserved)?;
    candidate(document, &staged)?;
    *transaction = staged;
    Ok(())
}

fn stage_object_migrations(
    document: &PortableShowDocument,
    transaction: &mut PortableShowTransaction,
    preserved: Option<&PortableShowObjectKey>,
) -> Result<(), ActionError> {
    let updates = {
        let candidate = candidate(document, transaction)?;
        objects::collect(candidate)?
    };
    stage_updates(transaction, updates, preserved);
    Ok(())
}

pub(super) fn stage_updates(
    transaction: &mut PortableShowTransaction,
    updates: Vec<ObjectUpdate>,
    preserved: Option<&PortableShowObjectKey>,
) {
    for update in updates {
        if !preserved.is_some_and(|key| update.targets(key)) {
            transaction.put(update.kind, update.id, update.body);
        }
    }
}

pub(super) fn candidate<'a>(
    document: &'a PortableShowDocument,
    transaction: &'a PortableShowTransaction,
) -> Result<PortableShowCandidate<'a>, ActionError> {
    document
        .candidate(transaction)
        .map_err(|error| invalid_candidate(format!("invalid portable show candidate: {error}")))
}

pub(super) fn invalid_object(
    object: PortableShowCandidateObject<'_>,
    error: impl std::fmt::Display,
) -> ActionError {
    invalid_candidate(format!(
        "invalid {} {}: {error}",
        object.key().kind(),
        object.key().id()
    ))
}

pub(super) struct ObjectUpdate {
    kind: String,
    id: String,
    body: Value,
}

impl ObjectUpdate {
    pub(super) fn from_object(object: PortableShowCandidateObject<'_>, body: Value) -> Self {
        Self {
            kind: object.key().kind().to_owned(),
            id: object.key().id().to_owned(),
            body,
        }
    }

    fn targets(&self, key: &PortableShowObjectKey) -> bool {
        self.kind == key.kind() && self.id == key.id()
    }
}
