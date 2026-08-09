//! A request body that accepts more than it understands.
//!
//! Unknown properties are accepted and logged, never rejected: a newer client talking to an older
//! server must not fail because it sent a field this build has not learned about yet. What is
//! logged is the route and the field path — never the value, which could be operator content.
//!
//! This mirrors the desk's extractor in `crates/light/adapters/headless/src/tolerant_json.rs`
//! rather than inventing a second answer to the same rule. The two are not shared yet because
//! neither product's wire types are shared; extraction follows callers, not speculation.

use std::collections::BTreeSet;

use axum::extract::{FromRequest, MatchedPath, Request};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::de::DeserializeOwned;

use crate::error::ApiError;

/// How many distinct unknown paths one request may log, so a hostile body cannot flood the log.
const MAX_LOGGED_UNKNOWN_FIELDS: usize = 16;

/// Deserializes a JSON body, tolerating unknown fields.
#[derive(Debug, Clone, Copy)]
pub struct TolerantJson<T>(pub T);

impl<S, T> FromRequest<S> for TolerantJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = Response;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        let route = request
            .extensions()
            .get::<MatchedPath>()
            .map(MatchedPath::as_str)
            .unwrap_or_else(|| request.uri().path())
            .to_owned();

        // Axum's own extractor does the typed validation, and its rejection names the field path
        // that failed — which is what the API rules require of a 4xx.
        let axum::Json(raw) = axum::Json::<serde_json::Value>::from_request(request, state)
            .await
            .map_err(invalid)?;
        let bytes = serde_json::to_vec(&raw).expect("a Value always serializes");
        let axum::Json(value) = axum::Json::<T>::from_bytes(&bytes).map_err(invalid)?;

        log_unknown_fields::<T>(&route, &bytes);
        Ok(Self(value))
    }
}

/// Turns axum's rejection into the API's error shape, keeping the message — which names the field
/// path that failed — so a client sees one consistent body for every failure.
fn invalid(rejection: axum::extract::rejection::JsonRejection) -> Response {
    ApiError::bad_request("body-invalid", rejection.body_text()).into_response()
}

fn log_unknown_fields<T: DeserializeOwned>(route: &str, bytes: &[u8]) {
    let mut paths = BTreeSet::new();
    let mut total = 0usize;
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);

    let outcome = serde_ignored::deserialize::<_, _, T>(&mut deserializer, |path| {
        total = total.saturating_add(1);
        if paths.len() < MAX_LOGGED_UNKNOWN_FIELDS {
            paths.insert(path.to_string());
        }
    });

    if outcome.is_err() || total == 0 {
        return;
    }
    tracing::info!(
        %route,
        unknown_fields = total,
        paths = ?paths,
        "accepting a body with fields this build does not know"
    );
}

impl<T: serde::Serialize> IntoResponse for TolerantJson<T> {
    fn into_response(self) -> Response {
        (StatusCode::OK, axum::Json(self.0)).into_response()
    }
}
