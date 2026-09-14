//! A rigging chain as it is hung: real hoist-chain links, a hoist body at one end and, at the
//! other, a bow shackle on the last link made fast to the truss: a steelflex round a chord of
//! three- or four-point truss, a pipe clamp on a pipe or ladder truss, or the shackle alone.
//!
//! The links follow round-link hoist chain of 7 mm wire: 21 mm inside length, which is the pitch
//! because each link sits inside its neighbour by one wire thickness, 35 mm outside length and
//! 24 mm outside width. Each link is a stadium drawn from six tubes — two straights and two
//! semicircular ends of two segments each — which keeps a rig of many long chains to a sensible
//! instance count while still reading as a chain at close range.

use super::super::{FrameInstances, MeshInstance, MeshKind};
use super::push_tube;
use super::truss::ChordLine;
use glam::{Mat3, Mat4, Quat, Vec3};
use viz_scene::{ChainRig, SceneryObject};

/// Diameter of the wire a link is bent from.
const WIRE: f32 = 0.007;
/// Inside length of a link, which is also how far one link's centre is from the next.
pub(super) const PITCH: f32 = 0.021;
/// Outside length of a link, end to end.
const OUTER_LENGTH: f32 = 0.035;
/// Outside width of a link, side to side.
const OUTER_WIDTH: f32 = 0.024;
/// The most links one chain is drawn with, about 84 m of chain.
const MAX_LINKS: usize = 4000;

/// A chain hoist's motor and gearbox body, in metres (across, along the chain, through).
const HOIST_BODY: Vec3 = Vec3::new(0.28, 0.42, 0.26);
/// Length of the hook a hoist hangs by or lifts with.
const HOOK: f32 = 0.12;

/// Diameter of the steelflex: a sheathed wire-rope sling about 22 mm thick.
const SLING_RADIUS: f32 = 0.011;
/// How far a chain end looks for a truss chord to make fast to.
const SNAP_DISTANCE: f32 = 0.5;
/// Tubes round the chord in one wrap.
const WRAP_SEGMENTS: usize = 10;
/// A clear saturated purple, sRGB #7B2FBE in linear light, so a sling reads at a glance.
const STEELFLEX: Vec3 = Vec3::new(0.1981, 0.0284, 0.5149);

/// A 2 t bow shackle: wire the body is bent from.
const SHACKLE_WIRE: f32 = 0.013;
/// Clear width between the shackle's walls.
const SHACKLE_INSIDE_WIDTH: f32 = 0.021;
/// Clear length from the bolt to the inside of the bow.
const SHACKLE_INSIDE_LENGTH: f32 = 0.048;
/// Radius of the shackle bolt, 16 mm across.
const BOLT_RADIUS: f32 = 0.008;
/// How far the bolt stands out past each wall; the head and the nut sit on that length.
const BOLT_PROTRUSION: f32 = 0.010;
/// Radius of the bolt head and the nut.
const BOLT_HEAD_RADIUS: f32 = 0.013;
/// Tubes in the shackle's half-circle bow.
const BOW_SEGMENTS: usize = 6;

/// Flat steel a pipe clamp's band is bent from.
const CLAMP_THICKNESS: f32 = 0.008;
/// Width of the clamp band along the tube.
const CLAMP_WIDTH: f32 = 0.030;
/// Clearance between the tube and the inside of the band.
const CLAMP_GAP: f32 = 0.001;
/// Flat pieces round the tube, six to each half-ring.
const CLAMP_SEGMENTS: usize = 12;
/// How far the bolted ears at the band's split stand off the band.
const CLAMP_EAR: f32 = 0.016;
/// The clamp's eye plate: width, length out from the band, thickness.
const PLATE: Vec3 = Vec3::new(0.040, 0.050, 0.008);
/// Diameter of the eye through the plate, and how far its centre is from the plate's tip.
const EYE_DIAMETER: f32 = 0.020;
const EYE_FROM_TIP: f32 = 0.020;

/// How many links a chain `length` long is drawn with. The pitch stays the real one, so a longer
/// chain gets more links rather than longer ones.
pub(in crate::instances) fn link_count(length: f32) -> usize {
    (((length - (OUTER_LENGTH - PITCH)) / PITCH).round().max(1.0) as usize).min(MAX_LINKS)
}

/// A rigging chain hanging the height of its size, centred on its placement, with its hoist at
/// the motor end and, at the other end, a bow shackle on the last link made fast to whatever is
/// there. The fittings sit beyond the chain's ends.
pub(super) fn push_chain(
    frame: &mut FrameInstances,
    object: &SceneryObject,
    orientation: Quat,
    colour: Vec3,
    chords: &[ChordLine],
) {
    let length = object.size.y.max(0.1);
    let axes = ChainAxes {
        up: orientation * Vec3::Y,
        across: orientation * Vec3::X,
        through: orientation * Vec3::Z,
    };
    let top = object.position + axes.up * (length * 0.5);
    let bottom = object.position - axes.up * (length * 0.5);
    push_links(frame, object, length, &axes, colour);
    let rough = object.roughness;
    let count = link_count(length);
    let (motor, outward, fixed, last) = match object.detail.chain {
        ChainRig::Plain => return,
        ChainRig::MotorTop => (top, -axes.up, bottom, count - 1),
        ChainRig::MotorBottom => (bottom, axes.up, top, 0),
    };
    push_hoist(frame, motor, -outward, orientation, colour, rough);
    let shackle = Shackle::on_link(object.position, &axes, count, last, outward);
    push_shackle(frame, &shackle, colour, rough);
    match fixing_for(fixed, chords) {
        Fixing::Steelflex(tube) => push_steelflex(frame, &shackle, tube),
        Fixing::Flange(tube) => push_flange(frame, &shackle, tube, colour, rough),
        Fixing::Shackle => {}
    }
}

/// A chain's own directions in the world: `up` along the chain, the other two across it.
pub(super) struct ChainAxes {
    pub up: Vec3,
    pub across: Vec3,
    pub through: Vec3,
}

impl ChainAxes {
    /// Centre of link `index` of `count` on a chain centred on `position`; link 0 is the top.
    fn link_centre(&self, position: Vec3, count: usize, index: usize) -> Vec3 {
        position - self.up * ((index as f32 - (count - 1) as f32 * 0.5) * PITCH)
    }

    /// The direction a link's sides lie in: each link turned a quarter to its neighbours.
    fn link_side(&self, index: usize) -> Vec3 {
        if index % 2 == 0 {
            self.across
        } else {
            self.through
        }
    }
}

/// The links, centred along the chain, each turned a quarter to its neighbours.
fn push_links(
    frame: &mut FrameInstances,
    object: &SceneryObject,
    length: f32,
    axes: &ChainAxes,
    colour: Vec3,
) {
    let count = link_count(length);
    let radius = (OUTER_WIDTH - WIRE) * 0.5;
    let straight = (OUTER_LENGTH - WIRE) * 0.5 - radius;
    let wire = WIRE * 0.5;
    let up = axes.up;
    for index in 0..count {
        let centre = axes.link_centre(object.position, count, index);
        let side = axes.link_side(index);
        for sign in [1.0, -1.0] {
            let rail = centre + side * (radius * sign);
            push_tube(
                frame,
                rail + up * straight,
                rail - up * straight,
                wire,
                colour,
                object.roughness,
            );
            // A semicircular end of two segments: from one side, over the crown, to the other.
            let end = centre + up * (straight * sign);
            let crown = end + up * (radius * sign);
            for from_side in [side, -side] {
                push_tube(
                    frame,
                    end + from_side * radius,
                    crown,
                    wire,
                    colour,
                    object.roughness,
                );
            }
        }
    }
}

/// A chain hoist at a chain end: the body beyond the end, and its hook beyond the body.
fn push_hoist(
    frame: &mut FrameInstances,
    end: Vec3,
    outward: Vec3,
    orientation: Quat,
    colour: Vec3,
    roughness: f32,
) {
    let centre = end + outward * (HOIST_BODY.y * 0.5);
    frame.mesh(MeshKind::Cube).push(MeshInstance::new(
        Mat4::from_scale_rotation_translation(HOIST_BODY, orientation, centre),
        Vec3::new(0.06, 0.06, 0.065),
        0.6,
        Vec3::ZERO,
        0.2,
    ));
    let hook = centre + outward * (HOIST_BODY.y * 0.5);
    push_tube(frame, hook, hook + outward * HOOK, 0.012, colour, roughness);
}

/// A round tube a chain end is made fast to: one truss chord.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Tube {
    /// The point on the tube's axis nearest the chain end.
    pub centre: Vec3,
    /// Unit direction of the tube.
    pub axis: Vec3,
    /// Outside radius of the tube.
    pub radius: f32,
}

impl Tube {
    /// The unit direction from the tube's axis towards `point`, square to the tube, or `fallback`
    /// squared to it when `point` is on the axis.
    fn toward(&self, point: Vec3, fallback: Vec3) -> Vec3 {
        let square = |v: Vec3| v - self.axis * v.dot(self.axis);
        square(point - self.centre)
            .normalize_or(square(fallback).normalize_or(self.axis.any_orthonormal_vector()))
    }
}

/// What the non-motor end of a chain is made fast to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Fixing {
    /// A steelflex round a chord of three- or four-point truss.
    Steelflex(Tube),
    /// A pipe clamp on a pipe or a chord of two-point ladder truss.
    Flange(Tube),
    /// Nothing in reach: the shackle alone, made fast to the steel.
    Shackle,
}

/// How the chain end `end` is made fast: by the kind of truss its nearest chord within reach
/// belongs to.
pub(super) fn fixing_for(end: Vec3, chords: &[ChordLine]) -> Fixing {
    let nearest = chords
        .iter()
        .map(|chord| (chord, closest_on_segment(end, chord.start, chord.end)))
        .map(|(chord, point)| (chord, point, point.distance(end)))
        .filter(|(_, _, distance)| *distance <= SNAP_DISTANCE)
        .min_by(|a, b| a.2.total_cmp(&b.2));
    let Some((chord, point, _)) = nearest else {
        return Fixing::Shackle;
    };
    let tube = Tube {
        centre: point,
        axis: (chord.end - chord.start).normalize_or(Vec3::X),
        radius: chord.radius,
    };
    if chord.chords >= 3 {
        Fixing::Steelflex(tube)
    } else {
        Fixing::Flange(tube)
    }
}

fn closest_on_segment(point: Vec3, start: Vec3, end: Vec3) -> Vec3 {
    let run = end - start;
    let length_squared = run.length_squared();
    if length_squared < 1e-8 {
        return start;
    }
    start + run * ((point - start).dot(run) / length_squared).clamp(0.0, 1.0)
}

/// A bow shackle hung on a chain's last link.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Shackle {
    /// Centre of the bolt.
    pub bolt: Vec3,
    /// Unit direction of the bolt, square to the last link's plane.
    pub bolt_axis: Vec3,
    /// Unit direction from the bolt to the bow, away from the chain.
    pub outward: Vec3,
}

impl Shackle {
    /// The shackle on link `last` of a chain of `count` links, the bolt through that link's
    /// opening with the link's end wire bearing on it.
    fn on_link(position: Vec3, axes: &ChainAxes, count: usize, last: usize, outward: Vec3) -> Self {
        let centre = axes.link_centre(position, count, last);
        let side = axes.link_side(last);
        Self {
            bolt: centre + outward * (PITCH * 0.5 - BOLT_RADIUS),
            bolt_axis: axes.up.cross(side).normalize_or(axes.through),
            outward,
        }
    }

    /// Distance from the bolt axis to either wall's centre line.
    fn wall_offset() -> f32 {
        (SHACKLE_INSIDE_WIDTH + SHACKLE_WIRE) * 0.5
    }

    /// Centre of the bow's half circle.
    fn bow_centre(&self) -> Vec3 {
        let inside = SHACKLE_INSIDE_WIDTH * 0.5;
        self.bolt + self.outward * (BOLT_RADIUS + SHACKLE_INSIDE_LENGTH - inside)
    }

    /// The centre line of the bow's bottom wire, where a flange's eye takes it.
    fn bow_bottom(&self) -> Vec3 {
        self.bow_centre() + self.outward * Self::wall_offset()
    }

    /// Where a sling bearing on the inside of the bow's bottom has its centre line.
    fn sling_seat(&self) -> Vec3 {
        self.bolt + self.outward * (BOLT_RADIUS + SHACKLE_INSIDE_LENGTH - SLING_RADIUS)
    }
}

/// The shackle: two straight walls, a half-circle bow, and the bolt with its head and nut.
fn push_shackle(frame: &mut FrameInstances, shackle: &Shackle, colour: Vec3, roughness: f32) {
    let wire = SHACKLE_WIRE * 0.5;
    let offset = Shackle::wall_offset();
    let (bolt, axis, outward) = (shackle.bolt, shackle.bolt_axis, shackle.outward);
    let bow = shackle.bow_centre();
    for sign in [1.0, -1.0] {
        let wall = axis * (offset * sign);
        let top = bolt + wall - outward * BOLT_RADIUS;
        push_tube(frame, top, bow + wall, wire, colour, roughness);
    }
    let point = |step: usize| {
        let angle = std::f32::consts::PI * step as f32 / BOW_SEGMENTS as f32;
        bow + (axis * angle.cos() + outward * angle.sin()) * offset
    };
    for step in 0..BOW_SEGMENTS {
        push_tube(frame, point(step), point(step + 1), wire, colour, roughness);
    }
    let face = offset + wire;
    let tip = face + BOLT_PROTRUSION;
    push_tube(
        frame,
        bolt - axis * tip,
        bolt + axis * tip,
        BOLT_RADIUS,
        colour,
        roughness,
    );
    let (head, nut) = (BOLT_PROTRUSION * 0.8, BOLT_PROTRUSION);
    let (at, to) = (bolt + axis * face, bolt + axis * (face + head));
    push_tube(frame, at, to, BOLT_HEAD_RADIUS, colour, roughness);
    let (at, to) = (bolt - axis * face, bolt - axis * (face + nut));
    push_tube(frame, at, to, BOLT_HEAD_RADIUS, colour, roughness);
}

/// A steelflex from the shackle's bow round a chord: two legs from the bow to where they leave
/// the wrap, and the wrap hugging the chord in the plane across it.
fn push_steelflex(frame: &mut FrameInstances, shackle: &Shackle, tube: Tube) {
    let meet = shackle.sling_seat();
    // The sling's centre line runs one sling radius off the chord's surface.
    let radius = tube.radius + SLING_RADIUS;
    let toward = tube.toward(meet, -shackle.outward);
    let reach = (meet - tube.centre)
        .reject_from_normalized(tube.axis)
        .length();
    let sideways = tube.axis.cross(toward);
    // The legs leave the wrap where they are tangent to it, which is what a sling pulled tight
    // round a tube does; at the working distance that puts 45° between them.
    let (along, beside) = if reach > radius {
        let cosine = radius / reach;
        (cosine, (1.0 - cosine * cosine).sqrt())
    } else {
        (0.0, 1.0)
    };
    for sign in [1.0, -1.0] {
        let tangent = tube.centre + (toward * along + sideways * (beside * sign)) * radius;
        push_tube(frame, meet, tangent, SLING_RADIUS, STEELFLEX, 0.35);
    }
    // Straight tubes between corners cut inside the circle; pushing the corners out keeps the
    // middle of every tube at the hugging radius, so none passes into the chord.
    let corner = radius / (std::f32::consts::PI / WRAP_SEGMENTS as f32).cos();
    let point = |step: usize| {
        let angle = std::f32::consts::TAU * step as f32 / WRAP_SEGMENTS as f32;
        tube.centre + (toward * angle.cos() + sideways * angle.sin()) * corner
    };
    for step in 0..WRAP_SEGMENTS {
        let (from, to) = (point(step), point(step + 1));
        push_tube(frame, from, to, SLING_RADIUS, STEELFLEX, 0.35);
    }
}

/// A pipe clamp: two half-rings of flat steel round the tube with a bolted ear at each split, and
/// an eye plate standing out from the band towards the shackle, whose bow passes through the eye.
fn push_flange(
    frame: &mut FrameInstances,
    shackle: &Shackle,
    tube: Tube,
    colour: Vec3,
    roughness: f32,
) {
    let toward = tube.toward(shackle.bow_bottom(), -shackle.outward);
    let sideways = tube.axis.cross(toward);
    let inner = tube.radius + CLAMP_GAP;
    let outer = inner + CLAMP_THICKNESS;
    let half = std::f32::consts::PI / CLAMP_SEGMENTS as f32;
    let length = 2.0 * outer * half.tan();
    for step in 0..CLAMP_SEGMENTS {
        let angle = half * (2 * step + 1) as f32;
        let radial = toward * angle.cos() + sideways * angle.sin();
        let tangent = tube.axis.cross(radial);
        let centre = tube.centre + radial * (inner + CLAMP_THICKNESS * 0.5);
        let size = Vec3::new(CLAMP_THICKNESS, length, CLAMP_WIDTH);
        push_block(
            frame,
            centre,
            [radial, tangent, tube.axis],
            size,
            colour,
            roughness,
        );
    }
    // The halves meet at the sides, where each carries an ear the clamp bolt goes through.
    for side in [sideways, -sideways] {
        let centre = tube.centre + side * (outer + CLAMP_EAR * 0.5);
        let size = Vec3::new(CLAMP_EAR, CLAMP_THICKNESS * 2.0, CLAMP_WIDTH);
        push_block(
            frame,
            centre,
            [side, toward, tube.axis],
            size,
            colour,
            roughness,
        );
    }
    push_eye_plate(
        frame,
        shackle,
        tube.centre + toward * outer,
        toward,
        colour,
        roughness,
    );
}

/// The clamp's eye plate from `root` on the band out along `toward`, flat across the shackle's
/// bow so the bow's bottom wire runs through the eye. It is drawn as the four strips round the
/// eye, so the eye reads as a hole.
fn push_eye_plate(
    frame: &mut FrameInstances,
    shackle: &Shackle,
    root: Vec3,
    toward: Vec3,
    colour: Vec3,
    roughness: f32,
) {
    let normal = shackle
        .bolt_axis
        .reject_from_normalized(toward)
        .normalize_or(toward.any_orthonormal_vector());
    let width = normal.cross(toward);
    let basis = [toward, width, normal];
    let eye = PLATE.y - EYE_FROM_TIP;
    let hole = EYE_DIAMETER * 0.5;
    let (near, far) = (eye - hole, eye + hole);
    let side = (PLATE.x - EYE_DIAMETER) * 0.5;
    // (from, to) along the plate, (centre, width) across it.
    let strips = [
        ((0.0, near), (0.0, PLATE.x)),
        ((far, PLATE.y), (0.0, PLATE.x)),
        ((near, far), ((hole + side * 0.5), side)),
        ((near, far), (-(hole + side * 0.5), side)),
    ];
    for ((from, to), (across, wide)) in strips {
        let centre = root + toward * ((from + to) * 0.5) + width * across;
        let size = Vec3::new(to - from, wide, PLATE.z);
        push_block(frame, centre, basis, size, colour, roughness);
    }
}

/// A steel block `size` along the three unit directions of a right-handed `basis`.
fn push_block(
    frame: &mut FrameInstances,
    centre: Vec3,
    [x, y, z]: [Vec3; 3],
    size: Vec3,
    colour: Vec3,
    roughness: f32,
) {
    let rotation = Quat::from_mat3(&Mat3::from_cols(x, y, z));
    frame.mesh(MeshKind::Cube).push(MeshInstance::new(
        Mat4::from_scale_rotation_translation(size, rotation, centre),
        colour,
        roughness,
        Vec3::ZERO,
        0.55,
    ));
}

#[cfg(test)]
mod tests {
    use super::super::truss::chord_lines;
    use super::*;
    use viz_scene::SceneryKind;

    /// A known length gets the links that fit it at the real 21 mm pitch.
    #[test]
    fn a_chain_is_built_from_links_at_the_real_pitch() {
        // Ninety-nine links span 98 pitches plus one link's outside length.
        let length = 98.0 * PITCH + OUTER_LENGTH;
        assert_eq!(link_count(length), 99);
        assert_eq!(link_count(3.0), 142);
        assert_eq!(link_count(0.01), 1);
        assert!((PITCH - (OUTER_LENGTH - 2.0 * WIRE)).abs() < 1e-6);
    }

    fn truss(chords: u8) -> SceneryObject {
        SceneryObject {
            position: Vec3::new(0.0, 5.0, 0.0),
            size: Vec3::new(4.0, 0.29, 0.29),
            kind: SceneryKind::Truss,
            chords,
            ..SceneryObject::default()
        }
    }

    fn tube_of(fixing: Fixing) -> Tube {
        match fixing {
            Fixing::Steelflex(tube) | Fixing::Flange(tube) => tube,
            Fixing::Shackle => panic!("nothing in reach"),
        }
    }

    /// A chain end below square truss takes a steelflex round the nearest chord, centred on that
    /// chord's axis; out of reach, it takes the shackle alone.
    #[test]
    fn a_chain_end_near_four_point_truss_takes_a_steelflex_round_the_nearest_chord() {
        let chords = chord_lines(&[truss(4)]);
        assert_eq!(chords.len(), 4);
        assert!(chords.iter().all(|chord| chord.chords == 4));
        let end = Vec3::new(0.7, 4.7, 0.05);
        let fixing = fixing_for(end, &chords);
        assert!(matches!(fixing, Fixing::Steelflex(_)), "{fixing:?}");
        let tube = tube_of(fixing);
        let chord = chords
            .iter()
            .min_by(|a, b| {
                let distance =
                    |c: &ChordLine| closest_on_segment(end, c.start, c.end).distance(end);
                distance(a).total_cmp(&distance(b))
            })
            .unwrap();
        let on_axis = closest_on_segment(tube.centre, chord.start, chord.end);
        assert!(on_axis.distance(tube.centre) < 1e-5, "{tube:?}");
        assert!(tube.centre.y < 5.0, "a bottom chord, not a top one");
        assert!((tube.axis.dot(Vec3::X)).abs() > 0.99);
        assert_eq!(tube.radius, chord.radius);

        let far = fixing_for(Vec3::new(0.7, 3.0, 0.0), &chords);
        assert_eq!(far, Fixing::Shackle, "no truss in reach");
    }

    /// A pipe or two-point ladder truss takes a pipe clamp; a triangle takes a steelflex.
    #[test]
    fn a_pipe_or_ladder_truss_takes_a_flange_and_a_triangle_a_steelflex() {
        let end = Vec3::new(0.7, 4.8, 0.0);
        for (count, flange) in [(1, true), (2, true), (3, false), (4, false)] {
            let fixing = fixing_for(end, &chord_lines(&[truss(count)]));
            assert_eq!(
                matches!(fixing, Fixing::Flange(_)),
                flange,
                "{count} chords: {fixing:?}"
            );
            assert_ne!(fixing, Fixing::Shackle, "{count} chords");
        }
    }

    fn straight_axes() -> ChainAxes {
        ChainAxes {
            up: Vec3::Y,
            across: Vec3::X,
            through: Vec3::Z,
        }
    }

    /// The bolt runs through the last link's opening, square to that link's plane, so it turns
    /// with whichever way the last link faces.
    #[test]
    fn the_shackle_bolt_is_square_to_the_last_link() {
        let axes = straight_axes();
        let position = Vec3::new(0.0, 3.0, 0.0);
        let mut bolts = Vec::new();
        for count in [141, 142] {
            for (last, outward) in [(count - 1, -Vec3::Y), (0, Vec3::Y)] {
                let shackle = Shackle::on_link(position, &axes, count, last, outward);
                let side = axes.link_side(last);
                assert!(shackle.bolt_axis.dot(side).abs() < 1e-6, "{shackle:?}");
                assert!(shackle.bolt_axis.dot(Vec3::Y).abs() < 1e-6, "{shackle:?}");
                assert!((shackle.bolt_axis.length() - 1.0).abs() < 1e-5);
                // Inside the link, bearing on its end wire.
                let centre = axes.link_centre(position, count, last);
                let bearing = (shackle.bolt - centre).dot(outward) + BOLT_RADIUS;
                assert!((bearing - PITCH * 0.5).abs() < 1e-5, "{bearing}");
                assert!(shackle.bow_bottom().dot(outward) > shackle.bolt.dot(outward));
                bolts.push(shackle.bolt_axis.abs());
            }
        }
        assert!(
            bolts.iter().any(|axis| axis.x > 0.99) && bolts.iter().any(|axis| axis.z > 0.99),
            "the bolt turns with the last link: {bolts:?}"
        );
    }

    /// The wrap's centre line sits one sling radius, 11 mm, off the chord's surface, so every
    /// tube of it hugs the chord without passing into it; at the working distance the legs have
    /// 45° between them.
    #[test]
    fn the_steelflex_hugs_the_chord_at_chord_radius_plus_its_own() {
        let tube = Tube {
            centre: Vec3::new(0.0, 5.0, 0.0),
            axis: Vec3::X,
            radius: 0.025,
        };
        let hugging = tube.radius + 0.011;
        // A shackle whose sling seat sits where tangent legs meet at 45°.
        let reach = hugging / (std::f32::consts::PI / 8.0).sin();
        let mut shackle = Shackle {
            bolt: Vec3::ZERO,
            bolt_axis: Vec3::X,
            outward: Vec3::Y,
        };
        shackle.bolt = tube.centre - Vec3::Y * reach - (shackle.sling_seat() - shackle.bolt);
        let mut frame = FrameInstances::default();
        push_steelflex(&mut frame, &shackle, tube);
        let tubes: Vec<[f32; 3]> = frame
            .meshes
            .iter()
            .flat_map(|(_, entries)| entries.iter())
            .map(|instance| {
                [
                    instance.model[3][0],
                    instance.model[3][1],
                    instance.model[3][2],
                ]
            })
            .collect();
        assert_eq!(tubes.len(), 2 + WRAP_SEGMENTS);
        for middle in &tubes[2..] {
            let off_axis = Vec3::from_array(*middle) - tube.centre;
            assert!((off_axis.x).abs() < 1e-5);
            assert!((off_axis.length() - hugging).abs() < 1e-4, "{off_axis}");
        }
        let seat = shackle.sling_seat();
        let legs: Vec<Vec3> = tubes[..2]
            .iter()
            .map(|middle| (Vec3::from_array(*middle) - seat).normalize())
            .collect();
        let between = legs[0].angle_between(legs[1]).to_degrees();
        assert!((between - 45.0).abs() < 0.1, "{between}");
    }
}
