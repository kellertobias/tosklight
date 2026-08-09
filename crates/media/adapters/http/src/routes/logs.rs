//! The log.
//!
//! A read, not a stream: an operator opens the viewer, sees the window the process holds, and asks
//! for what has arrived since. The cursor is the sequence number they already have, so a refresh
//! cannot show a record twice or step over one.

use axum::extract::{Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::diagnostics::LogQuery;
use crate::error::ApiError;
use crate::routes::ApiState;
use crate::routes::edit::{self, Proceed};
use crate::tolerant::TolerantJson;
use crate::wire::{LogsView, ServerLogLevelView, UpdateServerLogLevel};

/// How many records one read may carry.
///
/// A venue machine that has been running all day holds more than a browser should be handed at
/// once; a viewer asking for everything since its cursor gets it a page at a time.
const MAX_RECORDS: usize = 500;
const DEFAULT_RECORDS: usize = 200;

/// The query a viewer sends. Unknown parameters are ignored, like every other tolerant body.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LogsQuery {
    after: Option<u64>,
    level: Option<String>,
    limit: Option<usize>,
}

pub(super) async fn logs(
    State(state): State<ApiState>,
    Query(query): Query<LogsQuery>,
) -> Result<Response, ApiError> {
    if let Some(level) = &query.level
        && !is_level(level)
    {
        return Err(ApiError::bad_request(
            "unknown-level",
            format!("no log level is called {level}"),
        ));
    }

    let page = (state.diagnostics.logs)(&LogQuery {
        after: query.after,
        level: query.level,
        limit: query.limit.unwrap_or(DEFAULT_RECORDS).min(MAX_RECORDS),
    });
    Ok(axum::Json(LogsView::of(&page)).into_response())
}

/// The process filter, separate from the viewer filter above.
pub(super) async fn server_level(State(state): State<ApiState>) -> impl IntoResponse {
    axum::Json(ServerLogLevelView::of((state
        .diagnostics
        .log_level
        .current)()))
}

/// Changes the tracing filter for this process only.
///
/// This carries request identity because it is an operator edit, but it intentionally skips the
/// configuration commit path: restart restores `MEDIA_LOG`, exactly as the selected decision says.
pub(super) async fn update_server_level(
    State(state): State<ApiState>,
    TolerantJson(body): TolerantJson<UpdateServerLogLevel>,
) -> Result<Response, ApiError> {
    if let Proceed::Replay(response) = edit::begin(&state, &body.request_id)? {
        return Ok(response);
    }
    if !is_level(&body.level) {
        return Err(ApiError::bad_request(
            "unknown-level",
            format!("no log level is called {}", body.level),
        ));
    }

    (state.diagnostics.log_level.update)(&body.level).map_err(|detail| {
        tracing::error!(%detail, "the process log level could not be changed");
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "log-level-not-changed",
            "the server log level could not be changed",
        )
    })?;
    let view = ServerLogLevelView::of((state.diagnostics.log_level.current)());
    let serialized = serde_json::to_string(&view).unwrap_or_default();
    state.replays.remember(&body.request_id, serialized.clone());
    Ok(([(header::CONTENT_TYPE, "application/json")], serialized).into_response())
}

fn is_level(level: &str) -> bool {
    matches!(level, "error" | "warn" | "info" | "debug" | "trace")
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::Mutex;

    use axum::http::StatusCode;

    use crate::diagnostics::{Diagnostics, LogEntry, LogLevelControl, LogPage, LogQuery};
    use crate::routes::bench::{bench, bench_with, get, send};

    /// A log that records what it was asked for, so a test can prove the cursor travelled.
    fn recording() -> (Diagnostics, Arc<Mutex<Vec<LogQuery>>>) {
        let asked: Arc<Mutex<Vec<LogQuery>>> = Arc::new(Mutex::new(Vec::new()));
        let recorder = Arc::clone(&asked);
        let diagnostics = Diagnostics {
            logs: Arc::new(move |query: &LogQuery| {
                recorder.lock().unwrap().push(query.clone());
                LogPage {
                    entries: vec![LogEntry {
                        sequence: 12,
                        millis_since_start: 3_400,
                        level: "warn".to_owned(),
                        target: "media_runtime".to_owned(),
                        message: "no audio input; generated sources will run on silence".to_owned(),
                    }],
                    newest: 12,
                    dropped: 2,
                    capacity: 2_000,
                }
            }),
            ..Default::default()
        };
        (diagnostics, asked)
    }

    #[tokio::test]
    async fn the_log_is_served_with_what_it_holds_and_what_it_lost() {
        let (diagnostics, _) = recording();
        let bench = bench_with(diagnostics);
        let (status, body) = send(&bench.router, get("/api/v2/logs".into())).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["records"][0]["sequence"], 12);
        assert_eq!(body["records"][0]["level"], "warn");
        assert!(
            body["records"][0]["message"]
                .as_str()
                .unwrap()
                .contains("no audio input")
        );
        assert_eq!(body["newest"], 12);
        assert_eq!(body["dropped"], 2);
    }

    #[tokio::test]
    async fn a_viewer_asks_for_what_arrived_after_the_record_it_already_has() {
        let (diagnostics, asked) = recording();
        let bench = bench_with(diagnostics);
        send(
            &bench.router,
            get("/api/v2/logs?after=7&level=warn&limit=50".into()),
        )
        .await;

        let asked = asked.lock().unwrap();
        assert_eq!(asked[0].after, Some(7));
        assert_eq!(asked[0].level.as_deref(), Some("warn"));
        assert_eq!(asked[0].limit, 50);
    }

    #[tokio::test]
    async fn a_page_is_bounded_however_much_is_asked_for() {
        let (diagnostics, asked) = recording();
        let bench = bench_with(diagnostics);
        send(&bench.router, get("/api/v2/logs?limit=100000".into())).await;

        assert_eq!(
            asked.lock().unwrap()[0].limit,
            500,
            "a browser is not handed a whole day of records at once"
        );
    }

    #[tokio::test]
    async fn a_level_that_is_not_a_level_is_refused_by_name() {
        let bench = bench();
        let (status, body) = send(&bench.router, get("/api/v2/logs?level=shouting".into())).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "unknown-level");
        assert!(body["message"].as_str().unwrap().contains("shouting"));
    }

    #[tokio::test]
    async fn a_process_that_keeps_no_log_answers_with_an_empty_one() {
        let bench = bench();
        let (status, body) = send(&bench.router, get("/api/v2/logs".into())).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body["records"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn the_process_level_changes_for_this_run_and_replays_safely() {
        let level = Arc::new(Mutex::new("info".to_owned()));
        let reads = Arc::clone(&level);
        let writes = Arc::clone(&level);
        let diagnostics = Diagnostics {
            log_level: LogLevelControl {
                current: Arc::new(move || reads.lock().unwrap().clone()),
                update: Arc::new(move |next| {
                    *writes.lock().unwrap() = next.to_owned();
                    Ok(())
                }),
            },
            ..Default::default()
        };
        let bench = bench_with(diagnostics);

        let (status, before) = send(&bench.router, get("/api/v2/logs/level".into())).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(before["level"], "info");
        assert_eq!(before["resetsOnRestart"], true);

        let request = r#"{"requestId":"level-1","level":"debug"}"#;
        let (status, changed) = send(
            &bench.router,
            crate::routes::bench::post("/api/v2/logs/level/update".into(), request),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(changed["level"], "debug");

        *level.lock().unwrap() = "trace".to_owned();
        let (_, replayed) = send(
            &bench.router,
            crate::routes::bench::post("/api/v2/logs/level/update".into(), request),
        )
        .await;
        assert_eq!(replayed["level"], "debug", "a retry gets its first answer");
        assert_eq!(
            *level.lock().unwrap(),
            "trace",
            "a retry was not applied again"
        );
    }

    #[tokio::test]
    async fn an_unknown_process_level_is_refused_without_calling_the_runtime() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            crate::routes::bench::post(
                "/api/v2/logs/level/update".into(),
                r#"{"requestId":"level-2","level":"notice"}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "unknown-level");
    }
}
