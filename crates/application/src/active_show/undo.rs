use super::{
    ActiveShowObjectChange, UndoActiveShowObjectCommand, UndoActiveShowRecordingCommand,
    UndoActiveShowRecordingOperation, objects::PreparedObjectChanges,
};
use crate::{
    ActionError, ActionErrorKind,
    show_compiler::{
        prepare_show_candidate_exact_transaction, prepare_show_candidate_preserving_object,
    },
};
use light_show::{PortableShowDocument, PortableShowObjectUndo};
use std::collections::HashSet;

pub(super) fn validate_object_undo(
    document: &PortableShowDocument,
    command: &UndoActiveShowObjectCommand,
) -> Result<(), ActionError> {
    if document.id() != command.show_id {
        return Err(not_found("requested show is not active"));
    }
    if command.object_id.is_empty() {
        return Err(invalid("show object id cannot be empty"));
    }
    if let Some(current) = document.object(command.kind.as_str(), &command.object_id)
        && current.revision() != command.expected_object_revision
    {
        return Err(ActionError::new(
            ActionErrorKind::Conflict,
            format!(
                "stale {} {} revision",
                command.kind.as_str(),
                command.object_id
            ),
        )
        .at_revision(current.revision()));
    }
    Ok(())
}

pub(super) fn prepare_object_undo(
    document: &PortableShowDocument,
    command: &UndoActiveShowObjectCommand,
    undo: PortableShowObjectUndo,
) -> Result<PreparedObjectChanges, ActionError> {
    validate_prepared_undo(command, &undo)?;
    let body = undo.body().clone();
    let mut transaction = document.transaction();
    transaction.undo_object(undo);

    // Keep the target's exact historical JSON and compare-and-pop condition while allowing every
    // unrelated pending migration to join the same candidate transaction.
    let prepared = prepare_show_candidate_preserving_object(
        document,
        transaction,
        command.kind.as_str(),
        &command.object_id,
    )?;
    let (transaction, snapshot) = prepared.into_parts();
    Ok(PreparedObjectChanges {
        transaction,
        snapshot,
        changes: vec![ActiveShowObjectChange {
            kind: command.kind,
            object_id: command.object_id.clone(),
            object_revision: next_revision(command.expected_object_revision)?,
            body: Some(body),
            deleted: false,
        }],
    })
}

pub(super) fn prepare_recording_undo(
    document: &PortableShowDocument,
    command: &UndoActiveShowRecordingCommand,
    undoes: Vec<PortableShowObjectUndo>,
) -> Result<PreparedObjectChanges, ActionError> {
    if document.id() != command.show_id {
        return Err(not_found("requested show is not active"));
    }
    if command.objects.is_empty() {
        return Err(invalid("recording undo requires at least one object"));
    }
    let mut targets = HashSet::with_capacity(command.objects.len());
    let mut undoes = undoes.into_iter();
    let mut transaction = document.transaction();
    let mut changes = Vec::with_capacity(command.objects.len());
    for object in &command.objects {
        if object.object_id.is_empty() || !targets.insert((object.kind, object.object_id.as_str()))
        {
            return Err(invalid(
                "recording undo contains an invalid or duplicate object",
            ));
        }
        let current = document.object(object.kind.as_str(), &object.object_id);
        if current.map_or(0, |item| item.revision()) != object.expected_object_revision {
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                format!(
                    "stale {} {} revision",
                    object.kind.as_str(),
                    object.object_id
                ),
            )
            .at_revision(current.map_or(0, |item| item.revision())));
        }
        let body = match object.operation {
            UndoActiveShowRecordingOperation::RestorePrevious => {
                let undo = undoes
                    .next()
                    .ok_or_else(|| invalid("recording undo body is missing"))?;
                if undo.key().kind() != object.kind.as_str()
                    || undo.key().id() != object.object_id
                    || undo.expected_object_revision() != object.expected_object_revision
                {
                    return Err(ActionError::new(
                        ActionErrorKind::Internal,
                        "prepared recording undo does not match its object",
                    ));
                }
                let body = undo.body().clone();
                transaction.undo_object(undo);
                Some(body)
            }
            UndoActiveShowRecordingOperation::DeleteCreated => {
                transaction.delete(object.kind.as_str(), object.object_id.clone());
                None
            }
        };
        changes.push(ActiveShowObjectChange {
            kind: object.kind,
            object_id: object.object_id.clone(),
            object_revision: next_revision(object.expected_object_revision)?,
            body,
            deleted: matches!(
                object.operation,
                UndoActiveShowRecordingOperation::DeleteCreated
            ),
        });
    }
    if undoes.next().is_some() {
        return Err(invalid("recording undo contains an unexpected body"));
    }
    let prepared = prepare_show_candidate_exact_transaction(document, transaction)?;
    let (transaction, snapshot) = prepared.into_parts();
    Ok(PreparedObjectChanges {
        transaction,
        snapshot,
        changes,
    })
}

fn validate_prepared_undo(
    command: &UndoActiveShowObjectCommand,
    undo: &PortableShowObjectUndo,
) -> Result<(), ActionError> {
    let key = undo.key();
    if key.kind() != command.kind.as_str()
        || key.id() != command.object_id
        || undo.expected_object_revision() != command.expected_object_revision
    {
        return Err(ActionError::new(
            ActionErrorKind::Internal,
            "prepared object undo does not match its command",
        ));
    }
    Ok(())
}

fn next_revision(current: light_core::Revision) -> Result<light_core::Revision, ActionError> {
    current.checked_add(1).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Invalid,
            "show object revision cannot be incremented",
        )
        .at_revision(current)
    })
}

fn invalid(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}

fn not_found(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::NotFound, message)
}
