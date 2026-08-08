//! The shape every configuration edit shares.
//!
//! An edit of stored configuration is not live control: it carries a client-generated request id,
//! it is written to disk before it is answered, and it is *not applied* if it could not be stored.
//! An operator who tuned a look and restarted must find it there; one whose disk was full must not
//! be shown a change the next start will not have.
//!
//! Every editing route goes through here, so none of them can accidentally acquire a different
//! order of those three steps.

use std::sync::Arc;

use axum::http::{StatusCode, header};
use axum::response::{IntoResponse as _, Response};
use media_application::MediaConfiguration;
use serde::Serialize;

use crate::error::ApiError;
use crate::routes::ApiState;

/// What an edit does next, once its identity has been checked.
pub enum Proceed {
    /// This request id has been seen; answer with what it produced the first time.
    Replay(Response),
    /// A first attempt. Carry on and edit.
    Fresh,
}

/// Refuses an edit with no identity, and answers a retry rather than executing it again.
pub fn begin(state: &ApiState, request_id: &str) -> Result<Proceed, ApiError> {
    if request_id.trim().is_empty() {
        return Err(ApiError::bad_request(
            "missing-request-id",
            "an edit must carry a request id so a retry cannot become a second edit",
        ));
    }
    if let Some(stored) = state.replays.stored(request_id) {
        return Ok(Proceed::Replay(
            ([(header::CONTENT_TYPE, "application/json")], stored).into_response(),
        ));
    }
    Ok(Proceed::Fresh)
}

/// Stores an accepted configuration, publishes it, and answers with the projection.
///
/// The order is the contract: written, then live, then answered. Nothing between the write and the
/// publish can fail, so a stored edit is never invisible and a visible edit is never unstored.
pub fn commit<T: Serialize>(
    state: &ApiState,
    configuration: MediaConfiguration,
    request_id: &str,
    view: &T,
) -> Result<Response, ApiError> {
    (state.persist)(&configuration).map_err(|detail| {
        tracing::error!(%detail, "an accepted edit could not be stored");
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "configuration-not-written",
            "the change could not be saved; it has not been applied",
        )
    })?;
    let stored = Arc::new(configuration);
    state.configuration.store(Arc::clone(&stored));
    // Whatever is already running and can honour the change immediately does so now — after it
    // is both stored and published, so a subsystem never runs ahead of what a reload would show.
    (state.apply)(&stored);

    let serialized = serde_json::to_string(view).unwrap_or_default();
    state.replays.remember(request_id, serialized.clone());
    Ok(([(header::CONTENT_TYPE, "application/json")], serialized).into_response())
}
