use crate::command_line::{CommandKey, CommandKeyIntent, CommandKeyPhase, command_key_intent};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, HeaderValue, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use light_core::SessionId;
use light_programmer::{CommandLineReplaceError, CommandLineState, ProgrammerSelection};
use light_wire::v2::command_line::{
    CommandAcceptedAction, CommandHttpSource, CommandKey as WireCommandKey,
    CommandKeyPhase as WireCommandKeyPhase, CommandKeyRequest, CommandLineChangedEvent,
    CommandLineResponse, CommandOperationOutcome, CommandOperationResponse,
    CommandTarget as WireCommandTarget, CueMoveCopyChoice, ExecuteCommandLineRequest,
    ReplaceCommandLineRequest,
};
use parking_lot::Mutex;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};
use uuid::Uuid;

use super::{ApiError, AppState, Session};

/// Global replay horizon for mutating command-line requests.
///
/// A global bound matters here: a per-session limit can still grow without bound when clients
/// reconnect or an installation has many users. Request IDs remain exactly-once within this
/// documented in-memory horizon and are deliberately forgotten on server restart.
const REQUEST_CACHE_LIMIT: usize = 4_096;
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

#[derive(Clone, Default)]
pub(super) struct CommandHttpState {
    operation_locks: Arc<Mutex<HashMap<Uuid, Arc<Mutex<()>>>>>,
    request_cache: Arc<Mutex<RequestCache>>,
}

impl CommandHttpState {
    pub(super) fn operation_lock(&self, desk_id: Uuid) -> Arc<Mutex<()>> {
        let mut locks = self.operation_locks.lock();
        // A lock with no outstanding clone cannot be protecting an operation. Pruning those
        // entries keeps desk churn from growing this process-lifetime map without ever splitting
        // serialization for an in-flight operation.
        locks.retain(|existing_desk_id, lock| {
            *existing_desk_id == desk_id || Arc::strong_count(lock) > 1
        });
        Arc::clone(
            locks
                .entry(desk_id)
                .or_insert_with(|| Arc::new(Mutex::new(()))),
        )
    }

    fn cached(
        &self,
        desk_id: Uuid,
        session_id: SessionId,
        request_id: &str,
        fingerprint: &RequestFingerprint,
    ) -> Result<Option<CommandOperationResponse>, ApiError> {
        self.request_cache
            .lock()
            .get(desk_id, session_id, request_id, fingerprint)
    }

    fn remember(
        &self,
        desk_id: Uuid,
        session_id: SessionId,
        request_id: String,
        fingerprint: RequestFingerprint,
        response: CommandOperationResponse,
    ) {
        self.request_cache
            .lock()
            .insert(desk_id, session_id, request_id, fingerprint, response);
    }
}

#[derive(Default)]
struct RequestCache {
    entries: HashMap<RequestCacheKey, CachedRequest>,
    order: VecDeque<RequestCacheKey>,
}

impl RequestCache {
    fn get(
        &self,
        desk_id: Uuid,
        session_id: SessionId,
        request_id: &str,
        fingerprint: &RequestFingerprint,
    ) -> Result<Option<CommandOperationResponse>, ApiError> {
        let key = RequestCacheKey {
            desk_id,
            session_id,
            request_id: request_id.to_owned(),
        };
        let Some(cached) = self.entries.get(&key) else {
            return Ok(None);
        };
        if cached.fingerprint != *fingerprint {
            return Err(ApiError::conflict(
                "request_id was already used for a different command-line operation",
            ));
        }
        Ok(Some(cached.response.clone()))
    }

    fn insert(
        &mut self,
        desk_id: Uuid,
        session_id: SessionId,
        request_id: String,
        fingerprint: RequestFingerprint,
        response: CommandOperationResponse,
    ) {
        let key = RequestCacheKey {
            desk_id,
            session_id,
            request_id,
        };
        if !self.entries.contains_key(&key) {
            self.order.push_back(key.clone());
        }
        self.entries.insert(
            key,
            CachedRequest {
                fingerprint,
                response,
            },
        );
        while self.entries.len() > REQUEST_CACHE_LIMIT {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct RequestCacheKey {
    desk_id: Uuid,
    session_id: SessionId,
    request_id: String,
}

type RequestFingerprint = [u8; 32];

struct CachedRequest {
    fingerprint: RequestFingerprint,
    response: CommandOperationResponse,
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
    request_id: Option<&str>,
    policy: ExistingCommandPolicy,
) -> ExistingCommandOutcome {
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
            super::execute_programmer_command(state, session, command)
        }
        ExistingCommandPolicy::AtomicProgrammer => {
            state
                .programmers
                .with_staged_command(session.id, |staged_programmers| {
                    let mut staged_state = state.clone();
                    staged_state.programmers = staged_programmers.clone();
                    let applied =
                        super::execute_programmer_command(&staged_state, session, command)?;
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
    let operation_lock = state.command_http.operation_lock(desk_id);
    let _operation = operation_lock.lock();
    ensure_desk_unlocked(&state, desk_id)?;
    let before = command_state(&state, &session)?;
    let after = state
        .programmers
        .replace_command_line(session.id, expected_revision, input.text)
        .map_err(replace_error)?;
    publish_command_line_change(&state, &session, &before, &after, "http", None);
    Ok(with_etag(command_line_from_state(after)))
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
    let fingerprint = fingerprint("key", &input)?;
    let session = authenticate_desk_mutation(&state, &headers, desk_id)?;
    let operation_lock = state.command_http.operation_lock(desk_id);
    let _operation = operation_lock.lock();
    ensure_desk_unlocked(&state, desk_id)?;
    if let Some(response) =
        state
            .command_http
            .cached(desk_id, session.id, &input.request_id, &fingerprint)?
    {
        return Ok(with_etag(response));
    }

    let current = command_state(&state, &session)?;
    let response = match command_key_intent(&current, key, phase) {
        CommandKeyIntent::NoOp => accepted_response(
            &state,
            &session,
            input.request_id.clone(),
            CommandAcceptedAction::IgnoredRelease,
            None,
        )?,
        CommandKeyIntent::Edit(edit) => {
            validate_command(&edit.text)?;
            let before = current;
            let mut execute = false;
            let after = state
                .programmers
                .update_command_line(session.id, |actual| {
                    let CommandKeyIntent::Edit(edit) = command_key_intent(actual, key, phase)
                    else {
                        unreachable!("editing keys retain their intent under the registry lock")
                    };
                    execute = edit.execute;
                    (edit.text, edit.target, edit.pristine)
                })
                .ok_or_else(|| ApiError::not_found("programmer command line"))?;
            publish_command_line_change(
                &state,
                &session,
                &before,
                &after,
                "http_key",
                Some(&input.request_id),
            );
            if execute {
                execute_locked(&state, &session, input.request_id.clone(), None)?
            } else {
                accepted_response(
                    &state,
                    &session,
                    input.request_id.clone(),
                    CommandAcceptedAction::Edited,
                    None,
                )?
            }
        }
        CommandKeyIntent::Clear => clear_locked(&state, &session, input.request_id.clone())?,
        CommandKeyIntent::Undo => undo_locked(&state, &session, input.request_id.clone())?,
        CommandKeyIntent::Preload => preload_locked(&state, &session, input.request_id.clone())?,
        CommandKeyIntent::Shift { pressed } => {
            super::emit(
                &state,
                "command_key_phase",
                serde_json::json!({
                    "desk_id":session.desk.id,
                    "session_id":session.id,
                    "user_id":session.user.id,
                    "key":"SHIFT",
                    "phase":if pressed { "press" } else { "release" },
                    "source":"http",
                    "request_id":input.request_id,
                }),
            );
            accepted_response(
                &state,
                &session,
                input.request_id.clone(),
                if pressed {
                    CommandAcceptedAction::ShiftPressed
                } else {
                    CommandAcceptedAction::ShiftReleased
                },
                None,
            )?
        }
    };
    state.command_http.remember(
        desk_id,
        session.id,
        input.request_id,
        fingerprint,
        response.clone(),
    );
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
    let fingerprint = fingerprint("execute", &input)?;
    let session = authenticate_desk_mutation(&state, &headers, desk_id)?;
    let operation_lock = state.command_http.operation_lock(desk_id);
    let _operation = operation_lock.lock();
    ensure_desk_unlocked(&state, desk_id)?;
    if let Some(response) =
        state
            .command_http
            .cached(desk_id, session.id, &input.request_id, &fingerprint)?
    {
        return Ok(with_etag(response));
    }
    let response = execute_locked(
        &state,
        &session,
        input.request_id.clone(),
        input.command.as_deref(),
    )?;
    state.command_http.remember(
        desk_id,
        session.id,
        input.request_id,
        fingerprint,
        response.clone(),
    );
    Ok(with_etag(response))
}

fn execute_locked(
    state: &AppState,
    session: &Session,
    request_id: String,
    supplied_command: Option<&str>,
) -> Result<CommandOperationResponse, ApiError> {
    let selection_before = selection(state, session)?;
    let command_before = command_state(state, session)?;
    let command = supplied_command
        .unwrap_or_else(|| command_before.visible_text())
        .to_owned();
    let (audit_command, sensitive) = super::command_audit_projection(&command);
    let outcome = match execute_existing_command(
        state,
        session,
        &command,
        "http",
        Some(&request_id),
        ExistingCommandPolicy::AtomicProgrammer,
    ) {
        ExistingCommandOutcome::Accepted {
            applied,
            persistence_warning,
        } => {
            let after = command_state(state, session)?;
            publish_command_line_change(
                state,
                session,
                &command_before,
                &after,
                "http",
                Some(&request_id),
            );
            reconcile_selection_change(state, session, selection_before);
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
            super::emit(
                state,
                "programmer_changed",
                serde_json::json!({
                    "session_id":session.id,
                    "desk_id":session.desk.id,
                    "user_id":session.user.id,
                    "command":"programmer.execute",
                    "source":"http",
                    "request_id":request_id,
                }),
            );
            CommandOperationOutcome::Accepted {
                action: CommandAcceptedAction::Executed,
                applied: Some(applied),
                warning: persistence_warning,
            }
        }
        ExistingCommandOutcome::ChoiceRequired { pending_choice } => {
            retain_supplied_command(
                state,
                session,
                &command_before,
                supplied_command,
                &request_id,
            )?;
            let audit_choice = if sensitive {
                serde_json::json!({"type":"cue_move_copy","redacted":true})
            } else {
                pending_choice.clone()
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
                    "pending_choice":audit_choice,
                    "source":"http",
                }),
            );
            let pending_choice = serde_json::from_value(pending_choice)
                .map_err(|error| ApiError::internal(error.to_string()))?;
            CommandOperationOutcome::ChoiceRequired { pending_choice }
        }
        ExistingCommandOutcome::Rejected { error } => {
            retain_supplied_command(
                state,
                session,
                &command_before,
                supplied_command,
                &request_id,
            )?;
            let retained_error = if sensitive {
                "Sensitive input omitted"
            } else {
                error.as_str()
            };
            super::emit(
                state,
                "programmer_command_rejected",
                serde_json::json!({
                    "request_id":request_id,
                    "desk_id":session.desk.id,
                    "session_id":session.id,
                    "user_id":session.user.id,
                    "command":audit_command,
                    "error":retained_error,
                    "source":"http",
                }),
            );
            CommandOperationOutcome::Rejected { error }
        }
    };
    Ok(CommandOperationResponse {
        request_id,
        outcome,
        command_line: command_line_response(state, session)?,
    })
}

fn retain_supplied_command(
    state: &AppState,
    session: &Session,
    before: &CommandLineState,
    supplied_command: Option<&str>,
    request_id: &str,
) -> Result<(), ApiError> {
    let Some(supplied_command) = supplied_command else {
        return Ok(());
    };
    let supplied_command = supplied_command.to_owned();
    let after = state
        .programmers
        .update_command_line(session.id, |current| {
            let pristine = supplied_command.trim().is_empty()
                || supplied_command
                    .trim()
                    .eq_ignore_ascii_case(current.target.as_str());
            (supplied_command, current.target, pristine)
        })
        .ok_or_else(|| ApiError::not_found("programmer command line"))?;
    publish_command_line_change(state, session, before, &after, "http", Some(request_id));
    Ok(())
}

fn clear_locked(
    state: &AppState,
    session: &Session,
    request_id: String,
) -> Result<CommandOperationResponse, ApiError> {
    let command_before = command_state(state, session)?;
    let selection_before = selection(state, session)?;
    let action = state
        .programmers
        .with_staged_transaction(session.id, |staged| {
            let programmer = staged
                .get(session.id)
                .ok_or_else(|| "programmer does not exist".to_owned())?;
            let action = if programmer.blind {
                staged.clear_preload_pending(session.id);
                CommandAcceptedAction::ClearedPreload
            } else if !programmer.selected.is_empty() {
                staged.select(session.id, []);
                CommandAcceptedAction::ClearedSelection
            } else if !programmer.values.is_empty() || !programmer.group_values.is_empty() {
                staged.clear_values(session.id);
                CommandAcceptedAction::ClearedValues
            } else if command_before.pristine {
                CommandAcceptedAction::NoChange
            } else {
                CommandAcceptedAction::ClearedCommandLine
            };
            staged
                .update_command_line(session.id, |current| (String::new(), current.target, true))
                .ok_or_else(|| "programmer command line does not exist".to_owned())?;
            Ok::<_, String>(action)
        })
        .map_err(ApiError::not_found)?;
    let warning = match action {
        CommandAcceptedAction::ClearedPreload => persist_with_warning(
            state,
            session,
            "http",
            Some(&request_id),
            "programmer.clear_preload",
        ),
        CommandAcceptedAction::ClearedSelection => persist_with_warning(
            state,
            session,
            "http",
            Some(&request_id),
            "programmer.clear_selection",
        ),
        CommandAcceptedAction::ClearedValues => persist_with_warning(
            state,
            session,
            "http",
            Some(&request_id),
            "programmer.clear_values",
        ),
        _ => None,
    };
    let command_after = command_state(state, session)?;
    publish_command_line_change(
        state,
        session,
        &command_before,
        &command_after,
        "http_key",
        Some(&request_id),
    );
    reconcile_selection_change(state, session, selection_before);
    if matches!(
        action,
        CommandAcceptedAction::ClearedPreload
            | CommandAcceptedAction::ClearedSelection
            | CommandAcceptedAction::ClearedValues
    ) {
        super::emit(
            state,
            "programmer_changed",
            serde_json::json!({
                "desk_id":session.desk.id,
                "session_id":session.id,
                "user_id":session.user.id,
                "command":"programmer.clear_step",
                "source":"http",
                "request_id":request_id,
            }),
        );
    }
    accepted_response_with_warning(state, session, request_id, action, None, warning)
}

fn undo_locked(
    state: &AppState,
    session: &Session,
    request_id: String,
) -> Result<CommandOperationResponse, ApiError> {
    let selection_before = selection(state, session)?;
    let changed = state
        .programmers
        .with_staged_transaction(session.id, |staged| {
            Ok::<_, String>(staged.undo(session.id))
        })
        .map_err(ApiError::not_found)?;
    let mut warning = None;
    if changed {
        warning =
            persist_with_warning(state, session, "http", Some(&request_id), "programmer.undo");
        reconcile_selection_change(state, session, selection_before);
        super::emit(
            state,
            "programmer_changed",
            serde_json::json!({
                "desk_id":session.desk.id,
                "session_id":session.id,
                "user_id":session.user.id,
                "command":"programmer.undo",
                "source":"http",
                "request_id":request_id,
            }),
        );
    }
    accepted_response_with_warning(
        state,
        session,
        request_id,
        if changed {
            CommandAcceptedAction::Undone
        } else {
            CommandAcceptedAction::NoChange
        },
        None,
        warning,
    )
}

fn preload_locked(
    state: &AppState,
    session: &Session,
    request_id: String,
) -> Result<CommandOperationResponse, ApiError> {
    let programmer = state
        .programmers
        .get(session.id)
        .ok_or_else(|| ApiError::not_found("programmer"))?;
    let (action, warning) = if programmer.blind {
        match super::commit_preload(state, session) {
            Ok(committed) => {
                let warning = committed
                    .get("warnings")
                    .and_then(serde_json::Value::as_array)
                    .map(|warnings| {
                        warnings
                            .iter()
                            .filter_map(serde_json::Value::as_str)
                            .collect::<Vec<_>>()
                            .join("; ")
                    })
                    .filter(|warning| !warning.is_empty());
                (CommandAcceptedAction::PreloadCommitted, warning)
            }
            Err(error) => {
                super::emit(
                    state,
                    "preload_failed",
                    serde_json::json!({
                        "desk_id":session.desk.id,
                        "session_id":session.id,
                        "user_id":session.user.id,
                        "request_id":request_id,
                        "source":"http",
                        "error":error,
                    }),
                );
                return Ok(CommandOperationResponse {
                    request_id,
                    outcome: CommandOperationOutcome::Rejected { error },
                    command_line: command_line_response(state, session)?,
                });
            }
        }
    } else {
        let capture_programmer = state.configuration.read().preload_programmer_changes;
        state
            .programmers
            .with_staged_transaction(session.id, |staged| {
                if staged.arm_preload(session.id, capture_programmer) {
                    Ok::<_, String>(())
                } else {
                    Err("programmer does not exist".to_owned())
                }
            })
            .map_err(ApiError::not_found)?;
        let warning =
            persist_with_warning(state, session, "http", Some(&request_id), "preload.enter");
        super::reconcile_highlight_capture_mode(state, session, "preload");
        super::emit(
            state,
            "programmer_changed",
            serde_json::json!({
                "desk_id":session.desk.id,
                "session_id":session.id,
                "user_id":session.user.id,
                "preload_armed":true,
                "source":"http",
                "request_id":request_id,
            }),
        );
        (CommandAcceptedAction::PreloadEntered, warning)
    };
    accepted_response_with_warning(state, session, request_id, action, None, warning)
}

fn accepted_response(
    state: &AppState,
    session: &Session,
    request_id: String,
    action: CommandAcceptedAction,
    applied: Option<usize>,
) -> Result<CommandOperationResponse, ApiError> {
    accepted_response_with_warning(state, session, request_id, action, applied, None)
}

fn accepted_response_with_warning(
    state: &AppState,
    session: &Session,
    request_id: String,
    action: CommandAcceptedAction,
    applied: Option<usize>,
    warning: Option<String>,
) -> Result<CommandOperationResponse, ApiError> {
    Ok(CommandOperationResponse {
        request_id,
        outcome: CommandOperationOutcome::Accepted {
            action,
            applied,
            warning,
        },
        command_line: command_line_response(state, session)?,
    })
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

fn selection(state: &AppState, session: &Session) -> Result<ProgrammerSelection, ApiError> {
    state
        .programmers
        .selection(session.id)
        .ok_or_else(|| ApiError::not_found("programmer selection"))
}

fn reconcile_selection_change(state: &AppState, session: &Session, before: ProgrammerSelection) {
    if state
        .programmers
        .selection(session.id)
        .is_some_and(|after| after.revision != before.revision)
    {
        super::reconcile_highlight_selection(state, session, "programmer_selection");
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

fn replace_error(error: CommandLineReplaceError) -> ApiError {
    match error {
        CommandLineReplaceError::UnknownSession => ApiError::not_found("programmer command line"),
        CommandLineReplaceError::RevisionConflict { expected, actual } => ApiError::conflict(
            format!("command-line revision conflict: expected {expected}, actual {actual}"),
        ),
    }
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

fn fingerprint<T: Serialize>(kind: &str, input: &T) -> Result<RequestFingerprint, ApiError> {
    let payload =
        serde_json::to_vec(input).map_err(|error| ApiError::internal(error.to_string()))?;
    let mut digest = Sha256::new();
    digest.update(kind.as_bytes());
    digest.update([0]);
    digest.update(payload);
    Ok(digest.finalize().into())
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
