//! The routes.
//!
//! Reads return whole-object snapshots. Writes are intent-shaped and go through the application
//! reducer, so the API never decides what a value means — it carries a request in and a projection
//! out. Live-control actions that need no payload are plain `GET` URLs with `Cache-Control:
//! no-store`, so a microcontroller with minimal storage can trigger one.
//!
//! Volatile state is pushed rather than polled: the telemetry socket carries what changes many
//! times a second, and everything else is a snapshot a panel reads when it needs one.
//!
//! One module per subject. What they share is [`edit`], which owns the order every configuration
//! edit follows, and the state below.

mod audio;
#[cfg(test)]
pub(crate) mod bench;
mod edit;
mod fixtures;
mod health;
mod logs;
mod network;
mod outputs;
mod telemetry;
mod text;
mod visualizers;

use std::sync::Arc;

use arc_swap::ArcSwap;
use axum::Router;
use axum::routing::{get, post};
use media_application::MediaConfiguration;
use media_domain::catalog::CatalogSnapshot;
use media_domain::{MediaState, Timestamp};

use crate::assets;
use crate::diagnostics::Diagnostics;

/// Writes an accepted configuration wherever it belongs.
///
/// A function rather than a path, because the API adapter does not touch the filesystem — and
/// because a test needs to prove persistence without one.
pub type PersistConfiguration =
    Arc<dyn Fn(&MediaConfiguration) -> Result<(), String> + Send + Sync>;

/// Tells the running process that a stored edit was accepted.
///
/// Some settings can be honoured without a restart — the analysis tuning an operator is listening
/// to while they turn it. Which ones those are is the process's knowledge, not the API's, so the
/// API simply says that the configuration changed.
pub type ApplyConfiguration = Arc<dyn Fn(&MediaConfiguration) + Send + Sync>;

/// Everything the routes read and write.
#[derive(Clone)]
pub struct ApiState {
    /// The live configuration. Swapped when an edit is accepted, so a read after a write sees it.
    pub configuration: Arc<ArcSwap<MediaConfiguration>>,
    pub state: Arc<ArcSwap<MediaState>>,
    pub catalog: Arc<ArcSwap<CatalogSnapshot>>,
    /// Stamps commands. Injected so the API's behaviour is testable without real time passing.
    pub now: Arc<dyn Fn() -> Timestamp + Send + Sync>,
    pub persist: PersistConfiguration,
    pub apply: ApplyConfiguration,
    /// What the running process can tell the API about itself.
    pub diagnostics: Diagnostics,
    /// What recent edits produced, so a retry is answered rather than executed again.
    pub replays: Arc<crate::replay::Replays>,
}

impl std::fmt::Debug for ApiState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("ApiState").finish_non_exhaustive()
    }
}

/// An [`ApplyConfiguration`] that does nothing, for a process with nothing to retune.
pub fn applies_nothing() -> ApplyConfiguration {
    Arc::new(|_| {})
}

/// The versioned API.
///
/// The version is in the path because an incompatible change gets a new one rather than silently
/// altering what `/v2` means to a client already deployed against it.
pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/api/v2/health", get(health::health))
        .route("/api/v2/catalog", get(health::catalog))
        .route("/api/v2/logs", get(logs::logs))
        .route("/api/v2/telemetry", get(telemetry::telemetry))
        .route("/api/v2/fixtures", get(fixtures::fixtures))
        .route("/api/v2/fixtures/{name}", get(fixtures::fixture))
        .route("/api/v2/network", get(network::network))
        .route("/api/v2/network/update", post(network::update_network))
        .route("/api/v2/audio", get(audio::audio))
        .route("/api/v2/audio/update", post(audio::update_audio))
        .route("/api/v2/text", get(text::text))
        .route("/api/v2/text/create", post(text::create_text))
        .route(
            "/api/v2/text/{folder}/{file}/update",
            post(text::update_text),
        )
        .route(
            "/api/v2/text/{folder}/{file}/delete",
            post(text::delete_text),
        )
        .route("/api/v2/visualizers", get(visualizers::visualizers))
        .route(
            "/api/v2/visualizers/{folder}/{file}/update",
            post(visualizers::update_visualizer),
        )
        .route("/api/v2/outputs", get(outputs::outputs))
        .route("/api/v2/outputs/{output}/state", get(outputs::output_state))
        .route(
            "/api/v2/outputs/{output}/layers/{layer}/update",
            post(outputs::update_layer),
        )
        .route(
            "/api/v2/outputs/{output}/layers/{layer}/reset",
            get(outputs::reset_layer),
        )
        .with_state(state)
        // Anything the API did not claim is the administration frontend: its shell, its assets,
        // and its client-side routes.
        .fallback(assets::serve)
}
