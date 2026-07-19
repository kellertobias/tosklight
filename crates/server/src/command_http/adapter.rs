use super::events::{persist_with_warning, publish_osc_result};
use super::wire::application_choice;
use light_application::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ActionSource, ExecutionPolicy,
    ProgrammingCommand, ProgrammingExecution, ProgrammingLiveSnapshot, ProgrammingPorts,
    ProgrammingResult,
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
    let ports = ServerProgrammingPorts {
        state,
        session,
        source,
        require_unlocked: true,
    };
    state
        .programming
        .handle(ActionEnvelope { context, command }, &ports)
}

pub(super) fn run_snapshot(
    state: &AppState,
    session: &Session,
    context: ActionContext,
) -> Result<ProgrammingLiveSnapshot, ApiError> {
    let ports = ServerProgrammingPorts {
        state,
        session,
        source: "http",
        require_unlocked: false,
    };
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

struct ServerProgrammingPorts<'a> {
    state: &'a AppState,
    session: &'a Session,
    source: &'static str,
    require_unlocked: bool,
}

impl ProgrammingPorts for ServerProgrammingPorts<'_> {
    fn authorize(&self, context: &ActionContext) -> Result<(), ActionError> {
        let identity_matches = context.desk_id == self.session.desk.id
            && context.session_id == Some(self.session.id.0)
            && context.user_id == Some(self.session.user.id.0);
        if !identity_matches {
            return Err(ActionError::new(
                ActionErrorKind::Forbidden,
                "the action context does not match the authenticated operator session",
            ));
        }
        if self.require_unlocked && super::super::read_desk_lock(self.state, context.desk_id).locked
        {
            return Err(ActionError::new(
                ActionErrorKind::Conflict,
                "desk is locked",
            ));
        }
        Ok(())
    }

    fn execute(
        &self,
        _programmers: &ProgrammerRegistry,
        context: &ActionContext,
        command: &str,
        policy: ExecutionPolicy,
    ) -> ProgrammingExecution {
        let policy = match policy {
            ExecutionPolicy::AtomicProgrammer => ExistingCommandPolicy::AtomicProgrammer,
            ExecutionPolicy::Compatibility => ExistingCommandPolicy::Compatibility,
        };
        match execute_existing_command(
            self.state,
            self.session,
            command,
            self.source,
            context,
            policy,
        ) {
            ExistingCommandOutcome::Accepted {
                applied,
                persistence_warning,
            } => ProgrammingExecution::Accepted {
                applied,
                warning: persistence_warning,
            },
            ExistingCommandOutcome::ChoiceRequired { pending_choice } => {
                match application_choice(pending_choice) {
                    Ok(pending_choice) => ProgrammingExecution::ChoiceRequired { pending_choice },
                    Err(error) => ProgrammingExecution::Rejected { error },
                }
            }
            ExistingCommandOutcome::Rejected { error } => ProgrammingExecution::Rejected { error },
        }
    }

    fn persist(&self, context: &ActionContext, operation: &'static str) -> Option<String> {
        persist_with_warning(
            self.state,
            self.session,
            self.source,
            context.request_id.as_deref(),
            operation,
        )
    }

    fn capture_programmer_on_preload(&self, _context: &ActionContext) -> bool {
        self.state.configuration.read().preload_programmer_changes
    }

    fn reconcile(
        &self,
        _context: &ActionContext,
        reason: light_application::ProgrammingReconciliation,
    ) {
        let osc = self.source == "osc";
        match reason {
            light_application::ProgrammingReconciliation::SelectionChanged => {
                let source = if osc {
                    "osc_programmer_selection"
                } else {
                    "programmer_selection"
                };
                super::super::reconcile_highlight_selection(self.state, self.session, source);
            }
            light_application::ProgrammingReconciliation::CaptureModeChanged => {
                let source = if osc { "osc_preload" } else { "preload" };
                super::super::reconcile_highlight_capture_mode(self.state, self.session, source);
            }
        }
    }

    fn commit_preload(&self, _context: &ActionContext) -> Result<Option<String>, String> {
        let committed = super::super::commit_preload(self.state, self.session)?;
        Ok(committed
            .get("warnings")
            .and_then(serde_json::Value::as_array)
            .map(|warnings| {
                warnings
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .filter(|warning| !warning.is_empty()))
    }
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
