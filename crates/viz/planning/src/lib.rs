#![forbid(unsafe_code)]
//! Serving a planning document to the visualizer.
//!
//! The renderer is a client of a lighting desk, and it stays one: this exposes the read-only
//! subset of that API it actually consumes, backed by a show file instead of a running desk. The
//! renderer needs no new provider, no new protocol, and no knowledge that a desk is absent.
//!
//! Nothing here writes. Editing happens through the document's own patch boundary; this side only
//! projects what is currently in it.

mod wire;

#[cfg(test)]
mod tests;

pub use wire::{ObjectCollection, ObjectRecord, PatchSnapshotDto};

use axum::{
    Json, Router,
    extract::{
        Path, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use parking_lot::Mutex;
use std::{
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use uuid::Uuid;
use viz_document::PlanningDocument;

/// How often the event stream verifies the document's own revision.
///
/// Every editing command announces its change, so this is a backstop rather than the mechanism: it
/// catches a write that reached the document another way and costs one revision read.
const REVISION_POLL: Duration = Duration::from_secs(2);

/// The document the visualizer is currently looking at.
///
/// Held behind a lock the editor also holds, so a patch committed in the window is visible to the
/// next snapshot the renderer asks for. Swapping the document — opening another show — replaces
/// it here, and the renderer resynchronises onto the new one.
#[derive(Clone)]
pub struct SceneSource {
    document: Arc<Mutex<Option<PlanningDocument>>>,
    /// Bumped whenever what the renderer would draw has changed. The event stream reports it, so
    /// the renderer resynchronises on an edit instead of rediscovering it on a later reconnect.
    generation: Arc<AtomicU64>,
    changes: Arc<tokio::sync::watch::Sender<u64>>,
}

impl Default for SceneSource {
    fn default() -> Self {
        Self {
            document: Arc::new(Mutex::new(None)),
            generation: Arc::new(AtomicU64::new(0)),
            changes: Arc::new(tokio::sync::watch::Sender::new(0)),
        }
    }
}

impl SceneSource {
    pub fn new(document: PlanningDocument) -> Self {
        let source = Self::default();
        source.open(document);
        source
    }

    pub fn open(&self, document: PlanningDocument) {
        *self.document.lock() = Some(document);
        self.mark_changed();
    }

    pub fn is_open(&self) -> bool {
        self.document.lock().is_some()
    }

    /// Announce that the open document changed in a way the renderer has to see.
    ///
    /// Editing commands call this after they commit. A missed call costs at most one
    /// [`REVISION_POLL`], never a stale picture that never corrects itself.
    pub fn mark_changed(&self) {
        let generation = self.generation.fetch_add(1, Ordering::Relaxed) + 1;
        let _ = self.changes.send(generation);
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }

    /// Reads the open document, if there is one. The editor's own commands use this too, so the
    /// window and the renderer always see one document rather than two copies of one.
    pub fn with<T>(&self, action: impl FnOnce(&PlanningDocument) -> T) -> Option<T> {
        self.document.lock().as_ref().map(action)
    }
}

/// The routes the visualizer's lighting-desk provider calls.
pub fn router(source: SceneSource) -> Router {
    Router::new()
        .route("/api/v2/readiness", get(readiness))
        .route("/api/v2/sessions", post(open_session))
        .route("/api/v2/sessions/{id}", delete(close_session))
        .route("/api/v2/patch", get(patch))
        .route("/api/v2/objects/{kind}", get(objects))
        .route("/api/v2/document/download", get(download_document))
        .route("/api/v2/events", get(events))
        .with_state(source)
}

/// Serves `source` until the process ends, on the bound address.
pub async fn serve(source: SceneSource, address: SocketAddr) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(address).await?;
    axum::serve(listener, router(source)).await
}

/// Serves `source` on a listener the caller already bound, which is how an application that has
/// to publish its port learns the port before anything can connect to it.
pub async fn serve_on(
    source: SceneSource,
    listener: tokio::net::TcpListener,
) -> std::io::Result<()> {
    axum::serve(listener, router(source)).await
}

/// Binds without serving, so a caller that needs the resolved port can report it before the
/// server starts accepting.
pub async fn bind(address: SocketAddr) -> std::io::Result<(tokio::net::TcpListener, SocketAddr)> {
    let listener = tokio::net::TcpListener::bind(address).await?;
    let bound = listener.local_addr()?;
    Ok((listener, bound))
}

async fn readiness(State(source): State<SceneSource>) -> Json<wire::Readiness> {
    let open = source.with(|document| (document.show_id().0, document.patch_revision().ok()));
    match open {
        Some((show_id, revision)) => Json(wire::Readiness {
            status: "ready",
            active_show: Some(show_id),
            active_show_error: None,
            snapshot_revision: revision.unwrap_or_default(),
        }),
        // Readiness is honest about having nothing to show yet: the editor may still be waiting
        // for the operator to choose a file. The renderer keeps asking rather than failing.
        None => Json(wire::Readiness {
            status: "starting",
            active_show: None,
            active_show_error: None,
            snapshot_revision: 0,
        }),
    }
}

/// The open document as a portable show file, for a desk that wants to load what was planned here.
///
/// This is the same copy **Save As** writes — the profile revisions the rig uses travel with it —
/// so what the desk receives opens on a machine that has never seen this fixture library. It is a
/// copy and stays one: patching here afterwards does not reach into the desk's show, and the desk
/// loading it does not reach back into this document.
async fn download_document(State(source): State<SceneSource>) -> Response {
    let Some(name) = source.with(|document| document.name().ok()).flatten() else {
        return (StatusCode::NOT_FOUND, "no document is open").into_response();
    };
    // A complete copy has to be written before it can be sent, and it is written where temporary
    // work belongs rather than beside the operator's own file.
    let directory = std::env::temp_dir().join("tosklight-viz-download");
    if let Err(error) = std::fs::create_dir_all(&directory) {
        return (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response();
    }
    let path = directory.join(format!("{}.show", sanitised(&name)));
    let copied = source.with(|document| document.save_as(&path));
    match copied {
        Some(Ok(())) => {}
        Some(Err(error)) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response();
        }
        None => return (StatusCode::NOT_FOUND, "no document is open").into_response(),
    }
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()).into_response();
        }
    };
    let _ = std::fs::remove_file(&path);
    (
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}.show\"", sanitised(&name)),
            ),
        ],
        bytes,
    )
        .into_response()
}

/// A document name becomes a file name here, and an operator may have called it anything.
fn sanitised(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, '-' | '_' | ' ') {
                character
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = cleaned.trim().to_owned();
    if trimmed.is_empty() {
        "show".to_owned()
    } else {
        trimmed
    }
}

/// Sessions exist so the renderer's desk provider has one to open. There is nothing to
/// authenticate against a local file, and there is no second client to distinguish.
async fn open_session() -> Json<wire::SessionResponse> {
    Json(wire::SessionResponse {
        session_id: Uuid::new_v4(),
        token: Uuid::new_v4().to_string(),
        role: "read_only",
    })
}

async fn close_session(Path(_id): Path<String>) -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn patch(
    State(source): State<SceneSource>,
) -> Result<Json<wire::PatchSnapshotDto>, StatusCode> {
    let snapshot = source
        .with(|document| document.patch_snapshot())
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(wire::patch_snapshot(snapshot)))
}

/// The configuration event stream.
///
/// Without it the renderer's desk provider has nothing to wait on: it would finish reading and
/// immediately reconnect, rebinding its DMX receivers every couple of seconds for as long as the
/// window stayed open. It reports the same event kinds a desk does, so the renderer needs no
/// knowledge of which source it is subscribed to.
async fn events(State(source): State<SceneSource>, upgrade: WebSocketUpgrade) -> Response {
    upgrade
        .protocols(["light.events.v2", "light.v2"])
        .on_upgrade(move |socket| stream_events(socket, source))
}

async fn stream_events(mut socket: WebSocket, source: SceneSource) {
    let mut changes = source.changes.subscribe();
    // Only changes from here on are news: the renderer has just read the document.
    let _ = changes.borrow_and_update();
    let mut sequence = 0_u64;
    let mut revision = source
        .with(|document| document.patch_revision().ok())
        .flatten();
    loop {
        let announced = tokio::select! {
            changed = changes.changed() => {
                if changed.is_err() {
                    return;
                }
                true
            }
            // A renderer that has gone is noticed when it goes, not when the document next
            // changes: this stream outlives nothing.
            incoming = socket.recv() => {
                match incoming {
                    None | Some(Err(_)) | Some(Ok(Message::Close(_))) => return,
                    Some(Ok(_)) => false,
                }
            }
            _ = tokio::time::sleep(REVISION_POLL) => {
                let current = source
                    .with(|document| document.patch_revision().ok())
                    .flatten();
                let moved = current != revision;
                revision = current;
                moved
            }
        };
        if !announced {
            continue;
        }
        revision = source
            .with(|document| document.patch_revision().ok())
            .flatten();
        sequence += 1;
        let frame = format!("{{\"kind\":\"show_patch_changed\",\"sequence\":{sequence}}}");
        if socket.send(Message::Text(frame.into())).await.is_err() {
            return;
        }
    }
}

/// Show objects the renderer reads: output routes for its receivers, and stage layout and venue
/// geometry for placement.
async fn objects(
    State(source): State<SceneSource>,
    Path(kind): Path<String>,
) -> Result<Json<wire::ObjectCollection>, StatusCode> {
    let stored = source
        .with(|document| document.objects(&kind))
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(wire::ObjectCollection {
        objects: stored
            .into_iter()
            .map(|object| wire::ObjectRecord {
                id: object.id,
                revision: object.revision,
                body: object.body,
            })
            .collect(),
    }))
}
