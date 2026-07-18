use std::time::{Duration, Instant};

use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
};
use light_core::SessionId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::super::{ApiError, AppState, Session, authenticate, emit, persist_programmer};

const FILE_INPUT_CONTEXT_TTL: Duration = Duration::from_secs(120);

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FileInputAction {
    Rename,
    Copy,
    Move,
    Delete,
}

#[derive(Clone)]
pub(crate) struct FileInputContext {
    pub(crate) instance_id: String,
    pub(crate) action: FileInputAction,
    pub(crate) session_id: SessionId,
    pub(crate) desk_id: Uuid,
    pub(crate) expires_at: Instant,
}

#[derive(Serialize)]
pub(super) struct FileInputContextResponse {
    instance_id: String,
    action: FileInputAction,
    session_id: SessionId,
    desk_id: Uuid,
    expires_in_millis: u128,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FileInputOrigin {
    Pending,
    Toolbar,
}

#[derive(Deserialize)]
pub(super) struct ClaimFileInput {
    instance_id: String,
    action: FileInputAction,
    origin: FileInputOrigin,
}

#[derive(Default, Deserialize)]
pub(super) struct FileInputQuery {
    instance_id: Option<String>,
}

fn context_response(context: &FileInputContext) -> FileInputContextResponse {
    FileInputContextResponse {
        instance_id: context.instance_id.clone(),
        action: context.action,
        session_id: context.session_id,
        desk_id: context.desk_id,
        expires_in_millis: context
            .expires_at
            .saturating_duration_since(Instant::now())
            .as_millis(),
    }
}

fn prune_input_contexts(state: &AppState) {
    let now = Instant::now();
    state
        .file_input_contexts
        .lock()
        .retain(|_, context| context.expires_at > now);
}

pub(crate) fn try_claim_input_context(
    state: &AppState,
    context: FileInputContext,
    prepare: impl FnOnce() -> Result<(), ApiError>,
) -> Result<(), ApiError> {
    let mut contexts = state.file_input_contexts.lock();
    contexts.retain(|_, current| current.expires_at > Instant::now());
    if let Some(existing) = contexts.get(&context.desk_id)
        && existing.instance_id != context.instance_id
    {
        return Err(ApiError::conflict(
            "another File Manager instance owns this session's file input context",
        ));
    }
    // Keep the desk-context lock through the synchronous pending-command
    // transition. A losing pane can therefore never consume the command before
    // discovering that another pane already won the claim.
    prepare()?;
    contexts.insert(context.desk_id, context);
    Ok(())
}

pub(super) async fn input_context(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Option<FileInputContextResponse>>, ApiError> {
    let session = authenticate(&state, &headers)?;
    prune_input_contexts(&state);
    Ok(Json(
        state
            .file_input_contexts
            .lock()
            .get(&session.desk.id)
            .map(context_response),
    ))
}

pub(super) async fn claim_input_context(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<ClaimFileInput>,
) -> Result<Json<FileInputContextResponse>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let instance_id = validate_instance_id(&input.instance_id)?;
    let pending_origin = matches!(input.origin, FileInputOrigin::Pending);
    let context = FileInputContext {
        instance_id: instance_id.to_owned(),
        action: input.action,
        session_id: session.id,
        desk_id: session.desk.id,
        expires_at: Instant::now() + FILE_INPUT_CONTEXT_TTL,
    };
    try_claim_input_context(&state, context.clone(), || {
        prepare_pending_claim(&state, &session, input.action, pending_origin)
    })?;
    if pending_origin {
        emit(
            &state,
            "programmer_changed",
            serde_json::json!({"session_id":session.id}),
        );
    }
    emit_claim_changed(&state, session.id, session.desk.id, &context, true);
    Ok(Json(context_response(&context)))
}

fn validate_instance_id(value: &str) -> Result<&str, ApiError> {
    let value = value.trim();
    let invalid = value.is_empty()
        || value.len() > 128
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | ':' | '.')
        });
    if invalid {
        return Err(ApiError::bad_request("File Manager instance_id is invalid"));
    }
    Ok(value)
}

fn prepare_pending_claim(
    state: &AppState,
    session: &Session,
    action: FileInputAction,
    pending_origin: bool,
) -> Result<(), ApiError> {
    if !pending_origin {
        return Ok(());
    }
    let command_line = state
        .programmers
        .get(session.id)
        .map(|programmer| programmer.command_line)
        .ok_or_else(|| ApiError::not_found("programmer"))?;
    if pending_file_action(&command_line) != Some(action) {
        return Err(ApiError::conflict(
            "the desk does not have the matching unowned file action",
        ));
    }
    state
        .programmers
        .set_command_line(session.id, String::new());
    if let Err(error) = persist_programmer(state, session) {
        state.programmers.set_command_line(session.id, command_line);
        let _ = persist_programmer(state, session);
        return Err(error);
    }
    Ok(())
}

fn emit_claim_changed(
    state: &AppState,
    session_id: SessionId,
    desk_id: Uuid,
    context: &FileInputContext,
    claimed: bool,
) {
    emit(
        state,
        "file_input_context_changed",
        serde_json::json!({
            "session_id": session_id,
            "desk_id": desk_id,
            "instance_id": context.instance_id,
            "action": context.action,
            "claimed": claimed,
        }),
    );
}

pub(super) async fn release_input_context(
    State(state): State<AppState>,
    Query(query): Query<FileInputQuery>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let session = authenticate(&state, &headers)?;
    let released = {
        let mut contexts = state.file_input_contexts.lock();
        let matches = contexts.get(&session.desk.id).is_some_and(|context| {
            query
                .instance_id
                .as_deref()
                .is_none_or(|instance| instance == context.instance_id)
        });
        matches.then(|| contexts.remove(&session.desk.id)).flatten()
    };
    if let Some(context) = released {
        emit_claim_changed(&state, session.id, session.desk.id, &context, false);
    }
    Ok(StatusCode::NO_CONTENT)
}

pub(super) fn pending_file_action(command_line: &str) -> Option<FileInputAction> {
    match command_line.trim().to_ascii_uppercase().as_str() {
        "SET" => Some(FileInputAction::Rename),
        "CPY" | "COPY" => Some(FileInputAction::Copy),
        "MOV" | "MOVE" => Some(FileInputAction::Move),
        "DEL" | "DELETE" => Some(FileInputAction::Delete),
        _ => None,
    }
}

pub(crate) fn route_osc_input(state: &AppState, session: &Session, action: &str) -> bool {
    prune_input_contexts(state);
    let context = {
        let mut contexts = state.file_input_contexts.lock();
        let Some(context) = contexts.get_mut(&session.desk.id) else {
            return false;
        };
        if context.desk_id != session.desk.id {
            return false;
        }
        context.expires_at = Instant::now() + FILE_INPUT_CONTEXT_TTL;
        if !matches!(action, "enter" | "escape" | "esc") {
            return true;
        }
        let context = context.clone();
        if matches!(action, "escape" | "esc") {
            contexts.remove(&session.desk.id);
        }
        context
    };
    emit_input_action(state, session, &context, action);
    true
}

fn emit_input_action(
    state: &AppState,
    session: &Session,
    context: &FileInputContext,
    action: &str,
) {
    emit(
        state,
        "file_input_action",
        serde_json::json!({
            "session_id":context.session_id,
            "source_session_id":session.id,
            "desk_id":session.desk.id,
            "instance_id":context.instance_id,
            "operation":context.action,
            "action":if action == "enter" { "enter" } else { "escape" },
            "source":"osc",
        }),
    );
}

pub(crate) fn release_session_input(state: &AppState, session: &Session, reason: &str) {
    let released = {
        let mut contexts = state.file_input_contexts.lock();
        let owned = contexts
            .get(&session.desk.id)
            .is_some_and(|context| context.session_id == session.id);
        owned.then(|| contexts.remove(&session.desk.id)).flatten()
    };
    if let Some(context) = released {
        emit(
            state,
            "file_input_context_changed",
            serde_json::json!({
                "session_id":context.session_id,
                "desk_id":context.desk_id,
                "instance_id":context.instance_id,
                "action":context.action,
                "claimed":false,
                "reason":reason,
            }),
        );
    }
}
