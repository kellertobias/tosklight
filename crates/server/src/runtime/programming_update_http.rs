//! Authenticated strict v2 HTTP surface for Programming-owned Update workflows.

use super::{
    AppState, ServerProgrammingUpdatePorts, Session, authenticate, emit, parse_if_match,
    persist_server_configuration, programming_update_http_error::ProgrammingUpdateHttpError,
    programming_update_wire, programming_update_wire_output, read_desk_lock, update_settings_for,
};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use light_application::{
    ActionContext, ActionEnvelope, ActionError, ActionSource, ActiveShowService, ProgrammingService,
};
use light_core::ShowId;
use light_wire::v2::programming_update::{
    ProgrammingUpdateActionRequest, ProgrammingUpdatePreviewRequest, ProgrammingUpdateSettings,
    ProgrammingUpdateTargetsRequest,
};
use uuid::Uuid;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v2/shows/{show_id}/programming-update/preview",
            post(preview),
        )
        .route(
            "/api/v2/shows/{show_id}/programming-update/targets",
            post(targets),
        )
        .route(
            "/api/v2/shows/{show_id}/programming-update/actions",
            post(apply_action),
        )
        .route(
            "/api/v2/desks/{desk_id}/programming-update/settings",
            get(settings).put(put_settings),
        )
}

async fn preview(
    State(state): State<AppState>,
    Path(show_id): Path<String>,
    headers: HeaderMap,
    request: Result<Json<ProgrammingUpdatePreviewRequest>, JsonRejection>,
) -> Result<Response, ProgrammingUpdateHttpError> {
    let session = authenticate_update(&state, &headers)?;
    let show_id = parse_show_id(&show_id)?;
    let Json(request) = request.map_err(ProgrammingUpdateHttpError::json)?;
    validate_request_id(&request.request_id)?;
    let (request_id, command) = programming_update_wire::preview_request(show_id, request)
        .map_err(ProgrammingUpdateHttpError::invalid)?;
    let action = ActionEnvelope {
        context: http_context(&session).with_request_id(request_id),
        command,
    };
    let result = run_update(state, session, false, move |service, active, ports| {
        service.preview_update(action, active, ports)
    })
    .await?;
    let response = programming_update_wire_output::preview_response(show_id, result)
        .map_err(ProgrammingUpdateHttpError::application)?;
    Ok(json_with_etag(response.show_revision, response))
}

async fn targets(
    State(state): State<AppState>,
    Path(show_id): Path<String>,
    headers: HeaderMap,
    request: Result<Json<ProgrammingUpdateTargetsRequest>, JsonRejection>,
) -> Result<Response, ProgrammingUpdateHttpError> {
    let session = authenticate_update(&state, &headers)?;
    let show_id = parse_show_id(&show_id)?;
    let Json(request) = request.map_err(ProgrammingUpdateHttpError::json)?;
    validate_request_id(&request.request_id)?;
    let (request_id, command) = programming_update_wire::targets_request(show_id, request);
    let action = ActionEnvelope {
        context: http_context(&session).with_request_id(request_id),
        command,
    };
    let result = run_update(state, session, false, move |service, active, ports| {
        service.update_targets(action, active, ports)
    })
    .await?;
    let response = programming_update_wire_output::targets_response(show_id, result)
        .map_err(ProgrammingUpdateHttpError::application)?;
    Ok(json_with_etag(response.show_revision, response))
}

async fn apply_action(
    State(state): State<AppState>,
    Path(show_id): Path<String>,
    headers: HeaderMap,
    request: Result<Json<ProgrammingUpdateActionRequest>, JsonRejection>,
) -> Result<Response, ProgrammingUpdateHttpError> {
    let session = authenticate_update(&state, &headers)?;
    let show_id = parse_show_id(&show_id)?;
    let expected_show_revision =
        parse_if_match(&headers).map_err(ProgrammingUpdateHttpError::api)?;
    let Json(request) = request.map_err(ProgrammingUpdateHttpError::json)?;
    validate_request_id(&request.request_id)?;
    let (request_id, command) =
        programming_update_wire::action_request(show_id, expected_show_revision, request)
            .map_err(ProgrammingUpdateHttpError::invalid)?;
    let action = ActionEnvelope {
        context: http_context(&session)
            .with_request_id(request_id)
            .with_expected_revision(expected_show_revision),
        command,
    };
    let result = run_update(state, session, true, move |service, active, ports| {
        service.handle_update(action, active, ports)
    })
    .await?;
    let response = programming_update_wire_output::action_outcome(result)
        .map_err(ProgrammingUpdateHttpError::application)?;
    let revision = match &response {
        light_wire::v2::programming_update::ProgrammingUpdateActionOutcome::Changed {
            show_revision,
            ..
        } => *show_revision,
    };
    Ok(json_with_etag(revision, response))
}

async fn settings(
    State(state): State<AppState>,
    Path(desk_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ProgrammingUpdateHttpError> {
    let session = authenticate_update(&state, &headers)?;
    let desk_id = exact_desk(&session, &desk_id)?;
    let settings = update_settings_for(&state, desk_id);
    Ok(Json(programming_update_wire::wire_settings(desk_id, &settings)).into_response())
}

async fn put_settings(
    State(state): State<AppState>,
    Path(desk_id): Path<String>,
    headers: HeaderMap,
    request: Result<Json<ProgrammingUpdateSettings>, JsonRejection>,
) -> Result<Response, ProgrammingUpdateHttpError> {
    let session = authenticate_update(&state, &headers)?;
    let desk_id = exact_desk(&session, &desk_id)?;
    let Json(request) = request.map_err(ProgrammingUpdateHttpError::json)?;
    let operation_lock = state.programming.desk_lock(desk_id);
    let _operation = operation_lock.lock();
    if read_desk_lock(&state, desk_id).locked {
        return Err(ProgrammingUpdateHttpError::conflict("desk is locked"));
    }
    let current = update_settings_for(&state, desk_id);
    let mut settings = current.clone();
    programming_update_wire::apply_settings(&mut settings, request);
    if settings == current {
        return Ok(
            Json(programming_update_wire::wire_settings(desk_id, &settings)).into_response(),
        );
    }
    let previous = state
        .configuration
        .write()
        .update_settings_by_desk
        .insert(desk_id, settings.clone());
    if let Err(error) = persist_server_configuration(&state) {
        restore_settings(&state, desk_id, previous);
        return Err(ProgrammingUpdateHttpError::api(error));
    }
    emit(
        &state,
        "update_settings_changed",
        serde_json::json!({"desk_id":desk_id,"settings":settings}),
    );
    Ok(Json(programming_update_wire::wire_settings(desk_id, &settings)).into_response())
}

fn restore_settings(
    state: &AppState,
    desk_id: Uuid,
    previous: Option<light_application::programming_update::UpdateSettings>,
) {
    let mut configuration = state.configuration.write();
    if let Some(previous) = previous {
        configuration
            .update_settings_by_desk
            .insert(desk_id, previous);
    } else {
        configuration.update_settings_by_desk.remove(&desk_id);
    }
}

async fn run_update<T, F>(
    state: AppState,
    session: Session,
    require_unlocked: bool,
    operation: F,
) -> Result<T, ProgrammingUpdateHttpError>
where
    T: Send + 'static,
    F: FnOnce(
            &ProgrammingService,
            &ActiveShowService,
            &ServerProgrammingUpdatePorts,
        ) -> Result<T, ActionError>
        + Send
        + 'static,
{
    tokio::task::spawn_blocking(move || {
        let ports =
            ServerProgrammingUpdatePorts::new(state.clone(), session, false, require_unlocked);
        operation(&state.programming, &state.active_show_service, &ports)
    })
    .await
    .map_err(ProgrammingUpdateHttpError::blocking)?
    .map_err(ProgrammingUpdateHttpError::application)
}

fn authenticate_update(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<Session, ProgrammingUpdateHttpError> {
    authenticate(state, headers).map_err(ProgrammingUpdateHttpError::api)
}

fn parse_show_id(value: &str) -> Result<ShowId, ProgrammingUpdateHttpError> {
    parse_non_nil_uuid(value, "show_id").map(ShowId)
}

fn exact_desk(session: &Session, value: &str) -> Result<Uuid, ProgrammingUpdateHttpError> {
    let desk_id = parse_non_nil_uuid(value, "desk_id")?;
    if desk_id != session.desk.id {
        return Err(ProgrammingUpdateHttpError::forbidden(
            "settings scope does not match the authenticated desk",
        ));
    }
    Ok(desk_id)
}

fn parse_non_nil_uuid(value: &str, name: &str) -> Result<Uuid, ProgrammingUpdateHttpError> {
    let id = Uuid::parse_str(value)
        .map_err(|_| ProgrammingUpdateHttpError::invalid(format!("{name} must be a UUID")))?;
    if id.is_nil() {
        return Err(ProgrammingUpdateHttpError::invalid(format!(
            "{name} must not be nil"
        )));
    }
    Ok(id)
}

fn validate_request_id(value: &str) -> Result<(), ProgrammingUpdateHttpError> {
    if value.trim().is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
        return Err(ProgrammingUpdateHttpError::invalid(
            "request_id must contain 1-128 printable bytes",
        ));
    }
    Ok(())
}

fn http_context(session: &Session) -> ActionContext {
    ActionContext::operator(
        session.desk.id,
        session.user.id.0,
        session.id.0,
        ActionSource::Http,
    )
}

fn json_with_etag<T: serde::Serialize>(revision: u64, body: T) -> Response {
    let mut response = Json(body).into_response();
    response
        .headers_mut()
        .insert(header::ETAG, revision_etag(revision));
    response
}

fn revision_etag(revision: u64) -> HeaderValue {
    HeaderValue::from_str(&format!("\"{revision}\""))
        .expect("a numeric Show revision always forms a valid ETag")
}
