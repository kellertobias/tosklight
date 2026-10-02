//! `GET /api/v2/programming/color/native-pages`: Direct (native) Color pages 3/4, the modal
//! overflow and the reference head of one ordered fixture selection (TL-554).
//!
//! API rules: a read (rule 1) answered with a complete typed snapshot (rule 5); the optional
//! `X-Tosk-Show` guard rejects a show-switch race with 409 and a malformed id with 400 (rule 6);
//! there is no show or desk segment in the path. Unknown query parameters are ignored. The
//! response is `no-store`: it follows the patch, the runtime contract and the accepted output.
//! Reading it never changes the Programmer, its history or any lease.

use super::{ApiError, AppState, ShowContext, authenticate};
use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, header},
    response::{IntoResponse, Response},
    routing::get,
};
use light_core::FixtureId;
use serde::Deserialize;
use uuid::Uuid;

/// Fixtures per request; matches the family-encoder-pages and readout routes.
const MAX_PAGE_FIXTURES: usize = 512;

pub(super) fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v2/programming/color/native-pages",
        get(native_color_pages),
    )
}

#[derive(Deserialize)]
struct NativePagesQuery {
    /// Comma-separated fixture ids in selection order; duplicates are kept in the echo.
    #[serde(default)]
    fixture_ids: Option<String>,
    /// The operator's reference fixture (a member of the selection).
    #[serde(default)]
    reference: Option<String>,
    /// The reference fixture's head; the first verified root head when absent.
    #[serde(default)]
    head: Option<String>,
}

async fn native_color_pages(
    State(state): State<AppState>,
    show: ShowContext,
    headers: HeaderMap,
    Query(query): Query<NativePagesQuery>,
) -> Result<Response, ApiError> {
    authenticate(&state, &headers)?;
    show.verify(&state)?;
    let requested = requested_fixtures(query.fixture_ids.as_deref())?;
    let reference = parse_id(query.reference.as_deref(), "reference")?.map(FixtureId);
    let head = parse_id(query.head.as_deref(), "head")?;
    if head.is_some() && reference.is_none() {
        return Err(ApiError::bad_request("head: requires reference"));
    }
    let pages = super::native_color_pages::native_color_pages(
        &state,
        &requested,
        reference.map(|reference| (reference, head)),
    );
    Ok(([(header::CACHE_CONTROL, "no-store")], Json(pages)).into_response())
}

fn parse_id(value: Option<&str>, field: &str) -> Result<Option<Uuid>, ApiError> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            Uuid::parse_str(value.trim())
                .map_err(|_| ApiError::bad_request(format!("{field}: invalid id `{value}`")))
        })
        .transpose()
}

fn requested_fixtures(list: Option<&str>) -> Result<Vec<FixtureId>, ApiError> {
    let fixtures = list
        .filter(|list| !list.trim().is_empty())
        .map(|list| {
            list.split(',')
                .map(|id| {
                    Uuid::parse_str(id.trim()).map(FixtureId).map_err(|_| {
                        ApiError::bad_request(format!("fixture_ids: invalid fixture id `{id}`"))
                    })
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    if fixtures.len() > MAX_PAGE_FIXTURES {
        return Err(ApiError::bad_request(format!(
            "fixture_ids: at most {MAX_PAGE_FIXTURES} fixtures per request"
        )));
    }
    Ok(fixtures)
}
