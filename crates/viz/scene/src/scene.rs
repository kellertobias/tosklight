//! Structural scene: what exists and where it is. Live values live in [`crate::SceneValues`].

use crate::diagnostics::FallbackReason;
use glam::{Quat, Vec3};
use std::sync::Arc;
use uuid::Uuid;

/// Compose an Euler triple the way the existing ToskLight stage does: `Rx * Ry * Rz`, degrees,
/// about the world axes. Keeping one shared helper stops the renderer from inventing a second
/// rotation order.
pub fn euler_degrees(rotation: Vec3) -> Quat {
    Quat::from_rotation_x(rotation.x.to_radians())
        * Quat::from_rotation_y(rotation.y.to_radians())
        * Quat::from_rotation_z(rotation.z.to_radians())
}

/// One complete, internally consistent scene revision.
///
/// `emitters` is flattened across every fixture so the renderer and the projection layer can
/// address one light-producing head by a stable index without walking a tree per frame.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Scene {
    pub revision: u64,
    pub show_id: Option<Uuid>,
    pub show_name: String,
    pub source_identity: String,
    pub fixtures: Vec<FixtureInstance>,
    pub emitters: Vec<EmitterInstance>,
    pub scenery: Vec<SceneryObject>,
    /// Fixture models read from the library, referenced by [`FixtureInstance::model`].
    pub models: Vec<crate::FixtureModel>,
    /// Gobo artwork read from the profiles in this scene, referenced by [`GoboSlot::artwork`].
    /// One image per distinct piece of glass, however many wheels point at it.
    pub gobo_artwork: Vec<GoboArtwork>,
    pub bounds: Aabb,
}

impl Scene {
    /// Recompute the scene bounds from fixture and scenery placement.
    /// The rig's own extent: the fixtures, without the room invented around them.
    ///
    /// [`Self::bounds`] includes the scenery, and the stage floor is a slab sized to the rig plus an
    /// apron — so the downstage edge of the bounds is metres in front of the downstage edge of the
    /// rig. Anything placing a camera relative to "the front of the stage" has to mean the rig, or
    /// it stands that much further back and fills the frame with floor.
    pub fn rig_bounds(&self) -> Aabb {
        let mut bounds = Aabb::empty();
        for fixture in &self.fixtures {
            bounds.expand(fixture.position);
        }
        if bounds.is_empty() { self.bounds } else { bounds }
    }

    pub fn recompute_bounds(&mut self) {
        let mut bounds = Aabb::empty();
        for fixture in &self.fixtures {
            bounds.expand(fixture.position);
        }
        for object in &self.scenery {
            bounds.expand(object.position - object.size * 0.5);
            bounds.expand(object.position + object.size * 0.5);
        }
        if bounds.is_empty() {
            bounds = Aabb {
                min: Vec3::new(-6.0, 0.0, -6.0),
                max: Vec3::new(6.0, 8.0, 6.0),
            };
        }
        self.bounds = bounds;
    }

    /// The bounds the operator wants in frame: the rig itself, padded, and never above the stage
    /// floor. A house floor or a cyc wall can be far larger than the rig, and framing on those
    /// leaves the rig a speck in the middle of an empty picture.
    pub fn framing_bounds(&self) -> Aabb {
        let mut rig = Aabb::empty();
        for fixture in &self.fixtures {
            rig.expand(fixture.position);
        }
        if rig.is_empty() {
            return self.bounds;
        }
        let padding = (rig.extent().max_element() * 0.12).max(1.0);
        let (min, max) = (
            rig.min - Vec3::splat(padding),
            rig.max + Vec3::splat(padding),
        );
        rig.expand(min);
        rig.expand(max);
        rig.min.y = rig.min.y.min(self.bounds.min.y);
        rig
    }

    /// Index of the fixture owning `emitter_index`, or `None` when the index is out of range.
    pub fn fixture_of_emitter(&self, emitter_index: usize) -> Option<&FixtureInstance> {
        let emitter = self.emitters.get(emitter_index)?;
        self.fixtures.get(emitter.fixture_index as usize)
    }
}

/// One gobo's artwork, as the mask it is: light passes where the image is white.
///
/// Square and single-channel by the time it gets here. A gate is a disc a few hundred pixels
/// across at most, so every piece of glass is resampled to one size on the way in and the
/// renderer can hold the whole library of them in one array.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GoboArtwork {
    pub edge: u32,
    /// `edge * edge` transmission values, row by row.
    pub mask: Vec<u8>,
}

/// One physical fixture instance. Multi-patch instances of one logical fixture each appear here
/// with their own transform while sharing `fixture_id`.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct FixtureInstance {
    pub instance_id: Uuid,
    pub fixture_id: Uuid,
    pub name: String,
    pub number: Option<u32>,
    /// Metres, stage space.
    pub position: Vec3,
    /// Mounting rotation in degrees about the world axes, applied `Rx * Ry * Rz`.
    pub rotation_degrees: Vec3,
    /// Degrees the mounting bracket is set to, positive nose-down.
    ///
    /// It turns about the fixture's own transverse axis, after the mounting rotation, because that
    /// is what a clamp or a yoke does: the bar decides which way the lantern faces and the bracket
    /// decides how far down it looks.
    pub bracket_degrees: f32,
    /// Degrees a fitted shaper or barn-door module is turned to, or `None` when none is fitted.
    pub shaper_degrees: Option<f32>,
    /// Installed source/CCT/gel multiplier in linear RGB for this exact physical instance.
    pub installed_colour: [f32; 3],
    /// Installed blade angles, used only where the fixture profile exposes the corresponding
    /// canonical shaper role. A renderer must not infer support from arbitrary model node names.
    pub installed_shaper_angles_degrees: [f32; 4],
    pub body: FixtureBody,
    /// `false` while the fixture is part of the show but has no DMX address.
    pub patched: bool,
    /// Logical universe and start address of the first patched split, for the plan-view label.
    pub address: Option<(u16, u16)>,
    /// Index into [`Scene::models`] when this fixture's profile carries one it could read.
    pub model: Option<u32>,
    /// Present when the scene had to substitute generic behaviour for this fixture.
    pub fallback: Option<FallbackReason>,
}

impl FixtureInstance {
    /// Mounting rotation as a quaternion, with the bracket angle on top of it.
    ///
    /// The bracket turns in the fixture's own frame, so it is composed after the placement
    /// rotation rather than added to it: a lantern turned to face across the stage and then
    /// angled down in its clamp points where both of those say, in that order.
    pub fn orientation(&self) -> Quat {
        euler_degrees(self.rotation_degrees) * self.bracket_rotation()
    }

    /// The bracket's own rotation, about the fixture's transverse axis.
    pub fn bracket_rotation(&self) -> Quat {
        if self.bracket_degrees.abs() < f32::EPSILON {
            return Quat::IDENTITY;
        }
        Quat::from_rotation_x(self.bracket_degrees.to_radians())
    }
}

/// Bounded proxy dimensions used when no model asset resolves.
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct FixtureBody {
    /// Metres.
    pub size: Vec3,
    pub kind: BodyKind,
}

impl Default for FixtureBody {
    fn default() -> Self {
        Self {
            size: Vec3::new(0.2, 0.24, 0.2),
            kind: BodyKind::Generic,
        }
    }
}

/// Broad body silhouette. The renderer draws a matching procedural proxy.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum BodyKind {
    Generic,
    /// Static lantern on a hook clamp: profile, Fresnel, PAR, flood, cyc.
    Lantern,
    /// Base plus yoke plus head; the yoke follows pan and the head follows tilt.
    MovingHead,
    /// Long bar of cells: strips, blinders, battens.
    Bar,
    /// Panel of cells.
    Matrix,
    /// Non-light-producing machine: hazers, foggers, fans.
    Machine,
}

/// One light-producing head projected into flat renderer form.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EmitterInstance {
    /// Index into [`Scene::fixtures`].
    pub fixture_index: u32,
    pub head_index: u16,
    pub label: String,
    /// Emitter origin in fixture-local space, metres, before pan/tilt.
    pub local_origin: Vec3,
    /// The point tilt turns the emitter about, in the same space. Zero is the fixture's origin.
    ///
    /// A moving head tilts about its own trunnions, and its body is drawn that way. An emitter
    /// swung about the hanging point instead travels through an arc the size of the fixture as
    /// the head tilts, and the beam leaves from somewhere off the side of the lamp.
    pub tilt_pivot: Vec3,
    /// Emitter aim in fixture-local space, degrees, before pan/tilt.
    pub local_orientation_degrees: Vec3,
    /// Pan binding, absent for a fixed emitter.
    pub pan: Option<MotionAxis>,
    /// Tilt binding, absent for a fixed emitter.
    pub tilt: Option<MotionAxis>,
    /// Narrow (peak) cone angle in degrees at zoom `0`.
    pub beam_angle_degrees: f32,
    /// Wide (field) cone angle in degrees at zoom `1`.
    pub field_angle_degrees: f32,
    /// What this head's light looks like before the desk asks it for anything.
    pub optics: EmitterOptics,
    pub kind: EmitterKind,
    /// Cell offsets in fixture-local space, metres. Always at least one entry.
    pub cells: EmitterLayoutCells,
    /// Scanner geometry, present exactly when `kind` is [`EmitterKind::Laser`].
    pub laser: Option<LaserOptics>,
    /// Whether the profile carries a live canonical angle attribute for each blade. Live values
    /// override the corresponding installed angle rather than being added to it.
    pub live_shaper_angle_roles: [bool; 4],
    /// Whether the profile carries any canonical role for each blade. Installed angles are only
    /// meaningful for those explicitly supported components.
    pub shaper_roles: [bool; 4],
    /// Whether live `shaper.rotation` owns the module pose for this emitter.
    pub live_shaper_rotation_role: bool,
}

/// What a laser projector's scanner can physically reach, resolved from the profile.
///
/// The scan engine works in a normalised square and knows nothing about the fixture it is running
/// in; these are the numbers that turn its output into rays in the room.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LaserOptics {
    /// The fixture's scan engine, as source text. Absent when the profile ships no script, which
    /// leaves the laser dark and diagnosed rather than guessing a pattern for it.
    pub script: Option<Arc<str>>,
    /// Identifies the exact script text. A change recompiles it, and that is the one seam live
    /// reload works through: replace the source, bump the key, and the next frame runs the new
    /// engine.
    pub script_key: u64,
    /// Half the full optical scan angle across X, in radians. A script `x` of `1` deflects the
    /// beam by exactly this much.
    pub scan_half_angle_x: f32,
    /// Half the full optical scan angle across Y, in radians.
    pub scan_half_angle_y: f32,
    /// Scanner speed in points per second. Together with the point count this decides how many
    /// complete scans fall inside one displayed frame, and therefore whether the figure reads as
    /// solid or as a moving dot.
    pub points_per_second: f32,
    /// Beam divergence in radians — full angle, not half. Around one milliradian for a show laser.
    pub divergence: f32,
    /// Beam diameter at the output window, metres, before divergence opens it up.
    pub aperture_metres: f32,
    /// Total optical output in watts with every colour at full, which is what separates a 500 mW
    /// projector from a 5 W one at the same DMX value.
    pub optical_power_watts: f32,
}

impl Default for LaserOptics {
    fn default() -> Self {
        Self {
            script: None,
            script_key: 0,
            // A 25-degree full scan angle each way: the middle of what show projectors offer.
            scan_half_angle_x: 25.0_f32.to_radians() * 0.5,
            scan_half_angle_y: 25.0_f32.to_radians() * 0.5,
            points_per_second: 30_000.0,
            divergence: 0.001,
            aperture_metres: 0.003,
            optical_power_watts: 1.0,
        }
    }
}

impl LaserOptics {
    /// Beam radius at `distance` metres from the window.
    pub fn radius_at(&self, distance: f32) -> f32 {
        self.aperture_metres * 0.5 + self.divergence * 0.5 * distance.max(0.0)
    }
}

/// The optical character of one head.
///
/// Two fixtures pointed at the same spot with the same angle and the same intensity still do not
/// look alike: a profile lays down a flat disc with a crisp rim, a PAR a bright middle inside a
/// soft halo, a Fresnel something in between. These are the numbers that carry that difference —
/// the beam angle sits beside them on the emitter, and the desk's own controls (zoom, iris, focus,
/// frost, shapers) are applied on top of them per frame.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EmitterOptics {
    /// Relative output, `1.0` being an ordinary fixture of its class. A 400 W engine against a
    /// 100 W one, not a dimmer level.
    pub output: f32,
    /// How hard the rim of the field is: `1.0` is a profile at focus, `0.0` a wash that has no
    /// edge to speak of. A focus or frost channel softens whatever this says.
    pub sharpness: f32,
    /// How evenly the field is filled: `1.0` is flat to the rim, `0.0` a bright centre that falls
    /// away quickly. Independent of sharpness — a lamp can have a soft rim and still be even
    /// across the middle, and a hard-edged one can have a hot spot.
    pub uniformity: f32,
    /// The lit surface the light leaves through.
    pub source: LightSource,
    /// The gobo wheel this head turns: one entry per slot, counting the open slot as zero.
    ///
    /// Empty means the profile declares no wheel, and the renderer divides the gobo channel into
    /// its own default number of drawn patterns — which is what every profile did before packages
    /// could carry artwork.
    pub gobo_wheel: Vec<GoboSlot>,
}

impl Default for EmitterOptics {
    fn default() -> Self {
        Self {
            output: 1.0,
            sharpness: 0.6,
            uniformity: 0.7,
            source: LightSource::default(),
            gobo_wheel: Vec::new(),
        }
    }
}

/// One slot on a declared gobo wheel.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GoboSlot {
    /// Index into [`Scene::gobo_artwork`], or `None` for a slot that declares none — the open
    /// slot, and any slot whose artwork could not be read.
    pub artwork: Option<u32>,
    /// What the profile calls this slot, for the surfaces that name one.
    pub name: String,
}

/// The shape and size of the light-emitting surface.
///
/// This belongs to the fixture rather than to one patched instance: every Source Four of the same
/// type has the same lens. A point-source spot has a small round one; a cyc flood a wide
/// rectangular one; a PAR an oval.
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct LightSource {
    pub form: SourceForm,
    /// Metres, across the head's own X axis.
    pub width: f32,
    /// Metres, across the other axis of the emitting face.
    ///
    /// A face stands across the aim, and an emitter aims along its own `-Y`, so this is measured
    /// on the head's `Z`: for a bar hung pointing down, the width runs along the bar and this runs
    /// front to back.
    pub height: f32,
}

impl LightSource {
    pub fn round(diameter: f32) -> Self {
        Self {
            form: SourceForm::Round,
            width: diameter,
            height: diameter,
        }
    }

    /// The radius a circular approximation of this source would have, for the geometry that has
    /// to be round.
    pub fn mean_radius(&self) -> f32 {
        (self.width + self.height) * 0.25
    }
}

impl Default for LightSource {
    fn default() -> Self {
        Self::round(0.12)
    }
}

/// The outline of the emitting surface.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SourceForm {
    #[default]
    Round,
    /// Wider than it is tall, or the reverse: a PAR's lens, a linear engine.
    Oval,
    /// A panel: cyc floods, blinders, LED bricks.
    Rectangular,
}

impl EmitterInstance {
    /// Cone half-angle in radians for a normalised zoom position.
    pub fn cone_half_angle(&self, zoom: f32) -> f32 {
        let narrow = self.beam_angle_degrees.max(0.5);
        let wide = self.field_angle_degrees.max(narrow);
        let angle = narrow + (wide - narrow) * zoom.clamp(0.0, 1.0);
        (angle * 0.5).to_radians()
    }
}

/// How an emitter is allowed to appear.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum EmitterKind {
    /// Projects a beam: aperture, aim, cone, surface hit, and volumetric shaft.
    Beam,
    /// Emissive only: visible source and glow, never an invented beam.
    Emissive,
    /// Contributes atmosphere instead of light, for example a hazer's fog outlet.
    Atmosphere,
    /// Draws a scanned path rather than a cone: a laser projector.
    ///
    /// Separate from [`Self::Beam`] because none of the beam machinery applies. There is no cone
    /// to size, no field to feather, and no single direction to aim — a laser's output for one
    /// frame is a path of hundreds of points, each with its own deflection, colour and dwell, and
    /// what it looks like is decided by the fixture's own scan engine rather than by any DMX
    /// parameter the desk can name.
    Laser,
}

/// Resolved pixel-cell offsets, metres, fixture-local.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EmitterLayoutCells {
    pub offsets: Vec<Vec3>,
}

impl EmitterLayoutCells {
    pub fn single() -> Self {
        Self {
            offsets: vec![Vec3::ZERO],
        }
    }

    pub fn len(&self) -> usize {
        self.offsets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.offsets.is_empty()
    }
}

/// One rotation binding driven by a decoded fixture parameter.
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct MotionAxis {
    /// Rotation axis in the parent node's space.
    pub axis: Vec3,
    /// Physical angle in degrees at parameter `0`.
    pub min_degrees: f32,
    /// Physical angle in degrees at parameter `1`.
    pub max_degrees: f32,
}

impl MotionAxis {
    pub fn degrees_at(&self, level: f32) -> f32 {
        self.min_degrees + (self.max_degrees - self.min_degrees) * level.clamp(0.0, 1.0)
    }
}

/// Visual-only stage object that occludes and receives light.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SceneryObject {
    pub id: Uuid,
    pub name: String,
    pub position: Vec3,
    pub rotation_degrees: Vec3,
    /// Metres.
    pub size: Vec3,
    pub colour: [f32; 3],
    pub roughness: f32,
    pub kind: SceneryKind,
    /// Chords in a truss cross-section: `1` is a pipe, `2` a ladder, `3` a triangle, `4` a box.
    /// Ignored by every other kind.
    pub chords: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SceneryKind {
    Floor,
    Wall,
    Riser,
    Truss,
    /// A hanging drape, drawn with folds rather than as a flat slab.
    Curtain,
    /// A handrail around a riser: posts and rails.
    Railing,
    /// A faceted mirror ball.
    MirrorBall,
    Prop,
}

/// Axis-aligned bounds used for orthographic framing and volumetric bounds.
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Default for Aabb {
    fn default() -> Self {
        Self::empty()
    }
}

impl Aabb {
    pub fn empty() -> Self {
        Self {
            min: Vec3::splat(f32::INFINITY),
            max: Vec3::splat(f32::NEG_INFINITY),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.min.x > self.max.x || self.min.y > self.max.y || self.min.z > self.max.z
    }

    pub fn expand(&mut self, point: Vec3) {
        self.min = self.min.min(point);
        self.max = self.max.max(point);
    }

    pub fn centre(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    pub fn extent(&self) -> Vec3 {
        (self.max - self.min).max(Vec3::splat(0.001))
    }

    pub fn radius(&self) -> f32 {
        self.extent().length() * 0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_fall_back_to_a_default_room_when_the_scene_is_empty() {
        let mut scene = Scene::default();
        scene.recompute_bounds();
        assert!(!scene.bounds.is_empty());
        assert!(scene.bounds.radius() > 1.0);
    }

    /// The bracket turns the fixture in its own frame, after the mounting rotation.
    ///
    /// A lantern turned to face across the stage and then angled down in its clamp looks down and
    /// across; adding the two angles as world Euler terms would instead have it look somewhere
    /// neither the bar nor the bracket says.
    #[test]
    fn the_bracket_angle_turns_the_fixture_about_its_own_transverse_axis() {
        let mut fixture = FixtureInstance {
            instance_id: Uuid::nil(),
            fixture_id: Uuid::nil(),
            name: "Lantern".into(),
            number: None,
            position: Vec3::ZERO,
            rotation_degrees: Vec3::new(0.0, 90.0, 0.0),
            bracket_degrees: 0.0,
            shaper_degrees: None,
            installed_colour: [1.0; 3],
            installed_shaper_angles_degrees: [0.0; 4],
            body: FixtureBody {
                size: Vec3::splat(0.3),
                kind: BodyKind::Lantern,
            },
            patched: true,
            address: None,
            model: None,
            fallback: None,
        };
        // Hung level and facing stage right, an emitter's rest direction is straight down.
        let level = fixture.orientation() * Vec3::NEG_Y;
        assert!((level.y + 1.0).abs() < 1e-5, "{level:?}");

        // Angled 90 degrees in its bracket, it looks along the way the yoke turned it.
        fixture.bracket_degrees = -90.0;
        let angled = fixture.orientation() * Vec3::NEG_Y;
        assert!(angled.y.abs() < 1e-5, "no longer looking down: {angled:?}");
        assert!(
            angled.x.abs() > 0.99,
            "it looks the way the mounting rotation faces it: {angled:?}"
        );
    }

    #[test]
    fn a_fixture_with_no_bracket_angle_keeps_its_mounting_rotation_exactly() {
        let fixture = FixtureInstance {
            instance_id: Uuid::nil(),
            fixture_id: Uuid::nil(),
            name: "Lantern".into(),
            number: None,
            position: Vec3::ZERO,
            rotation_degrees: Vec3::new(12.0, 34.0, 56.0),
            bracket_degrees: 0.0,
            shaper_degrees: None,
            installed_colour: [1.0; 3],
            installed_shaper_angles_degrees: [0.0; 4],
            body: FixtureBody {
                size: Vec3::splat(0.3),
                kind: BodyKind::Lantern,
            },
            patched: true,
            address: None,
            model: None,
            fallback: None,
        };
        let expected = euler_degrees(fixture.rotation_degrees);
        assert!((fixture.orientation().dot(expected).abs() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn framing_ignores_a_floor_far_wider_than_the_rig() {
        let mut scene = Scene::default();
        scene.fixtures.push(FixtureInstance {
            instance_id: Uuid::nil(),
            fixture_id: Uuid::nil(),
            name: "Spot".into(),
            number: Some(1),
            position: Vec3::new(0.0, 5.0, 0.0),
            rotation_degrees: Vec3::ZERO,
            bracket_degrees: 0.0,
            shaper_degrees: None,
            installed_colour: [1.0; 3],
            installed_shaper_angles_degrees: [0.0; 4],
            body: FixtureBody::default(),
            patched: true,
            address: None,
            model: None,
            fallback: None,
        });
        scene.scenery.push(SceneryObject {
            id: Uuid::nil(),
            name: "House floor".into(),
            position: Vec3::new(0.0, 0.0, 0.0),
            rotation_degrees: Vec3::ZERO,
            size: Vec3::new(200.0, 0.1, 200.0),
            colour: [0.2, 0.2, 0.2],
            roughness: 0.8,
            kind: SceneryKind::Floor,
            chords: 0,
        });
        scene.recompute_bounds();
        assert!(scene.bounds.radius() > 100.0);
        let framing = scene.framing_bounds();
        assert!(
            framing.radius() < 10.0,
            "the rig, not the room, decides the frame: {framing:?}"
        );
        assert!(framing.min.y <= 0.0, "the stage floor stays in shot");
    }

    #[test]
    fn cone_half_angle_interpolates_between_beam_and_field() {
        let emitter = EmitterInstance {
            fixture_index: 0,
            head_index: 0,
            label: "Main".into(),
            local_origin: Vec3::ZERO,
            tilt_pivot: Vec3::ZERO,
            local_orientation_degrees: Vec3::ZERO,
            pan: None,
            tilt: None,
            beam_angle_degrees: 10.0,
            field_angle_degrees: 30.0,
            optics: EmitterOptics::default(),
            kind: EmitterKind::Beam,
            cells: EmitterLayoutCells::single(),
            laser: None,
            live_shaper_angle_roles: [false; 4],
            shaper_roles: [false; 4],
            live_shaper_rotation_role: false,
        };
        assert!((emitter.cone_half_angle(0.0) - 5.0_f32.to_radians()).abs() < 1e-6);
        assert!((emitter.cone_half_angle(1.0) - 15.0_f32.to_radians()).abs() < 1e-6);
        assert!((emitter.cone_half_angle(0.5) - 10.0_f32.to_radians()).abs() < 1e-6);
    }
}
