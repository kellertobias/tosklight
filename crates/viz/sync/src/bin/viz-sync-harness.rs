#![forbid(unsafe_code)]
//! A headless Architect for end-to-end tests of Control ↔ Architect synchronization.
//!
//! It holds one planning document and runs the very `SyncEngine` the Architect runs, behind a
//! small loopback JSON API, so a Playwright test can bind a document to a real desk, make
//! gestures, go offline, restart, and read what the document and the sync status say — without a
//! window. Every edit is one gesture, exactly as an Architect command is.
//!
//! `viz-sync-harness --data-dir <dir>` prints `LISTENING http://127.0.0.1:<port>` once it serves.

use axum::{
    Json, Router,
    extract::{Path as UrlPath, State},
    http::StatusCode,
    routing::{get, post},
};
use light_application::PatchFixturesCommand;
use parking_lot::{Mutex, ReentrantMutex};
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;
use viz_document::PlanningDocument;
use viz_sync::{
    DocumentHost, Resolution, Start, SyncBinding, SyncBindingStore, SyncEngine, SyncStatus,
};

type Reply = Result<Json<Value>, (StatusCode, String)>;

fn failed(error: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::BAD_REQUEST, error.to_string())
}

/// The open document and the lock every gesture and every remote change holds.
#[derive(Default)]
struct Document {
    gesture: ReentrantMutex<()>,
    open: Mutex<Option<PlanningDocument>>,
    status: Mutex<Option<SyncStatus>>,
    remote_changes: std::sync::atomic::AtomicU64,
}

struct Host(Arc<Document>);

impl DocumentHost for Host {
    fn apply_remote(
        &self,
        edit: &mut dyn FnMut(&PlanningDocument) -> Result<bool, String>,
    ) -> Result<(), String> {
        let _gesture = self.0.gesture.lock();
        let open = self.0.open.lock();
        let document = open.as_ref().ok_or("no document is open")?;
        if edit(document)? {
            self.0
                .remote_changes
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
        Ok(())
    }

    fn status_changed(&self, status: &SyncStatus) {
        *self.0.status.lock() = Some(status.clone());
    }
}

#[derive(Clone)]
struct Harness {
    document: Arc<Document>,
    engine: Arc<Mutex<Option<SyncEngine>>>,
    store: SyncBindingStore,
    runtime: tokio::runtime::Handle,
    notice: Arc<Mutex<Option<String>>>,
}

impl Harness {
    fn close(&self) {
        if let Some(engine) = self.engine.lock().take() {
            engine.stop();
        }
        *self.document.open.lock() = None;
        *self.document.status.lock() = None;
    }

    fn open(&self, path: &Path, fresh: Option<SyncBinding>) -> Result<Value, String> {
        self.close();
        let document = PlanningDocument::open(path).map_err(|e| e.to_string())?;
        let _gesture = self.document.gesture.lock();
        let (binding, start, notice) = match fresh {
            Some(binding) => {
                self.store.bind(path, &binding)?;
                (Some(binding), Start::FreshCopy, None)
            }
            None => match self.store.for_document(path) {
                Ok(binding) => (binding, Start::Reopen, None),
                Err(error) => (None, Start::Reopen, Some(format!("opens unbound: {error}"))),
            },
        };
        if let Some(binding) = binding {
            let engine = SyncEngine::start(
                &document,
                binding,
                self.store.clone(),
                Arc::new(Host(self.document.clone())),
                &self.runtime,
                start,
            )?;
            *self.engine.lock() = Some(engine);
        }
        *self.document.open.lock() = Some(document);
        *self.notice.lock() = notice.clone();
        Ok(json!({"bound": self.engine.lock().is_some(), "notice": notice}))
    }

    /// Runs `edit` as one gesture and journals it when the document is bound.
    fn gesture(&self, edit: impl FnOnce(&PlanningDocument) -> Result<(), String>) -> Reply {
        let _gesture = self.document.gesture.lock();
        let open = self.document.open.lock();
        let document = open.as_ref().ok_or_else(|| failed("no document is open"))?;
        edit(document).map_err(failed)?;
        if let Some(engine) = self.engine.lock().as_ref() {
            engine.capture(document).map_err(failed)?;
        }
        Ok(Json(json!({"ok": true})))
    }

    fn engine(&self) -> Result<SyncEngine, (StatusCode, String)> {
        self.engine
            .lock()
            .clone()
            .ok_or_else(|| failed("the document is not bound"))
    }
}

#[derive(Deserialize)]
struct OpenFromDesk {
    base_url: String,
    show_id: Uuid,
    path: PathBuf,
    #[serde(default = "desk_name")]
    desk_name: String,
}

fn desk_name() -> String {
    "Control".into()
}

async fn open_from_desk(
    State(harness): State<Harness>,
    Json(request): Json<OpenFromDesk>,
) -> Reply {
    let client = reqwest::Client::new();
    let base = request.base_url.trim_end_matches('/').to_owned();
    let session: Value = client
        .post(format!("{base}/api/v2/sessions"))
        .json(&json!({"role": "visualizer"}))
        .send()
        .await
        .map_err(failed)?
        .json()
        .await
        .map_err(failed)?;
    let token = session["token"].as_str().unwrap_or_default().to_owned();
    let bytes = client
        .get(format!("{base}/api/v2/shows/{}/download", request.show_id))
        .bearer_auth(&token)
        .send()
        .await
        .map_err(failed)?
        .error_for_status()
        .map_err(failed)?
        .bytes()
        .await
        .map_err(failed)?;
    let readiness: Value = client
        .get(format!("{base}/api/v2/readiness"))
        .send()
        .await
        .map_err(failed)?
        .json()
        .await
        .map_err(failed)?;
    let desk = readiness["desk_identity"]
        .as_str()
        .and_then(|id| id.parse().ok());
    std::fs::write(&request.path, bytes).map_err(failed)?;
    let revision = PlanningDocument::open(&request.path)
        .and_then(|document| document.portable_revision())
        .map_err(failed)?;
    let binding = SyncBinding::new(desk, request.show_id, base, request.desk_name, revision);
    let harness = harness.clone();
    let opened = tokio::task::spawn_blocking(move || harness.open(&request.path, Some(binding)))
        .await
        .map_err(failed)?
        .map_err(failed)?;
    Ok(Json(opened))
}

#[derive(Deserialize)]
struct OpenPath {
    path: PathBuf,
}

#[derive(Deserialize)]
struct Create {
    path: PathBuf,
    name: String,
}

/// A new document made on this computer: standalone, bound to nothing.
async fn create(State(harness): State<Harness>, Json(request): Json<Create>) -> Reply {
    PlanningDocument::create(&request.path, &request.name).map_err(failed)?;
    open(State(harness), Json(OpenPath { path: request.path })).await
}

async fn open(State(harness): State<Harness>, Json(request): Json<OpenPath>) -> Reply {
    let opened = tokio::task::spawn_blocking(move || harness.open(&request.path, None))
        .await
        .map_err(failed)?
        .map_err(failed)?;
    Ok(Json(opened))
}

/// One edit of a gesture.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Edit {
    Put {
        kind: String,
        id: String,
        body: Value,
    },
    Delete {
        kind: String,
        id: String,
    },
    MoveFixture {
        fixture_id: Uuid,
        x: i32,
        y: i32,
        z: i32,
    },
    Metadata {
        key: String,
        value: String,
    },
}

fn apply_edit(document: &PlanningDocument, edit: &Edit) -> Result<(), String> {
    match edit {
        Edit::Put { kind, id, body } => document
            .put_object(kind, id, body)
            .map_err(|e| e.to_string()),
        Edit::Delete { kind, id } => document
            .delete_object(kind, id)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        Edit::MoveFixture {
            fixture_id,
            x,
            y,
            z,
        } => {
            let snapshot = document.patch_snapshot().map_err(|e| e.to_string())?;
            let fixture = snapshot
                .fixtures
                .iter()
                .find(|fixture| fixture.patch.fixture_id.0 == *fixture_id)
                .ok_or("no such fixture")?;
            let mut input = light_patch_wire::patch_input(light_patch_wire::wire_fixture(fixture));
            input.location.x += x;
            input.location.y += y;
            input.location.z += z;
            document
                .patch_fixtures(PatchFixturesCommand {
                    show_id: document.show_id(),
                    fixtures: vec![light_patch_wire::application_fixture(input)?],
                    remove_fixture_ids: Vec::new(),
                    placements: Vec::new(),
                    vector_spreads: Vec::new(),
                    fixture_updates: Vec::new(),
                })
                .map(|_| ())
                .map_err(|e| e.to_string())
        }
        Edit::Metadata { key, value } => document
            .replace_metadata_values(&[(key.as_str(), Some(value.as_str()))])
            .map_err(|e| e.to_string()),
    }
}

#[derive(Deserialize)]
struct Gesture {
    edits: Vec<Edit>,
}

async fn gesture(State(harness): State<Harness>, Json(request): Json<Gesture>) -> Reply {
    tokio::task::spawn_blocking(move || {
        harness.gesture(|document| {
            request
                .edits
                .iter()
                .try_for_each(|edit| apply_edit(document, edit))
        })
    })
    .await
    .map_err(failed)?
}

async fn status(State(harness): State<Harness>) -> Reply {
    let engine = harness.engine.lock().clone();
    let notice = harness.notice.lock().clone();
    Ok(Json(match engine {
        Some(engine) => json!({
            "bound": true,
            "status": engine.status(),
            "binding": serde_json::to_value(engine.binding()).map_err(failed)?,
            "remote_changes": harness.document.remote_changes.load(std::sync::atomic::Ordering::SeqCst),
            "snapshot_reads": engine.snapshot_reads(),
        }),
        None => json!({"bound": false, "notice": notice}),
    }))
}

async fn conflicts(State(harness): State<Harness>) -> Reply {
    Ok(Json(
        serde_json::to_value(harness.engine()?.conflicts().map_err(failed)?).map_err(failed)?,
    ))
}

#[derive(Deserialize)]
struct Resolve {
    entry: i64,
    resolution: Resolution,
}

async fn resolve(State(harness): State<Harness>, Json(request): Json<Resolve>) -> Reply {
    let engine = harness.engine()?;
    tokio::task::spawn_blocking(move || engine.resolve(request.entry, request.resolution))
        .await
        .map_err(failed)?
        .map_err(failed)?;
    Ok(Json(json!({"ok": true})))
}

async fn objects(State(harness): State<Harness>, UrlPath(kind): UrlPath<String>) -> Reply {
    let open = harness.document.open.lock();
    let document = open.as_ref().ok_or_else(|| failed("no document is open"))?;
    let objects = document.objects(&kind).map_err(failed)?;
    Ok(Json(json!(
        objects
            .into_iter()
            .map(|object| json!({"id": object.id, "body": object.body}))
            .collect::<Vec<_>>()
    )))
}

async fn fixtures(State(harness): State<Harness>) -> Reply {
    let open = harness.document.open.lock();
    let document = open.as_ref().ok_or_else(|| failed("no document is open"))?;
    let snapshot = document.patch_snapshot().map_err(failed)?;
    Ok(Json(json!(
        snapshot
            .fixtures
            .iter()
            .map(
                |fixture| serde_json::to_value(light_patch_wire::patch_input(
                    light_patch_wire::wire_fixture(fixture)
                ))
                .unwrap_or_default()
            )
            .collect::<Vec<_>>()
    )))
}

async fn document_facts(State(harness): State<Harness>) -> Reply {
    let open = harness.document.open.lock();
    let document = open.as_ref().ok_or_else(|| failed("no document is open"))?;
    Ok(Json(json!({
        "show_id": document.show_id().0,
        "name": document.name().map_err(failed)?,
        "path": document.path(),
        "metadata": document.metadata_with_prefixes(&["previs.", "architect."]).map_err(failed)?,
    })))
}

#[derive(Deserialize)]
struct Online {
    online: bool,
}

async fn online(State(harness): State<Harness>, Json(request): Json<Online>) -> Reply {
    harness.engine()?.set_online(request.online);
    Ok(Json(json!({"ok": true})))
}

async fn lose_next_reply(State(harness): State<Harness>) -> Reply {
    harness.engine()?.lose_next_reply();
    Ok(Json(json!({"ok": true})))
}

async fn dismiss_error(State(harness): State<Harness>) -> Reply {
    harness.engine()?.dismiss_error();
    Ok(Json(json!({"ok": true})))
}

async fn save_as(State(harness): State<Harness>, Json(request): Json<OpenPath>) -> Reply {
    tokio::task::spawn_blocking(move || {
        let name = {
            let open = harness.document.open.lock();
            let document = open.as_ref().ok_or("no document is open")?;
            let name = format!("{} copy", document.name().map_err(|e| e.to_string())?);
            document
                .fork_to(&request.path, &name)
                .map_err(|e| e.to_string())?;
            name
        };
        harness
            .open(&request.path, None)
            .map(|opened| json!({"name": name, "opened": opened}))
    })
    .await
    .map_err(failed)?
    .map(Json)
    .map_err(failed)
}

async fn close(State(harness): State<Harness>) -> Reply {
    tokio::task::spawn_blocking(move || harness.close())
        .await
        .map_err(failed)?;
    Ok(Json(json!({"ok": true})))
}

async fn shutdown() -> Reply {
    tokio::spawn(async {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        std::process::exit(0);
    });
    Ok(Json(json!({"ok": true})))
}

#[tokio::main]
async fn main() {
    let mut arguments = std::env::args().skip(1);
    let mut data = None;
    while let Some(argument) = arguments.next() {
        if argument == "--data-dir" {
            data = arguments.next().map(PathBuf::from);
        }
    }
    let data = data.expect("--data-dir is required");
    let harness = Harness {
        document: Arc::new(Document::default()),
        engine: Arc::new(Mutex::new(None)),
        store: SyncBindingStore::at(data.join("show-sync")),
        runtime: tokio::runtime::Handle::current(),
        notice: Arc::new(Mutex::new(None)),
    };
    let router = Router::new()
        .route("/open-from-desk", post(open_from_desk))
        .route("/open", post(open))
        .route("/create", post(create))
        .route("/close", post(close))
        .route("/gesture", post(gesture))
        .route("/status", get(status))
        .route("/conflicts", get(conflicts))
        .route("/resolve", post(resolve))
        .route("/dismiss-error", post(dismiss_error))
        .route("/objects/{kind}", get(objects))
        .route("/fixtures", get(fixtures))
        .route("/document", get(document_facts))
        .route("/online", post(online))
        .route("/lose-next-reply", post(lose_next_reply))
        .route("/save-as", post(save_as))
        .route("/shutdown", post(shutdown))
        .with_state(harness);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind a loopback port");
    let address = listener.local_addr().expect("a bound address");
    println!("LISTENING http://{address}");
    axum::serve(listener, router).await.expect("serve");
}
