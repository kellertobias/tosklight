use super::super::wire::wire_choice;
use light_application::{PendingCommandChoice, ProgrammingResult};

use super::super::super::{AppState, Session};

pub(super) fn publish_osc(state: &AppState, session: &Session, choice: &PendingCommandChoice) {
    super::super::super::emit(
        state,
        "programmer_choice_requested",
        serde_json::json!({
            "desk_id":session.desk.id,
            "session_id":session.id,
            "pending_choice":wire_choice(choice.clone()),
            "source":"osc",
        }),
    );
}

pub(super) fn publish_http(
    state: &AppState,
    session: &Session,
    result: &ProgrammingResult,
    choice: &PendingCommandChoice,
    request_id: Option<&str>,
    supplied_command: Option<&str>,
) {
    let command = supplied_command.unwrap_or_else(|| result.command_line_before.visible_text());
    let (audit_command, sensitive) = super::super::super::command_audit_projection(command);
    let pending_choice = if sensitive {
        serde_json::json!({
            "type": match choice {
                PendingCommandChoice::CueMoveCopy(_) => "cue_move_copy",
                PendingCommandChoice::DynamicInstance(_) => "dynamic_instance",
            },
            "redacted":true
        })
    } else {
        serde_json::to_value(wire_choice(choice.clone()))
            .expect("the application Cue choice satisfies the wire contract")
    };
    super::super::super::emit(
        state,
        "programmer_choice_requested",
        serde_json::json!({
            "request_id":request_id,
            "desk_id":session.desk.id,
            "session_id":session.id,
            "command":audit_command,
            "pending_choice":pending_choice,
            "source":"http",
        }),
    );
}
