use super::*;

pub(super) fn publish_compatibility_events(
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
        serde_json::json!({
            "session_id":session.id,
            "user_id":session.user.id,
            "desk_id":session.desk.id,
            "command":command.command,
            "changes":changes,
        }),
    );
}
