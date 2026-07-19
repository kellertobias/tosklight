use super::{
    ActiveShowObjectChange, ActiveShowObjectKind, ActiveShowObjectMutation,
    ActiveShowObjectMutationKind, MutateActiveShowObjectsCommand,
};
use crate::{ActionError, ActionErrorKind, lossless_json, prepare_show_candidate};
use light_core::Revision;
use light_playback::CueList;
use light_programmer::{GroupDefinition, Preset};
use light_show::{PortableShowDocument, PortableShowObject, PortableShowTransaction};
use serde_json::Value;
use std::collections::HashSet;

pub(super) struct PreparedObjectChanges {
    pub(super) transaction: PortableShowTransaction,
    pub(super) snapshot: light_engine::EngineSnapshot,
    pub(super) changes: Vec<ActiveShowObjectChange>,
}

pub(super) fn prepare_object_mutation(
    document: &PortableShowDocument,
    command: &MutateActiveShowObjectsCommand,
) -> Result<PreparedObjectChanges, ActionError> {
    validate_command(document, command)?;
    let mut transaction = document.transaction();
    let mut changes = Vec::with_capacity(command.mutations.len());
    for mutation in &command.mutations {
        let existing = document.object(mutation.kind.as_str(), &mutation.object_id);
        validate_object_revision(existing, mutation)?;
        changes.push(apply_mutation(&mut transaction, existing, mutation)?);
    }
    let prepared = prepare_show_candidate(document, transaction)?;
    let (transaction, snapshot) = prepared.into_parts();
    Ok(PreparedObjectChanges {
        transaction,
        snapshot,
        changes,
    })
}

fn validate_command(
    document: &PortableShowDocument,
    command: &MutateActiveShowObjectsCommand,
) -> Result<(), ActionError> {
    if document.id() != command.show_id {
        return Err(not_found("requested show is not active"));
    }
    if command.mutations.is_empty() {
        return Err(invalid("at least one show-object mutation is required"));
    }
    let mut targets = HashSet::with_capacity(command.mutations.len());
    for mutation in &command.mutations {
        if mutation.object_id.is_empty() {
            return Err(invalid("show object id cannot be empty"));
        }
        if !targets.insert((mutation.kind, mutation.object_id.as_str())) {
            return Err(invalid(format!(
                "duplicate {} {} mutation",
                mutation.kind.as_str(),
                mutation.object_id
            )));
        }
    }
    Ok(())
}

fn validate_object_revision(
    existing: Option<&PortableShowObject>,
    mutation: &ActiveShowObjectMutation,
) -> Result<(), ActionError> {
    let current = existing.map_or(0, PortableShowObject::revision);
    if matches!(mutation.mutation, ActiveShowObjectMutationKind::Delete) && existing.is_none() {
        return Err(not_found(format!(
            "{} {} does not exist",
            mutation.kind.as_str(),
            mutation.object_id
        )));
    }
    if current == mutation.expected_object_revision {
        Ok(())
    } else {
        Err(ActionError::new(
            ActionErrorKind::Conflict,
            format!(
                "stale {} {} revision",
                mutation.kind.as_str(),
                mutation.object_id
            ),
        )
        .at_revision(current))
    }
}

fn apply_mutation(
    transaction: &mut PortableShowTransaction,
    existing: Option<&PortableShowObject>,
    mutation: &ActiveShowObjectMutation,
) -> Result<ActiveShowObjectChange, ActionError> {
    let revision = next_revision(mutation.expected_object_revision)?;
    match &mutation.mutation {
        ActiveShowObjectMutationKind::Put { body } => {
            let body = normalize_body(existing.map(PortableShowObject::body), mutation, body)?;
            transaction.put(
                mutation.kind.as_str(),
                mutation.object_id.clone(),
                body.clone(),
            );
            Ok(ActiveShowObjectChange {
                kind: mutation.kind,
                object_id: mutation.object_id.clone(),
                object_revision: revision,
                body: Some(body),
                deleted: false,
            })
        }
        ActiveShowObjectMutationKind::Delete => {
            transaction.delete(mutation.kind.as_str(), mutation.object_id.clone());
            Ok(ActiveShowObjectChange {
                kind: mutation.kind,
                object_id: mutation.object_id.clone(),
                object_revision: revision,
                body: None,
                deleted: true,
            })
        }
    }
}

fn normalize_body(
    existing: Option<&Value>,
    mutation: &ActiveShowObjectMutation,
    request: &Value,
) -> Result<Value, ActionError> {
    match mutation.kind {
        ActiveShowObjectKind::CueList => normalize_cue_list(existing, mutation, request),
        ActiveShowObjectKind::Group => normalize_group(existing, mutation, request),
        ActiveShowObjectKind::Preset => normalize_preset(existing, mutation, request),
    }
}

fn normalize_cue_list(
    existing: Option<&Value>,
    mutation: &ActiveShowObjectMutation,
    request: &Value,
) -> Result<Value, ActionError> {
    let requested = serde_json::from_value::<CueList>(request.clone()).map_err(invalid)?;
    let stored = existing
        .map(|body| serde_json::from_value::<CueList>(body.clone()).map_err(invalid))
        .transpose()?;
    let mut normalized = requested.clone();
    if let Ok(id) = uuid::Uuid::parse_str(&mutation.object_id) {
        normalized.id = light_core::CueListId(id);
    }
    normalized.validate().map_err(invalid)?;
    lossless_json::merge_typed_request(existing, stored.as_ref(), request, &requested, &normalized)
        .map_err(invalid)
}

fn normalize_group(
    existing: Option<&Value>,
    mutation: &ActiveShowObjectMutation,
    request: &Value,
) -> Result<Value, ActionError> {
    let requested = serde_json::from_value::<GroupDefinition>(request.clone()).map_err(invalid)?;
    let stored = existing
        .map(|body| serde_json::from_value::<GroupDefinition>(body.clone()).map_err(invalid))
        .transpose()?;
    let mut normalized = requested.clone();
    normalized.id.clone_from(&mutation.object_id);
    lossless_json::merge_typed_request(existing, stored.as_ref(), request, &requested, &normalized)
        .map_err(invalid)
}

fn normalize_preset(
    existing: Option<&Value>,
    mutation: &ActiveShowObjectMutation,
    request: &Value,
) -> Result<Value, ActionError> {
    let requested = serde_json::from_value::<Preset>(request.clone()).map_err(invalid)?;
    let stored = existing
        .map(|body| serde_json::from_value::<Preset>(body.clone()).map_err(invalid))
        .transpose()?;
    let mut normalized = requested.clone();
    normalized
        .reconcile_address(&mutation.object_id)
        .map_err(invalid)?;
    lossless_json::merge_typed_request(existing, stored.as_ref(), request, &requested, &normalized)
        .map_err(invalid)
}

fn next_revision(current: Revision) -> Result<Revision, ActionError> {
    current.checked_add(1).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Invalid,
            "show object revision cannot be incremented",
        )
        .at_revision(current)
    })
}

fn invalid(error: impl std::fmt::Display) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, error.to_string())
}

fn not_found(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::NotFound, message)
}

#[cfg(test)]
mod tests;
