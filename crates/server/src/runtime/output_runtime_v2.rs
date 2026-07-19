//! Authenticated repair projection for installation-global output runtime.

use super::{AppState, Session, authenticate, output_runtime_service};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::HeaderMap,
    routing::get,
};
use light_application::{
    ActionContext, ActionSource, OutputRuntimeChange, OutputRuntimeIdentity,
    OutputRuntimeProjection, OutputRuntimeSnapshot,
};
use light_wire::v2::events as wire;
use uuid::Uuid;

pub(super) fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v2/desks/{desk_id}/output-runtime/{identity}",
        get(output_runtime_snapshot),
    )
}

async fn output_runtime_snapshot(
    State(state): State<AppState>,
    Path((desk_id, identity)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Json<wire::OutputRuntimeSnapshot>, super::ApiError> {
    let session = authenticated_desk(&state, &headers, &desk_id)?;
    let identity = parse_identity(&identity)?;
    let _activation = state.activation_lock.clone().lock_owned().await;
    let snapshot =
        output_runtime_service::snapshot(&state, &session, http_context(&session), identity)?;
    Ok(Json(wire_snapshot(snapshot)))
}

fn authenticated_desk(
    state: &AppState,
    headers: &HeaderMap,
    path_desk_id: &str,
) -> Result<Session, super::ApiError> {
    let session = authenticate(state, headers)?;
    let desk_id = Uuid::parse_str(path_desk_id)
        .map_err(|_| super::ApiError::bad_request("desk_id must be a UUID"))?;
    if session.desk.id != desk_id {
        return Err(super::ApiError::forbidden(
            "session is not authorized for this desk",
        ));
    }
    Ok(session)
}

fn parse_identity(value: &str) -> Result<OutputRuntimeIdentity, super::ApiError> {
    match value {
        "global-master" => Ok(OutputRuntimeIdentity::GlobalMaster),
        _ => Err(super::ApiError::not_found("output runtime identity")),
    }
}

fn http_context(session: &Session) -> ActionContext {
    ActionContext::operator(
        session.desk.id,
        session.user.id.0,
        session.id.0,
        ActionSource::Http,
    )
}

fn wire_snapshot(snapshot: OutputRuntimeSnapshot) -> wire::OutputRuntimeSnapshot {
    wire::OutputRuntimeSnapshot {
        cursor: wire::EventSnapshotCursor {
            sequence: snapshot.event_sequence,
        },
        projection: wire_projection(snapshot.projection),
    }
}

pub(super) fn wire_change(change: OutputRuntimeChange) -> wire::OutputRuntimeChange {
    wire::OutputRuntimeChange {
        projection: wire_projection(change.projection),
    }
}

fn wire_projection(projection: OutputRuntimeProjection) -> wire::OutputRuntimeProjection {
    wire::OutputRuntimeProjection {
        scope: wire::OutputRuntimeScope {
            show_id: projection.scope.show_id,
            show_revision: projection.scope.show_revision,
        },
        identity: match projection.identity {
            OutputRuntimeIdentity::GlobalMaster => wire::OutputRuntimeIdentity::GlobalMaster,
        },
        grand_master: projection.grand_master,
        blackout: projection.blackout,
    }
}
