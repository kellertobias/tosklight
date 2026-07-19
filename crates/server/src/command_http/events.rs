use super::state_event::publish_command_line_change;
use super::wire::wire_choice;
use light_application::{
    CueMoveCopyChoice as ApplicationCueChoice, ProgrammingAction, ProgrammingOutcome,
    ProgrammingResult,
};

use super::super::{AppState, Session};

pub(super) fn publish_osc_result(
    state: &AppState,
    session: &Session,
    desk_alias: &str,
    result: &ProgrammingResult,
) {
    if result.replayed {
        return;
    }
    match &result.outcome {
        ProgrammingOutcome::Accepted { action, .. } => {
            publish_osc_accepted(state, session, desk_alias, result, *action)
        }
        ProgrammingOutcome::ChoiceRequired { pending_choice } => super::super::emit(
            state,
            "programmer_choice_requested",
            serde_json::json!({
                "desk_id":session.desk.id,
                "session_id":session.id,
                "user_id":session.user.id,
                "pending_choice":wire_choice(pending_choice.clone()),
                "source":"osc",
            }),
        ),
        ProgrammingOutcome::Rejected { error } => publish_osc_error(state, session, result, error),
    }
}

fn publish_osc_error(state: &AppState, session: &Session, result: &ProgrammingResult, error: &str) {
    let (command, sensitive) =
        super::super::command_audit_projection(result.command_line_before.visible_text());
    super::super::emit(
        state,
        "programmer_command_rejected",
        serde_json::json!({
            "desk_id":session.desk.id,
            "session_id":session.id,
            "user_id":session.user.id,
            "command":command,
            "error":if sensitive { "Sensitive input omitted" } else { error },
            "source":"osc",
        }),
    );
}

fn publish_osc_accepted(
    state: &AppState,
    session: &Session,
    desk_alias: &str,
    result: &ProgrammingResult,
    action: ProgrammingAction,
) {
    if action == ProgrammingAction::Executed {
        publish_osc_applied(state, session, desk_alias, result);
    }
    super::super::emit(
        state,
        "programmer_changed",
        serde_json::json!({
            "desk_id":session.desk.id,
            "session_id":session.id,
            "user_id":session.user.id,
            "command":action_name(action),
            "source":"osc",
            "command_line":result.command_line.visible_text(),
            "command_revision":result.command_line.revision,
            "changes":change_categories(action),
        }),
    );
}

fn publish_osc_applied(
    state: &AppState,
    session: &Session,
    desk_alias: &str,
    result: &ProgrammingResult,
) {
    let (command, _) =
        super::super::command_audit_projection(result.command_line_before.visible_text());
    super::super::emit(
        state,
        "command_applied",
        serde_json::json!({
            "desk_id":session.desk.id,
            "session_id":session.id,
            "user_id":session.user.id,
            "desk_alias":desk_alias,
            "command":command,
            "source":"osc",
        }),
    );
}

pub(super) fn publish_service_result(
    state: &AppState,
    session: &Session,
    result: &ProgrammingResult,
    source: &str,
    request_id: Option<&str>,
    supplied_command: Option<&str>,
) -> Option<String> {
    let persistence_warning = edited_persistence_warning(result);
    if result.replayed {
        return persistence_warning;
    }
    publish_command_line_change(
        state,
        session,
        &result.command_line_before,
        &result.command_line,
        source,
        request_id,
    );
    publish_operation_event(state, session, result, request_id, supplied_command);
    persistence_warning
}

fn edited_persistence_warning(result: &ProgrammingResult) -> Option<String> {
    match &result.outcome {
        ProgrammingOutcome::Accepted {
            action: ProgrammingAction::Edited,
            warning,
            ..
        } => warning.clone(),
        _ => None,
    }
}

fn publish_operation_event(
    state: &AppState,
    session: &Session,
    result: &ProgrammingResult,
    request_id: Option<&str>,
    supplied_command: Option<&str>,
) {
    match &result.outcome {
        ProgrammingOutcome::Accepted { action, .. } => {
            publish_accepted_event(state, session, result, *action, request_id)
        }
        ProgrammingOutcome::ChoiceRequired { pending_choice } => publish_choice_event(
            state,
            session,
            result,
            pending_choice,
            request_id,
            supplied_command,
        ),
        ProgrammingOutcome::Rejected { error } => {
            publish_rejection_event(state, session, result, error, request_id, supplied_command)
        }
    }
}

fn publish_accepted_event(
    state: &AppState,
    session: &Session,
    result: &ProgrammingResult,
    action: ProgrammingAction,
    request_id: Option<&str>,
) {
    if publish_key_phase_if_needed(state, session, action, request_id) {
        return;
    }
    if action == ProgrammingAction::Executed {
        super::super::emit(
            state,
            "command_applied",
            serde_json::json!({
                "request_id":request_id,
                "desk_id":session.desk.id,
                "session_id":session.id,
                "user_id":session.user.id,
                "command":"programmer.execute",
                "source":"http",
            }),
        );
    }
    if changes_programmer(action) {
        publish_programmer_changed(state, session, result, action, request_id);
    }
}

fn publish_key_phase_if_needed(
    state: &AppState,
    session: &Session,
    action: ProgrammingAction,
    request_id: Option<&str>,
) -> bool {
    if !matches!(
        action,
        ProgrammingAction::ShiftPressed | ProgrammingAction::ShiftReleased
    ) {
        return false;
    }
    super::super::emit(
        state,
        "command_key_phase",
        serde_json::json!({
            "desk_id":session.desk.id,
            "session_id":session.id,
            "user_id":session.user.id,
            "key":"SHIFT",
            "phase":if action == ProgrammingAction::ShiftPressed { "press" } else { "release" },
            "source":"http",
            "request_id":request_id,
        }),
    );
    true
}

fn publish_programmer_changed(
    state: &AppState,
    session: &Session,
    result: &ProgrammingResult,
    action: ProgrammingAction,
    request_id: Option<&str>,
) {
    super::super::emit(
        state,
        "programmer_changed",
        serde_json::json!({
            "desk_id":session.desk.id,
            "session_id":session.id,
            "user_id":session.user.id,
            "command":action_name(action),
            "source":"http",
            "request_id":request_id,
            "preload_armed":action == ProgrammingAction::PreloadEntered,
            "command_revision":result.command_line.revision,
            "changes":change_categories(action),
        }),
    );
}

fn publish_choice_event(
    state: &AppState,
    session: &Session,
    result: &ProgrammingResult,
    choice: &ApplicationCueChoice,
    request_id: Option<&str>,
    supplied_command: Option<&str>,
) {
    let command = supplied_command.unwrap_or_else(|| result.command_line_before.visible_text());
    let (audit_command, sensitive) = super::super::command_audit_projection(command);
    let pending_choice = if sensitive {
        serde_json::json!({"type":"cue_move_copy","redacted":true})
    } else {
        serde_json::to_value(wire_choice(choice.clone()))
            .expect("the application Cue choice satisfies the wire contract")
    };
    super::super::emit(
        state,
        "programmer_choice_requested",
        serde_json::json!({
            "request_id":request_id,
            "desk_id":session.desk.id,
            "session_id":session.id,
            "user_id":session.user.id,
            "command":audit_command,
            "pending_choice":pending_choice,
            "source":"http",
        }),
    );
}

fn publish_rejection_event(
    state: &AppState,
    session: &Session,
    result: &ProgrammingResult,
    error: &str,
    request_id: Option<&str>,
    supplied_command: Option<&str>,
) {
    let command = supplied_command.unwrap_or_else(|| result.command_line_before.visible_text());
    let (audit_command, sensitive) = super::super::command_audit_projection(command);
    super::super::emit(
        state,
        "programmer_command_rejected",
        serde_json::json!({
            "request_id":request_id,
            "desk_id":session.desk.id,
            "session_id":session.id,
            "user_id":session.user.id,
            "command":audit_command,
            "error":if sensitive { "Sensitive input omitted" } else { error },
            "source":"http",
        }),
    );
}

pub(super) fn persist_with_warning(
    state: &AppState,
    session: &Session,
    source: &str,
    request_id: Option<&str>,
    operation: &str,
) -> Option<String> {
    let error = super::super::persist_programmer(state, session).err()?;
    let warning = format!(
        "the operation succeeded but Programmer persistence failed: {}",
        error.message
    );
    super::super::emit(
        state,
        "programmer_persistence_failed",
        serde_json::json!({
            "desk_id":session.desk.id,
            "session_id":session.id,
            "user_id":session.user.id,
            "request_id":request_id,
            "operation":operation,
            "source":source,
            "error":error.message,
        }),
    );
    Some(warning)
}

const fn changes_programmer(action: ProgrammingAction) -> bool {
    matches!(
        action,
        ProgrammingAction::Executed
            | ProgrammingAction::ClearedPreload
            | ProgrammingAction::ClearedSelection
            | ProgrammingAction::ClearedValues
            | ProgrammingAction::Undone
            | ProgrammingAction::PreloadEntered
            | ProgrammingAction::PreloadCommitted
            | ProgrammingAction::SelectionReplaced
            | ProgrammingAction::SelectionGestureApplied
            | ProgrammingAction::GroupSelected
            | ProgrammingAction::SelectionRuleApplied
    )
}

const fn action_name(action: ProgrammingAction) -> &'static str {
    match action {
        ProgrammingAction::Executed => "programmer.execute",
        ProgrammingAction::ClearedPreload => "programmer.clear_preload",
        ProgrammingAction::ClearedSelection => "programmer.clear_selection",
        ProgrammingAction::ClearedValues => "programmer.clear_values",
        ProgrammingAction::Undone => "programmer.undo",
        ProgrammingAction::PreloadEntered => "preload.enter",
        ProgrammingAction::PreloadCommitted => "preload.go",
        ProgrammingAction::SelectionReplaced => "programmer.selection.replace",
        ProgrammingAction::SelectionGestureApplied => "programmer.selection.gesture",
        ProgrammingAction::GroupSelected => "programmer.selection.group",
        ProgrammingAction::SelectionRuleApplied => "programmer.selection.rule",
        _ => "programmer.command_line",
    }
}

const fn change_categories(action: ProgrammingAction) -> &'static [&'static str] {
    match action {
        ProgrammingAction::Edited
        | ProgrammingAction::ClearedCommandLine
        | ProgrammingAction::ClearedSelection
        | ProgrammingAction::NoChange
        | ProgrammingAction::ShiftPressed
        | ProgrammingAction::ShiftReleased
        | ProgrammingAction::IgnoredRelease => &["interaction"],
        ProgrammingAction::SelectionReplaced
        | ProgrammingAction::SelectionGestureApplied
        | ProgrammingAction::GroupSelected
        | ProgrammingAction::SelectionRuleApplied => &["interaction"],
        ProgrammingAction::Executed
        | ProgrammingAction::ClearedPreload
        | ProgrammingAction::ClearedValues
        | ProgrammingAction::Undone
        | ProgrammingAction::PreloadEntered
        | ProgrammingAction::PreloadCommitted => &["interaction", "values", "runtime"],
    }
}
