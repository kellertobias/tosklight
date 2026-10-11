//! Authenticated, desk-scoped v2 Playback command and narrow repair endpoints.

mod wire;

pub(super) use wire::{
    action_outcome, application_command, desk_projection, runtime_change, runtime_projection,
    telemetry_tick,
};

use super::{
    ApiError, AppState, DeskContext, ProgrammingLockPolicy, Session, ShowContext, authenticate,
    authoritative_playback_controls, playback_service, run_programming_interaction, runtime_wire,
    session_for_desk,
};
use crate::tolerant_json::TolerantJson;
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use light_application::{ActionContext, ActionSource};
use light_wire::v2::playback::{
    PlaybackActionRequest, PlaybackErrorKind, PlaybackErrorResponse, PlaybackOverview,
    PlaybackRuntimeSnapshotRequest,
};
pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/playback-actions", post(playback_action))
        .route(
            "/api/v2/playbacks/{page}/{position}/{action}",
            get(explicit_page_plain_action).post(explicit_page_plain_value_action),
        )
        .route(
            "/api/v2/playbacks/current/{position}/{action}",
            get(current_page_plain_action).post(current_page_plain_value_action),
        )
        .route("/api/v2/playback-runtime/snapshot", post(playback_snapshot))
        .route("/api/v2/playback-overview", get(playback_overview))
}

#[derive(serde::Deserialize)]
struct PlainPlaybackValueRequest {
    value: f32,
}

async fn current_page_plain_action(
    State(state): State<AppState>,
    Path((position, action)): Path<(u16, String)>,
    show: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
) -> Result<impl IntoResponse, PlaybackHttpError> {
    execute_plain_action(
        state,
        show,
        desk,
        headers,
        light_application::PlaybackAddress::CurrentPage { position },
        plain_button_action(&action)?,
    )
    .await
}

async fn explicit_page_plain_action(
    State(state): State<AppState>,
    Path((page, position, action)): Path<(u8, u16, String)>,
    show: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
) -> Result<impl IntoResponse, PlaybackHttpError> {
    execute_plain_action(
        state,
        show,
        desk,
        headers,
        light_application::PlaybackAddress::Playback(
            light_playback::PlaybackAddress::new(page, position)
                .map_err(PlaybackHttpError::invalid)?,
        ),
        plain_button_action(&action)?,
    )
    .await
}

async fn current_page_plain_value_action(
    State(state): State<AppState>,
    Path((position, action)): Path<(u16, String)>,
    show: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<PlainPlaybackValueRequest>,
) -> Result<impl IntoResponse, PlaybackHttpError> {
    execute_plain_action(
        state,
        show,
        desk,
        headers,
        light_application::PlaybackAddress::CurrentPage { position },
        plain_value_action(&action, request.value)?,
    )
    .await
}

async fn explicit_page_plain_value_action(
    State(state): State<AppState>,
    Path((page, position, action)): Path<(u8, u16, String)>,
    show: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
    TolerantJson(request): TolerantJson<PlainPlaybackValueRequest>,
) -> Result<impl IntoResponse, PlaybackHttpError> {
    execute_plain_action(
        state,
        show,
        desk,
        headers,
        light_application::PlaybackAddress::Playback(
            light_playback::PlaybackAddress::new(page, position)
                .map_err(PlaybackHttpError::invalid)?,
        ),
        plain_value_action(&action, request.value)?,
    )
    .await
}

async fn execute_plain_action(
    state: AppState,
    show: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
    address: light_application::PlaybackAddress,
    action: light_application::PlaybackAction,
) -> Result<impl IntoResponse, PlaybackHttpError> {
    let session = session_for_desk(&state, &headers, &desk).map_err(PlaybackHttpError::api)?;
    let _activation = state.active_show.acquire().await;
    show.verify(&state).map_err(PlaybackHttpError::api)?;
    let context = http_context(&session);
    let playback_context = context.clone();
    let result = run_programming_interaction(
        &state,
        &session,
        &context,
        "http",
        ProgrammingLockPolicy::RequireUnlocked,
        || {
            playback_service::execute(
                &state,
                Some(&session),
                Some(&session.desk),
                playback_context,
                light_application::PlaybackCommand {
                    address,
                    action,
                    control_source: light_playback::PlaybackControlSource {
                        surface_id: format!("http:{}", session.id.0),
                        control_id: 0,
                    },
                },
            )
        },
    )
    .map_err(PlaybackHttpError::api)?
    .output
    .map_err(PlaybackHttpError::api)?;
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(wire::action_outcome(result)),
    ))
}

fn plain_button_action(
    action: &str,
) -> Result<light_application::PlaybackAction, PlaybackHttpError> {
    use light_application::PlaybackAction;
    Ok(match action {
        "on" => PlaybackAction::On { pressed: true },
        "off" => PlaybackAction::Off { pressed: true },
        "toggle" => PlaybackAction::Toggle { pressed: true },
        "pause" => PlaybackAction::Pause { pressed: true },
        "dynamic-restart" => PlaybackAction::DynamicRestart { pressed: true },
        "dynamic-double-speed" => PlaybackAction::DynamicDoubleSpeed { pressed: true },
        "dynamic-half-speed" => PlaybackAction::DynamicHalfSpeed { pressed: true },
        "dynamic-learn-speed" => PlaybackAction::DynamicLearnSpeed { pressed: true },
        _ => return Err(PlaybackHttpError::invalid("unknown plain Playback action")),
    })
}

fn plain_value_action(
    action: &str,
    value: f32,
) -> Result<light_application::PlaybackAction, PlaybackHttpError> {
    if action != "master" || !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(PlaybackHttpError::invalid(
            "master value must be finite and within 0-1",
        ));
    }
    Ok(light_application::PlaybackAction::Master(
        light_application::PlaybackLevel::new(value),
    ))
}

async fn playback_action(
    State(state): State<AppState>,
    show: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
    request: Result<TolerantJson<PlaybackActionRequest>, JsonRejection>,
) -> Result<Response, PlaybackHttpError> {
    let session = session_for_desk(&state, &headers, &desk).map_err(PlaybackHttpError::api)?;
    let TolerantJson(request) =
        request.map_err(|error| PlaybackHttpError::invalid(error.body_text()))?;
    let command = wire::application_command(request).map_err(PlaybackHttpError::invalid)?;
    let _activation = state.active_show.acquire().await;
    show.verify(&state).map_err(PlaybackHttpError::api)?;
    let context = http_context(&session);
    let playback_context = context.clone();
    let result = run_programming_interaction(
        &state,
        &session,
        &context,
        "http",
        ProgrammingLockPolicy::RequireUnlocked,
        || {
            playback_service::execute(
                &state,
                Some(&session),
                Some(&session.desk),
                playback_context,
                command,
            )
        },
    )
    .map_err(PlaybackHttpError::api)?
    .output
    .map_err(PlaybackHttpError::api)?;
    Ok(Json(wire::action_outcome(result)).into_response())
}

async fn playback_snapshot(
    State(state): State<AppState>,
    show: ShowContext,
    desk: DeskContext,
    headers: HeaderMap,
    request: Result<Json<PlaybackRuntimeSnapshotRequest>, JsonRejection>,
) -> Result<Response, PlaybackHttpError> {
    let session = session_for_desk(&state, &headers, &desk).map_err(PlaybackHttpError::api)?;
    let Json(request) = request.map_err(|error| PlaybackHttpError::invalid(error.body_text()))?;
    let identities =
        wire::application_identities(request.identities).map_err(PlaybackHttpError::invalid)?;
    let _activation = state.active_show.acquire().await;
    show.verify(&state).map_err(PlaybackHttpError::api)?;
    let context = http_context(&session);
    let snapshot = playback_service::snapshot(&state, &session, context, &identities)
        .map_err(PlaybackHttpError::api)?;
    Ok(Json(wire::runtime_snapshot(snapshot)).into_response())
}

async fn playback_overview(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<PlaybackOverview>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let snapshot = state.output.snapshot();
    let selected_playback = state
        .active_show
        .current()
        .as_ref()
        .map(|show| {
            state
                .installation
                .selected_playback(session.desk.id, show.id)
        })
        .transpose()
        .map_err(ApiError::store)?
        .flatten();
    let surfaces = super::playback_surfaces_http::surface_snapshot(&state, session.desk.id)?;
    Ok(Json(PlaybackOverview {
        cue_lists: json_values(&snapshot.cue_lists)?,
        pages: json_values(&snapshot.playback_pages)?,
        active: json_values(&state.output.playback_runtime_status())?,
        desk: runtime_wire::desk(session.desk),
        surfaces,
        selected_playback: selected_playback.map(|address| {
            light_wire::v2::playback::PlaybackAddressValue {
                page: address.page,
                position: address.position,
            }
        }),
        authoritative_controls: authoritative_playback_controls(&state),
    }))
}

fn json_values<T: serde::Serialize>(values: &[T]) -> Result<Vec<serde_json::Value>, ApiError> {
    values
        .iter()
        .map(|value| {
            serde_json::to_value(value).map_err(|error| ApiError::internal(error.to_string()))
        })
        .collect()
}

fn http_context(session: &Session) -> ActionContext {
    ActionContext::operator(session.desk.id, session.id.0, ActionSource::Http)
}

struct PlaybackHttpError {
    status: StatusCode,
    body: PlaybackErrorResponse,
}

impl PlaybackHttpError {
    fn api(error: super::ApiError) -> Self {
        let kind = match error.status {
            StatusCode::UNAUTHORIZED => PlaybackErrorKind::Unauthorized,
            StatusCode::FORBIDDEN => PlaybackErrorKind::Forbidden,
            StatusCode::NOT_FOUND => PlaybackErrorKind::NotFound,
            StatusCode::CONFLICT => PlaybackErrorKind::Conflict,
            StatusCode::SERVICE_UNAVAILABLE => PlaybackErrorKind::Unavailable,
            status if status.is_server_error() => PlaybackErrorKind::Internal,
            _ => PlaybackErrorKind::Invalid,
        };
        Self::new(
            error.status,
            kind,
            error.message,
            error.status == StatusCode::SERVICE_UNAVAILABLE,
        )
    }

    fn invalid(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            PlaybackErrorKind::Invalid,
            message,
            false,
        )
    }

    fn new(
        status: StatusCode,
        kind: PlaybackErrorKind,
        error: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self {
            status,
            body: PlaybackErrorResponse {
                kind,
                error: error.into(),
                retryable,
            },
        }
    }
}

impl IntoResponse for PlaybackHttpError {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
    }
}
