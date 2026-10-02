//! `GET /api/v2/programming/family-encoder-pages`: the semantic family encoder pages and slot
//! descriptors of one fixture selection (TL-549/550/551 UI foundation).
//!
//! API rules: a read (rule 1) answered with a complete typed snapshot (rule 5); the optional
//! `X-Tosk-Show` guard rejects a show-switch race with 409 and a malformed id with 400 (rule 6);
//! there is no show or desk segment in the path. Unknown query parameters are ignored. The
//! response is `no-store`: it follows the patch, the runtime contract and the desk's Color
//! presentation, each of which can change without a URL change.

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

/// Fixtures per request; matches the readout route's bound.
const MAX_PAGE_FIXTURES: usize = 512;

pub(super) fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v2/programming/family-encoder-pages",
        get(family_encoder_pages),
    )
}

#[derive(Deserialize)]
struct PagesQuery {
    /// Comma-separated fixture ids in selection order; duplicates are kept.
    #[serde(default)]
    fixture_ids: Option<String>,
}

async fn family_encoder_pages(
    State(state): State<AppState>,
    show: ShowContext,
    headers: HeaderMap,
    Query(query): Query<PagesQuery>,
) -> Result<Response, ApiError> {
    authenticate(&state, &headers)?;
    show.verify(&state)?;
    let requested = requested_fixtures(query.fixture_ids.as_deref())?;
    let presentation = state.installation.configuration().color_presentation;
    let engine = state.output.engine();
    let snapshot = engine.snapshot();
    let pages = super::family_encoder_pages::family_encoder_pages(
        &super::family_encoder_pages::FamilyEncoderInputs {
            fixtures: &snapshot.fixtures,
            requested: &requested,
            supported_contract: engine.supported_programming_contract(),
            presentation,
            show_revision: snapshot.revision,
        },
    );
    Ok(([(header::CACHE_CONTROL, "no-store")], Json(pages)).into_response())
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
