use super::*;

pub(super) fn ws_preset_recall_action(
    state: &AppState,
    command: &WsActionRequest,
    context: &light_application::ActionContext,
    ports: &command_http::ServerProgrammingPorts<'_>,
) -> Result<WsTypedProgrammingAction, String> {
    #[derive(Deserialize)]
    struct Input {
        show_id: uuid::Uuid,
        request: light_wire::v2::preset_recall::PresetRecallRequest,
    }
    let input: Input =
        serde_json::from_value(command.payload.clone()).map_err(|error| error.to_string())?;
    crate::tolerant_json::log_unknown_value_fields::<Input>(
        "/api/v2/events preset.recall.action",
        &command.payload,
    );
    let expectation = light_application::ProgrammingPresetRecallRevisionExpectation::Exact;
    let request = input.request;
    let result = state
        .programming
        .handle_preset_recall(
            light_application::ActionEnvelope {
                context: context.clone(),
                command: light_application::ProgrammingPresetRecallRequest {
                    show_id: light_core::ShowId(input.show_id),
                    address: command_http::preset_address(request.address)?,
                    expected_preset_revision: expectation(request.expected_preset_revision),
                    expected_show_revision: expectation(request.expected_show_revision),
                    expected_values_revision: expectation(request.expected_programmer_revision),
                    expected_preload_values_revision: request
                        .expected_preload_values_revision
                        .map_or(
                            light_application::ProgrammingPresetRecallRevisionExpectation::Current,
                            expectation,
                        ),
                    expected_capture_mode_revision: expectation(
                        request.expected_capture_mode_revision,
                    ),
                    expected_selection_revision: expectation(request.expected_selection_revision),
                },
            },
            ports,
        )
        .map_err(|error| error.message)?;
    let payload = serde_json::to_value(command_http::preset_recall_outcome(result))
        .map_err(|error| error.to_string())?;
    Ok(WsTypedProgrammingAction { payload })
}
