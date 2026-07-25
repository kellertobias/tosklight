use light_application as application;
use light_wire::v2::cue_deletion as wire;

const JAVASCRIPT_MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

pub(super) fn application_command(
    show_id: light_core::ShowId,
    request: wire::CueDeletionRequest,
) -> Result<(String, application::ProgrammingCueDeletionRequest), String> {
    validate_request(&request)?;
    Ok((
        request.request_id,
        application::ProgrammingCueDeletionRequest {
            show_id,
            address: address(request.address),
            cue_number: application::CueNumber::new(request.cue_number),
            expectation: application::ProgrammingCueDeletionExpectation::Exact(
                application::ProgrammingCueDeletionAuthority {
                    playback_number: request.authority.playback_number,
                    cue_list_id: light_core::CueListId(request.authority.cue_list_id),
                    object_id: request.authority.object_id,
                    object_revision: request.authority.object_revision,
                    cue_id: request.authority.cue_id,
                },
            ),
        },
    ))
}

pub(super) fn outcome(
    result: application::ProgrammingCueDeletionResult,
) -> wire::CueDeletionOutcome {
    let outcome = result.outcome;
    let cue_list = projection(outcome.cue_list);
    let deleted_cue = wire::DeletedCueProjection {
        id: outcome.deleted_cue.id,
        number: outcome.deleted_cue.number.value(),
    };
    match outcome.state {
        application::ProgrammingCueDeletionState::Changed {
            show_event_sequence,
        } => wire::CueDeletionOutcome::Changed {
            request_id: result.request_id,
            correlation_id: result.correlation_id,
            replayed: result.replayed,
            show_id: outcome.show_id.0,
            show_revision: outcome.show_revision.value(),
            cue_list,
            deleted_cue,
            show_event_sequence,
            persistence_warning: outcome.persistence_warning,
        },
        application::ProgrammingCueDeletionState::NoChange => wire::CueDeletionOutcome::NoChange {
            request_id: result.request_id,
            correlation_id: result.correlation_id,
            replayed: result.replayed,
            show_id: outcome.show_id.0,
            show_revision: outcome.show_revision.value(),
            cue_list,
            deleted_cue,
            persistence_warning: outcome.persistence_warning,
        },
    }
}

fn validate_request(request: &wire::CueDeletionRequest) -> Result<(), String> {
    if !request.cue_number.is_finite() || request.cue_number <= 0.0 {
        return Err("cue_number must be finite and greater than zero".into());
    }
    if request.authority.cue_list_id.is_nil() || request.authority.cue_id.is_nil() {
        return Err("Cue deletion authority IDs must not be nil".into());
    }
    if request.authority.object_revision > JAVASCRIPT_MAX_SAFE_INTEGER {
        return Err("object_revision exceeds the JavaScript maximum safe integer".into());
    }
    Ok(())
}

const fn address(value: wire::CueDeletionAddress) -> application::ProgrammingCueDeletionAddress {
    match value {
        wire::CueDeletionAddress::Pool { playback_number } => {
            application::ProgrammingCueDeletionAddress::Pool { playback_number }
        }
        wire::CueDeletionAddress::CurrentPage {
            expected_page,
            slot,
        } => application::ProgrammingCueDeletionAddress::CurrentPage {
            expected_page,
            slot,
        },
        wire::CueDeletionAddress::PageSlot { page, slot } => {
            application::ProgrammingCueDeletionAddress::PageSlot { page, slot }
        }
    }
}

fn projection(
    value: application::ProgrammingCueDeletionObjectProjection,
) -> wire::CueDeletionObjectProjection {
    wire::CueDeletionObjectProjection {
        cue_list_id: value.cue_list_id.0,
        object_id: value.object_id,
        object_revision: value.object_revision,
        body: value.raw_body,
    }
}
