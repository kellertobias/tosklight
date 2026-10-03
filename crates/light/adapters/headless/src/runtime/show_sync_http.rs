//! `POST /api/v2/show-sync/transactions`: one Architect gesture as one atomic transaction.
//!
//! This route is the documented exception to two API rules (`docs/engineering/show-sync.md`):
//! it carries several objects per request (§3), so a CAD gesture commits atomically, and it
//! compares each field with the client's base instead of applying last-write-wins (§7), so an
//! offline Architect cannot silently overwrite a desk edit. Desk UI, OSC and object intents keep
//! their own rules.

use super::{
    AppState, ServerShowPatchPorts, ShowContext, TolerantJson, authenticate,
    show_patch_adapter::ServerShowPatchUnitOfWork, show_sync_wire,
};
use axum::{
    Json, Router,
    extract::{State, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
};
use light_application::{
    ActionContext, ActionEnvelope, ActionError, ActionErrorKind, ActionSource,
    PatchFixtureCandidate, PatchFixtureProjection, show_sync::ShowSyncPorts,
};
use light_core::ShowId;
use light_engine::EngineSnapshot;
use light_show::SyncAppliedRequest;
use light_wire::v2::show_sync as wire;
use std::sync::Arc;
use uuid::Uuid;

pub(super) fn router() -> Router<AppState> {
    Router::new().route("/api/v2/show-sync/transactions", post(transaction))
}

async fn transaction(
    State(state): State<AppState>,
    show: ShowContext,
    headers: HeaderMap,
    request: Result<TolerantJson<wire::ShowSyncTransactionRequest>, JsonRejection>,
) -> Result<Json<wire::ShowSyncTransactionOutcome>, ShowSyncHttpError> {
    let session = authenticate(&state, &headers).map_err(ShowSyncHttpError::api)?;
    let TolerantJson(request) =
        request.map_err(|error| ShowSyncHttpError::invalid(error.body_text()))?;
    validate(&request)?;
    show.verify(&state)
        .map_err(|error| ShowSyncHttpError::not_active(&state, error.message))?;
    verify_desk(&state, &request)?;
    let show_id = ShowId(request.show_id);
    let active = state.active_show.current().map(|entry| entry.id);
    if active != Some(show_id) {
        return Err(ShowSyncHttpError::not_active(
            &state,
            "the bound show is not active on Control",
        ));
    }
    let command = show_sync_wire::application_command(show_id, &request)
        .map_err(ShowSyncHttpError::invalid)?;
    let context = ActionContext::operator(session.desk.id, session.id.0, ActionSource::Http)
        .with_request_id(request.request_id.clone());
    let envelope = ActionEnvelope { context, command };
    let worker = state.clone();
    let result = tokio::task::spawn_blocking(move || {
        let ports = ServerShowPatchPorts::new(worker.clone());
        worker.active_show.synchronize(envelope, &ports)
    })
    .await
    .map_err(|error| ShowSyncHttpError::internal(format!("sync task failed: {error}")))?
    .map_err(|error| ShowSyncHttpError::application(&state, error))?;
    Ok(Json(show_sync_wire::wire_outcome(result)))
}

fn validate(request: &wire::ShowSyncTransactionRequest) -> Result<(), ShowSyncHttpError> {
    if request.request_id.trim().is_empty() || request.request_id.len() > 128 {
        return Err(ShowSyncHttpError::invalid(
            "request_id must contain 1-128 characters",
        ));
    }
    if request.show_id.is_nil() || request.association_id.is_nil() {
        return Err(ShowSyncHttpError::invalid(
            "show_id and association_id must be non-nil UUIDs",
        ));
    }
    if request.operations.is_empty() || request.operations.len() > 1024 {
        return Err(ShowSyncHttpError::invalid(
            "a sync transaction carries 1-1024 operations",
        ));
    }
    Ok(())
}

/// A binding names one desk installation; another desk answering on the same address is refused.
fn verify_desk(
    state: &AppState,
    request: &wire::ShowSyncTransactionRequest,
) -> Result<(), ShowSyncHttpError> {
    let Some(expected) = request.origin.desk_identity else {
        return Ok(());
    };
    let identity = state
        .installation
        .desk_identity()
        .map_err(|error| ShowSyncHttpError::unavailable(error.to_string()))?;
    if identity == expected {
        Ok(())
    } else {
        Err(ShowSyncHttpError {
            status: StatusCode::CONFLICT,
            body: wire::ShowSyncErrorResponse {
                error: "this transaction is bound to a different desk".into(),
                kind: wire::ShowSyncErrorKind::DeskMismatch,
                active_show_id: None,
                retryable: false,
            },
        })
    }
}

impl ShowSyncPorts for ServerShowPatchPorts {
    fn applied_sync_request(
        &self,
        unit: &ServerShowPatchUnitOfWork,
        association_id: Uuid,
        request_id: &str,
    ) -> Result<Option<SyncAppliedRequest>, ActionError> {
        unit.active_show_unit()
            .sync_applied_request(association_id, request_id)
    }

    fn patch_fixture_document(
        &self,
        fixture: &PatchFixtureProjection,
    ) -> Result<serde_json::Value, ActionError> {
        show_sync_wire::patch_fixture_document(fixture)
            .map_err(|error| ActionError::new(ActionErrorKind::Internal, error))
    }

    fn patch_fixture_candidate(
        &self,
        document: serde_json::Value,
    ) -> Result<PatchFixtureCandidate, ActionError> {
        show_sync_wire::patch_fixture_candidate(document)
            .map_err(|error| ActionError::new(ActionErrorKind::Invalid, error))
    }

    fn installed_snapshot(&self) -> Option<Arc<EngineSnapshot>> {
        Some(self.state().output.snapshot())
    }
}

struct ShowSyncHttpError {
    status: StatusCode,
    body: wire::ShowSyncErrorResponse,
}

impl ShowSyncHttpError {
    fn with(status: StatusCode, kind: wire::ShowSyncErrorKind, error: impl Into<String>) -> Self {
        Self {
            status,
            body: wire::ShowSyncErrorResponse {
                error: error.into(),
                kind,
                active_show_id: None,
                retryable: matches!(kind, wire::ShowSyncErrorKind::Unavailable),
            },
        }
    }

    fn invalid(error: impl Into<String>) -> Self {
        Self::with(
            StatusCode::BAD_REQUEST,
            wire::ShowSyncErrorKind::Invalid,
            error,
        )
    }

    fn unavailable(error: impl Into<String>) -> Self {
        Self::with(
            StatusCode::SERVICE_UNAVAILABLE,
            wire::ShowSyncErrorKind::Unavailable,
            error,
        )
    }

    fn internal(error: impl Into<String>) -> Self {
        Self::with(
            StatusCode::INTERNAL_SERVER_ERROR,
            wire::ShowSyncErrorKind::Internal,
            error,
        )
    }

    /// The client holds its edits Offline ("Show not active on Control"); nothing was written.
    fn not_active(state: &AppState, error: impl Into<String>) -> Self {
        let mut response = Self::with(
            StatusCode::CONFLICT,
            wire::ShowSyncErrorKind::ShowNotActive,
            error,
        );
        response.body.active_show_id = state.active_show.current().map(|entry| entry.id.0);
        response
    }

    fn api(error: super::ApiError) -> Self {
        let kind = if error.status == StatusCode::SERVICE_UNAVAILABLE {
            wire::ShowSyncErrorKind::Unavailable
        } else if error.status.is_server_error() {
            wire::ShowSyncErrorKind::Internal
        } else {
            wire::ShowSyncErrorKind::Invalid
        };
        Self::with(error.status, kind, error.message)
    }

    fn application(state: &AppState, error: ActionError) -> Self {
        match error.kind {
            ActionErrorKind::NotFound => Self::not_active(state, error.message),
            ActionErrorKind::Conflict
                if error.message == light_application::show_sync::REQUEST_REUSED_MESSAGE =>
            {
                Self::with(
                    StatusCode::CONFLICT,
                    wire::ShowSyncErrorKind::RequestReused,
                    error.message,
                )
            }
            // A conflicting commit means the file moved underneath the unit; the same request
            // re-reads and re-resolves every field on retry.
            ActionErrorKind::Conflict | ActionErrorKind::Busy | ActionErrorKind::Unavailable => {
                Self::unavailable(error.message)
            }
            ActionErrorKind::Invalid => Self::invalid(error.message),
            ActionErrorKind::Unauthorized => Self::with(
                StatusCode::UNAUTHORIZED,
                wire::ShowSyncErrorKind::Invalid,
                error.message,
            ),
            ActionErrorKind::Forbidden => Self::with(
                StatusCode::FORBIDDEN,
                wire::ShowSyncErrorKind::Invalid,
                error.message,
            ),
            ActionErrorKind::Internal => Self::internal(error.message),
        }
    }
}

impl IntoResponse for ShowSyncHttpError {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
    }
}
