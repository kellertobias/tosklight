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
    pub(super) fn verify(&self, state: &AppState) -> Result<(), ApiError> {
        let Some(requested) = self.0 else {
            return Ok(());
        };
        let active = state
            .active_show
            .read()
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
    fn resolve(&self, state: &AppState) -> Result<ControlDesk, ApiError> {
        let store = state.desk.lock();
        match self.0 {
            Some(id) => store
                .control_desk(id)
                .map_err(ApiError::store)?
                .ok_or_else(|| ApiError::not_found("X-Tosk-Desk control desk")),
            None => {
                let mut desks = store.desks().map_err(ApiError::store)?;
                if desks.len() > 1 {
                    desks.sort_by(|left, right| {
                        left.name
                            .to_ascii_lowercase()
                            .cmp(&right.name.to_ascii_lowercase())
                            .then_with(|| left.id.cmp(&right.id))
                    });
                }
                desks
                    .iter()
                    .find(|desk| desk.osc_alias.eq_ignore_ascii_case("main"))
                    .cloned()
                    .or_else(|| desks.into_iter().next())
                    .ok_or_else(|| ApiError::not_found("control desk"))
            }
        }
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

pub(super) fn session_for_desk(
    state: &AppState,
    headers: &HeaderMap,
    context: &DeskContext,
) -> Result<super::Session, ApiError> {
    let session = super::authenticate(state, headers)?;
    let desk = context.resolve(state)?;
    if session.desk.id != desk.id {
        return Err(ApiError::forbidden(
            "the authenticated session does not belong to this desk",
        ));
    }
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
