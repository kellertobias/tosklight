//! Browsing for and switching to another configuration and media folder.
//!
//! A change is validated and recorded by the process before it is answered; the server then
//! restarts itself so the chosen folder's configuration and library are what it runs. A refused
//! folder changes nothing, and the running configuration stays exactly as it was.

use std::path::Path;

use axum::extract::{Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::data_folder::FolderRefusal;
use crate::error::ApiError;
use crate::routes::ApiState;
use crate::routes::edit::{self, Proceed};
use crate::tolerant::TolerantJson;
use crate::wire::{DataFolderChangeView, DataFolderListingView, DataFolderQuery, UpdateDataFolder};

fn refused(refusal: FolderRefusal) -> ApiError {
    let status = match refusal {
        FolderRefusal::Invalid(_) | FolderRefusal::InvalidConfiguration(_) => {
            StatusCode::UNPROCESSABLE_ENTITY
        }
        FolderRefusal::Unreadable(_) | FolderRefusal::Unwritable(_) => StatusCode::FORBIDDEN,
    };
    ApiError::new(status, refusal.code(), refusal.to_string())
}

/// Lists the subfolders of one folder on the Media Server computer.
pub(super) async fn folders(
    State(state): State<ApiState>,
    Query(query): Query<DataFolderQuery>,
) -> Result<Response, ApiError> {
    let directory = query
        .directory
        .as_deref()
        .map(str::trim)
        .filter(|directory| !directory.is_empty())
        .map(Path::new);
    let listing = (state.data_folders.browse)(directory).map_err(refused)?;
    Ok(axum::Json(DataFolderListingView::of(&listing)).into_response())
}

/// Makes a folder the configuration and media-library root, then restarts to load it.
pub(super) async fn update(
    State(state): State<ApiState>,
    TolerantJson(body): TolerantJson<UpdateDataFolder>,
) -> Result<Response, ApiError> {
    let _edit = match edit::begin(&state, &body.request_id).await? {
        Proceed::Replay(response) => return Ok(response),
        Proceed::Fresh(guard) => guard,
    };
    let directory = body.directory.trim();
    if directory.is_empty() {
        return Err(refused(FolderRefusal::Invalid(
            "Choose a folder on the Media Server computer.".into(),
        )));
    }
    let current = state.configuration.load();
    let change = (state.data_folders.change)(Path::new(directory), &current).map_err(|error| {
        tracing::warn!(%error, directory, "the chosen data folder was refused");
        refused(error)
    })?;
    tracing::info!(
        directory = %change.directory.display(),
        loaded_existing = change.loaded_existing,
        "the data folder changed; restarting to load it"
    );
    let serialized = serde_json::to_string(&DataFolderChangeView::of(&change)).unwrap_or_default();
    state.replays.remember(&body.request_id, serialized.clone());
    (state.data_folders.restart)();
    Ok(([(header::CONTENT_TYPE, "application/json")], serialized).into_response())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use axum::http::StatusCode;

    use crate::data_folder::{
        DataFolders, FolderChange, FolderEntry, FolderListing, FolderRefusal,
    };
    use crate::routes::bench::{bench, get, post, send};

    fn folders(changes: Arc<Mutex<Vec<PathBuf>>>, restarts: Arc<AtomicUsize>) -> DataFolders {
        DataFolders {
            browse: Arc::new(|directory| {
                let directory = directory.unwrap_or(Path::new("/srv")).to_path_buf();
                if directory == Path::new("/locked") {
                    return Err(FolderRefusal::Unreadable("cannot read /locked".into()));
                }
                Ok(FolderListing {
                    parent: directory.parent().map(Path::to_path_buf),
                    has_configuration: false,
                    folders: vec![FolderEntry {
                        name: "Show".into(),
                        directory: directory.join("Show"),
                        has_configuration: true,
                    }],
                    directory,
                })
            }),
            change: Arc::new(move |directory, _| {
                if directory == Path::new("/broken") {
                    return Err(FolderRefusal::InvalidConfiguration(
                        "media-server.json in /broken is not usable".into(),
                    ));
                }
                changes.lock().unwrap().push(directory.to_path_buf());
                Ok(FolderChange {
                    directory: directory.to_path_buf(),
                    loaded_existing: true,
                })
            }),
            restart: Arc::new(move || {
                restarts.fetch_add(1, Ordering::SeqCst);
            }),
        }
    }

    #[tokio::test]
    async fn the_picker_lists_subfolders_and_reports_an_unreadable_folder() {
        let bench = bench();
        let mut api = bench.api.clone();
        api.data_folders = folders(Arc::default(), Arc::default());
        let router = crate::router(api);

        let (status, body) = send(
            &router,
            get("/api/v2/runtime/data-directory/folders".into()),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["directory"], "/srv");
        assert_eq!(body["parent"], "/");
        assert_eq!(body["folders"][0]["name"], "Show");
        assert_eq!(body["folders"][0]["hasConfiguration"], true);

        let (status, body) = send(
            &router,
            get("/api/v2/runtime/data-directory/folders?directory=%2Flocked".into()),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "data-folder-unreadable");
    }

    #[tokio::test]
    async fn an_accepted_folder_restarts_once_and_a_retry_is_answered_not_redone() {
        let bench = bench();
        let changes = Arc::new(Mutex::new(Vec::new()));
        let restarts = Arc::new(AtomicUsize::new(0));
        let mut api = bench.api.clone();
        api.data_folders = folders(changes.clone(), restarts.clone());
        let router = crate::router(api);
        let request = r#"{"requestId":"move","directory":" /srv/Show "}"#;

        for _ in 0..2 {
            let (status, body) = send(
                &router,
                post("/api/v2/runtime/data-directory/update".into(), request),
            )
            .await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(body["directory"], "/srv/Show");
            assert_eq!(body["loadedExisting"], true);
            assert_eq!(body["restarting"], true);
        }
        assert_eq!(*changes.lock().unwrap(), vec![PathBuf::from("/srv/Show")]);
        assert_eq!(restarts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_refused_folder_neither_restarts_nor_changes_the_configuration() {
        let bench = bench();
        let restarts = Arc::new(AtomicUsize::new(0));
        let mut api = bench.api.clone();
        api.data_folders = folders(Arc::default(), restarts.clone());
        let router = crate::router(api);
        let before = bench.configuration.load_full();

        for (body, code) in [
            (
                r#"{"requestId":"broken","directory":"/broken"}"#,
                "data-folder-configuration-invalid",
            ),
            (
                r#"{"requestId":"empty","directory":"  "}"#,
                "data-folder-invalid",
            ),
        ] {
            let (status, answer) = send(
                &router,
                post("/api/v2/runtime/data-directory/update".into(), body),
            )
            .await;
            assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
            assert_eq!(answer["code"], code);
        }
        assert_eq!(restarts.load(Ordering::SeqCst), 0);
        assert!(bench.stored.lock().unwrap().is_empty());
        assert!(Arc::ptr_eq(&before, &bench.configuration.load_full()));
    }
}
