use super::*;

const LIVE_ABSOLUTE_COMMANDS: &[&str] = &[
    "selection.set",
    "selection.gesture",
    "selection.macro",
    "group.select",
    "programmer.set",
    "programmer.set_many",
    "programmer.set_value",
    "programmer.control_action",
    "programmer.priority",
    "programmer.release",
    "programmer.group.set",
    "programmer.group.release",
    "programmer.align",
    "programmer.command_line",
    "programmer.command_target",
    "programmer.execute",
    "programmer.clear",
    "programmer.undo",
    "programmer.redo",
    "programmer.mode",
    "master.set",
    "group.master.set",
    "group.master.flash",
    "preload.enter",
    "preload.group.set",
    "preload.go",
    "preload.clear",
    "preload.release",
    "playback.go",
    "playback.back",
    "playback.pause",
    "playback.release",
    "preset.apply",
];

const PROGRAMMING_INTERACTION_COMMANDS: &[&str] = &[
    "selection.set",
    "selection.gesture",
    "selection.macro",
    "group.select",
    "programmer.set",
    "programmer.set_many",
    "programmer.set_value",
    "programmer.control_action",
    "programmer.priority",
    "programmer.release",
    "programmer.group.set",
    "programmer.group.release",
    "programmer.align",
    "programmer.command_line",
    "programmer.command_target",
    "programmer.execute",
    "programmer.clear",
    "programmer.undo",
    "programmer.redo",
    "programmer.mode",
    "preload.enter",
    "preload.group.set",
    "preload.go",
    "preload.clear",
    "preload.release",
    "preset.apply",
];

fn dispatch_ws_payload(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
    context: Option<&light_application::ActionContext>,
) -> Result<serde_json::Value, String> {
    match command.command.as_str() {
        "selection.set" => ws_selection_set(state, session, command),
        "selection.gesture" => ws_selection_gesture(state, session, command),
        "group.select" => ws_group_select(state, session, command),
        "selection.macro" => ws_selection_macro(state, session, command),
        "programmer.align" => ws_programmer_align(state, session, command),
        "programmer.group.set" => ws_programmer_group_set(state, session, command),
        "programmer.group.release" => ws_programmer_group_release(state, session, command),
        "programmer.priority" => ws_programmer_priority(state, session, command),
        "programmer.set" => ws_programmer_set(state, session, command),
        "programmer.set_many" => ws_programmer_set_many(state, session, command),
        "programmer.set_value" => ws_programmer_set_value(state, session, command),
        "preset.generate_fixture_values" => {
            ws_preset_generate_fixture_values(state, session, command)
        }
        "programmer.release" => ws_programmer_release(state, session, command),
        "programmer.clear" => ws_programmer_clear(state, session, command),
        "preload.enter" => ws_preload_enter(state, session, command),
        "preload.group.set" => ws_preload_group_set(state, session, command),
        "preload.go" => commit_preload_while_show_stable(state, session),
        "preload.clear" => ws_preload_clear(state, session, command),
        "preload.release" => ws_preload_release(state, session, command),
        "programmer.undo" => Ok(serde_json::json!({"changed":state.programmers.undo(session.id)})),
        "programmer.redo" => Ok(serde_json::json!({"changed":state.programmers.redo(session.id)})),
        "programmer.command_line" => ws_programmer_command_line(state, session, command),
        "programmer.command_target" => ws_programmer_command_target(state, session, command),
        "programmer.execute" => ws_programmer_execute(state, session, command, context),
        "preset.apply" => ws_preset_apply(state, session, command),
        "programmer.mode" => ws_programmer_mode(state, session, command),
        "master.set" => ws_master_set(state, session, command),
        "group.master.set" => ws_group_master_set(state, session, command),
        "group.master.flash" => ws_group_master_flash(state, session, command),
        "playback.go" | "playback.back" | "playback.pause" | "playback.release" => {
            ws_playback_go(state, session, command)
        }
        _ => Err("unknown command".into()),
    }
}

pub(super) fn dispatch_ws_command(
    state: &AppState,
    session: &Session,
    command: WsCommand,
) -> WsResponse {
    let revision = state.engine.snapshot().revision;
    let live_absolute = match validate_ws_command(state, session, &command, revision) {
        Ok(live_absolute) => live_absolute,
        Err(error) => return failed_ws_response(&command, revision, error),
    };
    let result = dispatch_validated_ws_command(state, session, &command, live_absolute);
    match result.response {
        Ok(payload) => successful_ws_response(state, session, command, payload, result.changes),
        Err(error) => failed_ws_response(&command, revision, error),
    }
}

fn validate_ws_command(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
    revision: u64,
) -> Result<bool, String> {
    validate_ws_identity(state, session, command)?;
    let live_absolute = LIVE_ABSOLUTE_COMMANDS.contains(&command.command.as_str());
    if !live_absolute
        && command
            .expected_revision
            .is_some_and(|expected| expected != revision)
    {
        return Err(format!("revision conflict: current revision is {revision}"));
    }
    Ok(live_absolute)
}

fn validate_ws_identity(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
) -> Result<(), String> {
    if read_desk_lock(state, session.desk.id).locked {
        return Err("desk is locked".into());
    }
    if command.protocol_version != 1 {
        return Err("unsupported protocol_version".into());
    }
    if command.session_id != session.id {
        return Err("session_id does not own this connection".into());
    }
    Ok(())
}

fn dispatch_validated_ws_command(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
    live_absolute: bool,
) -> WsProgrammingOutput {
    if !live_absolute {
        return WsProgrammingOutput::untracked(dispatch_ws_payload(state, session, command, None));
    }
    if !PROGRAMMING_INTERACTION_COMMANDS.contains(&command.command.as_str()) {
        return WsProgrammingOutput::untracked(dispatch_ws_payload(state, session, command, None));
    }
    let _activation = match try_programming_activation(state) {
        Ok(activation) => activation,
        Err(error) => return WsProgrammingOutput::untracked(Err(error)),
    };
    let context = interaction_context(session, command);
    let ports = command_http::ServerProgrammingPorts::new(state, session, "software", true);
    match state
        .programming
        .run_external_interaction(&context, &ports, || {
            dispatch_live_interaction(state, session, command, &context)
        }) {
        Ok(completed) => completed.output.with_changes(
            completed.event_sequence.is_some(),
            completed.values_event_sequence.is_some(),
        ),
        Err(error) => WsProgrammingOutput::untracked(Err(error.message)),
    }
}

fn dispatch_live_interaction(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
    context: &light_application::ActionContext,
) -> WsProgrammingOutput {
    let before = tracked_state(state, session);
    let (mut response, transient_changed) = dispatch_live_payload(state, session, command, context);
    if let Err(error) = persist_undo_redo(state, session, command, &response) {
        response = Err(error);
    }
    let mutated = tracked_state(state, session);
    reconcile_interaction(state, session, command, &before, &mutated, response.is_ok());
    WsProgrammingOutput {
        response,
        changes: transient_changed
            .then_some("transient_control")
            .into_iter()
            .collect(),
    }
}

fn dispatch_live_payload(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
    context: &light_application::ActionContext,
) -> (Result<serde_json::Value, String>, bool) {
    if command.command != "programmer.control_action" {
        return (
            dispatch_ws_payload(state, session, command, Some(context)),
            false,
        );
    }
    match ws_programmer_control_action(state, session, command) {
        Ok(result) => (Ok(result.payload), result.transient_changed),
        Err(error) => (Err(error), false),
    }
}

fn persist_undo_redo(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
    response: &Result<serde_json::Value, String>,
) -> Result<(), String> {
    if response.is_ok()
        && matches!(
            command.command.as_str(),
            "programmer.undo" | "programmer.redo"
        )
    {
        persist_programmer(state, session).map_err(|error| error.message)?;
    }
    Ok(())
}

fn successful_ws_response(
    state: &AppState,
    session: &Session,
    command: WsCommand,
    payload: serde_json::Value,
    changes: Vec<&'static str>,
) -> WsResponse {
    publish_compatibility_events(state, session, &command, &payload, &changes);
    WsResponse {
        protocol_version: 1,
        request_id: command.request_id,
        ok: true,
        revision: state.engine.snapshot().revision,
        payload: Some(payload),
        error: None,
    }
}

fn failed_ws_response(command: &WsCommand, revision: u64, error: String) -> WsResponse {
    WsResponse {
        protocol_version: 1,
        request_id: command.request_id.clone(),
        ok: false,
        revision,
        payload: None,
        error: Some(error),
    }
}

fn publish_compatibility_events(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
    payload: &serde_json::Value,
    changes: &[&str],
) {
    let no_op_release = command.command == "preload.release"
        && payload.get("released").and_then(serde_json::Value::as_bool) == Some(false);
    if !no_op_release {
        emit_command_applied(state, session, command);
    }
    if !changes.is_empty() {
        emit_programmer_changed(state, session, command, changes);
    }
}

fn emit_command_applied(state: &AppState, session: &Session, command: &WsCommand) {
    emit(
        state,
        "command_applied",
        serde_json::json!({"request_id":command.request_id,"session_id":session.id,"command":command.command}),
    );
}

fn emit_programmer_changed(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
    changes: &[&str],
) {
    emit(
        state,
        "programmer_changed",
        serde_json::json!({"session_id":session.id,"command":command.command,"changes":changes}),
    );
}

struct WsProgrammingOutput {
    response: Result<serde_json::Value, String>,
    changes: Vec<&'static str>,
}

impl WsProgrammingOutput {
    fn untracked(response: Result<serde_json::Value, String>) -> Self {
        Self {
            response,
            changes: Vec::new(),
        }
    }

    fn with_changes(mut self, interaction: bool, values: bool) -> Self {
        if interaction {
            self.changes.push("interaction");
        }
        if values {
            self.changes.push("values");
        }
        self
    }
}

fn reconcile_interaction(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
    before: &WsTrackedState,
    mutated: &WsTrackedState,
    response_succeeded: bool,
) {
    let capture_mode_changed = before.capture_mode() != mutated.capture_mode();
    let explicit_highlight_succeeded = command.command == "programmer.mode"
        && command
            .payload
            .get("highlight")
            .is_some_and(|value| !value.is_null())
        && response_succeeded;
    if capture_mode_changed {
        if !explicit_highlight_succeeded {
            reconcile_highlight_capture_mode(state, session, "programmer_capture_mode");
        }
    } else if before.selection_revision() != mutated.selection_revision() {
        reconcile_highlight_selection(state, session, "programmer_selection");
    }
}

struct WsTrackedState {
    interaction: Option<light_programmer::ProgrammerInteractionVersion>,
}

impl WsTrackedState {
    fn selection_revision(&self) -> Option<u64> {
        self.interaction
            .as_ref()
            .map(|state| state.selection_revision)
    }

    fn capture_mode(&self) -> Option<light_programmer::ProgrammerCaptureMode> {
        self.interaction.as_ref().map(|state| state.capture_mode)
    }
}

fn tracked_state(state: &AppState, session: &Session) -> WsTrackedState {
    WsTrackedState {
        interaction: state.programmers.interaction_version(session.id),
    }
}

fn interaction_context(session: &Session, command: &WsCommand) -> light_application::ActionContext {
    light_application::ActionContext::operator(
        session.desk.id,
        session.user.id.0,
        session.id.0,
        light_application::ActionSource::UserInterface,
    )
    .with_request_id(&command.request_id)
}
