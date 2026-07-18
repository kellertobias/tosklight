use super::*;

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
    let fail = |message: String| WsResponse {
        protocol_version: 1,
        request_id: command.request_id.clone(),
        ok: false,
        revision,
        payload: None,
        error: Some(message),
    };
    if read_desk_lock(state, session.desk.id).locked {
        return fail("desk is locked".into());
    }
    if command.protocol_version != 1 {
        return fail("unsupported protocol_version".into());
    }
    if command.session_id != session.id {
        return fail("session_id does not own this connection".into());
    }
    let live_absolute = matches!(
        command.command.as_str(),
        "selection.set"
            | "selection.gesture"
            | "selection.macro"
            | "group.select"
            | "programmer.set"
            | "programmer.set_many"
            | "programmer.set_value"
            | "programmer.control_action"
            | "programmer.priority"
            | "programmer.release"
            | "programmer.group.set"
            | "programmer.group.release"
            | "programmer.align"
            | "programmer.command_line"
            | "programmer.command_target"
            | "programmer.execute"
            | "programmer.clear"
            | "programmer.undo"
            | "programmer.redo"
            | "programmer.mode"
            | "master.set"
            | "group.master.set"
            | "group.master.flash"
            | "preload.enter"
            | "preload.group.set"
            | "preload.go"
            | "preload.clear"
            | "preload.release"
            | "playback.go"
            | "playback.back"
            | "playback.pause"
            | "playback.release"
            | "preset.apply"
    );
    let command_operation = live_absolute.then(|| state.programming.desk_lock(session.desk.id));
    let _command_operation_guard = command_operation.as_ref().map(|lock| lock.lock());
    let selection_revision_before = state
        .programmers
        .selection(session.id)
        .map(|selection| selection.revision);
    if !live_absolute
        && command
            .expected_revision
            .is_some_and(|expected| expected != revision)
    {
        return fail(format!("revision conflict: current revision is {revision}"));
    }
    let result = dispatch_ws_payload(state, session, &command);
    if result.is_ok()
        && state
            .programmers
            .selection(session.id)
            .map(|selection| selection.revision)
            != selection_revision_before
    {
        reconcile_highlight_selection(state, session, "programmer_selection");
    }
    if result.is_ok()
        && matches!(
            command.command.as_str(),
            "programmer.undo" | "programmer.redo"
        )
        && let Err(error) = persist_programmer(state, session)
    {
        return fail(error.message);
    }
    match result {
        Ok(payload) => {
            let no_op_release = command.command == "preload.release"
                && payload.get("released").and_then(serde_json::Value::as_bool) == Some(false);
            if !no_op_release {
                emit(
                    state,
                    "command_applied",
                    serde_json::json!({"request_id":command.request_id,"session_id":session.id,"command":command.command}),
                );
            }
            if matches!(
                command.command.as_str(),
                "programmer.set"
                    | "programmer.set_many"
                    | "programmer.set_value"
                    | "programmer.control_action"
                    | "programmer.release"
                    | "programmer.group.set"
                    | "programmer.group.release"
                    | "selection.set"
                    | "selection.gesture"
                    | "selection.macro"
                    | "group.select"
                    | "programmer.execute"
                    | "programmer.undo"
                    | "programmer.redo"
                    | "preload.group.set"
                    | "preload.clear"
            ) {
                emit(
                    state,
                    "programmer_changed",
                    serde_json::json!({"session_id":session.id,"command":command.command}),
                );
            }
            WsResponse {
                protocol_version: 1,
                request_id: command.request_id,
                ok: true,
                revision: state.engine.snapshot().revision,
                payload: Some(payload),
                error: None,
            }
        }
        Err(error) => fail(error),
    }
}
