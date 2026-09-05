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
    Fresh(EditGuard),
}

/// Retained until the handler has remembered its response or returned an error.
pub struct EditGuard {
    _request: tokio::sync::OwnedMutexGuard<()>,
    _transaction: Option<tokio::sync::OwnedMutexGuard<()>>,
}

/// Refuses an edit with no identity, then serializes its complete configuration transaction.
pub async fn begin(state: &ApiState, request_id: &str) -> Result<Proceed, ApiError> {
    begin_inner(state, request_id, true).await
}

/// Uploads await client bytes without locking configuration editing. Their filesystem adapter
/// owns publication; the request lease still prevents two executions of the same upload.
pub async fn begin_upload(state: &ApiState, request_id: &str) -> Result<Proceed, ApiError> {
    begin_inner(state, request_id, false).await
}

async fn begin_inner(
    state: &ApiState,
    request_id: &str,
    configuration: bool,
) -> Result<Proceed, ApiError> {
    if request_id.trim().is_empty() {
        return Err(ApiError::bad_request(
            "missing-request-id",
            "an edit must carry a request id so a retry cannot become a second edit",
        ));
    }
    if request_id.len() > 256 {
        return Err(ApiError::bad_request(
            "invalid-request-id",
            "a request id must be at most 256 bytes",
        ));
    }
    let request = state.replays.request(request_id).await;
    if let Some(stored) = state.replays.stored(request_id) {
        return Ok(Proceed::Replay(
            ([(header::CONTENT_TYPE, "application/json")], stored).into_response(),
        ));
    }
    let transaction = if configuration {
        Some(state.replays.transaction().await)
    } else {
        None
    };
    Ok(Proceed::Fresh(EditGuard {
        _request: request,
        _transaction: transaction,
    }))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routes::bench::{bench, post, send};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn overlapping_retries_persist_once_and_return_the_same_response() {
        let bench = bench();
        let mut api = bench.api.clone();
        let writes = Arc::new(AtomicUsize::new(0));
        let recorded = writes.clone();
        api.persist = Arc::new(move |_| {
            recorded.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(30));
            Ok(())
        });
        let router = crate::router(api);
        let mut tasks = Vec::new();
        for _ in 0..4 {
            let router = router.clone();
            tasks.push(tokio::spawn(async move {
                send(
                    &router,
                    post(
                        "/api/v2/time/update".into(),
                        r#"{"requestId":"same","utcOffsetMinutes":60}"#,
                    ),
                )
                .await
            }));
        }
        for task in tasks {
            let (status, view) = task.await.unwrap();
            assert_eq!(status, StatusCode::OK);
            assert_eq!(view["utcOffsetMinutes"], 60);
        }
        assert_eq!(writes.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn overlapping_edits_preserve_both_configuration_objects() {
        let bench = bench();
        let mut api = bench.api.clone();
        let entered = Arc::new(tokio::sync::Notify::new());
        let started = entered.clone();
        api.persist = Arc::new(move |_| {
            started.notify_one();
            std::thread::sleep(Duration::from_millis(50));
            Ok(())
        });
        let router = crate::router(api);
        let first = router.clone();
        let task = tokio::spawn(async move {
            send(
                &first,
                post(
                    "/api/v2/time/update".into(),
                    r#"{"requestId":"time","utcOffsetMinutes":60}"#,
                ),
            )
            .await
        });
        entered.notified().await;
        let (status, _) = send(
            &router,
            post(
                "/api/v2/audio/update".into(),
                r#"{"requestId":"audio","inputGain":2.5}"#,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(task.await.unwrap().0, StatusCode::OK);
        let configuration = bench.configuration.load();
        assert_eq!(configuration.time.utc_offset_minutes, 60);
        assert_eq!(configuration.audio.input_gain, 2.5);
    }

    #[tokio::test]
    async fn an_upload_does_not_block_configuration_and_cancellation_releases_its_identity() {
        let bench = bench();
        let first = begin_upload(&bench.api, "upload").await.unwrap();
        assert!(matches!(first, Proceed::Fresh(_)));
        let other = tokio::time::timeout(Duration::from_millis(100), begin(&bench.api, "settings"))
            .await
            .unwrap()
            .unwrap();
        drop(other);
        assert!(
            tokio::time::timeout(
                Duration::from_millis(20),
                begin_upload(&bench.api, "upload")
            )
            .await
            .is_err()
        );
        drop(first);
        assert!(matches!(
            begin_upload(&bench.api, "upload").await.unwrap(),
            Proceed::Fresh(_)
        ));
    }
}
