use light_application as application;
use light_core::Revision;
use light_wire::v2::group_management as wire;

pub(super) fn operation(
    value: wire::GroupManagementOperation,
) -> application::GroupManagementOperation {
    match value {
        wire::GroupManagementOperation::UpdateProperties { properties } => {
            application::GroupManagementOperation::UpdateProperties(
                application::GroupPropertiesUpdate {
                    name: properties.name,
                    color: properties.color,
                    icon: properties.icon,
                },
            )
        }
        wire::GroupManagementOperation::Undo {} => application::GroupManagementOperation::Undo,
        wire::GroupManagementOperation::RefreshFrozen { expected_source } => {
            application::GroupManagementOperation::RefreshFrozen {
                expected_source: expected_source.map(source_expectation),
            }
        }
        wire::GroupManagementOperation::DetachDerived { expected_source } => {
            application::GroupManagementOperation::DetachDerived {
                expected_source: expected_source.map(source_expectation),
            }
        }
    }
}

fn source_expectation(value: wire::GroupSourceExpectation) -> application::GroupSourceExpectation {
    application::GroupSourceExpectation {
        source_group_id: value.source_group_id,
        expected_source_revision: value.expected_source_revision.map(Revision::from),
    }
}

pub(super) fn outcome(result: application::GroupManagementResult) -> wire::GroupManagementOutcome {
    let application::GroupManagementResult {
        context,
        request_id,
        replayed,
        outcome,
        persistence_warning,
    } = result;
    match outcome {
        application::GroupManagementOutcome::Changed {
            projection,
            show_revision,
            event_sequence,
        } => wire::GroupManagementOutcome::Changed {
            request_id,
            correlation_id: context.correlation_id,
            replayed,
            show_id: projection.show_id.0,
            show_revision: show_revision.value(),
            group: object_projection(&projection),
            show_event_sequence: event_sequence,
            persistence_warning,
        },
        application::GroupManagementOutcome::NoChange {
            projection,
            show_revision,
        } => wire::GroupManagementOutcome::NoChange {
            request_id,
            correlation_id: context.correlation_id,
            replayed,
            show_id: projection.show_id.0,
            show_revision: show_revision.value(),
            group: object_projection(&projection),
            persistence_warning,
        },
    }
}

fn object_projection(
    projection: &application::GroupManagementProjection,
) -> wire::GroupManagementObjectProjection {
    wire::GroupManagementObjectProjection {
        object_id: projection.object_id.clone(),
        object_revision: projection.object_revision,
        body: projection.raw_body.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_exposes_only_supported_management_operations() {
        assert!(matches!(
            operation(wire::GroupManagementOperation::Undo {}),
            application::GroupManagementOperation::Undo
        ));
        assert!(matches!(
            operation(wire::GroupManagementOperation::RefreshFrozen {
                expected_source: None
            }),
            application::GroupManagementOperation::RefreshFrozen {
                expected_source: None
            }
        ));
        assert!(matches!(
            operation(wire::GroupManagementOperation::DetachDerived {
                expected_source: None
            }),
            application::GroupManagementOperation::DetachDerived {
                expected_source: None
            }
        ));
    }

    #[test]
    fn a_declared_source_expectation_is_carried_exactly() {
        let application::GroupManagementOperation::RefreshFrozen {
            expected_source: Some(expectation),
        } = operation(wire::GroupManagementOperation::RefreshFrozen {
            expected_source: Some(wire::GroupSourceExpectation {
                source_group_id: "source".into(),
                expected_source_revision: Some(3),
            }),
        })
        else {
            panic!("a declared source expectation must survive translation")
        };
        assert_eq!(expectation.source_group_id, "source");
        assert_eq!(expectation.expected_source_revision, Some(3));
    }
}
