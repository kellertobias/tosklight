//! Snapshot and intent DTOs for the desk-owned visualizer view.
//!
//! A connected visualizer renders what the desk tells it to look at. The view is desk-level
//! presentation state, not portable show content: it says which way a renderer is pointing and how
//! hard it is working, and it travels with the installation rather than with the show file.
//!
//! Every field is optional in a patch (api-rules §3): selecting a view must not resubmit a camera
//! nobody moved. Request bodies tolerate unknown fields (api-rules §5).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The eight named modes. These values are the renderer's own wire spelling; the two sides must
/// agree exactly, because a mode the renderer cannot name is a view it cannot present.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum VisualizerViewMode {
    TopDown,
    LeftToRight,
    RightToLeft,
    FrontToBack,
    BackToFront,
    #[serde(rename = "lines_3d")]
    Lines3d,
    #[serde(rename = "simple_3d")]
    Simple3d,
    #[serde(rename = "full_3d")]
    Full3d,
}

/// Bounded rendering cost tier.
#[derive(Clone, Copy, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum VisualizerRenderQuality {
    Draft,
    Standard,
    High,
    Ultra,
}

/// A camera described without Euler-order ambiguity: where it is, what it is looking at, and
/// which way is up, in the stage's own metric coordinates.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct VisualizerCamera {
    pub position: [f32; 3],
    pub target: [f32; 3],
    pub up: [f32; 3],
    /// Vertical field of view in degrees, for the perspective modes.
    pub fov_degrees: f32,
    /// Half-height in metres, for the orthographic modes.
    pub orthographic_size: f32,
}

/// One renderer target's complete view state.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct VisualizerViewProjection {
    /// Which renderer this addresses. `main` is what a renderer follows unless it was started
    /// for another target, so one desk can drive one renderer without moving every camera in
    /// the building.
    pub target: String,
    pub mode: VisualizerViewMode,
    pub quality: VisualizerRenderQuality,
    /// Absent means the renderer frames the named view from the scene itself, which is what an
    /// operator selecting **Top Down** almost always wants.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<VisualizerCamera>,
    /// Operator-safe exposure multiplier on top of the renderer's automatic adaptation.
    pub exposure: f32,
    /// How brightly everything that is not a light source is lit, `0..=1`.
    pub ambient: f32,
    /// Increments on every accepted change, so a renderer can tell a genuine instruction from a
    /// re-read of the same state.
    #[ts(type = "number")]
    pub revision: u64,
    #[ts(type = "number")]
    pub physics_reset_generation: u64,
}

/// Every configured target, newest state.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct VisualizerViewSnapshot {
    /// Whether at least one external visualizer currently has an active desk session.
    pub connected: bool,
    pub views: Vec<VisualizerViewProjection>,
}

/// Only the fields being changed.
#[derive(Clone, Debug, Default, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct VisualizerViewPatch {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<VisualizerViewMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<VisualizerRenderQuality>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<VisualizerCamera>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exposure: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ambient: Option<f32>,
    /// Momentary authoritative command. The server increments the target's reset generation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset_physics: Option<bool>,
}

/// Body of the idempotent visualizer-view update.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct VisualizerViewUpdateRequest {
    /// Client-generated idempotency identity, scoped to the authenticated desk session.
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    pub patch: VisualizerViewPatch,
}

/// Authoritative result of an update.
#[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize, TS)]
pub struct VisualizerViewUpdateOutcome {
    pub request_id: String,
    pub view: VisualizerViewProjection,
    /// `true` when idempotency replay returned the already committed authoritative result.
    pub replayed: bool,
    /// `false` when the patch asked for what was already stored.
    pub changed: bool,
}
