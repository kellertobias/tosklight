use super::*;

pub(super) fn ws_programmer_align(
    state: &AppState,
    request: light_wire::v2::live_action::ProgrammingAlignLiveActionRequest,
    context: &light_application::ActionContext,
    ports: &command_http::ServerProgrammingPorts<'_>,
) -> Result<light_wire::v2::live_action::ProgrammingAlignOutcome, String> {
    use light_programmer::ProgrammerAlignmentMode as DomainMode;
    use light_wire::v2::live_action::ProgrammingAlignAction as WireMode;

    let domain_mode = match request.mode {
        WireMode::Off => None,
        WireMode::Left => Some(DomainMode::Left),
        WireMode::Right => Some(DomainMode::Right),
        WireMode::Out => Some(DomainMode::Out),
        WireMode::In => Some(DomainMode::In),
        WireMode::Cycle => None,
    };
    let active = if request.mode == WireMode::Cycle {
        state.programming.cycle_alignment(context, ports)
    } else {
        state.programming.set_alignment(context, ports, domain_mode)
    }
    .map_err(|error| error.message)?;
    let projection = command_http::alignment_projection(&state.programming.alignment_projection());
    Ok(light_wire::v2::live_action::ProgrammingAlignOutcome {
        request_id: request.request_id,
        // The resulting mode: an Align activation that changed nothing (no selection) is Off.
        mode: projection.mode,
        revision: active.as_ref().map(|state| state.revision),
        bound_attribute: active
            .as_ref()
            .and_then(|state| state.binding.as_ref())
            .map(|binding| binding.attribute.0.to_string()),
        fixture_count: projection.fixture_count,
        alignment: projection,
    })
}
