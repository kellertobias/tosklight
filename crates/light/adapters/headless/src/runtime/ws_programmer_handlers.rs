use super::*;

pub(super) fn ws_programmer_capture_mode(
    state: &AppState,
    session: &Session,
    request: light_wire::v2::live_action::ProgrammerCaptureModeLiveActionRequest,
) -> Result<light_wire::v2::live_action::ProgrammerCaptureModeOutcome, String> {
    let request_id = request.request_id;
    state.programming.set_modes(
        session.id,
        request.blind,
        request.preview,
        None,
        request.active_context,
    );
    persist_programmer(state, session).map_err(|error| error.message)?;
    let programmer = state
        .programming
        .get(session.id)
        .ok_or_else(|| "programmer not found".to_owned())?;
    Ok(light_wire::v2::live_action::ProgrammerCaptureModeOutcome {
        request_id,
        blind: programmer.blind,
        preview: programmer.preview,
        active_context: programmer.active_context.clone(),
    })
}

pub(super) fn ws_programmer_command_line_replace(
    state: &AppState,
    command: &WsActionRequest,
    context: &light_application::ActionContext,
    ports: &command_http::ServerProgrammingPorts<'_>,
) -> Result<WsTypedProgrammingAction, String> {
    #[derive(Deserialize)]
    struct Input {
        request_id: String,
        expected_revision: u64,
        text: String,
    }
    let input: Input =
        serde_json::from_value(command.payload.clone()).map_err(|error| error.to_string())?;
    crate::tolerant_json::log_unknown_value_fields::<Input>(
        "/api/v2/events programmer.command_line.replace",
        &command.payload,
    );
    if input.request_id != command.request_id {
        return Err("Command-line payload request_id must match the WebSocket request_id".into());
    }
    command_http::validate_command(&input.text).map_err(|error| error.message)?;
    let result = state
        .programming
        .handle(
            light_application::ActionEnvelope {
                context: context
                    .clone()
                    .with_expected_revision(input.expected_revision),
                command: light_application::ProgrammingCommand::ReplaceCommandLine {
                    text: input.text,
                    expected_revision: input.expected_revision,
                },
            },
            ports,
        )
        .map_err(|error| error.message)?;
    if let light_application::ProgrammingOutcome::Accepted {
        warning: Some(warning),
        ..
    } = &result.outcome
    {
        return Err(warning.clone());
    }
    let payload = serde_json::to_value(command_http::command_line_from_state(result.command_line))
        .map_err(|error| error.to_string())?;
    Ok(WsTypedProgrammingAction { payload })
}

pub(super) fn ws_programmer_selection_action(
    state: &AppState,
    command: &WsActionRequest,
    context: &light_application::ActionContext,
    ports: &command_http::ServerProgrammingPorts<'_>,
) -> Result<WsTypedProgrammingAction, String> {
    let request: light_wire::v2::command_line::ProgrammingSelectionActionRequest =
        serde_json::from_value(command.payload.clone()).map_err(|error| error.to_string())?;
    crate::tolerant_json::log_unknown_value_fields::<
        light_wire::v2::command_line::ProgrammingSelectionActionRequest,
    >(
        "/api/v2/events programmer.selection.action",
        &command.payload,
    );
    if request.request_id != command.request_id {
        return Err("Selection payload request_id must match the WebSocket request_id".into());
    }
    command_http::validate_selection_request(&request).map_err(|error| error.message)?;
    let request_id = request.request_id;
    let command = command_http::selection_command(request.action).map_err(|error| error.message)?;
    let result = state
        .programming
        .handle(
            light_application::ActionEnvelope {
                context: context.clone(),
                command,
            },
            ports,
        )
        .map_err(|error| error.message)?;
    let payload = serde_json::to_value(
        command_http::selection_response(request_id, result).map_err(|error| error.message)?,
    )
    .map_err(|error| error.to_string())?;
    Ok(WsTypedProgrammingAction { payload })
}

pub(super) fn ws_programmer_values_action(
    state: &AppState,
    command: &WsActionRequest,
    context: &light_application::ActionContext,
    ports: &command_http::ServerProgrammingPorts<'_>,
) -> Result<WsTypedProgrammingAction, String> {
    let request: light_wire::v2::programming::ProgrammingValuesActionRequest =
        serde_json::from_value(command.payload.clone()).map_err(|error| error.to_string())?;
    crate::tolerant_json::log_unknown_value_fields::<
        light_wire::v2::programming::ProgrammingValuesActionRequest,
    >("/api/v2/events programmer.values.action", &command.payload);
    if request.request_id != command.request_id {
        return Err(
            "Programmer values payload request_id must match the WebSocket request_id".into(),
        );
    }
    let request_id = request.request_id;
    let context = context
        .clone()
        .with_expected_revision(request.expected_revision);
    let colors = command_http::color_attribute_index(state);
    let session_id = context
        .session_id
        .map(SessionId)
        .ok_or_else(|| "Programmer values require a session".to_owned())?;
    let action = indexed_presets::programming_action(state, session_id, request.action)
        .map_err(|error| error.message)?;
    let command = command_http::values_command(action, &colors).map_err(|error| error.message)?;
    let result = state
        .programming
        .handle_values(
            light_application::ActionEnvelope {
                context,
                command: light_application::ProgrammingValuesRequest {
                    expected_capture_mode_revision: request.expected_capture_mode_revision,
                    command,
                },
            },
            ports,
        )
        .map_err(|error| error.message)?;
    let payload = serde_json::to_value(command_http::values_outcome(request_id, result))
        .map_err(|error| error.to_string())?;
    Ok(WsTypedProgrammingAction { payload })
}

pub(super) fn ws_programmer_priority_action(
    state: &AppState,
    command: &WsActionRequest,
    context: &light_application::ActionContext,
    ports: &command_http::ServerProgrammingPorts<'_>,
) -> Result<WsTypedProgrammingAction, String> {
    let request: light_wire::v2::programmer_priority::ProgrammerPriorityActionRequest =
        serde_json::from_value(command.payload.clone()).map_err(|error| error.to_string())?;
    crate::tolerant_json::log_unknown_value_fields::<
        light_wire::v2::programmer_priority::ProgrammerPriorityActionRequest,
    >(
        "/api/v2/events programmer.priority.action",
        &command.payload,
    );
    if request.request_id != command.request_id {
        return Err("Priority payload request_id must match the WebSocket request_id".into());
    }
    let result = state
        .programming
        .handle_priority(
            light_application::ActionEnvelope {
                context: context.clone(),
                command: light_application::ProgrammingPriorityRequest {
                    expected_revision:
                        light_application::ProgrammingPriorityRevisionExpectation::Exact(
                            request.expected_revision,
                        ),
                    priority: request.priority,
                },
            },
            ports,
        )
        .map_err(|error| error.message)?;
    let payload = serde_json::to_value(command_http::programmer_priority_outcome(result))
        .map_err(|error| error.to_string())?;
    Ok(WsTypedProgrammingAction { payload })
}

pub(super) struct WsControlActionResult {
    pub(super) payload: serde_json::Value,
}

pub(super) fn ws_programmer_control_action(
    state: &AppState,
    session: &Session,
    command: &WsActionRequest,
) -> Result<WsControlActionResult, String> {
    #[derive(Deserialize)]
    struct Input {
        fixture_id: light_core::FixtureId,
        action_id: Uuid,
        active: bool,
    }
    let input: Input =
        serde_json::from_value(command.payload.clone()).map_err(|e| e.to_string())?;
    let snapshot = state.output.snapshot();
    let (assignments, pulse_duration, kind) = control_action_programmer_values(
        &snapshot,
        input.fixture_id,
        input.action_id,
        input.active,
    )?;
    let transient_source = format!("fixture-control:{}:{}", input.fixture_id.0, input.action_id);
    let transient_generation = match (kind, input.active) {
        (light_fixture::ControlActionKind::Latched, _) => {
            state.programming.set_many(session.id, assignments);
            persist_programmer(state, session).map_err(|e| e.message)?;
            None
        }
        (_, true) => state.programming.set_transient_action(
            session.id,
            transient_source.clone(),
            assignments,
        ),
        (_, false) => {
            state
                .programming
                .release_transient_action(session.id, &transient_source, None);
            None
        }
    };
    if let (Some(duration_millis), Some(generation)) = (pulse_duration, transient_generation) {
        let task_state = state.clone();
        let task_session = session.clone();
        let task_source = transient_source.clone();
        let lifecycle = task_state.lifecycle.clone();
        if let Err(error) = lifecycle.schedule(async move {
            tokio::time::sleep(Duration::from_millis(duration_millis)).await;
            if task_state.programming.release_transient_action(
                task_session.id,
                &task_source,
                Some(generation),
            ) {
                emit(
                    &task_state,
                    "programmer_changed",
                    serde_json::json!({
                        "session_id":task_session.id,
                        "desk_id":task_session.desk.id,
                        "command":"programmer.control_action",
                        "changes":["transient_control"],
                        "action_id":input.action_id,
                        "active":false,
                        "timed_pulse_complete":true,
                    }),
                );
            }
            Ok(())
        }) {
            state.programming.release_transient_action(
                session.id,
                &transient_source,
                Some(generation),
            );
            return Err(error.to_string());
        }
    }
    Ok(WsControlActionResult {
        payload: serde_json::json!({
            "action_id":input.action_id,
            "active":input.active,
            "kind":kind,
            "pulse_duration_millis":pulse_duration,
            "programmer":state.programming.get(session.id),
        }),
    })
}

pub(super) fn ws_programmer_control_actions(
    state: &AppState,
    session: &Session,
    request: light_wire::v2::live_action::FixtureControlsLiveActionRequest,
) -> Result<WsControlActionResult, String> {
    if request.targets.is_empty() {
        return Err("Indexed Preset control action requires at least one target".into());
    }
    let interaction = state
        .programming
        .programmers()
        .interaction_state(session.id)
        .ok_or_else(|| "Programmer interaction authority is unavailable".to_owned())?;
    if interaction.selection.revision != request.expected_selection_revision {
        return Err(format!(
            "Indexed Preset expected selection revision {}, but the current revision is {}",
            request.expected_selection_revision, interaction.selection.revision
        ));
    }
    let selected = interaction
        .selection
        .selected
        .iter()
        .copied()
        .collect::<HashSet<_>>();
    let snapshot = state.output.snapshot();
    let (assignments, pulse_duration, kind) =
        validate_control_action_targets(&request, &selected, &snapshot)?;
    let source = format!(
        "indexed-control:{}",
        request
            .targets
            .iter()
            .map(|target| format!("{}:{}", target.fixture_id, target.action_id))
            .collect::<Vec<_>>()
            .join(",")
    );
    let generation = match (kind, request.active) {
        (light_fixture::ControlActionKind::Latched, _) => {
            state.programming.set_many(session.id, assignments);
            persist_programmer(state, session).map_err(|error| error.message)?;
            None
        }
        (_, true) => {
            state
                .programming
                .set_transient_action(session.id, source.clone(), assignments)
        }
        (_, false) => {
            state
                .programming
                .release_transient_action(session.id, &source, None);
            None
        }
    };
    schedule_control_action_release(state, session, &source, pulse_duration, generation)?;
    Ok(WsControlActionResult {
        payload: serde_json::json!({
            "active":request.active,
            "kind":kind,
            "pulse_duration_millis":pulse_duration,
            "programmer":state.programming.get(session.id),
        }),
    })
}

type ControlActionAssignments = Vec<(
    light_core::FixtureId,
    light_core::AttributeKey,
    light_core::AttributeValue,
)>;

fn validate_control_action_targets(
    request: &light_wire::v2::live_action::FixtureControlsLiveActionRequest,
    selected: &HashSet<light_core::FixtureId>,
    snapshot: &light_engine::EngineSnapshot,
) -> Result<
    (
        ControlActionAssignments,
        Option<u64>,
        light_fixture::ControlActionKind,
    ),
    String,
> {
    let mut seen = HashSet::new();
    let mut assignments = Vec::new();
    let mut compatibility = None;
    let mut pulse_duration = None;
    let mut kind = None;
    for target in &request.targets {
        if !seen.insert((target.fixture_id, target.action_id)) {
            return Err("Indexed Preset control action contains a duplicate target".into());
        }
        let fixture_id = light_core::FixtureId(target.fixture_id);
        if !selected.contains(&fixture_id) {
            return Err("Indexed Preset control target is no longer selected".into());
        }
        let fixture = snapshot
            .fixtures
            .iter()
            .find(|fixture| {
                fixture.fixture_id == fixture_id
                    || fixture
                        .logical_heads
                        .iter()
                        .any(|head| head.fixture_id == fixture_id)
            })
            .ok_or_else(|| {
                "Indexed Preset control fixture is not in the active patch".to_owned()
            })?;
        if fixture.definition.hazardous {
            return Err(
                "Hazardous fixture control actions require their dedicated confirmed surface"
                    .into(),
            );
        }
        let profile = fixture
            .definition
            .profile_snapshot
            .as_deref()
            .ok_or_else(|| "Indexed Preset control fixture has no embedded profile".to_owned())?;
        if profile.revision != target.expected_profile_revision {
            return Err("Indexed Preset control fixture profile changed".into());
        }
        let mode = fixture
            .definition
            .mode_id
            .and_then(|mode_id| profile.mode(mode_id))
            .ok_or_else(|| "Indexed Preset control fixture mode is unavailable".to_owned())?;
        let action = mode
            .control_actions
            .iter()
            .find(|action| action.id == target.action_id)
            .ok_or_else(|| "Indexed Preset control action is no longer available".to_owned())?;
        let current = (
            action.name.clone(),
            action.semantic,
            action.kind,
            action.duration_millis,
        );
        if let Some(expected) = &compatibility {
            if expected != &current {
                return Err("Indexed Preset control targets are not behavior-compatible".into());
            }
        } else {
            compatibility = Some(current);
        }
        let (next_assignments, next_duration, next_kind) = control_action_programmer_values(
            &snapshot,
            fixture_id,
            target.action_id,
            request.active,
        )?;
        assignments.extend(next_assignments);
        pulse_duration = next_duration;
        kind = Some(next_kind);
    }
    let kind = kind.ok_or_else(|| "Indexed Preset control action has no targets".to_owned())?;
    Ok((assignments, pulse_duration, kind))
}

fn schedule_control_action_release(
    state: &AppState,
    session: &Session,
    source: &str,
    pulse_duration: Option<u64>,
    generation: Option<u64>,
) -> Result<(), String> {
    if let (Some(duration_millis), Some(generation)) = (pulse_duration, generation) {
        let task_state = state.clone();
        let task_session = session.clone();
        let task_source = source.to_owned();
        let lifecycle = task_state.lifecycle.clone();
        if let Err(error) = lifecycle.schedule(async move {
            tokio::time::sleep(Duration::from_millis(duration_millis)).await;
            task_state.programming.release_transient_action(
                task_session.id,
                &task_source,
                Some(generation),
            );
            Ok(())
        }) {
            state
                .programming
                .release_transient_action(session.id, &source, Some(generation));
            return Err(error.to_string());
        }
    }
    Ok(())
}
