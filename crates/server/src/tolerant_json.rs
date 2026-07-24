use std::collections::BTreeSet;

use axum::{
    Json,
    extract::{FromRequest, MatchedPath, Request, rejection::JsonRejection},
};
use serde::de::DeserializeOwned;

const MAX_LOGGED_UNKNOWN_FIELDS: usize = 32;
const MAX_LOGGED_FIELD_PATH_LENGTH: usize = 256;

/// JSON request body that keeps known-field validation strict while accepting additional fields.
///
/// Unknown field paths are logged once per request with the matched route template. Values are
/// deliberately never included in the log entry.
pub(crate) struct TolerantJson<T>(pub(crate) T);

pub(crate) fn log_unknown_value_fields<T>(route: &str, raw: &serde_json::Value)
where
    T: DeserializeOwned,
{
    let bytes = serde_json::to_vec(raw).expect("a serde_json::Value always serializes as JSON");
    collect_and_log_unknown_fields::<T>(route, &bytes);
}

impl<T, S> FromRequest<S> for TolerantJson<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = JsonRejection;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let route = req
            .extensions()
            .get::<MatchedPath>()
            .map(MatchedPath::as_str)
            .unwrap_or_else(|| req.uri().path())
            .to_owned();
        let Json(raw) = Json::<serde_json::Value>::from_request(req, state).await?;
        let bytes =
            serde_json::to_vec(&raw).expect("a serde_json::Value always serializes as JSON");
        let Json(value) = Json::<T>::from_bytes(&bytes)?;

        collect_and_log_unknown_fields::<T>(&route, &bytes);

        Ok(Self(value))
    }
}

fn collect_and_log_unknown_fields<T>(route: &str, bytes: &[u8])
where
    T: DeserializeOwned,
{
    let mut unknown_fields = BTreeSet::new();
    let mut unknown_field_count = 0usize;
    let mut fields_truncated = false;
    let mut deserializer = serde_json::Deserializer::from_slice(&bytes);
    let ignored_result = serde_ignored::deserialize::<_, _, T>(&mut deserializer, |path| {
        unknown_field_count = unknown_field_count.saturating_add(1);
        let path = truncate_field_path(path.to_string());
        if !unknown_fields.contains(&path) {
            if unknown_fields.len() < MAX_LOGGED_UNKNOWN_FIELDS {
                unknown_fields.insert(path);
            } else {
                fields_truncated = true;
            }
        }
    });

    if ignored_result.is_err() {
        tracing::error!(
            route = %route,
            "tolerant JSON field collection failed after the same body passed typed validation"
        );
    } else if unknown_field_count > 0 {
        log_unknown_fields(
            &route,
            unknown_field_count,
            unknown_fields,
            fields_truncated,
        );
    }
}

fn truncate_field_path(mut path: String) -> String {
    if path.len() > MAX_LOGGED_FIELD_PATH_LENGTH {
        path.truncate(MAX_LOGGED_FIELD_PATH_LENGTH);
        path.push('…');
    }
    path
}

fn log_unknown_fields(
    route: &str,
    unknown_field_count: usize,
    unknown_fields: BTreeSet<String>,
    fields_truncated: bool,
) {
    tracing::warn!(
        route,
        unknown_field_count,
        unknown_fields = ?unknown_fields,
        fields_truncated,
        "accepted unknown JSON request fields"
    );
}

#[cfg(test)]
mod tests {
    use std::{
        io,
        sync::{Arc, Mutex},
    };

    use axum::{
        Router,
        body::{Body, to_bytes},
        http::{Request, StatusCode, header},
        routing::post,
    };
    use serde::Deserialize;
    use tower::ServiceExt;
    use tracing_subscriber::fmt::MakeWriter;

    use super::TolerantJson;

    #[derive(Clone, Default)]
    struct SharedLog(Arc<Mutex<Vec<u8>>>);

    struct SharedLogWriter(Arc<Mutex<Vec<u8>>>);

    impl io::Write for SharedLogWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl<'writer> MakeWriter<'writer> for SharedLog {
        type Writer = SharedLogWriter;

        fn make_writer(&'writer self) -> Self::Writer {
            SharedLogWriter(self.0.clone())
        }
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct TestBody {
        count: u64,
        nested: NestedBody,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct NestedBody {
        enabled: bool,
    }

    async fn echo(TolerantJson(body): TolerantJson<TestBody>) -> String {
        format!("{}:{}", body.count, body.nested.enabled)
    }

    fn request(body: serde_json::Value) -> Request<Body> {
        Request::post("/test")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn unknown_fields_are_accepted_and_logged_without_values() {
        let log = SharedLog::default();
        let subscriber = tracing_subscriber::fmt()
            .without_time()
            .with_ansi(false)
            .with_writer(log.clone())
            .finish();
        let _guard = tracing::subscriber::set_default(subscriber);
        let app = Router::new().route("/test", post(echo));

        let response = app
            .oneshot(request(serde_json::json!({
                "count": 7,
                "nested": {
                    "enabled": true,
                    "future_flag": "do-not-log-this-value"
                },
                "future_root": 12345
            })))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            to_bytes(response.into_body(), usize::MAX).await.unwrap(),
            "7:true"
        );
        let output = String::from_utf8(log.0.lock().unwrap().clone()).unwrap();
        assert!(output.contains("route=\"/test\""));
        assert!(output.contains("future_root"));
        assert!(output.contains("nested.future_flag"));
        assert!(!output.contains("do-not-log-this-value"));
        assert!(!output.contains("12345"));
    }

    #[tokio::test]
    async fn known_field_type_mismatch_is_a_path_naming_4xx() {
        let app = Router::new().route("/test", post(echo));
        let response = app
            .oneshot(request(serde_json::json!({
                "count": "seven",
                "nested": {"enabled": true}
            })))
            .await
            .unwrap();

        assert!(response.status().is_client_error());
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert!(String::from_utf8(body.to_vec()).unwrap().contains("count"));
    }

    #[tokio::test]
    async fn clean_body_keeps_the_typed_behavior() {
        let app = Router::new().route("/test", post(echo));
        let response = app
            .oneshot(request(serde_json::json!({
                "count": 7,
                "nested": {"enabled": false}
            })))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            to_bytes(response.into_body(), usize::MAX).await.unwrap(),
            "7:false"
        );
    }
}
