use super::events::{persist_with_warning, publish_osc_result};
use super::programming_ports::ServerProgrammingPorts;
use light_application::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ActionSource, ExecutionPolicy,
    ProgrammingCommand, ProgrammingLiveSnapshot, ProgrammingResult,
};
use light_programmer::ProgrammerRegistry;
use light_programmer::command_line::{CommandKey, CommandKeyPhase};

use super::super::{ApiError, AppState, Session};

pub(crate) enum ExistingCommandOutcome {
    Accepted {
        applied: usize,
        persistence_warning: Option<String>,
    },
    ChoiceRequired {
        pending_choice: serde_json::Value,
    },
    Rejected {
        error: String,
    },
}

#[derive(Clone, Copy)]
pub(crate) enum ExistingCommandPolicy {
    /// Temporary adapter for the legacy WebSocket and OSC grammar while owning services migrate.
    Compatibility,
    /// Public v2 guarantee: only commands whose complete mutation is isolated in Programmer.
    AtomicProgrammer,
}

/// Executes the existing grammar while keeping transport envelopes out of the domain path.
pub(crate) fn execute_existing_command(
    state: &AppState,
    session: &Session,
    command: &str,
    source: &str,
    context: &ActionContext,
    policy: ExistingCommandPolicy,
) -> ExistingCommandOutcome {
    let request_id = context.request_id.as_deref();
    if let Some(error) = atomic_policy_error(command, policy) {
        super::super::record_command_history(
            state, session, command, "rejected", &error, source, request_id,
        );
        return ExistingCommandOutcome::Rejected { error };
    }
    if let Some(pending_choice) = super::super::pending_cue_transfer_choice(command) {
        return ExistingCommandOutcome::ChoiceRequired { pending_choice };
    }
    let result = execute_with_policy(state, session, command, context, policy);
    finish_existing_command(state, session, command, source, request_id, result)
}

fn atomic_policy_error(command: &str, policy: ExistingCommandPolicy) -> Option<String> {
    if !matches!(policy, ExistingCommandPolicy::AtomicProgrammer) {
        return None;
    }
    match compatibility_only_family(command) {
        Err(error) => Some(error),
        Ok(Some(family)) => Some(format!(
            "{family} commands are not yet available through the atomic command-line HTTP API"
        )),
        Ok(None) => None,
    }
}

pub(super) fn preset_record_address(
    command: &str,
) -> Result<Option<light_programmer::PresetAddress>, String> {
    let (tokens, timing) = super::super::tokenize_programmer_command(command)?;
    if !tokens
        .first()
        .is_some_and(|token| matches!(token.as_str(), "RECORD" | "REC"))
        || timing.fade_millis.is_some()
        || timing.delay_millis.is_some()
        || tokens.len() != 4
    {
        return Ok(None);
    }
    Ok(super::super::command_preset_address(&tokens[1..]).ok())
}

pub(super) fn group_record_command(
    command: &str,
) -> Result<Option<(String, light_application::ProgrammingGroupRecordOperation)>, String> {
    let tokens = command.split_whitespace().collect::<Vec<_>>();
    let parsed = match tokens.as_slice() {
        [record, group, id] if is_record(record) && group.eq_ignore_ascii_case("GROUP") => Some((
            (*id).to_owned(),
            light_application::ProgrammingGroupRecordOperation::Overwrite,
        )),
        [record, operation, group, id]
            if is_record(record) && group.eq_ignore_ascii_case("GROUP") =>
        {
            let operation = match *operation {
                "+" => light_application::ProgrammingGroupRecordOperation::Merge,
                "-" => light_application::ProgrammingGroupRecordOperation::Subtract,
                _ => return Ok(None),
            };
            Some(((*id).to_owned(), operation))
        }
        [delete, group, id] if is_delete(delete) && group.eq_ignore_ascii_case("GROUP") => Some((
            (*id).to_owned(),
            light_application::ProgrammingGroupRecordOperation::Delete,
        )),
        _ => None,
    };
    Ok(parsed)
}

fn is_record(token: &str) -> bool {
    token.eq_ignore_ascii_case("RECORD") || token.eq_ignore_ascii_case("REC")
}

fn is_delete(token: &str) -> bool {
    token.eq_ignore_ascii_case("DELETE") || token.eq_ignore_ascii_case("DEL")
}

fn execute_with_policy(
    state: &AppState,
    session: &Session,
    command: &str,
    context: &ActionContext,
    policy: ExistingCommandPolicy,
) -> Result<usize, String> {
    match policy {
        ExistingCommandPolicy::Compatibility => {
            // Cross-user reconciliation must not run while one user's mutation gate is held.
            super::super::execute_programmer_command_from(state, session, command, context)
        }
        ExistingCommandPolicy::AtomicProgrammer => {
            state
                .programmers
                .with_staged_command(session.id, |staged_programmers| {
                    execute_staged(state, session, command, context, staged_programmers)
                })
        }
    }
}

fn execute_staged(
    state: &AppState,
    session: &Session,
    command: &str,
    context: &ActionContext,
    staged_programmers: &ProgrammerRegistry,
) -> Result<usize, String> {
    let mut staged_state = state.clone();
    staged_state.programmers = staged_programmers.clone();
    let applied =
        super::super::execute_programmer_command_from(&staged_state, session, command, context)?;
    staged_programmers
        .update_command_line(session.id, |current| (String::new(), current.target, true))
        .ok_or_else(|| "programmer command line does not exist".to_owned())?;
    Ok(applied)
}

fn finish_existing_command(
    state: &AppState,
    session: &Session,
    command: &str,
    source: &str,
    request_id: Option<&str>,
    result: Result<usize, String>,
) -> ExistingCommandOutcome {
    match result {
        Ok(applied) => accepted_command(state, session, command, source, request_id, applied),
        Err(error) => {
            super::super::record_command_history(
                state, session, command, "rejected", &error, source, request_id,
            );
            ExistingCommandOutcome::Rejected { error }
        }
    }
}

fn accepted_command(
    state: &AppState,
    session: &Session,
    command: &str,
    source: &str,
    request_id: Option<&str>,
    applied: usize,
) -> ExistingCommandOutcome {
    let warning = persist_with_warning(state, session, source, request_id, "programmer.execute");
    let feedback = warning.as_ref().map_or_else(
        || format!("Applied to {applied} target(s)"),
        |warning| format!("Applied to {applied} target(s); {warning}"),
    );
    super::super::record_command_history(
        state, session, command, "accepted", &feedback, source, request_id,
    );
    ExistingCommandOutcome::Accepted {
        applied,
        persistence_warning: warning,
    }
}

pub(super) fn compatibility_only_family(command: &str) -> Result<Option<&'static str>, String> {
    if preset_record_address(command)?.is_some()
        || group_record_command(command)?.is_some()
        || super::cue_recording_command::parse(command)?.is_some()
    {
        return Ok(None);
    }
    let Some(family) = super::super::normalized_programmer_command_family(command)? else {
        return Ok(None);
    };
    Ok(match family.as_str() {
        "CUE" => Some("CUE"),
        "SPD" => Some("SPD GRP"),
        "RECORD" | "REC" => Some("RECORD"),
        "UPDATE" => Some("UPDATE"),
        "DELETE" | "DEL" => Some("DELETE"),
        "MOVE" | "MOV" => Some("MOVE"),
        "COPY" | "CPY" => Some("COPY"),
        "SET" => Some("SET"),
        _ => None,
    })
}

pub(super) fn run_service(
    state: &AppState,
    session: &Session,
    context: ActionContext,
    command: ProgrammingCommand,
) -> Result<ProgrammingResult, ApiError> {
    run_service_with_source(state, session, context, command, "http").map_err(action_error)
}

fn run_service_with_source(
    state: &AppState,
    session: &Session,
    context: ActionContext,
    command: ProgrammingCommand,
    source: &'static str,
) -> Result<ProgrammingResult, ActionError> {
    let ports = ServerProgrammingPorts::new(state, session, source, true);
    state
        .programming
        .handle(ActionEnvelope { context, command }, &ports)
}

pub(super) fn run_snapshot(
    state: &AppState,
    session: &Session,
    context: ActionContext,
) -> Result<ProgrammingLiveSnapshot, ApiError> {
    let ports = ServerProgrammingPorts::new(state, session, "http", false);
    state
        .programming
        .snapshot(&context, &ports)
        .map_err(action_error)
}

pub(crate) fn route_osc_command_key(
    state: &AppState,
    session: &Session,
    desk_alias: &str,
    action: &str,
) -> bool {
    let Some(key) = osc_command_key(action) else {
        return false;
    };
    let Ok(_activation) = state.activation_lock.clone().try_lock_owned() else {
        publish_osc_rejection(
            state,
            session,
            "the active show is changing; retry the Programmer action".into(),
        );
        return true;
    };
    let context = ActionContext::operator(
        session.desk.id,
        session.user.id.0,
        session.id.0,
        ActionSource::Osc,
    );
    let command = ProgrammingCommand::ApplyKey {
        key,
        phase: CommandKeyPhase::Press,
        execute_policy: ExecutionPolicy::Compatibility,
    };
    match run_service_with_source(state, session, context, command, "osc") {
        Ok(result) => publish_osc_result(state, session, desk_alias, &result),
        Err(error) => publish_osc_rejection(state, session, error.message),
    }
    true
}

fn publish_osc_rejection(state: &AppState, session: &Session, error: String) {
    super::super::emit(
        state,
        "programmer_command_rejected",
        serde_json::json!({
            "desk_id":session.desk.id,
            "session_id":session.id,
            "user_id":session.user.id,
            "source":"osc",
            "error":error,
        }),
    );
}

pub(crate) fn osc_command_key(action: &str) -> Option<CommandKey> {
    if let Some(digit) = action.strip_prefix("digit-") {
        return digit
            .parse::<u8>()
            .ok()
            .filter(|digit| *digit <= 9)
            .map(CommandKey::Digit);
    }
    Some(match action {
        "set" => CommandKey::Set,
        "grp" | "group" => CommandKey::Group,
        "cue" => CommandKey::Cue,
        "record" => CommandKey::Record,
        "undo" => CommandKey::Undo,
        "clear" => CommandKey::Clear,
        "del" | "delete" => CommandKey::Delete,
        "mov" | "move" => CommandKey::Move,
        "cpy" | "copy" => CommandKey::Copy,
        "thru" => CommandKey::Thru,
        "div" => CommandKey::Divide,
        "backspace" => CommandKey::Backspace,
        "at" => CommandKey::At,
        "enter" => CommandKey::Enter,
        "preload" => CommandKey::Preload,
        "time" => CommandKey::Time,
        "delay" => CommandKey::Delay,
        "select" => CommandKey::Select,
        "plus" | "add" => CommandKey::Plus,
        "minus" | "subtract" => CommandKey::Minus,
        "dot" => CommandKey::Dot,
        _ => return None,
    })
}

fn action_error(error: ActionError) -> ApiError {
    match error.kind {
        ActionErrorKind::Invalid => ApiError::bad_request(error.message),
        ActionErrorKind::Unauthorized => ApiError::unauthorized(error.message),
        ActionErrorKind::Forbidden => ApiError::forbidden(error.message),
        ActionErrorKind::NotFound => ApiError::not_found(error.message),
        ActionErrorKind::Conflict | ActionErrorKind::Busy => ApiError::conflict(error.message),
        ActionErrorKind::Unavailable => ApiError::unavailable(error.message),
        ActionErrorKind::Internal => ApiError::internal(error.message),
    }
}
