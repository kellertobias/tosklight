use light_application as application;
use light_wire::v2::{command_line, cue_transfer as wire};

const JAVASCRIPT_MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

pub(super) fn application_command(
    show_id: light_core::ShowId,
    request: wire::CueTransferRequest,
) -> Result<(String, application::ProgrammingCueTransferRequest), String> {
    if request.choice_id.is_nil() {
        return Err("choice_id must not be nil".into());
    }
    if request.expected_command_line_revision > JAVASCRIPT_MAX_SAFE_INTEGER {
        return Err(
            "expected_command_line_revision must not exceed the JavaScript maximum safe integer"
                .into(),
        );
    }
    Ok((
        request.request_id,
        application::ProgrammingCueTransferRequest {
            show_id,
            choice_id: request.choice_id,
            mode: application_mode(request.mode),
            expected_command_line_revision: request.expected_command_line_revision,
        },
    ))
}

pub(super) fn outcome(
    result: application::ProgrammingCueTransferResult,
) -> wire::CueTransferOutcome {
    let outcome = result.outcome;
    wire::CueTransferOutcome::Changed {
        request_id: result.request_id,
        correlation_id: result.correlation_id,
        replayed: result.replayed,
        show_id: outcome.show_id.0,
        choice_id: result.choice_id,
        summary: summary(outcome.summary),
        show_revision: outcome.show_revision.value(),
        projections: outcome.projections.iter().map(object_projection).collect(),
        show_event_sequence: outcome.show_event_sequence,
        command_line: super::wire::command_line_from_state(outcome.command_line),
        interaction_event_sequence: outcome.interaction_event_sequence,
        persistence_warning: outcome.persistence_warning,
    }
}

const fn application_mode(value: wire::CueTransferMode) -> application::ProgrammingCueTransferMode {
    match value {
        wire::CueTransferMode::Plain => application::ProgrammingCueTransferMode::Plain,
        wire::CueTransferMode::Status => application::ProgrammingCueTransferMode::Status,
    }
}

const fn wire_mode(value: application::ProgrammingCueTransferMode) -> wire::CueTransferMode {
    match value {
        application::ProgrammingCueTransferMode::Plain => wire::CueTransferMode::Plain,
        application::ProgrammingCueTransferMode::Status => wire::CueTransferMode::Status,
    }
}

const fn operation(value: application::CueTransferOperation) -> command_line::CueTransferOperation {
    match value {
        application::CueTransferOperation::Copy => command_line::CueTransferOperation::Copy,
        application::CueTransferOperation::Move => command_line::CueTransferOperation::Move,
    }
}

fn summary(value: application::ProgrammingCueTransferSummary) -> wire::CueTransferSummary {
    wire::CueTransferSummary {
        operation: operation(value.operation),
        mode: wire_mode(value.mode),
        source_cue_id: value.source_cue_id,
        source_cue_number: value.source_cue_number.value(),
        destination_cue_id: value.destination_cue_id,
        destination_cue_number: value.destination_cue_number.value(),
    }
}

fn object_projection(
    value: &application::ProgrammingCueTransferObjectProjection,
) -> wire::CueTransferObjectProjection {
    wire::CueTransferObjectProjection {
        cue_list_id: value.cue_list_id.0,
        object_id: value.object_id.clone(),
        object_revision: value.object_revision,
        body: std::sync::Arc::clone(&value.raw_body),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_mapping_rejects_nil_choice_authority() {
        let request = wire::CueTransferRequest {
            request_id: "transfer-1".into(),
            choice_id: uuid::Uuid::nil(),
            mode: wire::CueTransferMode::Plain,
            expected_command_line_revision: 7,
        };
        assert!(application_command(light_core::ShowId::new(), request).is_err());
    }

    #[test]
    fn request_mapping_rejects_unsafe_javascript_revision() {
        let request = wire::CueTransferRequest {
            request_id: "transfer-1".into(),
            choice_id: uuid::Uuid::new_v4(),
            mode: wire::CueTransferMode::Plain,
            expected_command_line_revision: JAVASCRIPT_MAX_SAFE_INTEGER + 1,
        };
        assert!(application_command(light_core::ShowId::new(), request).is_err());
    }
}
