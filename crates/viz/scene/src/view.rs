//! Named views, camera, and rendering quality.

use crate::scene::Aabb;
use glam::Vec3;

/// The eight named modes. Source-visible names and wire values map one to one.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ViewMode {
    TopDown,
    LeftToRight,
    RightToLeft,
    FrontToBack,
    BackToFront,
    Lines3d,
    Simple3d,
    Full3d,
}

impl ViewMode {
    pub const ALL: [Self; 8] = [
        Self::TopDown,
        Self::LeftToRight,
        Self::RightToLeft,
        Self::FrontToBack,
        Self::BackToFront,
        Self::Lines3d,
        Self::Simple3d,
        Self::Full3d,
    ];

    /// Operator-visible name.
    pub fn label(self) -> &'static str {
        match self {
            Self::TopDown => "Top Down",
            Self::LeftToRight => "Left \u{2192} Right",
            Self::RightToLeft => "Right \u{2192} Left",
            Self::FrontToBack => "Front \u{2192} Back",
            Self::BackToFront => "Back \u{2192} Front",
            Self::Lines3d => "3D Lines",
            Self::Simple3d => "3D Simple",
            Self::Full3d => "3D Full",
        }
    }

    /// Stable wire value.
    pub fn wire(self) -> &'static str {
        match self {
            Self::TopDown => "top_down",
            Self::LeftToRight => "left_to_right",
            Self::RightToLeft => "right_to_left",
            Self::FrontToBack => "front_to_back",
            Self::BackToFront => "back_to_front",
            Self::Lines3d => "lines_3d",
            Self::Simple3d => "simple_3d",
            Self::Full3d => "full_3d",
        }
    }

    pub fn from_wire(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.wire() == value)
    }

    pub fn is_orthographic(self) -> bool {
        matches!(
            self,
            Self::TopDown
                | Self::LeftToRight
                | Self::RightToLeft
                | Self::FrontToBack
                | Self::BackToFront
        )
    }

    /// Whether this mode draws beams and surface lighting at all.
    pub fn renders_beams(self) -> bool {
        matches!(self, Self::Simple3d | Self::Full3d)
    }

    /// Whether this mode draws aim lines instead of beam volumes.
    pub fn renders_aim_lines(self) -> bool {
        matches!(self, Self::Lines3d) || self.is_orthographic()
    }

    /// Whether this mode simulates light at all.
    ///
    /// [`Self::Lines3d`] does not. It is a diagram of a rig — where the lamps are and where they
    /// are pointed — drawn as outlines, and an outline has no lighting to be right or wrong. That
    /// is what makes it the cheap view: nothing in it depends on a render style or on how brightly
    /// the room is lit, so neither setting is offered for it.
    pub fn simulates_light(self) -> bool {
        self.renders_beams()
    }

    /// Whether this mode draws each fixture's own model, or a box standing in for it.
    ///
    /// A box is the point of [`Self::Lines3d`], not a shortfall in it: an operator checking
    /// coverage wants the position and the aim, and a hundred detailed bodies are in the way of
    /// seeing them.
    pub fn draws_fixture_models(self) -> bool {
        !matches!(self, Self::Lines3d)
    }

    /// Whether this mode draws aim guidelines for every directional emitter, lit or not.
    ///
    /// In [`Self::Lines3d`] the guidelines are the picture, so they are always drawn rather than
    /// being something to switch on. Everywhere else the beams say where the light goes and a
    /// second set of lines over them is clutter.
    pub fn always_draws_aim_guides(self) -> bool {
        matches!(self, Self::Lines3d)
    }

    /// Whether this mode draws one kind of scenery at all.
    ///
    /// [`Self::Lines3d`] keeps what an operator stands a fixture on or aims one at — the staging,
    /// the walls, the props — because those are what the rig is arranged around, and drops the
    /// rigging and the soft goods. A truss drawn as a box is a wall across the picture hiding the
    /// lamps hanging off it, and a drape is a box hiding the stage behind it; neither tells the
    /// operator anything the aim lines were not already saying.
    pub fn draws_scenery(self, kind: crate::scene::SceneryKind) -> bool {
        use crate::scene::SceneryKind;
        if !matches!(self, Self::Lines3d) {
            return true;
        }
        matches!(
            kind,
            SceneryKind::Floor | SceneryKind::Wall | SceneryKind::Riser | SceneryKind::Prop
        )
    }

    /// Whether this mode is a drawn plan rather than a rendered picture.
    ///
    /// A plan view is outlines and labels on a plain page — a stage plot — not a shaded scene.
    pub fn is_plot(self) -> bool {
        self.is_orthographic()
    }

    /// Short operator-facing description of what is on screen, for the status bar.
    pub fn surface_label(self) -> &'static str {
        if self.is_plot() { "2D" } else { "3D" }
    }

    /// Deterministic view direction for the orthographic presets, looking along `-direction`.
    pub fn orthographic_direction(self) -> Option<Vec3> {
        match self {
            // Looking straight down.
            Self::TopDown => Some(Vec3::new(0.0, -1.0, 0.0)),
            // Standing in house left, which is stage right, and looking across the stage.
            //
            // Stage left and right are the performer's, facing the audience, so they are the
            // mirror of the operator's own left and right. The wire names are the axis rather
            // than either convention, and the operator-facing labels say both.
            Self::LeftToRight => Some(Vec3::new(1.0, 0.0, 0.0)),
            Self::RightToLeft => Some(Vec3::new(-1.0, 0.0, 0.0)),
            // Standing in the audience, looking upstage.
            Self::FrontToBack => Some(Vec3::new(0.0, 0.0, -1.0)),
            Self::BackToFront => Some(Vec3::new(0.0, 0.0, 1.0)),
            _ => None,
        }
    }
}

/// Bounded rendering cost tier. Independent from [`ViewMode`]; `Full3d` at `Draft` still renders
/// volumetrics, just with a smaller budget.
///
/// The four tiers are a ladder of what is in the beam, and each one adds to the one below it:
///
/// - **Draft** — the light cones, and nothing in them.
/// - **Standard** — and the gobos, so a projected pattern is a pattern rather than a plain cone.
/// - **High** — and the fall-off: shaped edges, shadows where a beam meets something opaque.
/// - **Ultra** — and the haze itself, drifting and uneven, so a beam through it varies along
///   its length instead of running through a uniform slab.
#[derive(
    Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub enum RenderQuality {
    Draft,
    Standard,
    High,
    Ultra,
}

impl RenderQuality {
    pub const ALL: [Self; 4] = [Self::Draft, Self::Standard, Self::High, Self::Ultra];

    pub fn label(self) -> &'static str {
        match self {
            Self::Draft => "Draft",
            Self::Standard => "Standard",
            Self::High => "High",
            Self::Ultra => "Ultra",
        }
    }

    pub fn wire(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Standard => "standard",
            Self::High => "high",
            Self::Ultra => "ultra",
        }
    }

    pub fn from_wire(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|quality| quality.wire() == value)
    }

    /// Ray-march steps through the participating medium.
    pub fn volumetric_steps(self) -> u32 {
        match self {
            Self::Draft => 12,
            Self::Standard => 24,
            Self::High => 40,
            Self::Ultra => 64,
        }
    }

    /// Maximum number of shadow-casting lights before the renderer degrades visibly.
    ///
    /// Shadows are part of beam fall-off — a beam stopping where it meets something opaque — so
    /// they start at High, with the rest of it.
    pub fn shadow_budget(self) -> u32 {
        match self {
            Self::Draft | Self::Standard => 0,
            Self::High => 6,
            Self::Ultra => 10,
        }
    }

    /// Whether a beam carries the pattern on its glass.
    ///
    /// Draft draws the cone alone: an operator checking where a rig is pointed does not need to
    /// know which gobo is in it, and sampling one costs a texture lookup at every march step.
    pub fn draws_gobos(self) -> bool {
        !matches!(self, Self::Draft)
    }

    /// Whether a beam is drawn with its fall-off: a feathered field edge, the light dropping away
    /// across the pool, and shadows where it meets something opaque.
    ///
    /// Below this a beam is an evenly filled cone. That reads clearly and costs almost nothing,
    /// which is what the cheap tiers are for.
    pub fn draws_beam_falloff(self) -> bool {
        matches!(self, Self::High | Self::Ultra)
    }

    /// Render-target scale applied before the upscale/composite pass.
    pub fn resolution_scale(self) -> f32 {
        match self {
            Self::Draft => 0.75,
            Self::Standard => 1.0,
            Self::High => 1.0,
            Self::Ultra => 1.0,
        }
    }

    /// How much the haze is allowed to vary through the room, `0..=1`.
    ///
    /// Real haze is never a uniform slab: it drifts, it is thicker where it was last pumped, and
    /// a beam crossing it brightens and thins along its length. Only Ultra pays for that. Every
    /// tier below it keeps the uniform medium, because the noise costs a lookup at every march
    /// step of every beam and it is the last thing an operator needs to see the rig.
    pub fn fog_detail(self) -> f32 {
        match self {
            Self::Draft | Self::Standard | Self::High => 0.0,
            Self::Ultra => 0.7,
        }
    }

    pub fn bloom_enabled(self) -> bool {
        !matches!(self, Self::Draft)
    }
}

/// Camera described without Euler-order ambiguity.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Camera {
    pub position: Vec3,
    pub target: Vec3,
    pub up: Vec3,
    /// Vertical field of view in degrees for perspective modes.
    pub fov_degrees: f32,
    /// Half-height in metres for orthographic modes.
    pub orthographic_size: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 6.0, 14.0),
            target: Vec3::new(0.0, 3.0, -2.0),
            up: Vec3::Y,
            fov_degrees: 45.0,
            orthographic_size: 8.0,
        }
    }
}

/// Where an audience's eyes are: standing height, centred, and seven metres into the room.
///
/// Stated rather than derived. A camera placed by framing maths is placed wherever the rig happens
/// to make it land, which is nowhere anybody sits — and a designer looking at a rig is asking what
/// it looks like from the seats, not from a bounding box.
const AUDIENCE_EYE_METRES: f32 = 1.65;
const AUDIENCE_DISTANCE_METRES: f32 = 7.0;

/// How much of the frame the floor in front of the stage may take, from the bottom.
///
/// This is what "looking slightly up" means as a number. An audience does not stare at the carpet;
/// the ground between them and the stage is a strip along the bottom of what they see, and the rig
/// has the rest.
const AUDIENCE_FLOOR_SHARE: f32 = 0.15;

impl Camera {
    /// The house view: standing in the audience, looking at the stage.
    pub fn audience(bounds: Aabb) -> Self {
        let centre = bounds.centre();
        let fov_degrees = 45.0_f32;
        // Seven metres in front of the downstage edge, at standing eye height, on the centre line.
        let front = if bounds.is_empty() { 0.0 } else { bounds.max.z };
        let floor = if bounds.is_empty() { 0.0 } else { bounds.min.y };
        let position = Vec3::new(
            centre.x,
            floor + AUDIENCE_EYE_METRES,
            front + AUDIENCE_DISTANCE_METRES,
        );

        /*
         * The pitch that puts the stage edge exactly where the floor share says it should be.
         *
         * The edge of the stage sits `AUDIENCE_EYE_METRES` below the eye and
         * `AUDIENCE_DISTANCE_METRES` away, so the angle down to it is fixed. Asking for it to land
         * a given fraction up the frame fixes the angle it must sit below the centre of the frame.
         * The difference between the two is how far the camera looks up — a few degrees, which is
         * what an audience does without noticing.
         */
        let down_to_edge = (AUDIENCE_EYE_METRES / AUDIENCE_DISTANCE_METRES).atan();
        let half_fov = (fov_degrees * 0.5).to_radians();
        let below_centre = ((1.0 - 2.0 * AUDIENCE_FLOOR_SHARE) * half_fov.tan()).atan();
        let pitch = down_to_edge - below_centre;

        // Far enough along the aim to be well past the rig, so the target is a direction rather
        // than a distance: nothing here orbits about it.
        let reach = (bounds.radius() * 2.0).max(AUDIENCE_DISTANCE_METRES * 2.0);
        Self {
            position,
            target: position + Vec3::new(0.0, -pitch.sin(), -pitch.cos()) * reach,
            up: Vec3::Y,
            fov_degrees,
            orthographic_size: bounds.radius().max(1.0),
        }
    }

    /// Deterministic framing of `bounds` for one orthographic preset.
    pub fn framed(mode: ViewMode, bounds: Aabb) -> Self {
        let centre = bounds.centre();
        let radius = bounds.radius().max(1.0);
        match mode.orthographic_direction() {
            Some(direction) => {
                let up = if direction.y.abs() > 0.9 {
                    Vec3::new(0.0, 0.0, -1.0)
                } else {
                    Vec3::Y
                };
                Self {
                    position: centre - direction * (radius * 3.0),
                    target: centre,
                    up,
                    fov_degrees: 45.0,
                    orthographic_size: radius * 1.15,
                }
            }
            None => Self::audience(bounds),
        }
    }
}

impl crate::values::ExternalCameraState {
    /// Convert the DMX-facing Yaw/Pitch/Roll pose to the renderer's ambiguity-free camera.
    ///
    /// Zero looks straight along world `-Z` with `+Y` up. Euler axes stay at the fixture boundary;
    /// the resulting direction/up vectors are what rendering and local override retain.
    pub fn as_camera(&self) -> Camera {
        let rotation = glam::Quat::from_euler(
            glam::EulerRot::YXZ,
            self.yaw_degrees.to_radians(),
            self.pitch_degrees.to_radians(),
            self.roll_degrees.to_radians(),
        );
        let position = Vec3::from_array(self.position_metres);
        Camera {
            position,
            target: position + rotation * Vec3::NEG_Z,
            up: rotation * Vec3::Y,
            fov_degrees: self.vertical_fov_degrees,
            orthographic_size: Camera::default().orthographic_size,
        }
    }
}

/// Which way round the picture is drawn.
///
/// A rendered stage is naturally light on dark. A stage plot is traditionally printed dark on
/// light, and gets taken into a rehearsal room on paper, so both have to look deliberate.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Theme {
    LightOnDark,
    DarkOnLight,
}

impl Theme {
    pub const ALL: [Self; 2] = [Self::LightOnDark, Self::DarkOnLight];

    pub fn label(self) -> &'static str {
        match self {
            Self::LightOnDark => "Light on dark",
            Self::DarkOnLight => "Dark on light",
        }
    }

    pub fn wire(self) -> &'static str {
        match self {
            Self::LightOnDark => "light_on_dark",
            Self::DarkOnLight => "dark_on_light",
        }
    }

    pub fn from_wire(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|theme| theme.wire() == value)
    }

    pub fn toggled(self) -> Self {
        match self {
            Self::LightOnDark => Self::DarkOnLight,
            Self::DarkOnLight => Self::LightOnDark,
        }
    }

    /// Page colour behind everything.
    pub fn background(self) -> [f32; 3] {
        match self {
            Self::LightOnDark => [0.004, 0.005, 0.007],
            Self::DarkOnLight => [0.94, 0.94, 0.945],
        }
    }

    /// Ink colour for plot outlines and labels.
    pub fn ink(self) -> [f32; 3] {
        match self {
            Self::LightOnDark => [0.82, 0.86, 0.92],
            Self::DarkOnLight => [0.08, 0.09, 0.11],
        }
    }

    /// Ink colour for secondary detail: scenery, and fixtures that make no light.
    pub fn faint_ink(self) -> [f32; 3] {
        match self {
            Self::LightOnDark => [0.34, 0.38, 0.44],
            Self::DarkOnLight => [0.52, 0.55, 0.6],
        }
    }

    /// The one colour every beam is drawn in on a plan, kept readable against the page.
    pub fn beam_ink(self) -> [f32; 3] {
        match self {
            Self::LightOnDark => [1.0, 0.78, 0.16],
            Self::DarkOnLight => [0.72, 0.5, 0.0],
        }
    }

    /// Ink for a fixture symbol or outline.
    ///
    /// Quieter than [`Self::ink`]. A rig has far more lanterns on it than anything else in the
    /// picture, and drawn at full strength they are what the eye lands on instead of the light —
    /// which is the one thing an operator opened the view to look at.
    pub fn symbol_ink(self) -> [f32; 3] {
        match self {
            Self::LightOnDark => [0.16, 0.18, 0.21],
            Self::DarkOnLight => [0.55, 0.58, 0.62],
        }
    }

    /// Ink for a fixture the operator has selected — the one thing allowed to stand out.
    ///
    /// Blue rather than a brighter grey, because "which of these am I about to change" has to be
    /// answerable at a glance and across a rig where every other symbol is the same shape.
    pub fn selected_ink(self) -> [f32; 3] {
        match self {
            Self::LightOnDark => [0.25, 0.62, 1.0],
            Self::DarkOnLight => [0.05, 0.35, 0.85],
        }
    }

    /// Ink for the fixture number and its patch address.
    pub fn label_ink(self) -> [f32; 3] {
        match self {
            Self::LightOnDark => [0.46, 0.68, 1.0],
            Self::DarkOnLight => [0.08, 0.3, 0.82],
        }
    }
}

/// Complete view state applied by the renderer.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ViewConfiguration {
    pub mode: ViewMode,
    pub camera: Camera,
    pub quality: RenderQuality,
    /// Operator-safe exposure multiplier.
    pub exposure: f32,
    /// How brightly everything that is not a light source is lit, `0..=1`. At zero the rig is
    /// visible only where a fixture puts light on it.
    pub ambient: f32,
    /// What every laser is drawn at, `1.0` being the built-in strength.
    ///
    /// Lasers are the one thing in the picture with no honest reference: how strong a beam looks
    /// against the rest of a rig depends on the haze, the room, the projector and the camera or
    /// eye it is landing in, and no single choice suits a designer checking a figure and one
    /// showing an audience what the night looks like. So it is the operator's, like the fog.
    pub laser_brightness: f32,
    pub theme: Theme,
    /// Draw screen-space fixture numbers and patch addresses beside fixtures in every Stage view.
    pub show_labels: bool,
    /// The colour behind everything, linear RGB.
    ///
    /// `None` leaves it to the theme, which is what a stage plot wants: a plot is ink on paper and
    /// the page is part of that choice. A rendered stage is a room, and what colour the far wall
    /// of the room is belongs to whoever is looking at it.
    pub background: Option<[f32; 3]>,
    /// Draw the reference grid on the ground plane.
    pub floor_grid: bool,
}

/// The room the rig hangs in, before anyone chooses otherwise: very dark, and blue rather than
/// neutral, because a stage seen from the house is never black and never grey.
pub const DEFAULT_BACKGROUND: [f32; 3] = [0.008, 0.010, 0.016];

impl Default for ViewConfiguration {
    fn default() -> Self {
        Self {
            mode: ViewMode::Full3d,
            camera: Camera::default(),
            quality: RenderQuality::High,
            exposure: 1.0,
            ambient: 0.06,
            laser_brightness: 1.0,
            theme: Theme::LightOnDark,
            show_labels: true,
            background: Some(DEFAULT_BACKGROUND),
            floor_grid: true,
        }
    }
}

impl ViewConfiguration {
    /// The colour to clear to: the operator's, where they have chosen one and the mode is a
    /// rendered picture rather than a printed plan.
    pub fn background_colour(&self) -> [f32; 3] {
        match self.background {
            Some(colour) if !self.mode.is_plot() => colour,
            _ => self.theme.background(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_named_mode_round_trips_through_its_wire_value() {
        for mode in ViewMode::ALL {
            assert_eq!(ViewMode::from_wire(mode.wire()), Some(mode));
            assert!(!mode.label().is_empty());
        }
    }

    #[test]
    fn external_camera_zero_pose_looks_forward_and_keeps_the_decoded_vertical_fov() {
        let state = crate::values::ExternalCameraState {
            fixture_id: uuid::Uuid::nil(),
            instance_id: uuid::Uuid::nil(),
            position_metres: [1.0, 2.0, 3.0],
            yaw_degrees: 0.0,
            pitch_degrees: 0.0,
            roll_degrees: 0.0,
            focal_length_millimetres: 50.0,
            vertical_fov_degrees: 26.991_466,
            patched: true,
            stale: false,
        };
        let camera = state.as_camera();
        assert_eq!(camera.position, Vec3::new(1.0, 2.0, 3.0));
        assert!(camera.target.abs_diff_eq(Vec3::new(1.0, 2.0, 2.0), 1e-6));
        assert!(camera.up.abs_diff_eq(Vec3::Y, 1e-6));
        assert_eq!(camera.fov_degrees, state.vertical_fov_degrees);
    }

    #[test]
    fn orthographic_presets_look_along_the_documented_stage_axes() {
        assert_eq!(
            ViewMode::FrontToBack.orthographic_direction(),
            Some(Vec3::new(0.0, 0.0, -1.0))
        );
        assert_eq!(
            ViewMode::LeftToRight.orthographic_direction(),
            Some(Vec3::new(1.0, 0.0, 0.0))
        );
        assert!(ViewMode::Full3d.orthographic_direction().is_none());
    }

    /// The house view is where an audience actually sits, and looks where they actually look.
    ///
    /// Both halves matter. A camera at the right height staring at the carpet shows a rig from the
    /// seats and still shows the wrong thing.
    #[test]
    fn the_audience_view_stands_in_the_house_and_looks_slightly_up() {
        let bounds = Aabb {
            min: Vec3::new(-5.0, 0.0, -5.0),
            max: Vec3::new(5.0, 6.0, 0.0),
        };
        let camera = Camera::audience(bounds);

        assert!(
            (camera.position.y - 1.65).abs() < 1e-4,
            "standing eye height, got {}",
            camera.position.y
        );
        assert!(
            (camera.position.x - bounds.centre().x).abs() < 1e-4,
            "on the centre line"
        );
        assert!(
            (camera.position.z - (bounds.max.z + 7.0)).abs() < 1e-4,
            "seven metres in front of the downstage edge, got {}",
            camera.position.z
        );

        let aim = (camera.target - camera.position).normalize();
        assert!(aim.z < 0.0, "looking upstage");
        assert!(
            aim.y > 0.0,
            "and slightly up rather than down at the floor, got {}",
            aim.y
        );
    }

    /// The floor between the audience and the stage is a strip along the bottom, not the subject.
    #[test]
    fn the_floor_in_front_of_the_stage_keeps_to_its_share_of_the_frame() {
        let bounds = Aabb {
            min: Vec3::new(-5.0, 0.0, -5.0),
            max: Vec3::new(5.0, 6.0, 0.0),
        };
        let camera = Camera::audience(bounds);

        // Where the downstage edge lands up the frame, as a fraction from the bottom.
        let edge = Vec3::new(camera.position.x, bounds.min.y, bounds.max.z);
        let aim = (camera.target - camera.position).normalize();
        let to_edge = (edge - camera.position).normalize();
        // Signed angle of the edge below the aim, in the vertical plane.
        let below = aim.y.asin() - to_edge.y.asin();
        let half_fov = (camera.fov_degrees * 0.5).to_radians();
        let share = (1.0 - below.tan() / half_fov.tan()) * 0.5;

        assert!(
            (share - 0.15).abs() < 0.02,
            "the ground in front of the stage takes {share:.3} of the frame, not the 0.15 asked for"
        );
    }

    #[test]
    fn top_down_framing_places_the_camera_above_the_scene() {
        let bounds = Aabb {
            min: Vec3::new(-5.0, 0.0, -5.0),
            max: Vec3::new(5.0, 6.0, 5.0),
        };
        let camera = Camera::framed(ViewMode::TopDown, bounds);
        assert!(camera.position.y > bounds.max.y);
    }
}
