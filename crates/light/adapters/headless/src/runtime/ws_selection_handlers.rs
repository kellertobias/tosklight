use super::*;

pub(super) fn ws_programmer_align(
    state: &AppState,
    request: light_wire::v2::live_action::ProgrammingAlignLiveActionRequest,
    context: &light_application::ActionContext,
    ports: &command_http::ServerProgrammingPorts<'_>,
) -> Result<light_wire::v2::live_action::ProgrammingAlignOutcome, String> {
    use light_programmer::ProgrammerAlignmentMode as DomainMode;
    use light_wire::v2::live_action::ProgrammingAlignMode as WireMode;

    let domain_mode = match request.mode {
        WireMode::Off => None,
        WireMode::Left => Some(DomainMode::Left),
        WireMode::Right => Some(DomainMode::Right),
        WireMode::Out => Some(DomainMode::Out),
        WireMode::In => Some(DomainMode::In),
    };
    let state = state
        .programming
        .set_alignment(context, ports, domain_mode)
        .map_err(|error| error.message)?;
    Ok(light_wire::v2::live_action::ProgrammingAlignOutcome {
        request_id: request.request_id,
        mode: request.mode,
        revision: state.as_ref().map(|state| state.revision),
        bound_attribute: state
            .as_ref()
            .and_then(|state| state.binding.as_ref())
            .map(|binding| binding.attribute.0.to_string()),
        fixture_count: state.as_ref().map_or(0, |state| state.fixtures.len()),
    })
}
