use super::{ApiError, AppState, ControlDesk};
use axum::{
    extract::FromRequestParts,
    http::{HeaderMap, request::Parts},
};
use light_core::ShowId;
use uuid::Uuid;

pub(super) const SHOW_CONTEXT_HEADER: &str = "x-tosk-show";
pub(super) const DESK_CONTEXT_HEADER: &str = "x-tosk-desk";

/// Optional guard against applying a request after the desk switched Shows.
pub(super) struct ShowContext(Option<ShowId>);

impl ShowContext {
    pub(super) fn resolve(&self, state: &AppState) -> Result<ShowId, ApiError> {
        let active = state
            .active_show
            .current()
            .as_ref()
            .map(|show| show.id)
            .ok_or_else(|| ApiError::conflict("no show is active"))?;
        if let Some(requested) = self.0 {
            if active != requested {
                return Err(ApiError::conflict(
                    "X-Tosk-Show does not match the active show",
                ));
            }
        }
        Ok(active)
    }

    pub(super) fn verify(&self, state: &AppState) -> Result<(), ApiError> {
        let Some(requested) = self.0 else {
            return Ok(());
        };
        let active = state
            .active_show
            .current()
            .as_ref()
            .map(|show| show.id)
            .ok_or_else(|| ApiError::conflict("no show is active"))?;
        if active != requested {
            return Err(ApiError::conflict(
                "X-Tosk-Show does not match the active show",
            ));
        }
        Ok(())
    }
}

impl FromRequestParts<AppState> for ShowContext {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let context = Self(optional_uuid_header(&parts.headers, SHOW_CONTEXT_HEADER)?.map(ShowId));
        Ok(context)
    }
}

/// The control desk against which a context-sensitive HTTP operation is resolved.
pub(super) struct DeskContext(Option<Uuid>);

impl DeskContext {
    /// There is one desk. `X-Tosk-Desk` used to pick between several; a header naming one from
    /// before the collapse reaches the desk rather than a missing one.
    fn resolve(&self, state: &AppState) -> Result<ControlDesk, ApiError> {
        state.installation.desk().map_err(ApiError::store)
    }
}

impl FromRequestParts<AppState> for DeskContext {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(Self(optional_uuid_header(
            &parts.headers,
            DESK_CONTEXT_HEADER,
        )?))
    }
}

/// Authenticate a request that names a desk.
///
/// The desk context is still resolved, so a header naming nothing at all is still an error — a
/// client sending a malformed desk is a client bug. What is gone is the comparison against the
/// session's own desk: there is one desk, so a header naming a record from before the collapse
/// addresses it rather than being refused as somebody else's.
pub(super) fn session_for_desk(
    state: &AppState,
    headers: &HeaderMap,
    context: &DeskContext,
) -> Result<super::Session, ApiError> {
    let session = super::authenticate(state, headers)?;
    context.resolve(state)?;
    Ok(session)
}

fn optional_uuid_header(headers: &HeaderMap, name: &'static str) -> Result<Option<Uuid>, ApiError> {
    let Some(value) = headers.get(name) else {
        return Ok(None);
    };
    let value = value
        .to_str()
        .map_err(|_| ApiError::bad_request(format!("{name} must be a UUID")))?;
    let id = Uuid::parse_str(value)
        .map_err(|_| ApiError::bad_request(format!("{name} must be a UUID")))?;
    if id.is_nil() {
        return Err(ApiError::bad_request(format!("{name} must not be nil")));
    }
    Ok(Some(id))
}
