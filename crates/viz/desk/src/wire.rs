//! Read-only projections of the desk API bodies the visualizer consumes.
//!
//! Unknown fields are tolerated and never fatal: the desk may add fields at any time and the
//! renderer must keep working (api-rules §5).

use light_fixture::InstalledFixtureAppearance;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct Readiness {
    pub status: String,
    #[serde(default)]
    pub active_show: Option<Uuid>,
    #[serde(default)]
    pub active_show_error: Option<String>,
    #[serde(default)]
    pub snapshot_revision: u64,
}

#[derive(Debug, Deserialize)]
pub struct SessionResponse {
    pub session_id: Uuid,
    pub token: String,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub desk: Option<serde_json::Value>,
    /// Who the session belongs to. A preload is one operator's, so reading one needs this.
    #[serde(default)]
    pub user: Option<SessionUser>,
}

#[derive(Debug, Deserialize)]
pub struct SessionUser {
    pub id: Uuid,
}

/// One operator's preload, as the desk projects it.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct PreloadProjection {
    #[serde(default)]
    pub fixture_values: Vec<PreloadFixtureValue>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PreloadFixtureValue {
    pub fixture_id: Uuid,
    pub attribute: String,
    pub value: PreloadAttributeValue,
}

/// Only the normalized form is drawn. A preload can also name a colour or a raw slot; those are
/// carried for completeness and skipped, because the renderer's parameters are normalized and
/// guessing at a conversion would put a value on screen nobody set.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum PreloadAttributeValue {
    Normalized(f32),
    #[serde(other)]
    Other,
}

#[derive(Debug, Default, Deserialize)]
pub struct PreloadSnapshot {
    #[serde(default)]
    pub projection: PreloadProjection,
}

/// Every renderer target the desk is driving.
#[derive(Debug, Default, Deserialize)]
pub struct VisualizerViewSnapshot {
    #[serde(default)]
    pub views: Vec<VisualizerView>,
}

/// One target's view.
///
/// The named mode and quality arrive as their wire spellings and are looked up rather than
/// matched exhaustively: a desk newer than this renderer may name a view it cannot present, and
/// the answer to that is to keep the view it has, not to fail.
#[derive(Debug, Deserialize)]
pub struct VisualizerView {
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub quality: String,
    #[serde(default)]
    pub camera: Option<VisualizerCamera>,
    #[serde(default = "one")]
    pub exposure: f32,
    #[serde(default)]
    pub ambient: f32,
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub physics_reset_generation: u64,
}

fn one() -> f32 {
    1.0
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct VisualizerCamera {
    #[serde(default)]
    pub position: [f32; 3],
    #[serde(default)]
    pub target: [f32; 3],
    #[serde(default)]
    pub up: [f32; 3],
    #[serde(default)]
    pub fov_degrees: f32,
    #[serde(default)]
    pub orthographic_size: f32,
}

#[derive(Debug, Deserialize)]
pub struct PatchSnapshot {
    pub show_id: Uuid,
    #[serde(default)]
    pub show_revision: u64,
    #[serde(default)]
    pub patch_revision: u64,
    #[serde(default)]
    pub fixtures: Vec<PatchFixture>,
    #[serde(default)]
    pub profile_revisions: Vec<ProfileRevision>,
}

#[derive(Debug, Deserialize)]
pub struct PatchFixture {
    pub fixture_id: Uuid,
    #[serde(default)]
    pub fixture_number: Option<u32>,
    #[serde(default)]
    pub name: String,
    pub profile_id: Uuid,
    #[serde(default)]
    pub profile_revision: u64,
    pub mode_id: Uuid,
    #[serde(default)]
    pub split_patches: Vec<SplitAssignment>,
    #[serde(default)]
    pub location: Location,
    #[serde(default)]
    pub rotation: Rotation,
    #[serde(default)]
    pub multipatch: Vec<MultiPatch>,
    #[serde(default)]
    pub invert_pan: bool,
    #[serde(default)]
    pub invert_tilt: bool,
    /// Degrees the mounting bracket is set to, positive nose-down.
    #[serde(default)]
    pub bracket_angle: f32,
    /// Degrees a fitted shaper or barn-door module is turned to.
    #[serde(default)]
    pub shaper_angle: Option<f32>,
    #[serde(default)]
    pub installed_appearance: InstalledFixtureAppearance,
}

#[derive(Debug, Deserialize)]
pub struct SplitAssignment {
    pub split: u16,
    #[serde(default)]
    pub universe: Option<u16>,
    #[serde(default)]
    pub address: Option<u16>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
pub struct Location {
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
    #[serde(default)]
    pub z: i32,
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
pub struct Rotation {
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    #[serde(default)]
    pub z: f32,
}

#[derive(Debug, Deserialize)]
pub struct MultiPatch {
    pub id: Uuid,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub split_patches: Vec<SplitAssignment>,
    #[serde(default)]
    pub location: Location,
    #[serde(default)]
    pub rotation: Rotation,
    #[serde(default)]
    pub invert_pan: bool,
    #[serde(default)]
    pub invert_tilt: bool,
    #[serde(default)]
    pub bracket_angle: f32,
    #[serde(default)]
    pub shaper_angle: Option<f32>,
    #[serde(default)]
    pub installed_appearance: InstalledFixtureAppearance,
}

#[derive(Debug, Deserialize)]
pub struct ProfileRevision {
    pub profile_id: Uuid,
    #[serde(default)]
    pub profile_revision: u64,
    #[serde(default)]
    pub manufacturer: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub fixture_type: String,
    #[serde(default)]
    pub patch_policy: String,
    /// The server-resolved immutable profile snapshot. Older servers omit it.
    #[serde(default)]
    pub profile_snapshot: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct ObjectCollection {
    #[serde(default)]
    pub show_id: Option<Uuid>,
    #[serde(default)]
    pub show_revision: u64,
    #[serde(default)]
    pub objects: Vec<ObjectRecord>,
}

#[derive(Debug, Deserialize)]
pub struct ObjectRecord {
    pub id: String,
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub body: serde_json::Value,
}

/// Stored 3D stage position. The axes are the desk's own: `x` across, `y` upstage, `z` up.
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct StagePosition3d {
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    #[serde(default)]
    pub z: f32,
    #[serde(default, rename = "rotationX")]
    pub rotation_x: f32,
    #[serde(default, rename = "rotationY")]
    pub rotation_y: f32,
    #[serde(default, rename = "rotationZ")]
    pub rotation_z: f32,
    /// Optional authored Crowd Area footprint. Older layouts omit both and retain the package
    /// defaults. These are absolute metres so package revision changes cannot silently resize a
    /// show that the operator already authored.
    #[serde(default, rename = "crowdWidthMetres")]
    pub crowd_width_metres: Option<f32>,
    #[serde(default, rename = "crowdDepthMetres")]
    pub crowd_depth_metres: Option<f32>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct StagePosition2d {
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    #[serde(default)]
    pub rotation: f32,
}

#[derive(Debug, Default, Deserialize)]
pub struct StageLayoutBody {
    #[serde(default)]
    pub positions: std::collections::HashMap<String, StagePosition2d>,
    #[serde(default)]
    pub positions3d: std::collections::HashMap<String, StagePosition3d>,
}

#[derive(Debug, Deserialize)]
pub struct OutputRouteBody {
    #[serde(default)]
    pub protocol: String,
    #[serde(default)]
    pub logical_universe: u16,
    #[serde(default)]
    pub destination_universe: u16,
    #[serde(default)]
    pub delivery_mode: Option<String>,
    #[serde(default)]
    pub destination: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

const fn default_true() -> bool {
    true
}

/// One event frame from `/api/v2/events`.
#[derive(Debug, Deserialize)]
pub struct EventFrame {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub sequence: u64,
    #[serde(default, rename = "rendererSettings")]
    pub renderer_settings: Option<viz_scene::RendererSettingsUpdate>,
}

/// Preview values a planning window is driving the rig with.
///
/// Only the planning provider serves these; a lighting desk answers 404, and must, because a
/// desk's live values have to arrive as real Art-Net or sACN. Declared here rather than imported
/// so the renderer stays a client of an HTTP contract instead of depending on the editor's crate.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct PreviewSnapshot {
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub universes: Vec<PreviewUniverse>,
}

/// The desk's own live output, as universes.
///
/// Read only by a renderer the desk is running inside its own window. A renderer on the network
/// gets its values as real Art-Net or sACN and this is none of its business — see
/// [`crate::DeskConnection::values_from_desk_output`] for why the distinction is not a loophole.
#[derive(Clone, Debug, Deserialize)]
pub struct OutputDmxSnapshot {
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub universes: Vec<PreviewUniverse>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PreviewUniverse {
    pub universe: u16,
    /// All 512 slots, as the planning window projected them.
    pub slots: Vec<u8>,
}

/// A desk's typed event envelope, which names the change inside its payload.
#[derive(Debug, Deserialize)]
struct TypedEventMessage {
    #[serde(default)]
    event: Option<TypedEvent>,
}

#[derive(Debug, Deserialize)]
struct TypedEvent {
    #[serde(default)]
    sequence: u64,
    #[serde(default)]
    payload: Option<TypedPayload>,
}

#[derive(Debug, Deserialize)]
struct TypedPayload {
    #[serde(default, rename = "type")]
    kind: String,
}

impl EventFrame {
    /// Read one frame from either source.
    ///
    /// A desk sends a typed envelope and names the change in its payload; the planning window
    /// sends the kind on its own. Both say the same thing, and the renderer cares about the same
    /// word either way, so it is taken from wherever it is.
    pub fn parse(text: &str) -> Option<Self> {
        if let Ok(message) = serde_json::from_str::<TypedEventMessage>(text)
            && let Some(event) = message.event
            && let Some(payload) = event.payload
            && !payload.kind.is_empty()
        {
            return Some(Self {
                kind: payload.kind,
                sequence: event.sequence,
                renderer_settings: None,
            });
        }
        let frame = serde_json::from_str::<Self>(text).ok()?;
        (!frame.kind.is_empty()).then_some(frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The desk names the change inside a typed envelope; the planning window names it on its
    /// own. Both are read, because the renderer connects to both.
    #[test]
    fn an_event_is_read_from_either_source() {
        let desk = r#"{"type":"event","event":{"sequence":41,"payload":{"type":"show_patch_changed","change":{}}}}"#;
        let frame = EventFrame::parse(desk).expect("a desk envelope");
        assert_eq!(frame.kind, "show_patch_changed");
        assert_eq!(frame.sequence, 41);

        let planner = r#"{"kind":"show_patch_changed","sequence":7}"#;
        let frame = EventFrame::parse(planner).expect("a planning frame");
        assert_eq!(frame.kind, "show_patch_changed");
        assert_eq!(frame.sequence, 7);
    }

    #[test]
    fn a_planning_settings_event_carries_its_exact_changed_fields() {
        let text = serde_json::json!({
            "kind": "renderer_settings_changed",
            "sequence": 8,
            "rendererSettings": {
                "revision": 12,
                "source": "editor",
                "changed": ["quality", "fog"],
                "settings": viz_scene::RendererSettings {
                    quality: Some("draft".into()),
                    fog: 0.02,
                    ..viz_scene::RendererSettings::default()
                }
            }
        })
        .to_string();
        let frame = EventFrame::parse(&text).expect("settings event");
        let update = frame.renderer_settings.expect("settings payload");
        assert_eq!(update.changed, ["quality", "fog"]);
        assert_eq!(update.settings.quality.as_deref(), Some("draft"));
        assert_eq!(update.settings.fog, 0.02);
    }

    /// A frame that names nothing — the socket's own hello, or an error — is not a change.
    #[test]
    fn a_frame_that_names_no_change_is_not_one() {
        assert!(EventFrame::parse(r#"{"type":"ready","cursor":{"sequence":0}}"#).is_none());
        assert!(EventFrame::parse(r#"{"type":"error","error":"nope"}"#).is_none());
        assert!(EventFrame::parse("not json at all").is_none());
    }

    #[test]
    fn unknown_fields_are_tolerated_across_the_read_boundary() {
        let snapshot: PatchSnapshot = serde_json::from_str(
            r#"{"show_id":"00000000-0000-0000-0000-000000000001","fixtures":[],
                "profile_revisions":[],"a_future_field":true}"#,
        )
        .expect("unknown fields must not be fatal");
        assert!(snapshot.fixtures.is_empty());
    }

    #[test]
    fn stage_positions_read_the_desk_field_names() {
        let position: StagePosition3d =
            serde_json::from_str(r#"{"x":1.5,"y":6.5,"z":3.5,"rotationY":-22}"#).unwrap();
        assert_eq!(position.z, 3.5);
        assert_eq!(position.rotation_y, -22.0);
    }

    #[test]
    fn an_output_route_defaults_to_enabled() {
        let route: OutputRouteBody = serde_json::from_str(
            r#"{"protocol":"art_net","logical_universe":1,"destination_universe":1}"#,
        )
        .unwrap();
        assert!(route.enabled);
        assert_eq!(route.protocol, "art_net");
    }
}
