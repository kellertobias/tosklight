//! The Architect's local editing API: the same edits its own windows make, reachable by a program.
//!
//! The Architect already serves HTTP — `viz_planning` — but that listener is read-only by design
//! and binds every interface, because a desk elsewhere in the building is meant to be able to take
//! a copy of what is planned here. Writes must not go there. A show that any host on the lighting
//! network could edit would be a different product with a different threat model, and that is not
//! what "give the Architect a local API" asked for.
//!
//! So this is a second listener with the opposite properties: **loopback only**, and writable. It
//! is reachable by a program on this machine and by nothing else, which is exactly the reach an MCP
//! server started by the operator needs.
//!
//! Two things make it safe to have at all:
//!
//! - It binds `127.0.0.1`, so the operating system refuses the connection to anything off-box.
//! - It still requires a token, because loopback is not a user boundary — every process the
//!   operator runs shares it. The token is written to a file in the app's own data directory, so a
//!   program the operator started can read it and a web page cannot.
//!
//! The route *names* mirror the desk's, deliberately, so a tool written against one reads the same
//! against the other. The *bodies* do not: the Architect's patch records are its own
//! (`contract::FixtureDto`, camelCase), and the desk's are the desk's. Pretending otherwise would
//! mean translating two fixture models through each other and getting it subtly wrong somewhere in
//! the middle. So a client gets one set of tools over two backends, and the backend it is talking
//! to decides the shape — which is what having a second backend meant in the first place.

use crate::contract::{MutationDto, OutcomeDto, SnapshotDto};
use crate::session::Session;
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};
use serde::Serialize;
use std::net::SocketAddr;
use std::path::PathBuf;
use tauri::Manager;
use uuid::Uuid;

/// What a program needs to find and open this API.
///
/// Written to the app data directory on startup and rewritten on every launch, so a stale file
/// from a previous run cannot point a tool at a port this process does not own.
#[derive(Clone, Debug, Serialize)]
pub struct LocalApiHandle {
    pub port: u16,
    pub token: String,
}

/// The file a program reads to find the API. Named for what it is, not for who uses it.
const HANDLE_FILE: &str = "local-api.json";

#[derive(Clone)]
struct ApiState {
    app: tauri::AppHandle,
    token: String,
}

/// Start the local editing API, and leave a handle for the program that wants it.
///
/// A failure here is reported and then dropped: the Architect is an editor first, and an operator
/// who is not using an external tool should not be stopped from planning a rig because a port
/// could not be bound.
pub fn start(app: &tauri::App) -> Result<LocalApiHandle, String> {
    let token = Uuid::new_v4().to_string();
    let state = ApiState {
        app: app.handle().clone(),
        token: token.clone(),
    };
    let (listener, address) = tauri::async_runtime::block_on(bind_loopback())
        .map_err(|error| format!("the local editing API could not bind: {error}"))?;
    let router = router(state);
    tauri::async_runtime::spawn(async move {
        if let Err(error) = axum::serve(listener, router).await {
            eprintln!("local editing API on {address}: {error}");
        }
    });
    let handle = LocalApiHandle {
        port: address.port(),
        token,
    };
    if let Some(path) = handle_path(app.handle()) {
        write_handle(&path, &handle)?;
    }
    Ok(handle)
}

async fn bind_loopback() -> std::io::Result<(tokio::net::TcpListener, SocketAddr)> {
    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).await?;
    let address = listener.local_addr()?;
    Ok((listener, address))
}

fn handle_path(app: &tauri::AppHandle) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|dir| dir.join(HANDLE_FILE))
}

fn write_handle(path: &PathBuf, handle: &LocalApiHandle) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let body = serde_json::to_vec_pretty(handle).map_err(|error| error.to_string())?;
    std::fs::write(path, body).map_err(|error| error.to_string())
}

fn router(state: ApiState) -> Router {
    Router::new()
        .route("/api/v2/readiness", get(readiness))
        .route("/api/v2/sessions", post(open_session))
        .route("/api/v2/patch", get(patch))
        .route("/api/v2/patch/fixtures", post(patch_fixtures))
        .route("/api/v2/fixture-library/profiles", get(library_profiles))
        .route("/api/v2/objects/{kind}", get(objects))
        .with_state(state)
}

/// Names the product as well as the state, because a tool pointed at the wrong port should find
/// out from the answer rather than from a confusing failure three calls later.
async fn readiness() -> impl IntoResponse {
    Json(serde_json::json!({ "ready": true, "product": "tosklight-architect" }))
}

/// The Architect has no users, so a session is only the token echoed back.
///
/// The shape matches the desk's so one client can open a session against either without knowing
/// which it is talking to.
async fn open_session(State(state): State<ApiState>) -> impl IntoResponse {
    Json(serde_json::json!({ "token": state.token }))
}

async fn patch(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<SnapshotDto>, ApiFailure> {
    authorize(&state, &headers)?;
    let session = state.app.state::<Session>();
    crate::session::patch_snapshot(session)
        .map(Json)
        .map_err(ApiFailure::from)
}

/// One patch mutation, applied exactly as a window's own edit would be.
///
/// The work is not done here: it goes through the same function the Tauri command calls, so the
/// announce-to-every-window step cannot be forgotten on this path and present on that one. The
/// difference is only that this call has no originating window, so nothing is excluded from the
/// announcement.
async fn patch_fixtures(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(mutation): Json<MutationDto>,
) -> Result<Json<OutcomeDto>, ApiFailure> {
    authorize(&state, &headers)?;
    let session = state.app.state::<Session>();
    let cad = state.app.state::<crate::cad::CadState>();
    expect_patch_revision(&session, &headers)?;
    crate::session::apply_patch_mutation(&state.app, &session, &cad, None, mutation)
        .map(Json)
        .map_err(ApiFailure::from)
}

/// Refuse an edit built on a patch that has since moved.
///
/// A program is not the only thing editing this document — the operator has one or more windows
/// open on the same rig — so a tool that read the patch, thought about it, and now writes may be
/// writing over a change made in between. `If-Match` carries the revision it read at, and a
/// mismatch is refused rather than applied. Omitting the header means the caller is not making
/// that claim, and the write proceeds: the desk's own routes treat an absent expectation the same
/// way, and a one-shot tool that reads and writes in the same breath should not be forced to
/// invent one.
fn expect_patch_revision(session: &Session, headers: &HeaderMap) -> Result<(), ApiFailure> {
    let Some(expected) = headers
        .get(axum::http::header::IF_MATCH)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim_matches('"'))
        .and_then(|value| value.parse::<u64>().ok())
    else {
        return Ok(());
    };
    let current = session
        .with(|document| document.patch_revision().map_err(|error| error.to_string()))
        .map_err(ApiFailure::from)?;
    if expected == current {
        Ok(())
    } else {
        Err(ApiFailure(
            StatusCode::CONFLICT,
            format!("stale patch revision: expected {expected}, current {current}"),
        ))
    }
}

/// The fixture library this editor has, in the shape the desk reports its own.
///
/// A tool that wants to add a fixture needs a profile id, a revision and a mode before it can ask
/// for anything, and those come from here on both products.
async fn library_profiles(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiFailure> {
    authorize(&state, &headers)?;
    let session = state.app.state::<Session>();
    let profiles = crate::session::library_profiles(session).map_err(ApiFailure::from)?;
    Ok(Json(serde_json::json!({ "profiles": profiles })))
}

/// One collection of stored show objects — patch layers, and whatever else a show carries.
///
/// Reads only. The Architect has no route that edits a show object, and this does not invent one:
/// a tool asking for something the editor itself cannot do should be told so rather than handed a
/// half-working path.
async fn objects(
    State(state): State<ApiState>,
    axum::extract::Path(kind): axum::extract::Path<String>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ApiFailure> {
    authorize(&state, &headers)?;
    let session = state.app.state::<Session>();
    let objects = session
        .with(|document| document.objects(&kind).map_err(|error| error.to_string()))
        .map_err(ApiFailure::from)?;
    let listed = objects
        .into_iter()
        .map(|object| {
            serde_json::json!({
                "id": object.id,
                "revision": object.revision,
                "body": object.body,
            })
        })
        .collect::<Vec<_>>();
    Ok(Json(serde_json::json!({ "objects": listed })))
}

/// Loopback is not a user boundary — every process the operator runs shares it — so the token is
/// what separates the tool they started from anything else that found the port.
fn authorize(state: &ApiState, headers: &HeaderMap) -> Result<(), ApiFailure> {
    let offered = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or_default();
    if offered == state.token {
        Ok(())
    } else {
        Err(ApiFailure(
            StatusCode::UNAUTHORIZED,
            "this API needs the token from the editor's local-api.json".into(),
        ))
    }
}

/// A refusal a caller can read, rather than a bare status.
struct ApiFailure(StatusCode, String);

impl From<String> for ApiFailure {
    fn from(error: String) -> Self {
        Self(StatusCode::CONFLICT, error)
    }
}

impl IntoResponse for ApiFailure {
    fn into_response(self) -> axum::response::Response {
        (self.0, Json(serde_json::json!({ "error": self.1 }))).into_response()
    }
}
