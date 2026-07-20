use light_application as application;
use light_wire::v2::group_recording as wire;

pub(super) const fn operation(
    value: wire::GroupRecordOperation,
) -> application::ProgrammingGroupRecordOperation {
    match value {
        wire::GroupRecordOperation::Overwrite => {
            application::ProgrammingGroupRecordOperation::Overwrite
        }
        wire::GroupRecordOperation::Merge => application::ProgrammingGroupRecordOperation::Merge,
        wire::GroupRecordOperation::Subtract => {
            application::ProgrammingGroupRecordOperation::Subtract
        }
        wire::GroupRecordOperation::Delete => application::ProgrammingGroupRecordOperation::Delete,
    }
}

pub(super) fn outcome(
    result: application::ProgrammingGroupRecordResult,
) -> Result<wire::GroupRecordOutcome, application::ActionError> {
    let application::ProgrammingGroupRecordResult {
        context,
        request_id,
        replayed,
        outcome,
        ..
    } = result;
    Ok(match outcome {
        application::ProgrammingGroupRecordOutcome::Changed {
            projection,
            show_revision,
            event_sequence,
        } => wire::GroupRecordOutcome::Changed {
            request_id,
            correlation_id: context.correlation_id,
            replayed,
            show_revision: show_revision.value(),
            group: recorded_projection(&projection)?,
            event_sequence,
        },
        application::ProgrammingGroupRecordOutcome::NoChange {
            projection,
            show_revision,
        } => wire::GroupRecordOutcome::NoChange {
            request_id,
            correlation_id: context.correlation_id,
            replayed,
            show_revision: show_revision.value(),
            group: recorded_stored_projection(&projection)?,
        },
    })
}

fn recorded_stored_projection(
    projection: &application::ProgrammingGroupProjection,
) -> Result<wire::RecordedStoredGroupProjection, application::ActionError> {
    match (projection.deleted, projection.raw_body.as_ref()) {
        (false, Some(body)) => Ok(wire::RecordedStoredGroupProjection::Stored {
            id: projection.object_id.clone(),
            revision: projection.object_revision,
            body: body.as_ref().clone(),
        }),
        _ => Err(inconsistent_projection()),
    }
}

fn recorded_projection(
    projection: &application::ProgrammingGroupProjection,
) -> Result<wire::RecordedGroupProjection, application::ActionError> {
    match (projection.deleted, projection.raw_body.as_ref()) {
        (true, None) => Ok(wire::RecordedGroupProjection::Deleted {
            id: projection.object_id.clone(),
            revision: projection.object_revision,
        }),
        (false, Some(body)) => Ok(wire::RecordedGroupProjection::Stored {
            id: projection.object_id.clone(),
            revision: projection.object_revision,
            body: body.as_ref().clone(),
        }),
        _ => Err(inconsistent_projection()),
    }
}

fn inconsistent_projection() -> application::ActionError {
    application::ActionError::new(
        application::ActionErrorKind::Internal,
        "Group recording returned an inconsistent authoritative projection",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_exposes_only_supported_record_operations() {
        assert_eq!(
            operation(wire::GroupRecordOperation::Overwrite),
            application::ProgrammingGroupRecordOperation::Overwrite
        );
        assert_eq!(
            operation(wire::GroupRecordOperation::Merge),
            application::ProgrammingGroupRecordOperation::Merge
        );
        assert_eq!(
            operation(wire::GroupRecordOperation::Subtract),
            application::ProgrammingGroupRecordOperation::Subtract
        );
        assert_eq!(
            operation(wire::GroupRecordOperation::Delete),
            application::ProgrammingGroupRecordOperation::Delete
        );
    }
}
