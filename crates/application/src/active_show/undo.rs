use super::{ActiveShowObjectChange, UndoActiveShowObjectCommand, objects::PreparedObjectChanges};
use crate::{
    ActionError, ActionErrorKind, show_compiler::prepare_show_candidate_preserving_object,
};
use light_show::{PortableShowDocument, PortableShowObjectUndo};

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
    let current = document
        .object(command.kind.as_str(), &command.object_id)
        .ok_or_else(|| {
            not_found(format!(
                "{} {} does not exist",
                command.kind.as_str(),
                command.object_id
            ))
        })?;
    if current.revision() != command.expected_object_revision {
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
