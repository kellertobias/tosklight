use super::{
    ActiveShowObjectChange, ActiveShowObjectKind, ActiveShowObjectMutation,
    ActiveShowObjectMutationKind, MutateActiveShowObjectsCommand,
};
use crate::{ActionError, ActionErrorKind, prepare_show_candidate};
use light_core::Revision;
use light_programmer::{GroupDefinition, Preset};
use light_show::{PortableShowDocument, PortableShowObject, PortableShowTransaction};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;

pub(super) struct PreparedObjectMutation {
    pub(super) transaction: PortableShowTransaction,
    pub(super) snapshot: light_engine::EngineSnapshot,
    pub(super) changes: Vec<ActiveShowObjectChange>,
}

pub(super) fn prepare_object_mutation(
    document: &PortableShowDocument,
    command: &MutateActiveShowObjectsCommand,
) -> Result<PreparedObjectMutation, ActionError> {
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
    Ok(PreparedObjectMutation {
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
        return Err(invalid("at least one Group or Preset mutation is required"));
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
        ActiveShowObjectKind::Group => {
            let mut group =
                serde_json::from_value::<GroupDefinition>(request.clone()).map_err(invalid)?;
            group.id.clone_from(&mutation.object_id);
            merge_typed_fields(existing, request, &group)
        }
        ActiveShowObjectKind::Preset => {
            let mut preset = serde_json::from_value::<Preset>(request.clone()).map_err(invalid)?;
            preset
                .reconcile_address(&mutation.object_id)
                .map_err(invalid)?;
            merge_typed_fields(existing, request, &preset)
        }
    }
}

fn merge_typed_fields<T: Serialize>(
    existing: Option<&Value>,
    request: &Value,
    typed: &T,
) -> Result<Value, ActionError> {
    let request = request
        .as_object()
        .ok_or_else(|| invalid("show object body must be an object"))?;
    let mut merged = existing
        .map(|body| {
            body.as_object()
                .cloned()
                .ok_or_else(|| invalid("stored show object body must be an object"))
        })
        .transpose()?
        .unwrap_or_default();
    let canonical = serde_json::to_value(typed).map_err(invalid)?;
    let canonical = canonical
        .as_object()
        .ok_or_else(|| invalid("serialized show object body must be an object"))?;
    for (key, value) in request {
        if !canonical.contains_key(key) {
            merged.insert(key.clone(), value.clone());
        }
    }
    for (key, value) in canonical {
        merged.insert(key.clone(), value.clone());
    }
    Ok(Value::Object(merged))
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
