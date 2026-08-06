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
            // Standing at stage left, looking towards stage right.
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
    pub fn shadow_budget(self) -> u32 {
        match self {
            Self::Draft => 0,
            Self::Standard => 3,
            Self::High => 6,
            Self::Ultra => 10,
        }
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
    /// a beam crossing it brightens and thins along its length. Cheap tiers keep the uniform
    /// medium because the noise costs a lookup at every march step.
    pub fn fog_detail(self) -> f32 {
        match self {
            Self::Draft => 0.0,
            Self::Standard => 0.0,
            Self::High => 0.45,
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

impl Camera {
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
            None => {
                // Stand in the audience: centred, a little above head height, and far enough back
                // that the widest of the rig's width and depth fits the frame.
                let extent = bounds.extent();
                let span = extent.x.max(extent.z);
                let eye = (bounds.min.y + extent.y * 0.45).max(1.8);
                Self {
                    position: Vec3::new(centre.x, eye, bounds.max.z + span * 0.75 + 4.0),
                    target: Vec3::new(centre.x, bounds.min.y + extent.y * 0.42, centre.z),
                    up: Vec3::Y,
                    fov_degrees: 45.0,
                    orthographic_size: radius,
                }
            }
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
    /// Draw fixture numbers and patch addresses beside each fixture in the plan views.
    pub show_labels: bool,
}

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
