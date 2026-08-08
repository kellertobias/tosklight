//! The GDTF fixtures a console imports to patch this server.
//!
//! Generated from the canonical personality on request rather than shipped as files, so a channel
//! cannot exist on the wire and be missing from what an operator patches.

use axum::extract::Path;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::error::ApiError;

pub(super) async fn fixtures() -> impl IntoResponse {
    let names: Vec<String> = media_application::gdtf::packages()
        .map(|packaged| packaged.into_iter().map(|(name, _)| name).collect())
        .unwrap_or_default();
    axum::Json(names)
}

/// One fixture, as a `.gdtf` archive.
pub(super) async fn fixture(Path(name): Path<String>) -> Result<Response, ApiError> {
    let packaged = media_application::gdtf::packages().map_err(|error| {
        tracing::error!(%error, "the GDTF fixtures could not be generated");
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "fixture-not-generated",
            "the fixture file could not be built",
        )
    })?;
    let (_, bytes) = packaged
        .into_iter()
        .find(|(candidate, _)| *candidate == name)
        .ok_or_else(|| {
            ApiError::not_found("unknown-fixture", format!("this server has no {name}"))
        })?;

    Ok((
        [
            (header::CONTENT_TYPE, "application/gdtf".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{name}\""),
            ),
        ],
        bytes,
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use axum::http::{StatusCode, header};
    use http_body_util::BodyExt as _;
    use tower::ServiceExt as _;

    use crate::routes::bench::{bench, get, send};

    #[tokio::test]
    async fn the_gdtf_fixtures_are_generated_on_request() {
        let bench = bench();
        let (status, body) = send(&bench.router, get("/api/v2/fixtures".into())).await;
        assert_eq!(status, StatusCode::OK);
        let names: Vec<String> = serde_json::from_value(body).expect("a list of names");
        assert_eq!(names.len(), 2, "one layer fixture and one master");

        let response = bench
            .router
            .clone()
            .oneshot(get(format!(
                "/api/v2/fixtures/{}",
                names[0].replace(' ', "%20")
            )))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("application/gdtf")
        );
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&bytes[..2], b"PK", "a GDTF file is an archive");
    }

    #[tokio::test]
    async fn asking_for_a_fixture_this_server_does_not_have_says_so() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            get("/api/v2/fixtures/Something%20Else.gdtf".into()),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "unknown-fixture");
    }
}
