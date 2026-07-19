//! Authenticated v2 catalog, preview, and apply endpoints for Selective Show Import.

use super::{
    AppState, ImportSourceSnapshot, ServerSelectiveImportPorts, Session, authenticate,
    parse_if_match, selective_import_wire,
};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use light_application::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ActionSource,
    ApplySelectiveShowImportCommand, SelectiveShowImportService,
};
use light_core::ShowId;
use light_wire::v2::selective_import::{
    SelectiveImportApplyRequest, SelectiveImportErrorResponse, SelectiveImportSelection,
};
use uuid::Uuid;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v2/shows/{target_show_id}/selective-imports/{source_show_id}/catalog",
            get(source_catalog),
        )
        .route(
            "/api/v2/shows/{target_show_id}/selective-imports/{source_show_id}/preview",
            post(preview_import),
        )
        .route(
            "/api/v2/shows/{target_show_id}/selective-imports/{source_show_id}/apply",
            post(apply_import),
        )
}

async fn source_catalog(
    State(state): State<AppState>,
    Path((target_show_id, source_show_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, SelectiveImportHttpError> {
    let session = authenticate(&state, &headers).map_err(SelectiveImportHttpError::api)?;
    let (target_show_id, source_show_id) = parse_show_ids(target_show_id, source_show_id)?;
    ensure_active_target(&state, target_show_id)?;
    let context = http_context(&session);
    let source = run_catalog(state, move |ports| {
        ports.source_catalog(&context, source_show_id)
    })
    .await?;
    let catalog = selective_import_wire::catalog(&source.document);
    Ok(json_with_etag(catalog.source_revision, catalog))
}

async fn preview_import(
    State(state): State<AppState>,
    Path((target_show_id, source_show_id)): Path<(String, String)>,
    headers: HeaderMap,
    request: Result<Json<SelectiveImportSelection>, JsonRejection>,
) -> Result<Response, SelectiveImportHttpError> {
    let session = authenticate(&state, &headers).map_err(SelectiveImportHttpError::api)?;
    let (target_show_id, source_show_id) = parse_show_ids(target_show_id, source_show_id)?;
    let Json(selection) =
        request.map_err(|error| SelectiveImportHttpError::bad(error.body_text()))?;
    let request =
        selective_import_wire::application_request(source_show_id, target_show_id, selection)
            .map_err(SelectiveImportHttpError::bad)?;
    let context = http_context(&session);
    let preview = run_service(state, move |service, ports| {
        service.preview(&context, request, ports)
    })
    .await?;
    let response = selective_import_wire::preview(preview);
    Ok(json_with_etag(response.target_revision, response))
}

async fn apply_import(
    State(state): State<AppState>,
    Path((target_show_id, source_show_id)): Path<(String, String)>,
    headers: HeaderMap,
    request: Result<Json<SelectiveImportApplyRequest>, JsonRejection>,
) -> Result<Response, SelectiveImportHttpError> {
    let session = authenticate(&state, &headers).map_err(SelectiveImportHttpError::api)?;
    let (target_show_id, source_show_id) = parse_show_ids(target_show_id, source_show_id)?;
    let expected_target_revision =
        parse_if_match(&headers).map_err(SelectiveImportHttpError::api)?;
    let Json(request) =
        request.map_err(|error| SelectiveImportHttpError::bad(error.body_text()))?;
    validate_apply_request(&request, expected_target_revision)?;
    let command = ApplySelectiveShowImportCommand {
        request: selective_import_wire::application_request(
            source_show_id,
            target_show_id,
            request.selection,
        )
        .map_err(SelectiveImportHttpError::bad)?,
        expected_source_revision: selective_import_wire::expected_revision(
            request.expected_source_revision,
        ),
        expected_target_revision: selective_import_wire::expected_revision(
            request.expected_target_revision,
        ),
    };
    let context = http_context(&session)
        .with_request_id(request.request_id)
        .with_expected_revision(expected_target_revision);
    let result = run_service(state, move |service, ports| {
        service.apply(ActionEnvelope { context, command }, ports)
    })
    .await?;
    let response = selective_import_wire::outcome(result);
    Ok(json_with_etag(response.show_revision, response))
}

async fn run_catalog<F>(
    state: AppState,
    operation: F,
) -> Result<ImportSourceSnapshot, SelectiveImportHttpError>
where
    F: FnOnce(&ServerSelectiveImportPorts) -> Result<ImportSourceSnapshot, ActionError>
        + Send
        + 'static,
{
    tokio::task::spawn_blocking(move || operation(&ServerSelectiveImportPorts::new(state)))
        .await
        .map_err(SelectiveImportHttpError::blocking)?
        .map_err(SelectiveImportHttpError::application)
}

async fn run_service<T, F>(state: AppState, operation: F) -> Result<T, SelectiveImportHttpError>
where
    T: Send + 'static,
    F: FnOnce(&SelectiveShowImportService, &ServerSelectiveImportPorts) -> Result<T, ActionError>
        + Send
        + 'static,
{
    let service = state.selective_show_import.clone();
    tokio::task::spawn_blocking(move || {
        let ports = ServerSelectiveImportPorts::new(state);
        operation(&service, &ports)
    })
    .await
    .map_err(SelectiveImportHttpError::blocking)?
    .map_err(SelectiveImportHttpError::application)
}

fn ensure_active_target(state: &AppState, target: ShowId) -> Result<(), SelectiveImportHttpError> {
    if state
        .active_show
        .read()
        .as_ref()
        .is_some_and(|show| show.id == target)
    {
        Ok(())
    } else {
        Err(SelectiveImportHttpError::application(ActionError::new(
            ActionErrorKind::Conflict,
            "selective import target must be the active show",
        )))
    }
}

fn parse_show_ids(
    target: String,
    source: String,
) -> Result<(ShowId, ShowId), SelectiveImportHttpError> {
    let target = Uuid::parse_str(&target)
        .map_err(|_| SelectiveImportHttpError::bad("target_show_id must be a UUID"))?;
    let source = Uuid::parse_str(&source)
        .map_err(|_| SelectiveImportHttpError::bad("source_show_id must be a UUID"))?;
    Ok((ShowId(target), ShowId(source)))
}

fn validate_apply_request(
    request: &SelectiveImportApplyRequest,
    expected_target_revision: u64,
) -> Result<(), SelectiveImportHttpError> {
    if request.request_id.is_empty()
        || request.request_id.len() > 128
        || request.request_id.chars().any(char::is_control)
    {
        return Err(SelectiveImportHttpError::bad(
            "request_id must contain 1-128 printable characters",
        ));
    }
    if request.expected_target_revision != expected_target_revision {
        return Err(SelectiveImportHttpError::bad(
            "If-Match must equal expected_target_revision",
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
    response.headers_mut().insert(
        header::ETAG,
        HeaderValue::from_str(&format!("\"{revision}\""))
            .expect("a numeric revision always forms a valid ETag"),
    );
    response
}

struct SelectiveImportHttpError {
    status: StatusCode,
    body: SelectiveImportErrorResponse,
}

impl SelectiveImportHttpError {
    fn application(error: ActionError) -> Self {
        Self {
            status: application_status(error.kind),
            body: SelectiveImportErrorResponse {
                error: error.message,
                current_revision: error.current_revision,
                retryable: error.retryable,
            },
        }
    }

    fn api(error: super::ApiError) -> Self {
        Self {
            status: error.status,
            body: SelectiveImportErrorResponse {
                error: error.message,
                current_revision: None,
                retryable: error.status == StatusCode::SERVICE_UNAVAILABLE,
            },
        }
    }

    fn bad(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            body: SelectiveImportErrorResponse {
                error: message.into(),
                current_revision: None,
                retryable: false,
            },
        }
    }

    fn blocking(error: tokio::task::JoinError) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            body: SelectiveImportErrorResponse {
                error: format!("selective import task failed: {error}"),
                current_revision: None,
                retryable: false,
            },
        }
    }
}

impl IntoResponse for SelectiveImportHttpError {
    fn into_response(self) -> Response {
        let revision = self.body.current_revision;
        let mut response = (self.status, Json(self.body)).into_response();
        if let Some(revision) = revision {
            response.headers_mut().insert(
                header::ETAG,
                HeaderValue::from_str(&format!("\"{revision}\""))
                    .expect("a numeric revision always forms a valid ETag"),
            );
        }
        response
    }
}

const fn application_status(kind: ActionErrorKind) -> StatusCode {
    match kind {
        ActionErrorKind::Invalid => StatusCode::BAD_REQUEST,
        ActionErrorKind::Unauthorized => StatusCode::UNAUTHORIZED,
        ActionErrorKind::Forbidden => StatusCode::FORBIDDEN,
        ActionErrorKind::NotFound => StatusCode::NOT_FOUND,
        ActionErrorKind::Conflict | ActionErrorKind::Busy => StatusCode::CONFLICT,
        ActionErrorKind::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        ActionErrorKind::Internal => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
