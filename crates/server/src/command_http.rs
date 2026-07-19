use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, HeaderValue, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use light_application::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ActionSource,
    CueMoveCopyChoice as ApplicationCueChoice, CueTransferOperation as ApplicationCueOperation,
    ExecutionPolicy, ProgrammingAction, ProgrammingChoiceOption,
    ProgrammingChoiceOptionId as ApplicationChoiceOptionId, ProgrammingCommand,
    ProgrammingExecution, ProgrammingOutcome, ProgrammingPorts, ProgrammingResult,
};
use light_programmer::command_line::{CommandKey, CommandKeyPhase};
use light_programmer::{CommandLineState, ProgrammerRegistry};
use light_wire::v2::command_line::{
    CommandAcceptedAction, CommandChoiceOption as WireChoiceOption,
    CommandChoiceOptionId as WireChoiceOptionId, CommandHttpSource, CommandKey as WireCommandKey,
    CommandKeyPhase as WireCommandKeyPhase, CommandKeyRequest, CommandLineChangedEvent,
    CommandLineResponse, CommandOperationOutcome, CommandOperationResponse,
    CommandTarget as WireCommandTarget, CueMoveCopyChoice, CueMoveCopyChoiceType as WireChoiceType,
    CueTransferOperation as WireCueOperation, ExecuteCommandLineRequest, ReplaceCommandLineRequest,
};
use serde::Serialize;
use uuid::Uuid;

use super::{ApiError, AppState, Session};

const REQUEST_ID_LIMIT: usize = 128;
const COMMAND_LINE_LIMIT: usize = 16 * 1024;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v2/desks/{desk_id}/command-line",
            get(get_command_line).put(put_command_line),
        )
        .route(
            "/api/v2/desks/{desk_id}/command-line/keys",
            post(apply_command_key),
        )
        .route(
            "/api/v2/desks/{desk_id}/command-line/execute",
            post(execute_command_line),
        )
        .layer(DefaultBodyLimit::max(32 * 1024))
}

pub(super) enum ExistingCommandOutcome {
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
pub(super) enum ExistingCommandPolicy {
    /// Temporary adapter for the legacy WebSocket and OSC grammar while owning services migrate.
    Compatibility,
    /// Public v2 guarantee: only commands whose complete mutation is isolated in Programmer.
    AtomicProgrammer,
}

/// Execute the existing command grammar while keeping transport envelopes out of the domain path.
/// WebSocket, HTTP and, during Stage 3, OSC all adapt through this function.
pub(super) fn execute_existing_command(
    state: &AppState,
    session: &Session,
    command: &str,
    source: &str,
    context: &ActionContext,
    policy: ExistingCommandPolicy,
) -> ExistingCommandOutcome {
    let request_id = context.request_id.as_deref();
    if matches!(policy, ExistingCommandPolicy::AtomicProgrammer) {
        let family = match compatibility_only_family(command) {
            Ok(family) => family,
            Err(error) => {
                super::record_command_history(
                    state, session, command, "rejected", &error, source, request_id,
                );
                return ExistingCommandOutcome::Rejected { error };
            }
        };
        if let Some(family) = family {
            let error = format!(
                "{family} commands are not yet available through the atomic command-line HTTP API"
            );
            super::record_command_history(
                state, session, command, "rejected", &error, source, request_id,
            );
            return ExistingCommandOutcome::Rejected { error };
        }
    }
    if let Some(pending_choice) = super::pending_cue_transfer_choice(command) {
        return ExistingCommandOutcome::ChoiceRequired { pending_choice };
    }
    let result = match policy {
        ExistingCommandPolicy::Compatibility => {
            // Compatibility families may refresh the shared Engine and all live selections. They
            // must not hold one user's mutation gate while that cross-user reconciliation acquires
            // every user gate, or concurrent show commands can deadlock in opposite directions.
            super::execute_programmer_command_from(state, session, command, context)
        }
        ExistingCommandPolicy::AtomicProgrammer => {
            state
                .programmers
                .with_staged_command(session.id, |staged_programmers| {
                    let mut staged_state = state.clone();
                    staged_state.programmers = staged_programmers.clone();
                    let applied = super::execute_programmer_command_from(
                        &staged_state,
                        session,
                        command,
                        context,
                    )?;
                    staged_programmers
                        .update_command_line(session.id, |current| {
                            (String::new(), current.target, true)
                        })
                        .ok_or_else(|| "programmer command line does not exist".to_owned())?;
                    Ok::<usize, String>(applied)
                })
        }
    };
    match result {
        Ok(applied) => {
            let persistence_warning =
                persist_with_warning(state, session, source, request_id, "programmer.execute");
            let feedback = persistence_warning.as_ref().map_or_else(
                || format!("Applied to {applied} target(s)"),
                |warning| format!("Applied to {applied} target(s); {warning}"),
            );
            super::record_command_history(
                state, session, command, "accepted", &feedback, source, request_id,
            );
            ExistingCommandOutcome::Accepted {
                applied,
                persistence_warning,
            }
        }
        Err(error) => {
            super::record_command_history(
                state, session, command, "rejected", &error, source, request_id,
            );
            ExistingCommandOutcome::Rejected { error }
        }
    }
}

fn compatibility_only_family(command: &str) -> Result<Option<&'static str>, String> {
    let Some(family) = super::normalized_programmer_command_family(command)? else {
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

async fn get_command_line(
    State(state): State<AppState>,
    Path(desk_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let session = authenticate_desk(&state, &headers, desk_id)?;
    let response = command_line_response(&state, &session)?;
    Ok(with_etag(response))
}

async fn put_command_line(
    State(state): State<AppState>,
    Path(desk_id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<ReplaceCommandLineRequest>,
) -> Result<Response, ApiError> {
    validate_command(&input.text)?;
    let session = authenticate_desk_mutation(&state, &headers, desk_id)?;
    let expected_revision = super::parse_if_match(&headers)?;
    let context = http_context(&session, None).with_expected_revision(expected_revision);
    let result = run_service(
        &state,
        &session,
        context,
        ProgrammingCommand::ReplaceCommandLine {
            text: input.text,
            expected_revision,
        },
    )?;
    publish_service_result(&state, &session, &result, "http", None, None);
    Ok(with_etag(command_line_from_state(result.command_line)))
}

async fn apply_command_key(
    State(state): State<AppState>,
    Path(desk_id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<CommandKeyRequest>,
) -> Result<Response, ApiError> {
    validate_request_id(&input.request_id)?;
    let key = command_key(input.key);
    let phase = command_key_phase(input.phase);
    let session = authenticate_desk_mutation(&state, &headers, desk_id)?;
    let context = http_context(&session, Some(&input.request_id));
    let result = run_service(
        &state,
        &session,
        context,
        ProgrammingCommand::ApplyKey {
            key,
            phase,
            execute_policy: ExecutionPolicy::AtomicProgrammer,
        },
    )?;
    publish_service_result(
        &state,
        &session,
        &result,
        "http_key",
        Some(&input.request_id),
        None,
    );
    let response = operation_response(input.request_id, result)?;
    Ok(with_etag(response))
}

async fn execute_command_line(
    State(state): State<AppState>,
    Path(desk_id): Path<Uuid>,
    headers: HeaderMap,
    Json(input): Json<ExecuteCommandLineRequest>,
) -> Result<Response, ApiError> {
    validate_request_id(&input.request_id)?;
    if let Some(command) = &input.command {
        validate_command(command)?;
    }
    let session = authenticate_desk_mutation(&state, &headers, desk_id)?;
    let context = http_context(&session, Some(&input.request_id));
    let result = run_service(
        &state,
        &session,
        context,
        ProgrammingCommand::Execute {
            command: input.command.clone(),
            policy: ExecutionPolicy::AtomicProgrammer,
        },
    )?;
    publish_service_result(
        &state,
        &session,
        &result,
        "http",
        Some(&input.request_id),
        input.command.as_deref(),
    );
    let response = operation_response(input.request_id, result)?;
    Ok(with_etag(response))
}

fn run_service(
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
    };
    state
        .programming
        .handle(ActionEnvelope { context, command }, &ports)
}

pub(super) fn route_osc_command_key(
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
        Err(error) => super::emit(
            state,
            "programmer_command_rejected",
            serde_json::json!({
                "desk_id":session.desk.id,
                "session_id":session.id,
                "user_id":session.user.id,
                "source":"osc",
                "error":error.message,
            }),
        ),
    }
    true
}

pub(super) fn osc_command_key(action: &str) -> Option<CommandKey> {
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

fn publish_osc_result(
    state: &AppState,
    session: &Session,
    desk_alias: &str,
    result: &ProgrammingResult,
) {
    if result.replayed {
        return;
    }
    if result.selection_revision_before != result.selection_revision {
        super::reconcile_highlight_selection(state, session, "osc_programmer_selection");
    }
    match &result.outcome {
        ProgrammingOutcome::Accepted { action, .. } => {
            publish_osc_accepted(state, session, desk_alias, result, *action)
        }
        ProgrammingOutcome::ChoiceRequired { pending_choice } => super::emit(
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
        ProgrammingOutcome::Rejected { error } => {
            let (command, sensitive) =
                super::command_audit_projection(result.command_line_before.visible_text());
            super::emit(
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
    }
}

fn publish_osc_accepted(
    state: &AppState,
    session: &Session,
    desk_alias: &str,
    result: &ProgrammingResult,
    action: ProgrammingAction,
) {
    if action == ProgrammingAction::Edited {
        let _ = super::persist_programmer(state, session);
    }
    if action == ProgrammingAction::PreloadEntered {
        super::reconcile_highlight_capture_mode(state, session, "osc_preload");
    }
    if action == ProgrammingAction::Executed {
        let (command, _) =
            super::command_audit_projection(result.command_line_before.visible_text());
        super::emit(
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
    super::emit(
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
        }),
    );
}

fn http_context(session: &Session, request_id: Option<&str>) -> ActionContext {
    let context = ActionContext::operator(
        session.desk.id,
        session.user.id.0,
        session.id.0,
        ActionSource::Http,
    );
    request_id.map_or(context.clone(), |id| context.with_request_id(id))
}

fn operation_response(
    request_id: String,
    result: ProgrammingResult,
) -> Result<CommandOperationResponse, ApiError> {
    let outcome = match result.outcome {
        ProgrammingOutcome::Accepted {
            action,
            applied,
            warning,
        } => CommandOperationOutcome::Accepted {
            action: wire_action(action),
            applied,
            warning,
        },
        ProgrammingOutcome::ChoiceRequired { pending_choice } => {
            CommandOperationOutcome::ChoiceRequired {
                pending_choice: wire_choice(pending_choice),
            }
        }
        ProgrammingOutcome::Rejected { error } => CommandOperationOutcome::Rejected { error },
    };
    Ok(CommandOperationResponse {
        request_id,
        outcome,
        command_line: command_line_from_state(result.command_line),
    })
}

const fn wire_action(action: ProgrammingAction) -> CommandAcceptedAction {
    match action {
        ProgrammingAction::Edited => CommandAcceptedAction::Edited,
        ProgrammingAction::Executed => CommandAcceptedAction::Executed,
        ProgrammingAction::ClearedCommandLine => CommandAcceptedAction::ClearedCommandLine,
        ProgrammingAction::ClearedPreload => CommandAcceptedAction::ClearedPreload,
        ProgrammingAction::ClearedSelection => CommandAcceptedAction::ClearedSelection,
        ProgrammingAction::ClearedValues => CommandAcceptedAction::ClearedValues,
        ProgrammingAction::Undone => CommandAcceptedAction::Undone,
        ProgrammingAction::NoChange => CommandAcceptedAction::NoChange,
        ProgrammingAction::PreloadEntered => CommandAcceptedAction::PreloadEntered,
        ProgrammingAction::PreloadCommitted => CommandAcceptedAction::PreloadCommitted,
        ProgrammingAction::ShiftPressed => CommandAcceptedAction::ShiftPressed,
        ProgrammingAction::ShiftReleased => CommandAcceptedAction::ShiftReleased,
        ProgrammingAction::IgnoredRelease => CommandAcceptedAction::IgnoredRelease,
    }
}

fn wire_choice(choice: ApplicationCueChoice) -> CueMoveCopyChoice {
    CueMoveCopyChoice {
        choice_type: WireChoiceType::CueMoveCopy,
        operation: match choice.operation {
            ApplicationCueOperation::Copy => WireCueOperation::Copy,
            ApplicationCueOperation::Move => WireCueOperation::Move,
        },
        command: choice.command,
        options: choice
            .options
            .into_iter()
            .map(|option| WireChoiceOption {
                id: match option.id {
                    ApplicationChoiceOptionId::Plain => WireChoiceOptionId::Plain,
                    ApplicationChoiceOptionId::Status => WireChoiceOptionId::Status,
                },
                label: option.label,
                command: option.command,
            })
            .collect(),
        cancel_label: choice.cancel_label,
    }
}

fn application_choice(value: serde_json::Value) -> Result<ApplicationCueChoice, String> {
    let choice: CueMoveCopyChoice =
        serde_json::from_value(value).map_err(|error| error.to_string())?;
    Ok(ApplicationCueChoice {
        operation: match choice.operation {
            WireCueOperation::Copy => ApplicationCueOperation::Copy,
            WireCueOperation::Move => ApplicationCueOperation::Move,
        },
        command: choice.command,
        options: choice
            .options
            .into_iter()
            .map(|option| ProgrammingChoiceOption {
                id: match option.id {
                    WireChoiceOptionId::Plain => ApplicationChoiceOptionId::Plain,
                    WireChoiceOptionId::Status => ApplicationChoiceOptionId::Status,
                },
                label: option.label,
                command: option.command,
            })
            .collect(),
        cancel_label: choice.cancel_label,
    })
}

fn publish_service_result(
    state: &AppState,
    session: &Session,
    result: &ProgrammingResult,
    source: &str,
    request_id: Option<&str>,
    supplied_command: Option<&str>,
) {
    if result.replayed {
        return;
    }
    publish_command_line_change(
        state,
        session,
        &result.command_line_before,
        &result.command_line,
        source,
        request_id,
    );
    if result.selection_revision_before != result.selection_revision {
        super::reconcile_highlight_selection(state, session, "programmer_selection");
    }
    publish_operation_event(state, session, result, request_id, supplied_command);
}

struct ServerProgrammingPorts<'a> {
    state: &'a AppState,
    session: &'a Session,
    source: &'static str,
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
        if super::read_desk_lock(self.state, context.desk_id).locked {
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

    fn commit_preload(&self, _context: &ActionContext) -> Result<Option<String>, String> {
        let committed = super::commit_preload(self.state, self.session)?;
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
    if matches!(
        action,
        ProgrammingAction::ShiftPressed | ProgrammingAction::ShiftReleased
    ) {
        super::emit(
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
        return;
    }
    if action == ProgrammingAction::PreloadEntered {
        super::reconcile_highlight_capture_mode(state, session, "preload");
    }
    if action == ProgrammingAction::Executed {
        super::emit(
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
        super::emit(
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
            }),
        );
    }
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
    let (audit_command, sensitive) = super::command_audit_projection(command);
    let pending_choice = if sensitive {
        serde_json::json!({"type":"cue_move_copy","redacted":true})
    } else {
        serde_json::to_value(wire_choice(choice.clone()))
            .expect("the application Cue choice satisfies the wire contract")
    };
    super::emit(
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
    let (audit_command, sensitive) = super::command_audit_projection(command);
    super::emit(
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
        _ => "programmer.command_line",
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

fn persist_with_warning(
    state: &AppState,
    session: &Session,
    source: &str,
    request_id: Option<&str>,
    operation: &str,
) -> Option<String> {
    let error = super::persist_programmer(state, session).err()?;
    let warning = format!(
        "the operation succeeded but Programmer persistence failed: {}",
        error.message
    );
    super::emit(
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

fn authenticate_desk(
    state: &AppState,
    headers: &HeaderMap,
    desk_id: Uuid,
) -> Result<Session, ApiError> {
    let session = super::authenticate(state, headers)?;
    if session.desk.id != desk_id {
        return Err(ApiError::forbidden(
            "the authenticated session does not belong to this desk",
        ));
    }
    Ok(session)
}

fn authenticate_desk_mutation(
    state: &AppState,
    headers: &HeaderMap,
    desk_id: Uuid,
) -> Result<Session, ApiError> {
    let session = authenticate_desk(state, headers, desk_id)?;
    ensure_desk_unlocked(state, desk_id)?;
    Ok(session)
}

fn ensure_desk_unlocked(state: &AppState, desk_id: Uuid) -> Result<(), ApiError> {
    if super::read_desk_lock(state, desk_id).locked {
        Err(ApiError::conflict("desk is locked"))
    } else {
        Ok(())
    }
}

fn command_state(state: &AppState, session: &Session) -> Result<CommandLineState, ApiError> {
    state
        .programmers
        .command_line_state(session.id)
        .ok_or_else(|| ApiError::not_found("programmer command line"))
}

fn command_line_response(
    state: &AppState,
    session: &Session,
) -> Result<CommandLineResponse, ApiError> {
    command_state(state, session).map(command_line_from_state)
}

fn command_line_from_state(state: CommandLineState) -> CommandLineResponse {
    let text = state.visible_text().to_owned();
    let pending_choice = super::pending_cue_transfer_choice(&text).map(|choice| {
        serde_json::from_value::<CueMoveCopyChoice>(choice)
            .expect("the server's Cue transfer choice must satisfy the v2 wire contract")
    });
    CommandLineResponse {
        text,
        target: match state.target {
            light_programmer::CommandTarget::Fixture => WireCommandTarget::Fixture,
            light_programmer::CommandTarget::Group => WireCommandTarget::Group,
        },
        pristine: state.pristine,
        revision: state.revision,
        pending_choice,
    }
}

const fn command_key_phase(phase: WireCommandKeyPhase) -> CommandKeyPhase {
    match phase {
        WireCommandKeyPhase::Press => CommandKeyPhase::Press,
        WireCommandKeyPhase::Release => CommandKeyPhase::Release,
    }
}

const fn command_key(key: WireCommandKey) -> CommandKey {
    match key {
        WireCommandKey::Set => CommandKey::Set,
        WireCommandKey::Group => CommandKey::Group,
        WireCommandKey::Cue => CommandKey::Cue,
        WireCommandKey::Undo => CommandKey::Undo,
        WireCommandKey::Clear => CommandKey::Clear,
        WireCommandKey::Delete => CommandKey::Delete,
        WireCommandKey::Move => CommandKey::Move,
        WireCommandKey::Copy => CommandKey::Copy,
        WireCommandKey::Thru => CommandKey::Thru,
        WireCommandKey::Divide => CommandKey::Divide,
        WireCommandKey::Backspace => CommandKey::Backspace,
        WireCommandKey::At => CommandKey::At,
        WireCommandKey::Enter => CommandKey::Enter,
        WireCommandKey::Preload => CommandKey::Preload,
        WireCommandKey::Record => CommandKey::Record,
        WireCommandKey::Escape => CommandKey::Escape,
        WireCommandKey::Shift => CommandKey::Shift,
        WireCommandKey::Time => CommandKey::Time,
        WireCommandKey::Select => CommandKey::Select,
        WireCommandKey::Plus => CommandKey::Plus,
        WireCommandKey::Minus => CommandKey::Minus,
        WireCommandKey::Dot => CommandKey::Dot,
        WireCommandKey::Digit0 => CommandKey::Digit(0),
        WireCommandKey::Digit1 => CommandKey::Digit(1),
        WireCommandKey::Digit2 => CommandKey::Digit(2),
        WireCommandKey::Digit3 => CommandKey::Digit(3),
        WireCommandKey::Digit4 => CommandKey::Digit(4),
        WireCommandKey::Digit5 => CommandKey::Digit(5),
        WireCommandKey::Digit6 => CommandKey::Digit(6),
        WireCommandKey::Digit7 => CommandKey::Digit(7),
        WireCommandKey::Digit8 => CommandKey::Digit(8),
        WireCommandKey::Digit9 => CommandKey::Digit(9),
    }
}

fn publish_command_line_change(
    state: &AppState,
    session: &Session,
    before: &CommandLineState,
    after: &CommandLineState,
    source: &str,
    request_id: Option<&str>,
) {
    if before == after {
        return;
    }
    super::emit_update_armed_transition(
        state,
        session,
        super::command_line_arms_update(before.visible_text()),
        super::command_line_arms_update(after.visible_text()),
        source,
    );
    let (retained_text, sensitive) = super::command_audit_projection(after.visible_text());
    let event = CommandLineChangedEvent {
        desk_id: session.desk.id,
        session_id: session.id.0,
        user_id: session.user.id.0,
        text: if sensitive {
            retained_text
        } else {
            after.visible_text().to_owned()
        },
        target: match after.target {
            light_programmer::CommandTarget::Fixture => WireCommandTarget::Fixture,
            light_programmer::CommandTarget::Group => WireCommandTarget::Group,
        },
        pristine: after.pristine,
        revision: after.revision,
        source: match source {
            "http" => CommandHttpSource::Http,
            "http_key" => CommandHttpSource::HttpKey,
            _ => unreachable!("the command HTTP adapter has a bounded source enum"),
        },
        request_id: request_id.map(str::to_owned),
        redacted: sensitive,
    };
    super::emit(
        state,
        "command_line_changed",
        serde_json::to_value(event).expect("command-line wire events serialize"),
    );
}

fn validate_request_id(request_id: &str) -> Result<(), ApiError> {
    if request_id.trim().is_empty()
        || request_id.len() > REQUEST_ID_LIMIT
        || request_id.chars().any(char::is_control)
    {
        return Err(ApiError::bad_request(
            "request_id must contain 1-128 printable bytes",
        ));
    }
    Ok(())
}

fn validate_command(command: &str) -> Result<(), ApiError> {
    if command.len() > COMMAND_LINE_LIMIT {
        return Err(ApiError::bad_request(
            "command line must not exceed 16384 bytes",
        ));
    }
    Ok(())
}

fn with_etag<T>(value: T) -> Response
where
    T: Serialize + HasCommandRevision,
{
    let revision = value.command_revision();
    let mut response = Json(value).into_response();
    response.headers_mut().insert(
        header::ETAG,
        HeaderValue::from_str(&format!("\"{revision}\""))
            .expect("a numeric command revision always forms a valid ETag"),
    );
    response
}

trait HasCommandRevision {
    fn command_revision(&self) -> u64;
}

impl HasCommandRevision for CommandLineResponse {
    fn command_revision(&self) -> u64 {
        self.revision
    }
}

impl HasCommandRevision for CommandOperationResponse {
    fn command_revision(&self) -> u64 {
        self.command_line.revision
    }
}

#[cfg(test)]
#[path = "command_http/unit_tests.rs"]
mod unit_tests;
