//! Text sources.
//!
//! A text slot is stored configuration, so all three edits carry a request id and are written
//! before they are answered. Which addresses are legal is the text catalog's rule — this route
//! translates its refusals into stable error codes rather than re-deciding them.

use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use media_application::MediaConfiguration;
use media_domain::MediaAddress;
use media_domain::text_catalog::TextCatalogError;

use crate::error::ApiError;
use crate::routes::ApiState;
use crate::routes::edit::{self, Proceed};
use crate::tolerant::TolerantJson;
use crate::wire::{CreateText, DeleteText, TextSlotView, UpdateText};

/// Which text entry answers at which address.
pub(super) async fn text(State(state): State<ApiState>) -> impl IntoResponse {
    let configuration = state.configuration.load();
    let views: Vec<TextSlotView> = configuration
        .text
        .slots
        .iter()
        .map(TextSlotView::of)
        .collect();
    axum::Json(views)
}

/// Puts a new text source at an address.
pub(super) async fn create_text(
    State(state): State<ApiState>,
    TolerantJson(body): TolerantJson<CreateText>,
) -> Result<Response, ApiError> {
    if let Proceed::Replay(response) = edit::begin(&state, &body.request_id)? {
        return Ok(response);
    }

    let slot = body
        .slot()
        .map_err(|error| ApiError::bad_request("text-invalid", error.to_string()))?;
    let mut configuration = MediaConfiguration::clone(&state.configuration.load());
    configuration
        .text
        .assign(slot.clone())
        .map_err(refused_assignment)?;

    let view = TextSlotView::of(&slot);
    edit::commit(&state, configuration, &body.request_id, &view)
}

/// Edits one text source.
pub(super) async fn update_text(
    State(state): State<ApiState>,
    Path((folder, file)): Path<(u8, u8)>,
    TolerantJson(body): TolerantJson<UpdateText>,
) -> Result<Response, ApiError> {
    if let Proceed::Replay(response) = edit::begin(&state, &body.request_id)? {
        return Ok(response);
    }

    let address = MediaAddress::new(folder, file);
    let mut configuration = MediaConfiguration::clone(&state.configuration.load());
    let slot = configuration
        .text
        .slots
        .iter_mut()
        .find(|slot| slot.address == address)
        .ok_or_else(|| unknown_text(address))?;
    body.apply(slot)
        .map_err(|error| ApiError::bad_request("text-invalid", error.to_string()))?;

    let view = TextSlotView::of(slot);
    edit::commit(&state, configuration, &body.request_id, &view)
}

/// Takes a text source away.
///
/// The address stops resolving, so a layer pointing at it reports a failed source rather than
/// drawing the words that used to be there.
pub(super) async fn delete_text(
    State(state): State<ApiState>,
    Path((folder, file)): Path<(u8, u8)>,
    TolerantJson(body): TolerantJson<DeleteText>,
) -> Result<Response, ApiError> {
    if let Proceed::Replay(response) = edit::begin(&state, &body.request_id)? {
        return Ok(response);
    }

    let address = MediaAddress::new(folder, file);
    let mut configuration = MediaConfiguration::clone(&state.configuration.load());
    if !configuration.text.remove(address) {
        return Err(unknown_text(address));
    }

    let views: Vec<TextSlotView> = configuration
        .text
        .slots
        .iter()
        .map(TextSlotView::of)
        .collect();
    edit::commit(&state, configuration, &body.request_id, &views)
}

fn refused_assignment(error: TextCatalogError) -> ApiError {
    match error {
        TextCatalogError::NotTextSpace => ApiError::bad_request(
            "not-text-space",
            "text lives in folders 200 to 249, and file 0 and file 255 are blank in every bank",
        ),
        TextCatalogError::AddressTaken => ApiError::bad_request(
            "address-taken",
            "another text source already answers at that address",
        ),
    }
}

fn unknown_text(address: MediaAddress) -> ApiError {
    ApiError::not_found("unknown-text", format!("no text answers at {address}"))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use crate::routes::bench::{bench, get, post, send};

    #[tokio::test]
    async fn the_shipped_text_sources_are_published_with_their_addresses_and_payloads() {
        let bench = bench();
        let (status, body) = send(&bench.router, get("/api/v2/text".into())).await;

        assert_eq!(status, StatusCode::OK);
        let slots = body.as_array().expect("a list");
        assert_eq!(slots.len(), 2);
        assert_eq!(slots[0]["address"]["folder"], 200);
        assert_eq!(slots[0]["address"]["class"], "text-bank");
        assert_eq!(slots[0]["kind"], "clock");
        assert_eq!(slots[1]["kind"], "countdown-duration");
        assert_eq!(slots[1]["durationSeconds"], 600.0);
        assert_eq!(slots[0]["style"]["alignment"], "center");
    }

    #[tokio::test]
    async fn a_new_text_source_is_stored_at_the_address_it_chose() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/text/create".into(),
                r#"{"requestId":"a","folder":201,"file":4,"name":"House open","kind":"static","text":"Doors in five"}"#,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["address"]["folder"], 201);
        assert_eq!(body["address"]["file"], 4);
        assert_eq!(body["text"], "Doors in five");

        let stored = bench.stored.lock().unwrap();
        assert_eq!(stored.len(), 1);
        assert!(
            stored[0]
                .text
                .resolve(media_domain::MediaAddress::new(201, 4))
                .is_some(),
            "the new slot reached storage"
        );
    }

    #[tokio::test]
    async fn text_may_only_be_put_where_text_lives() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/text/create".into(),
                r#"{"requestId":"a","folder":1,"file":4,"name":"Cue","kind":"clock"}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "not-text-space");

        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/text/create".into(),
                r#"{"requestId":"b","folder":200,"file":1,"name":"Second clock","kind":"clock"}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "address-taken");
        assert!(bench.stored.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn an_edit_changes_only_what_it_carries_and_is_stored_first() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/text/200/2/update".into(),
                r#"{"requestId":"a","durationSeconds":90}"#,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["durationSeconds"], 90.0);
        assert_eq!(body["name"], "Ten minutes", "nothing else moved");

        let stored = bench.stored.lock().unwrap();
        let saved = stored[0]
            .text
            .resolve(media_domain::MediaAddress::new(200, 2))
            .expect("still there");
        assert_eq!(
            saved.entry.kind,
            media_domain::text::TextKind::CountdownFromDuration {
                duration: std::time::Duration::from_secs(90)
            }
        );
    }

    #[tokio::test]
    async fn an_unusable_edit_is_refused_and_leaves_the_slot_alone() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/text/200/1/update".into(),
                r#"{"requestId":"a","kind":"countdown-duration"}"#,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "text-invalid");
        assert!(
            body["message"]
                .as_str()
                .unwrap()
                .contains("durationSeconds")
        );
        assert!(bench.stored.lock().unwrap().is_empty());
        assert_eq!(
            bench
                .configuration
                .load()
                .text
                .resolve(media_domain::MediaAddress::new(200, 1))
                .expect("still there")
                .entry
                .kind,
            media_domain::text::TextKind::Clock
        );
    }

    #[tokio::test]
    async fn a_text_source_can_be_taken_away_and_asking_twice_says_so() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            post("/api/v2/text/200/2/delete".into(), r#"{"requestId":"a"}"#),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            body.as_array().expect("what is left").len(),
            1,
            "the answer is the catalog that remains"
        );

        let (status, body) = send(
            &bench.router,
            post("/api/v2/text/200/2/delete".into(), r#"{"requestId":"b"}"#),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "unknown-text");
    }

    #[tokio::test]
    async fn editing_an_address_with_no_text_says_so() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/text/205/9/update".into(),
                r#"{"requestId":"a","name":"Nothing here"}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "unknown-text");
    }

    #[tokio::test]
    async fn a_retried_creation_does_not_make_a_second_slot() {
        let bench = bench();
        let body = r#"{"requestId":"same","folder":202,"file":1,"name":"Cue","kind":"clock"}"#;

        let (_, first) = send(&bench.router, post("/api/v2/text/create".into(), body)).await;
        let (status, second) = send(&bench.router, post("/api/v2/text/create".into(), body)).await;

        assert_eq!(status, StatusCode::OK, "not the address-taken refusal");
        assert_eq!(first, second);
        assert_eq!(bench.stored.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn an_edit_the_disk_refused_is_not_applied() {
        let bench = bench();
        bench
            .refuse
            .store(true, std::sync::atomic::Ordering::SeqCst);

        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/text/200/1/update".into(),
                r#"{"requestId":"a","name":"Never"}"#,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body["code"], "configuration-not-written");
        assert_eq!(
            bench
                .configuration
                .load()
                .text
                .resolve(media_domain::MediaAddress::new(200, 1))
                .expect("still there")
                .name,
            "Clock"
        );
    }
}
