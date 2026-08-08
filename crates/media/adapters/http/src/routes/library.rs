//! Importing media into the library.
//!
//! Starting an import is neither live control nor a configuration edit: it is work handed to the
//! process, so it answers with job identities and the jobs report themselves afterwards. No
//! connection is held for the length of a transcode.
//!
//! It still carries a request id, because starting one is not idempotent — a dropped response
//! followed by a retry would transcode a library twice — and the replay window answers the retry
//! with what the first attempt started.

use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use media_domain::MediaAddress;

use crate::error::ApiError;
use crate::routes::ApiState;
use crate::routes::edit::{self, Proceed};
use crate::tolerant::TolerantJson;
use crate::wire::{ImportJobView, ImportsView, PendingImportView, StartImport};

/// What is waiting to be imported, and what every import this run has done.
pub(super) async fn imports(State(state): State<ApiState>) -> impl IntoResponse {
    axum::Json(view_of(&state))
}

/// Starts importing: everything waiting, or one address.
pub(super) async fn start_import(
    State(state): State<ApiState>,
    TolerantJson(body): TolerantJson<StartImport>,
) -> Result<Response, ApiError> {
    if let Proceed::Replay(response) = edit::begin(&state, &body.request_id)? {
        return Ok(response);
    }
    if !state.diagnostics.imports.available {
        return Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "cannot-import",
            "this machine cannot convert media: FFmpeg is not installed or not on PATH",
        ));
    }

    // Naming half an address is not a selection, it is a mistake, and importing a whole folder
    // when one file was meant would be a long and surprising job.
    let address = match (body.folder, body.file) {
        (Some(folder), Some(file)) => Some(MediaAddress::new(folder, file)),
        (None, None) => None,
        _ => {
            return Err(ApiError::bad_request(
                "incomplete-address",
                "name both a folder and a file, or neither to import everything waiting",
            ));
        }
    };

    let started = (state.diagnostics.imports.start)(address);
    if started == 0 {
        return Err(ApiError::not_found(
            "nothing-to-import",
            match address {
                Some(address) => format!("nothing at {address} is waiting to be imported"),
                None => "nothing in the library is waiting to be imported".to_owned(),
            },
        ));
    }

    let view = view_of(&state);
    let serialized = serde_json::to_string(&view).unwrap_or_default();
    state.replays.remember(&body.request_id, serialized.clone());
    Ok(([(header::CONTENT_TYPE, "application/json")], serialized).into_response())
}

/// Stops one import. A payload-free action, so it is a `GET` an integrator can trigger.
pub(super) async fn cancel_import(
    State(state): State<ApiState>,
    Path(job): Path<String>,
) -> Result<Response, ApiError> {
    if !(state.diagnostics.imports.cancel)(&job) {
        return Err(ApiError::not_found(
            "unknown-import",
            "no import with that identity is still running",
        ));
    }
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        StatusCode::NO_CONTENT,
    )
        .into_response())
}

fn view_of(state: &ApiState) -> ImportsView {
    let (pending, jobs) = (state.diagnostics.imports.state)();
    ImportsView {
        pending: pending.iter().map(PendingImportView::of).collect(),
        jobs: jobs.iter().map(ImportJobView::of).collect(),
        can_import: state.diagnostics.imports.available,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use axum::http::StatusCode;

    use crate::diagnostics::{Diagnostics, ImportJob, ImportOutcome, Imports, PendingImport};
    use crate::routes::bench::{bench, bench_with, get, post, send};

    /// A process with two files waiting and a record of what it was asked to start.
    fn ready() -> (
        Diagnostics,
        Arc<Mutex<Vec<Option<media_domain::MediaAddress>>>>,
    ) {
        let asked: Arc<Mutex<Vec<Option<media_domain::MediaAddress>>>> =
            Arc::new(Mutex::new(Vec::new()));
        let recorder = Arc::clone(&asked);
        let diagnostics = Diagnostics {
            imports: Imports {
                state: Arc::new(|| {
                    (
                        vec![
                            PendingImport {
                                destination: media_domain::MediaAddress::new(1, 1),
                                name: "Bars".to_owned(),
                                filename: "001-Bars.mp4".to_owned(),
                            },
                            PendingImport {
                                destination: media_domain::MediaAddress::new(1, 4),
                                name: "LoopTest".to_owned(),
                                filename: "004-LoopTest.mp4".to_owned(),
                            },
                        ],
                        vec![ImportJob {
                            id: "job-1".to_owned(),
                            destination: media_domain::MediaAddress::new(2, 1),
                            filename: "001.png".to_owned(),
                            outcome: ImportOutcome::Running,
                            fraction: Some(0.5),
                            frames_done: Some(50),
                            frames_total: Some(100),
                        }],
                    )
                }),
                start: Arc::new(move |address| {
                    recorder.lock().unwrap().push(address);
                    2
                }),
                cancel: Arc::new(|job| job == "job-1"),
                available: true,
            },
            ..Default::default()
        };
        (diagnostics, asked)
    }

    #[tokio::test]
    async fn the_library_reports_what_is_waiting_and_what_is_running() {
        let (diagnostics, _) = ready();
        let bench = bench_with(diagnostics);
        let (status, body) = send(&bench.router, get("/api/v2/library/imports".into())).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["canImport"], true);
        assert_eq!(body["pending"][0]["address"]["folder"], 1);
        assert_eq!(body["pending"][0]["filename"], "001-Bars.mp4");
        assert_eq!(body["pending"][1]["name"], "LoopTest");
        assert_eq!(body["jobs"][0]["state"], "running");
        assert_eq!(body["jobs"][0]["fraction"], 0.5);
    }

    #[tokio::test]
    async fn importing_everything_waiting_names_no_address() {
        let (diagnostics, asked) = ready();
        let bench = bench_with(diagnostics);
        let (status, body) = send(
            &bench.router,
            post("/api/v2/library/import".into(), r#"{"requestId":"a"}"#),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(asked.lock().unwrap()[0], None, "everything waiting");
        assert!(body["jobs"].as_array().is_some());
    }

    #[tokio::test]
    async fn one_address_imports_only_that_one() {
        let (diagnostics, asked) = ready();
        let bench = bench_with(diagnostics);
        send(
            &bench.router,
            post(
                "/api/v2/library/import".into(),
                r#"{"requestId":"a","folder":1,"file":4}"#,
            ),
        )
        .await;

        assert_eq!(
            asked.lock().unwrap()[0],
            Some(media_domain::MediaAddress::new(1, 4))
        );
    }

    #[tokio::test]
    async fn half_an_address_is_refused_rather_than_guessed_at() {
        let (diagnostics, asked) = ready();
        let bench = bench_with(diagnostics);
        let (status, body) = send(
            &bench.router,
            post(
                "/api/v2/library/import".into(),
                r#"{"requestId":"a","folder":1}"#,
            ),
        )
        .await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["code"], "incomplete-address");
        assert!(
            asked.lock().unwrap().is_empty(),
            "a folder's worth of transcoding did not start by accident"
        );
    }

    #[tokio::test]
    async fn a_retried_start_does_not_transcode_the_library_twice() {
        let (diagnostics, asked) = ready();
        let bench = bench_with(diagnostics);
        let body = r#"{"requestId":"same"}"#;

        let (_, first) = send(&bench.router, post("/api/v2/library/import".into(), body)).await;
        let (status, second) =
            send(&bench.router, post("/api/v2/library/import".into(), body)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(first, second);
        assert_eq!(asked.lock().unwrap().len(), 1, "it was started once");
    }

    #[tokio::test]
    async fn a_machine_that_cannot_transcode_says_so_before_anything_is_queued() {
        let bench = bench();
        let (status, body) = send(
            &bench.router,
            post("/api/v2/library/import".into(), r#"{"requestId":"a"}"#),
        )
        .await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["code"], "cannot-import");
        assert!(body["message"].as_str().unwrap().contains("FFmpeg"));
    }

    #[tokio::test]
    async fn importing_when_nothing_is_waiting_says_so() {
        let diagnostics = Diagnostics {
            imports: Imports {
                available: true,
                start: Arc::new(|_| 0),
                ..Default::default()
            },
            ..Default::default()
        };
        let bench = bench_with(diagnostics);
        let (status, body) = send(
            &bench.router,
            post("/api/v2/library/import".into(), r#"{"requestId":"a"}"#),
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "nothing-to-import");
    }

    #[tokio::test]
    async fn a_running_import_can_be_stopped_and_stopping_it_twice_says_so() {
        let (diagnostics, _) = ready();
        let bench = bench_with(diagnostics);

        use tower::ServiceExt as _;
        let response = bench
            .router
            .clone()
            .oneshot(get("/api/v2/library/imports/job-1/cancel".into()))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            response.headers()[axum::http::header::CACHE_CONTROL],
            "no-store"
        );

        let (status, body) = send(
            &bench.router,
            get("/api/v2/library/imports/job-9/cancel".into()),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "unknown-import");
    }
}
