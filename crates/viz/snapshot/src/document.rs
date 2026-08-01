//! What a snapshot records beside its geometry.
//!
//! The geometry file carries bodies, trusses and lamp faces. This carries everything a modelling
//! package needs that triangles cannot say: which lamp is which, where its light goes, how wide,
//! how hard-edged, how bright, and what the room it was captured in looked like.
//!
//! # Space
//!
//! Every vector here is already in the **Z-up** space a modelling package works in, because the
//! geometry file is written in the glTF convention and every importer turns that Y-up scene into
//! a Z-up one on the way in. Converting here, once, in code that is tested, keeps the arithmetic
//! out of the import script where it could not be.

use serde::{Deserialize, Serialize};

/// Version of this document's shape. A reader refuses what it does not understand rather than
/// guessing at a rig.
pub const FORMAT_VERSION: u32 = 1;

/// The stage-space to modelling-package conversion, applied to every vector in this document.
///
/// The visualizer's world is Y-up: `+X` stage right, `+Y` up, `+Z` towards the audience. A
/// Z-up package receives the geometry file through an importer that maps `(x, y, z)` to
/// `(x, -z, y)`, so the lights have to make the same journey or they would point somewhere the
/// rig is not.
pub fn to_z_up(vector: [f32; 3]) -> [f32; 3] {
    [vector[0], -vector[2], vector[1]]
}

/// One captured moment of a rig.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SnapshotDocument {
    pub format: u32,
    pub application: String,
    /// Local time the operator pressed the key, as they would read it back.
    pub captured_at: String,
    pub show: String,
    /// The desk, file or planner the scene came from.
    pub source: String,
    pub scene_revision: u64,
    /// Geometry file beside this document.
    pub geometry_file: String,
    pub units: String,
    pub up_axis: String,
    pub counts: SnapshotCounts,
    pub look: SnapshotLook,
    pub camera: SnapshotCamera,
    pub bounds: SnapshotBounds,
    pub lights: Vec<SnapshotLight>,
    /// What the capture could not carry, in the operator's words. Never empty in practice: a
    /// gobo's artwork and a prism's copies are this renderer's own optics, not geometry.
    pub notes: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct SnapshotCounts {
    pub fixtures: usize,
    pub heads: usize,
    /// Heads that were actually emitting, which is what became a light.
    pub live_beams: usize,
    pub triangles: usize,
}

/// The renderer-local look the picture was being judged under.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
pub struct SnapshotLook {
    /// Haze, `0..=1`. The renderer's own setting; never taken from a hazer's DMX.
    pub fog: f32,
    /// How brightly everything that is not a light source was lit, `0..=1`.
    pub ambient: f32,
    /// Operator exposure trim.
    pub exposure: f32,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
pub struct SnapshotCamera {
    pub position: [f32; 3],
    pub target: [f32; 3],
    pub fov_degrees: f32,
    pub orthographic: bool,
    /// Half-height in metres, for an orthographic view.
    pub orthographic_size: f32,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
pub struct SnapshotBounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

/// One emitting head, as a spot light.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SnapshotLight {
    /// Unique, readable, and stable for one capture: what the light is called in the package.
    pub name: String,
    pub fixture: String,
    pub fixture_number: Option<u32>,
    /// `universe.address`, or absent while the fixture is unpatched.
    pub address: Option<String>,
    pub position: [f32; 3],
    /// Unit aim.
    pub direction: [f32; 3],
    /// Linear colour, each channel `0..=1`.
    pub colour: [f32; 3],
    /// Visible level after dimmer, shutter and strobe, `0..=1`.
    pub intensity: f32,
    /// Radiant power in watts. See [`watts`].
    pub power_watts: f32,
    /// Full cone angle at the field edge, degrees.
    pub cone_degrees: f32,
    /// Fraction of the cone that is the soft edge, `0` a cut rim and `1` no rim at all.
    pub blend: f32,
    /// Radius of the lit surface, metres. A lamp is not a point, and a soft shadow says so.
    pub radius: f32,
    /// How far the light reaches before it meets the floor, metres.
    pub reach: f32,
}

/// Radiant intensity, in watts per steradian, that an ordinary lantern at a forty-degree field
/// makes at full.
///
/// The renderer's own levels are relative — a picture on a screen under automatic exposure — so
/// there is no photometric figure to carry across, and this is a look rather than a measurement.
/// It is chosen so that a lamp six or seven metres above a deck lays down a well-exposed pool with
/// no exposure trim at all, which is where a designer wants to start.
pub const REFERENCE_INTENSITY: f32 = 400.0;

/// The cone every fixture's intensity is measured against: a forty-degree field, an ordinary stage
/// lantern. Narrower than this concentrates the same light and is brighter; wider spreads it and is
/// dimmer. The renderer measures against the same cone, so the two agree.
const REFERENCE_HALF_ANGLE: f32 = 0.349;

/// How much of the true spread relation to apply, matching the renderer. Literal inverse solid
/// angle puts several hundred to one between a beam light and a flood — true, and unusable in one
/// picture.
const SPREAD_COMPRESSION: f32 = 0.55;

/// Radiant power for one head, in the watts a spot light is set in.
///
/// A spot light spreads its power over the whole sphere and then masks off everything outside its
/// cone, so the power that produces a given intensity does not depend on the cone angle. The cone
/// still decides how bright the fixture is, for the reason it does on a real stage: the same lamp
/// through a narrower gate is brighter, and a flood laying the same light across a wall is dimmer.
/// That relation is applied here rather than left to the cone, which would otherwise throw it away.
pub fn watts(output: f32, intensity: f32, half_angle: f32) -> f32 {
    let solid_angle = |half: f32| (1.0 - half.cos()).max(1e-6);
    let spread = (solid_angle(REFERENCE_HALF_ANGLE) / solid_angle(half_angle.clamp(0.002, 1.55)))
        .powf(SPREAD_COMPRESSION)
        .clamp(0.15, 12.0);
    4.0 * std::f32::consts::PI
        * REFERENCE_INTENSITY
        * output.clamp(0.05, 8.0)
        * intensity.clamp(0.0, 1.0)
        * spread
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn up_is_up_after_the_conversion() {
        // A lamp six metres above the deck must be six metres above the deck, and a beam aimed
        // down at the stage must still be aimed down at it.
        assert_eq!(to_z_up([0.0, 6.0, 0.0]), [0.0, 0.0, 6.0]);
        assert_eq!(to_z_up([0.0, -1.0, 0.0]), [0.0, 0.0, -1.0]);
    }

    #[test]
    fn the_audience_stays_in_front_of_the_stage() {
        // `+Z` is downstage in the visualizer, which is `-Y` in a Z-up package.
        assert_eq!(to_z_up([0.0, 0.0, 4.0]), [0.0, -4.0, 0.0]);
        // Stage right is unchanged, so a plan never comes back mirrored.
        assert_eq!(to_z_up([3.0, 0.0, 0.0]), [3.0, 0.0, 0.0]);
    }

    #[test]
    fn the_conversion_keeps_the_rig_the_right_way_round() {
        // A right-handed frame must stay right-handed: a mirrored rig would put every gobo and
        // every shaper cut the wrong way round.
        let x = glam::Vec3::from(to_z_up([1.0, 0.0, 0.0]));
        let y = glam::Vec3::from(to_z_up([0.0, 1.0, 0.0]));
        let z = glam::Vec3::from(to_z_up([0.0, 0.0, 1.0]));
        assert!(x.cross(y).dot(z) > 0.0, "handedness was flipped");
    }

    #[test]
    fn a_dark_lamp_draws_no_power_and_a_brighter_engine_draws_more() {
        let ordinary = REFERENCE_HALF_ANGLE;
        assert_eq!(watts(1.0, 0.0, ordinary), 0.0);
        assert!(watts(2.0, 1.0, ordinary) > watts(1.0, 1.0, ordinary));
    }

    #[test]
    fn a_lantern_at_the_reference_cone_makes_the_reference_intensity() {
        // A spot spreads its power over the sphere and masks off everything outside the cone, so
        // the intensity inside it is the power over `4 pi`. Every other number here is chosen
        // against that relation.
        let intensity = watts(1.0, 1.0, REFERENCE_HALF_ANGLE) / (4.0 * std::f32::consts::PI);
        assert!(
            (intensity - REFERENCE_INTENSITY).abs() < 1.0,
            "{intensity} should be the reference intensity"
        );
    }

    #[test]
    fn the_same_engine_zoomed_in_is_brighter_and_zoomed_out_is_dimmer() {
        // The whole difference between a beam light and a flood built on the same engine.
        let beam = watts(1.0, 1.0, 4.0_f32.to_radians());
        let ordinary = watts(1.0, 1.0, REFERENCE_HALF_ANGLE);
        let flood = watts(1.0, 1.0, 45.0_f32.to_radians());
        assert!(beam > ordinary, "{beam} should beat {ordinary}");
        assert!(flood < ordinary, "{flood} should fall short of {ordinary}");
    }
}
