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

const PROGRAMMER_CHANGED_COMMANDS: &[&str] = &[
    "programmer.set",
    "programmer.set_many",
    "programmer.set_value",
    "programmer.control_action",
    "programmer.release",
    "programmer.group.set",
    "programmer.group.release",
    "selection.set",
    "selection.gesture",
    "selection.macro",
    "group.select",
    "programmer.execute",
    "programmer.undo",
    "programmer.redo",
    "preload.group.set",
    "preload.clear",
];

fn dispatch_ws_payload(
    state: &AppState,
    session: &Session,
    command: &WsCommand,
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
        "programmer.control_action" => ws_programmer_control_action(state, session, command),
        "preset.generate_fixture_values" => {
            ws_preset_generate_fixture_values(state, session, command)
        }
        "programmer.release" => ws_programmer_release(state, session, command),
        "programmer.clear" => ws_programmer_clear(state, session, command),
        "preload.enter" => ws_preload_enter(state, session, command),
        "preload.group.set" => ws_preload_group_set(state, session, command),
        "preload.go" => commit_preload(state, session),
        "preload.clear" => ws_preload_clear(state, session, command),
        "preload.release" => ws_preload_release(state, session, command),
        "programmer.undo" => Ok(serde_json::json!({"changed":state.programmers.undo(session.id)})),
        "programmer.redo" => Ok(serde_json::json!({"changed":state.programmers.redo(session.id)})),
        "programmer.command_line" => ws_programmer_command_line(state, session, command),
        "programmer.command_target" => ws_programmer_command_target(state, session, command),
        "programmer.execute" => ws_programmer_execute(state, session, command),
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
    reconcile_selection_if_needed(state, session, &result);
    if let Err(error) = persist_undo_redo(state, session, &command, &result.response) {
        return failed_ws_response(&command, revision, error);
    }
    match result.response {
        Ok(payload) => successful_ws_response(state, session, command, payload),
        Err(error) => failed_ws_response(&command, revision, error),
    }
}

fn reconcile_selection_if_needed(
    state: &AppState,
    session: &Session,
    result: &WsProgrammingOutput,
) {
    if result.response.is_ok() && result.selection_changed {
        reconcile_highlight_selection(state, session, "programmer_selection");
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
        return WsProgrammingOutput::untracked(dispatch_ws_payload(state, session, command));
    }
    state
        .programming
        .run_unit_of_work(WsProgrammingOperation {
            state,
            session,
            command,
        })
        .output
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
) -> WsResponse {
    publish_compatibility_events(state, session, &command, &payload);
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
) {
    let no_op_release = command.command == "preload.release"
        && payload.get("released").and_then(serde_json::Value::as_bool) == Some(false);
    if !no_op_release {
        emit_command_applied(state, session, command);
    }
    if PROGRAMMER_CHANGED_COMMANDS.contains(&command.command.as_str()) {
        emit_programmer_changed(state, session, command);
    }
}

fn emit_command_applied(state: &AppState, session: &Session, command: &WsCommand) {
    emit(
        state,
        "command_applied",
        serde_json::json!({"request_id":command.request_id,"session_id":session.id,"command":command.command}),
    );
}

fn emit_programmer_changed(state: &AppState, session: &Session, command: &WsCommand) {
    emit(
        state,
        "programmer_changed",
        serde_json::json!({"session_id":session.id,"command":command.command}),
    );
}

struct WsProgrammingOperation<'a> {
    state: &'a AppState,
    session: &'a Session,
    command: &'a WsCommand,
}

struct WsProgrammingOutput {
    response: Result<serde_json::Value, String>,
    selection_changed: bool,
}

impl WsProgrammingOutput {
    fn untracked(response: Result<serde_json::Value, String>) -> Self {
        Self {
            response,
            selection_changed: false,
        }
    }
}

impl light_application::ProgrammingUnitOfWork for WsProgrammingOperation<'_> {
    type Output = WsProgrammingOutput;

    fn desk_id(&self) -> Uuid {
        self.session.desk.id
    }

    fn execute(self) -> light_application::ProgrammingOperation<Self::Output> {
        let before = self.state.programmers.interaction_state(self.session.id);
        let response = dispatch_ws_payload(self.state, self.session, self.command);
        let after = self.state.programmers.interaction_state(self.session.id);
        let selection_changed = selection_revision(&before) != selection_revision(&after);
        let output = WsProgrammingOutput {
            response,
            selection_changed,
        };
        let events = interaction_event(self.session, self.command, &before, after, &output);
        light_application::ProgrammingOperation::with_events(output, events)
    }
}

fn selection_revision(state: &Option<light_programmer::ProgrammerInteractionState>) -> Option<u64> {
    state.as_ref().map(|state| state.selection.revision)
}

fn interaction_event(
    session: &Session,
    command: &WsCommand,
    before: &Option<light_programmer::ProgrammerInteractionState>,
    after: Option<light_programmer::ProgrammerInteractionState>,
    output: &WsProgrammingOutput,
) -> Vec<light_application::EventDraft> {
    let Some(after) = changed_interaction(before, after, output) else {
        return Vec::new();
    };
    vec![interaction_draft(session, command, after)]
}

fn changed_interaction(
    before: &Option<light_programmer::ProgrammerInteractionState>,
    after: Option<light_programmer::ProgrammerInteractionState>,
    output: &WsProgrammingOutput,
) -> Option<light_programmer::ProgrammerInteractionState> {
    after.filter(|after| output.response.is_ok() && Some(after) != before.as_ref())
}

fn interaction_draft(
    session: &Session,
    command: &WsCommand,
    interaction: light_programmer::ProgrammerInteractionState,
) -> light_application::EventDraft {
    light_application::EventDraft::programming_interaction_changed(
        &interaction_context(session, command),
        light_application::ProgrammingInteractionProjection {
            desk_id: session.desk.id,
            command_line: interaction.command_line,
            selection: interaction.selection,
        },
    )
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
