use super::{AppState, ServerShowPatchPorts, Session, ShowContext, authenticate, parse_if_match};
use axum::{
    Json, Router,
    extract::{State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use light_application::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ActionSource, PatchFixturesCommand,
};
use light_core::ShowId;
use light_wire::v2::patch::{PatchErrorResponse, PatchFixturesRequest};

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/patch", get(patch_snapshot))
        .route("/api/v2/patch/fixtures", post(patch_fixtures))
}

async fn patch_snapshot(
    State(state): State<AppState>,
    show: ShowContext,
    headers: HeaderMap,
) -> Result<Response, PatchHttpError> {
    let session = authenticate(&state, &headers).map_err(PatchHttpError::api)?;
    let show_id = show.resolve(&state).map_err(PatchHttpError::api)?;
    let context = http_context(&session);
    let snapshot = run_patch_snapshot(state, context, show_id).await?;
    let response = super::show_patch_wire::wire_snapshot(snapshot);
    Ok(json_with_etag(response.patch_revision, response))
}

async fn patch_fixtures(
    State(state): State<AppState>,
    show: ShowContext,
    headers: HeaderMap,
    request: Result<Json<PatchFixturesRequest>, JsonRejection>,
) -> Result<Response, PatchHttpError> {
    let session = authenticate(&state, &headers).map_err(PatchHttpError::api)?;
    let show_id = show.resolve(&state).map_err(PatchHttpError::api)?;
    let expected_patch_revision = parse_if_match(&headers).map_err(PatchHttpError::api)?;
    let Json(request) = request.map_err(|error| PatchHttpError::bad_request(error.body_text()))?;
    let action = patch_action(show_id, &session, expected_patch_revision, request)?;
    let result = run_patch_fixtures(state, action).await?;
    let response = super::show_patch_wire::wire_outcome(result);
    Ok(json_with_etag(response.delta.patch_revision, response))
}

async fn run_patch_snapshot(
    state: AppState,
    context: ActionContext,
    show_id: ShowId,
) -> Result<light_application::PatchSnapshot, PatchHttpError> {
    let active_show = state.active_show.clone();
    tokio::task::spawn_blocking(move || {
        let ports = ServerShowPatchPorts::new(state);
        active_show.patch_snapshot(&context, show_id, &ports)
    })
    .await
    .map_err(PatchHttpError::blocking)?
    .map_err(PatchHttpError::application)
}

async fn run_patch_fixtures(
    state: AppState,
    action: ActionEnvelope<PatchFixturesCommand>,
) -> Result<light_application::PatchFixturesResult, PatchHttpError> {
    let active_show = state.active_show.clone();
    tokio::task::spawn_blocking(move || {
        let ports = ServerShowPatchPorts::new(state);
        active_show.patch_fixtures(action, &ports)
    })
    .await
    .map_err(PatchHttpError::blocking)?
    .map_err(PatchHttpError::application)
}

fn patch_action(
    show_id: ShowId,
    session: &Session,
    expected_patch_revision: u64,
    request: PatchFixturesRequest,
) -> Result<ActionEnvelope<PatchFixturesCommand>, PatchHttpError> {
    let request_id = request.request_id.clone();
    let command = super::show_patch_wire::application_command(show_id, request)
        .map_err(PatchHttpError::bad_request)?;
    let context = http_context(session)
        .with_request_id(request_id)
        .with_expected_revision(expected_patch_revision);
    Ok(ActionEnvelope { context, command })
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
        .expect("a numeric patch revision always forms a valid ETag")
}

struct PatchHttpError {
    status: StatusCode,
    body: PatchErrorResponse,
}

impl PatchHttpError {
    fn application(error: ActionError) -> Self {
        Self {
            status: application_status(error.kind),
            body: PatchErrorResponse {
                error: error.message,
                current_revision: error.current_revision,
                retryable: error.retryable,
            },
        }
    }

    fn api(error: super::ApiError) -> Self {
        Self {
            status: error.status,
            body: PatchErrorResponse {
                error: error.message,
                current_revision: None,
                retryable: error.status == StatusCode::SERVICE_UNAVAILABLE,
            },
        }
    }

    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            body: PatchErrorResponse {
                error: message.into(),
                current_revision: None,
                retryable: false,
            },
        }
    }

    fn blocking(error: tokio::task::JoinError) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            body: PatchErrorResponse {
                error: format!("patch service task failed: {error}"),
                current_revision: None,
                retryable: false,
            },
        }
    }
}

impl IntoResponse for PatchHttpError {
    fn into_response(self) -> Response {
        let revision = self.body.current_revision;
        let mut response = (self.status, Json(self.body)).into_response();
        if let Some(revision) = revision {
            response
                .headers_mut()
                .insert(header::ETAG, revision_etag(revision));
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
