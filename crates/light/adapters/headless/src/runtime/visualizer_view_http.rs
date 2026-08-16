//! The desk-owned visualizer view: what a connected renderer is told to look at.
//!
//! A renderer takes its scene from the API and its live values from Art-Net or sACN. What it
//! cannot work out for itself is which way the operator wants to be looking, so the desk keeps
//! that here and publishes every accepted change. The state is desk-level presentation
//! configuration — never portable show content — and is addressed by renderer target, so a desk
//! can move one renderer's camera without moving every camera in the building.

use super::configuration::{
    VisualizerCamera, VisualizerRenderQuality, VisualizerView, VisualizerViewMode,
};
use super::{ApiError, AppState, TolerantJson, authenticate, emit};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::HeaderMap,
    routing::{get, post},
};
use light_wire::v2::visualizer_view as wire;
use std::collections::{HashMap, VecDeque};
use uuid::Uuid;

const REQUEST_CACHE_ENTRY_LIMIT: usize = 1_024;

/// The target a renderer follows when it was not started for a named one.
pub(super) const DEFAULT_TARGET: &str = "main";

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v2/visualizer-views", get(snapshot))
        .route("/api/v2/visualizer-views/{target}/update", post(update))
}

async fn snapshot(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<wire::VisualizerViewSnapshot>, ApiError> {
    authenticate(&state, &headers)?;
    let configuration = state.installation.configuration();
    let mut views: Vec<wire::VisualizerViewProjection> = configuration
        .visualizer_views
        .iter()
        .map(|(target, view)| projection(target, view))
        .collect();
    // A renderer that has never been told anything still has a view to follow, so the default
    // target is always answered rather than left for the client to invent.
    if !configuration.visualizer_views.contains_key(DEFAULT_TARGET) {
        views.insert(0, projection(DEFAULT_TARGET, &VisualizerView::default()));
    }
    Ok(Json(wire::VisualizerViewSnapshot {
        connected: state.sessions.has_visualizer_connection(),
        views,
    }))
}

async fn update(
    State(state): State<AppState>,
    Path(target): Path<String>,
    headers: HeaderMap,
    request: Result<TolerantJson<wire::VisualizerViewUpdateRequest>, JsonRejection>,
) -> Result<Json<wire::VisualizerViewUpdateOutcome>, ApiError> {
    let session = authenticate(&state, &headers)?;
    let TolerantJson(request) =
        request.map_err(|error| ApiError::bad_request(error.body_text()))?;
    validate_request_id(&request.request_id)?;
    let target = validated_target(target)?;
    let key = ReplayKey {
        session_id: session.id.0,
        request_id: request.request_id.clone(),
    };
    let action = ReplayAction {
        target: target.clone(),
        patch: request.patch.clone(),
    };
    if let Some(outcome) = state.replay.lookup_visualizer_view(&key, &action).await? {
        return Ok(Json(outcome));
    }

    let current = state
        .installation
        .configuration()
        .visualizer_views
        .get(&target)
        .copied()
        .unwrap_or_default();
    let mut next = current;
    if let Some(mode) = request.patch.mode {
        next.mode = domain_mode(mode);
    }
    if let Some(quality) = request.patch.quality {
        next.quality = domain_quality(quality);
    }
    if let Some(camera) = request.patch.camera {
        next.camera = Some(domain_camera(camera));
    }
    if let Some(exposure) = request.patch.exposure {
        next.exposure = exposure;
    }
    if let Some(ambient) = request.patch.ambient {
        next.ambient = ambient;
    }
    if request.patch.reset_physics == Some(true) {
        next.physics_reset_generation = current.physics_reset_generation.saturating_add(1);
    }
    next.validate()?;

    // The revision records the change rather than being part of it, so it is held equal across
    // the comparison: a patch asking for what is already displayed is not a change.
    let changed = next != current;
    if changed {
        next.revision = current.revision.saturating_add(1);
        state.installation.update_configuration(|configuration| {
            configuration.visualizer_views.insert(target.clone(), next);
        });
        super::persist_server_configuration(&state)?;
        // The view is desk configuration, and that is the capability event a renderer — or any
        // other client — is already subscribed to. Publishing under it means a connected
        // visualizer hears the change on the stream it is already listening to.
        emit(
            &state,
            "visualizer_view_changed",
            serde_json::json!({
                "target": target,
                "view": projection(&target, &next),
            }),
        );
    }
    let outcome = wire::VisualizerViewUpdateOutcome {
        request_id: request.request_id,
        view: projection(&target, &next),
        replayed: false,
        changed,
    };
    state
        .replay
        .insert_visualizer_view(key, action, outcome.clone())
        .await;
    Ok(Json(outcome))
}

fn validated_target(target: String) -> Result<String, ApiError> {
    let trimmed = target.trim();
    if trimmed.is_empty()
        || trimmed.len() > 64
        || trimmed.chars().any(|character| {
            character.is_control() || (!character.is_ascii_alphanumeric() && character != '-')
        })
    {
        return Err(ApiError::bad_request(
            "target must be 1-64 characters of a-z, 0-9 or -",
        ));
    }
    Ok(trimmed.to_ascii_lowercase())
}

fn validate_request_id(request_id: &str) -> Result<(), ApiError> {
    if request_id.trim().is_empty()
        || request_id.len() > 128
        || request_id.chars().any(char::is_control)
    {
        return Err(ApiError::bad_request(
            "request_id must contain 1-128 printable characters",
        ));
    }
    Ok(())
}

pub(super) fn projection(target: &str, view: &VisualizerView) -> wire::VisualizerViewProjection {
    wire::VisualizerViewProjection {
        target: target.to_owned(),
        mode: view_mode(view.mode),
        quality: render_quality(view.quality),
        camera: view.camera.map(wire_camera),
        exposure: view.exposure,
        ambient: view.ambient,
        revision: view.revision,
        physics_reset_generation: view.physics_reset_generation,
    }
}

fn view_mode(mode: VisualizerViewMode) -> wire::VisualizerViewMode {
    match mode {
        VisualizerViewMode::TopDown => wire::VisualizerViewMode::TopDown,
        VisualizerViewMode::LeftToRight => wire::VisualizerViewMode::LeftToRight,
        VisualizerViewMode::RightToLeft => wire::VisualizerViewMode::RightToLeft,
        VisualizerViewMode::FrontToBack => wire::VisualizerViewMode::FrontToBack,
        VisualizerViewMode::BackToFront => wire::VisualizerViewMode::BackToFront,
        VisualizerViewMode::Lines3d => wire::VisualizerViewMode::Lines3d,
        VisualizerViewMode::Simple3d => wire::VisualizerViewMode::Simple3d,
        VisualizerViewMode::Full3d => wire::VisualizerViewMode::Full3d,
    }
}

fn domain_mode(mode: wire::VisualizerViewMode) -> VisualizerViewMode {
    match mode {
        wire::VisualizerViewMode::TopDown => VisualizerViewMode::TopDown,
        wire::VisualizerViewMode::LeftToRight => VisualizerViewMode::LeftToRight,
        wire::VisualizerViewMode::RightToLeft => VisualizerViewMode::RightToLeft,
        wire::VisualizerViewMode::FrontToBack => VisualizerViewMode::FrontToBack,
        wire::VisualizerViewMode::BackToFront => VisualizerViewMode::BackToFront,
        wire::VisualizerViewMode::Lines3d => VisualizerViewMode::Lines3d,
        wire::VisualizerViewMode::Simple3d => VisualizerViewMode::Simple3d,
        wire::VisualizerViewMode::Full3d => VisualizerViewMode::Full3d,
    }
}

fn render_quality(quality: VisualizerRenderQuality) -> wire::VisualizerRenderQuality {
    match quality {
        VisualizerRenderQuality::Draft => wire::VisualizerRenderQuality::Draft,
        VisualizerRenderQuality::Standard => wire::VisualizerRenderQuality::Standard,
        VisualizerRenderQuality::High => wire::VisualizerRenderQuality::High,
        VisualizerRenderQuality::Ultra => wire::VisualizerRenderQuality::Ultra,
        VisualizerRenderQuality::Extreme => wire::VisualizerRenderQuality::Extreme,
    }
}

fn domain_quality(quality: wire::VisualizerRenderQuality) -> VisualizerRenderQuality {
    match quality {
        wire::VisualizerRenderQuality::Draft => VisualizerRenderQuality::Draft,
        wire::VisualizerRenderQuality::Standard => VisualizerRenderQuality::Standard,
        wire::VisualizerRenderQuality::High => VisualizerRenderQuality::High,
        wire::VisualizerRenderQuality::Ultra => VisualizerRenderQuality::Ultra,
        wire::VisualizerRenderQuality::Extreme => VisualizerRenderQuality::Extreme,
    }
}

fn wire_camera(camera: VisualizerCamera) -> wire::VisualizerCamera {
    wire::VisualizerCamera {
        position: camera.position,
        target: camera.target,
        up: camera.up,
        fov_degrees: camera.fov_degrees,
        orthographic_size: camera.orthographic_size,
    }
}

fn domain_camera(camera: wire::VisualizerCamera) -> VisualizerCamera {
    VisualizerCamera {
        position: camera.position,
        target: camera.target,
        up: camera.up,
        fov_degrees: camera.fov_degrees,
        orthographic_size: camera.orthographic_size,
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct ReplayKey {
    session_id: Uuid,
    request_id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ReplayAction {
    target: String,
    patch: wire::VisualizerViewPatch,
}

struct ReplayEntry {
    action: ReplayAction,
    outcome: wire::VisualizerViewUpdateOutcome,
}

#[derive(Default)]
pub(super) struct VisualizerViewReplayCache {
    entries: HashMap<ReplayKey, ReplayEntry>,
    order: VecDeque<ReplayKey>,
}

impl VisualizerViewReplayCache {
    pub(super) fn get(
        &self,
        key: &ReplayKey,
        action: &ReplayAction,
    ) -> Result<Option<wire::VisualizerViewUpdateOutcome>, ApiError> {
        let Some(entry) = self.entries.get(key) else {
            return Ok(None);
        };
        if &entry.action != action {
            return Err(ApiError::conflict(
                "request_id was already used for a different visualizer-view update",
            ));
        }
        let mut replay = entry.outcome.clone();
        replay.replayed = true;
        Ok(Some(replay))
    }

    pub(super) fn insert(
        &mut self,
        key: ReplayKey,
        action: ReplayAction,
        outcome: wire::VisualizerViewUpdateOutcome,
    ) {
        if !self.entries.contains_key(&key) {
            self.order.push_back(key.clone());
        }
        self.entries.insert(key, ReplayEntry { action, outcome });
        while self.entries.len() > REQUEST_CACHE_ENTRY_LIMIT {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_target_is_a_short_lowercase_name() {
        assert_eq!(validated_target("Main".into()).unwrap(), "main");
        assert_eq!(
            validated_target(" front-of-house ".into()).unwrap(),
            "front-of-house"
        );
        assert!(validated_target(String::new()).is_err());
        assert!(validated_target("../etc".into()).is_err());
        assert!(validated_target("a".repeat(65)).is_err());
    }

    #[test]
    fn every_named_mode_survives_the_round_trip_to_storage_and_back() {
        for mode in [
            wire::VisualizerViewMode::TopDown,
            wire::VisualizerViewMode::LeftToRight,
            wire::VisualizerViewMode::RightToLeft,
            wire::VisualizerViewMode::FrontToBack,
            wire::VisualizerViewMode::BackToFront,
            wire::VisualizerViewMode::Lines3d,
            wire::VisualizerViewMode::Simple3d,
            wire::VisualizerViewMode::Full3d,
        ] {
            assert_eq!(view_mode(domain_mode(mode)), mode);
        }
        for quality in [
            wire::VisualizerRenderQuality::Draft,
            wire::VisualizerRenderQuality::Standard,
            wire::VisualizerRenderQuality::High,
            wire::VisualizerRenderQuality::Ultra,
        ] {
            assert_eq!(render_quality(domain_quality(quality)), quality);
        }
    }

    /// The stored spelling is the renderer's own, because both sides read the same word.
    #[test]
    fn the_stored_mode_spells_itself_the_way_the_renderer_does() {
        let view = VisualizerView {
            mode: VisualizerViewMode::Lines3d,
            ..VisualizerView::default()
        };
        let stored = serde_json::to_value(view).expect("serialize");
        assert_eq!(stored["mode"], "lines_3d");
        assert_eq!(stored["quality"], "high");
    }

    #[test]
    fn an_impossible_camera_is_refused_rather_than_stored() {
        let mut view = VisualizerView::default();
        view.camera = Some(VisualizerCamera {
            position: [0.0, 2.0, 8.0],
            target: [0.0, 2.0, 0.0],
            up: [0.0, 0.0, 0.0],
            fov_degrees: 45.0,
            orthographic_size: 8.0,
        });
        assert!(view.validate().is_err());
        view.camera = Some(VisualizerCamera {
            position: [0.0, 2.0, 8.0],
            target: [0.0, 2.0, 0.0],
            up: [0.0, 1.0, 0.0],
            fov_degrees: 45.0,
            orthographic_size: 8.0,
        });
        assert!(view.validate().is_ok());
        view.exposure = 99.0;
        assert!(view.validate().is_err());
    }
}
