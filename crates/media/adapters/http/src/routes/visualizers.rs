//! The generated visualizers, and tuning one.

use axum::extract::{Path, State};
use axum::response::{IntoResponse, Response};
use media_application::MediaConfiguration;
use media_domain::MediaAddress;

use crate::error::ApiError;
use crate::routes::ApiState;
use crate::routes::edit::{self, Proceed};
use crate::tolerant::TolerantJson;
use crate::wire::{UpdateVisualizer, VisualizerView};

/// Which generated visualizer answers at which address.
///
/// Configuration rather than state: it changes when an operator reassigns an address, not from
/// frame to frame, so it is read once and not polled.
pub(super) async fn visualizers(State(state): State<ApiState>) -> impl IntoResponse {
    axum::Json(VisualizerView::all(&state.configuration.load().visualizers))
}

/// Edits one configured visualizer.
///
/// An object-intent update: only the fields being changed travel, and the request id makes a
/// retry safe. Unlike a layer selection this is stored configuration, so it is written to disk
/// before it is answered — an operator who tuned a look and restarted must find it there.
pub(super) async fn update_visualizer(
    State(state): State<ApiState>,
    Path((folder, file)): Path<(u8, u8)>,
    TolerantJson(body): TolerantJson<UpdateVisualizer>,
) -> Result<Response, ApiError> {
    if let Proceed::Replay(response) = edit::begin(&state, &body.request_id)? {
        return Ok(response);
    }

    let address = MediaAddress::new(folder, file);
    let mut configuration = MediaConfiguration::clone(&state.configuration.load());
    let entry = configuration
        .visualizers
        .entries
        .iter_mut()
        .find(|entry| entry.address == address)
        .ok_or_else(|| {
            ApiError::not_found(
                "unknown-visualizer",
                format!("no visualizer answers at {address}"),
            )
        })?;

    if let Some(name) = body.name {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(ApiError::bad_request(
                "empty-name",
                "a visualizer needs a name an operator can find it by",
            ));
        }
        entry.configuration.name = trimmed.to_owned();
    }
    if let Some(parameters) = body.parameters {
        entry.configuration.parameters = parameters.into_parameters();
    }

    let view = VisualizerView::of(address, &entry.configuration);
    edit::commit(&state, configuration, &body.request_id, &view)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use crate::routes::bench::{bench, get, post, send};

    #[tokio::test]
    async fn every_shipped_visualizer_is_published_with_its_address_and_its_controls() {
        let bench = bench();
        let (status, body) = send(&bench.router, get("/api/v2/visualizers".into())).await;

        assert_eq!(status, StatusCode::OK);
        let entries = body.as_array().expect("a list");
        assert_eq!(entries.len(), 21);

        let first = &entries[0];
        assert_eq!(first["address"]["folder"], 250);
        assert_eq!(first["address"]["file"], 1);
        assert_eq!(first["address"]["class"], "generated-visualizer");
        assert_eq!(first["typeId"], 0);
        assert_eq!(first["kind"], "Equalizer Bars");
        assert!(
            first["uses"]
                .as_array()
                .expect("a list")
                .contains(&serde_json::json!("count")),
            "an editor is told which controls do something"
        );
        assert_eq!(first["parameters"]["count"], 32);

        let city = entries
            .iter()
            .find(|entry| entry["typeId"] == 52)
            .expect("City Tunnel is a shipped built-in");
        assert_eq!(city["kind"], "City Tunnel");
        assert_eq!(city["address"]["folder"], 250);
        assert!(
            city["uses"]
                .as_array()
                .expect("a list")
                .contains(&serde_json::json!("speed")),
            "the operator can tune tunnel travel speed"
        );
    }

    #[tokio::test]
    async fn an_edited_visualizer_is_stored_before_it_is_answered() {
        let bench = bench();
        let uri = "/api/v2/visualizers/250/1/update".to_owned();
        let (status, body) = send(
            &bench.router,
            post(
                uri,
                r#"{"requestId":"a","name":"House bars","parameters":{"count":64,"size":0.1,"speed":1.0,"amount":1.0,"radius":0.3,"thickness":0.01,"reactivity":1.0,"decay":0.1,"zoom":1.0,"iterations":64,"threshold":0.5,"smoothing":0.5,"gravity":0.5,"lifetime":2.0,"curvature":0.2,"primaryRed":1.0,"primaryGreen":0.0,"primaryBlue":0.0,"secondaryRed":0.0,"secondaryGreen":0.0,"secondaryBlue":1.0,"mirror":true,"filled":false,"wireframe":false,"mode":0}}"#,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["name"], "House bars");
        assert_eq!(body["parameters"]["count"], 64);
        assert_eq!(body["parameters"]["mirror"], true);

        let stored = bench.stored.lock().unwrap();
        assert_eq!(stored.len(), 1, "the edit was written, not just answered");
        let saved = stored[0]
            .visualizers
            .resolve(media_domain::MediaAddress::new(250, 1))
            .expect("still there");
        assert_eq!(saved.name, "House bars");
        assert_eq!(saved.parameters.count, 64);
    }

    #[tokio::test]
    async fn resending_an_edit_answers_it_rather_than_doing_it_twice() {
        let bench = bench();
        let uri = "/api/v2/visualizers/250/1/update".to_owned();
        let edit = r#"{"requestId":"same","name":"First"}"#;

        let (_, first) = send(&bench.router, post(uri.clone(), edit)).await;
        let (status, second) = send(&bench.router, post(uri, edit)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            first, second,
            "a retry gets the outcome of the first attempt"
        );
        assert_eq!(
            bench.stored.lock().unwrap().len(),
            1,
            "and the edit was executed once"
        );
    }

    #[tokio::test]
    async fn an_edit_that_could_not_be_stored_is_not_applied() {
        let bench = bench();
        bench
            .refuse
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let uri = "/api/v2/visualizers/250/1/update".to_owned();

        let (status, body) = send(
            &bench.router,
            post(uri, r#"{"requestId":"b","name":"Never"}"#),
        )
        .await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body["code"], "configuration-not-written");

        let (_, published) = send(&bench.router, get("/api/v2/visualizers".into())).await;
        assert_eq!(
            published[0]["name"], "Equalizer Bars",
            "a change that was not saved must not be live either"
        );
        assert_eq!(
            bench.applied.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "and nothing running was told to honour it"
        );
    }

    #[tokio::test]
    async fn an_edit_without_a_request_id_or_for_an_unknown_address_is_refused() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/visualizers/250/1/update".into(),
                r#"{"requestId":"  ","name":"No"}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "missing-request-id");

        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/visualizers/1/1/update".into(),
                r#"{"requestId":"c","name":"No"}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "unknown-visualizer");
        assert!(bench.stored.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_visualizer_cannot_be_left_nameless() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/visualizers/250/1/update".into(),
                r#"{"requestId":"d","name":"   "}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "empty-name");
    }
}
