use std::time::{Duration, Instant};

use axum::{Json, extract::State, http::HeaderMap};
use light_core::SessionId;
pub(crate) use light_wire::v2::files::FileInputAction;
use light_wire::v2::files::{FileInputClaimRequest, FileInputOrigin, FileInputReleaseRequest};
use serde::Serialize;
use uuid::Uuid;

use crate::tolerant_json::TolerantJson;

use super::super::{ApiError, AppState, Session, authenticate, emit, persist_programmer};
use super::super::{desk_management_v2::ReplayKey, show_objects_v2::validate_request_id};

const FILE_INPUT_CONTEXT_TTL: Duration = Duration::from_secs(120);

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
    TolerantJson(input): TolerantJson<FileInputClaimRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    validate_request_id(&input.request_id)?;
    let key = ReplayKey::new(session.id, "file-input-claim", &input.request_id);
    let fingerprint =
        serde_json::to_value(&input).map_err(|error| ApiError::internal(error.to_string()))?;
    let mut replay = state.desk_management_replay.lock().await;
    if let Some(value) = replay.get(&key, &fingerprint)? {
        return Ok(Json(value));
    }
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
    let value = super::intent_value(context_response(&context), &input.request_id, false)?;
    replay.insert(key, fingerprint, value.clone());
    Ok(Json(value))
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
    headers: HeaderMap,
    TolerantJson(input): TolerantJson<FileInputReleaseRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let session = authenticate(&state, &headers)?;
    validate_request_id(&input.request_id)?;
    let key = ReplayKey::new(session.id, "file-input-release", &input.request_id);
    let fingerprint =
        serde_json::to_value(&input).map_err(|error| ApiError::internal(error.to_string()))?;
    let mut replay = state.desk_management_replay.lock().await;
    if let Some(value) = replay.get(&key, &fingerprint)? {
        return Ok(Json(value));
    }
    let released = {
        let mut contexts = state.file_input_contexts.lock();
        let matches = contexts.get(&session.desk.id).is_some_and(|context| {
            input
                .instance_id
                .as_deref()
                .is_none_or(|instance| instance == context.instance_id)
        });
        matches.then(|| contexts.remove(&session.desk.id)).flatten()
    };
    let was_released = released.is_some();
    if let Some(context) = released {
        emit_claim_changed(&state, session.id, session.desk.id, &context, false);
    }
    let value = super::intent_value(
        serde_json::json!({"released": was_released}),
        &input.request_id,
        false,
    )?;
    replay.insert(key, fingerprint, value.clone());
    Ok(Json(value))
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
