//! A rigging chain as it is hung: real hoist-chain links, a hoist body at one end and, at the
//! other, a bow shackle on the last link with a steelflex made fast to the truss, or the shackle
//! alone when nothing is in reach.
//!
//! The steelflex goes round the chords on the side of the truss the chain comes from — the top
//! chords for a chain from the roof. Where one chord is uppermost (a pipe, a ladder truss on
//! edge, a triangle with its apex up) the sling is choked round that chord alone and its legs go
//! up to the shackle. Where two chords are level (a box, a triangle with its flat side up, a
//! ladder lying flat) the sling is basketed under both, up the outside of each, and the legs
//! meet at the shackle at 45° once the chain is hung at its working height.
//!
//! The links follow round-link hoist chain of 7 mm wire: 21 mm inside length, which is the pitch
//! because each link sits inside its neighbour by one wire thickness, 35 mm outside length and
//! 24 mm outside width. Each link is a stadium drawn from six tubes — two straights and two
//! semicircular ends of two segments each — which keeps a rig of many long chains to a sensible
//! instance count while still reading as a chain at close range.

use super::super::{FrameInstances, MeshInstance, MeshKind};
use super::push_tube;
use super::truss::ChordLine;
use glam::{Mat4, Quat, Vec3};
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
/// Tubes in each quarter turn a basket makes under a chord.
const BASKET_QUARTER_SEGMENTS: usize = 4;
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
    match fixing_for(fixed, outward, chords) {
        Fixing::Wrap(tube) => push_steelflex(frame, &shackle, tube),
        Fixing::Basket(near, far) => push_basket(frame, &shackle, near, far),
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
    /// A steelflex choked round the one chord nearest the chain: a pipe, the top chord of a
    /// ladder truss on edge, the apex of a triangle.
    Wrap(Tube),
    /// A steelflex basketed under the two level chords on the chain's side of the truss: the
    /// top chords of a box, of a triangle with its flat side up, or of a ladder lying flat.
    Basket(Tube, Tube),
    /// Nothing in reach: the shackle alone, made fast to the steel.
    Shackle,
}

/// How the chain end `end` is made fast, `outward` being the direction the chain leaves it in:
/// by the chords of the nearest truss within reach that face the chain.
///
/// The chords facing the chain are the ones nearest the chain along its own line — the top
/// chords for a chain from the roof, the bottom chords for a climbing hoist under a roof truss.
/// One chord alone there is wrapped; two level ones are basketed; more than two (a truss stood on
/// end, whose every chord is as near as the next) fall back to wrapping the nearest.
pub(super) fn fixing_for(end: Vec3, outward: Vec3, chords: &[ChordLine]) -> Fixing {
    let nearest = chords
        .iter()
        .map(|chord| (chord, closest_on_segment(end, chord.start, chord.end)))
        .map(|(chord, point)| (chord, point, point.distance(end)))
        .filter(|(_, _, distance)| *distance <= SNAP_DISTANCE)
        .min_by(|a, b| a.2.total_cmp(&b.2));
    let Some((nearest, _, _)) = nearest else {
        return Fixing::Shackle;
    };
    let tube_of = |chord: &ChordLine| Tube {
        centre: closest_on_segment(end, chord.start, chord.end),
        axis: (chord.end - chord.start).normalize_or(Vec3::X),
        radius: chord.radius,
    };
    // Height of each chord of that truss towards the chain, measured where the chain reaches it.
    let facing = -outward;
    let mut ranked: Vec<(f32, Tube)> = chords
        .iter()
        .filter(|chord| chord.truss == nearest.truss)
        .map(|chord| {
            let tube = tube_of(chord);
            (tube.centre.dot(facing), tube)
        })
        .collect();
    ranked.sort_by(|a, b| b.0.total_cmp(&a.0));
    let Some(&(top, _)) = ranked.first() else {
        return Fixing::Shackle;
    };
    let level: Vec<Tube> = ranked
        .iter()
        .take_while(|(height, tube)| top - height <= tube.radius)
        .map(|(_, tube)| *tube)
        .collect();
    match level.as_slice() {
        [one] => Fixing::Wrap(*one),
        [first, second] => Fixing::Basket(*first, *second),
        _ => Fixing::Wrap(tube_of(nearest)),
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

/// A steelflex basketed under two level chords: a leg from the shackle's seat down the outside
/// of each chord, a quarter turn under each, and the run between them under the truss.
///
/// The legs leave the seat tangent to the chords, so a chain hung at its working height — the
/// seat a chord spacing above the chords — has 45° between them; hung lower, the legs open out,
/// as a sling pulled tight does.
fn push_basket(frame: &mut FrameInstances, shackle: &Shackle, first: Tube, second: Tube) {
    let seat = shackle.sling_seat();
    let axis = first.axis;
    // The plane the basket lies in is square to the chords: across from one to the other, and
    // up from between them towards the chain.
    let across = (second.centre - first.centre)
        .reject_from_normalized(axis)
        .normalize_or(axis.any_orthonormal_vector());
    let middle = (first.centre + second.centre) * 0.5;
    let toward = (seat - middle)
        .reject_from_normalized(axis)
        .reject_from_normalized(across)
        .normalize_or(axis.cross(across));
    // The sling's centre line runs one sling radius off the chord's surface.
    let radius = first.radius.max(second.radius) + SLING_RADIUS;
    let quarter = std::f32::consts::FRAC_PI_2;
    let mut undersides = [Vec3::ZERO; 2];
    // The first chord is on the negative `across` side, so its outside is at π; the second's is
    // at 0. Each is wrapped from its outside round through its underside.
    for (index, (tube, outside)) in [(&first, std::f32::consts::PI), (&second, 0.0)]
        .into_iter()
        .enumerate()
    {
        // The leg comes down from the seat tangent to the outside of the chord: of the two
        // tangents from the seat, the one further from the other chord. A seat too close for a
        // tangent meets the chord at its outside.
        let outer = tube.centre + across * (outside.cos() * radius);
        let flat = (seat - tube.centre).reject_from_normalized(axis);
        let distance = flat.length();
        let tangent = if distance > radius {
            let towards_seat = flat / distance;
            let sideways = axis.cross(towards_seat);
            let cosine = radius / distance;
            let sine = (1.0 - cosine * cosine).sqrt();
            [
                tube.centre + (towards_seat * cosine + sideways * sine) * radius,
                tube.centre + (towards_seat * cosine - sideways * sine) * radius,
            ]
            .into_iter()
            .min_by(|a, b| a.distance(outer).total_cmp(&b.distance(outer)))
            .unwrap_or(outer)
        } else {
            outer
        };
        // Where the leg meets the chord as an angle round it in the (across, toward) plane,
        // then the turn on to the underside: from π to 3π/2 on the first chord, from 0 back to
        // -π/2 on the second.
        let local = tangent - tube.centre;
        let start = local.dot(toward).atan2(local.dot(across));
        let (from, to) = if outside > 1.0 {
            (start.rem_euclid(std::f32::consts::TAU), quarter * 3.0)
        } else {
            let start = if start > quarter {
                start - std::f32::consts::TAU
            } else {
                start
            };
            (start, -quarter)
        };
        // Straight tubes between corners cut inside the circle; pushing the corners out keeps
        // the middle of every tube at the hugging radius, so none passes into the chord. The
        // leg ends on the first corner too, so the tube after it stays outside as well.
        let step = (to - from) / BASKET_QUARTER_SEGMENTS as f32;
        let corner = radius / (step * 0.5).cos();
        let point =
            |angle: f32| tube.centre + (across * angle.cos() + toward * angle.sin()) * corner;
        let mut last = point(from);
        push_tube(frame, seat, last, SLING_RADIUS, STEELFLEX, 0.35);
        for count in 1..=BASKET_QUARTER_SEGMENTS {
            let next = point(from + step * count as f32);
            push_tube(frame, last, next, SLING_RADIUS, STEELFLEX, 0.35);
            last = next;
        }
        undersides[index] = last;
    }
    // The run under the truss, from one chord's underside to the other's.
    push_tube(
        frame,
        undersides[0],
        undersides[1],
        SLING_RADIUS,
        STEELFLEX,
        0.35,
    );
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

    fn rolled(mut object: SceneryObject, roll: f32) -> SceneryObject {
        object.rotation_degrees.x = roll;
        object
    }

    /// A chain from above a box truss is basketed under its two top chords, whichever chord is
    /// nearest; out of reach, it takes the shackle alone.
    #[test]
    fn a_chain_from_above_four_point_truss_baskets_the_two_top_chords() {
        let chords = chord_lines(&[truss(4)]);
        assert_eq!(chords.len(), 4);
        assert!(chords.iter().all(|chord| chord.chords == 4));
        // Nearer a bottom chord than a top one, and still the top chords take it.
        let end = Vec3::new(0.7, 4.85, 0.16);
        let fixing = fixing_for(end, -Vec3::Y, &chords);
        let Fixing::Basket(first, second) = fixing else {
            panic!("{fixing:?}");
        };
        for tube in [first, second] {
            assert!(tube.centre.y > 5.0, "a top chord: {tube:?}");
            assert!(
                (tube.centre.x - 0.7).abs() < 1e-5,
                "where the chain reaches it"
            );
            assert!(tube.axis.dot(Vec3::X).abs() > 0.99);
            assert_eq!(tube.radius, chords[0].radius);
        }
        assert!(
            (first.centre.z - second.centre.z).abs() > 0.2,
            "both top chords"
        );

        let far = fixing_for(Vec3::new(0.7, 3.0, 0.0), -Vec3::Y, &chords);
        assert_eq!(far, Fixing::Shackle, "no truss in reach");
    }

    /// A climbing hoist under a roof truss baskets the two chords facing it: the bottom ones.
    #[test]
    fn a_chain_from_below_baskets_the_bottom_chords() {
        let chords = chord_lines(&[truss(4)]);
        let fixing = fixing_for(Vec3::new(0.7, 4.7, 0.0), Vec3::Y, &chords);
        let Fixing::Basket(first, second) = fixing else {
            panic!("{fixing:?}");
        };
        assert!(first.centre.y < 5.0 && second.centre.y < 5.0, "{fixing:?}");
    }

    /// One chord uppermost is wrapped alone: a pipe, a ladder on edge, a triangle apex up. Two
    /// level chords are basketed: a triangle flat side up, a ladder lying flat.
    #[test]
    fn one_top_chord_is_wrapped_and_two_level_ones_are_basketed() {
        let end = Vec3::new(0.7, 4.8, 0.0);
        let cases = [
            (truss(1), true),
            (truss(2), true),
            (truss(3), true),
            (rolled(truss(3), 180.0), false),
            (rolled(truss(2), 90.0), false),
            (truss(4), false),
        ];
        for (object, wrapped) in cases {
            let fixing = fixing_for(end, -Vec3::Y, &chord_lines(std::slice::from_ref(&object)));
            assert_eq!(
                matches!(fixing, Fixing::Wrap(_)),
                wrapped,
                "{} chords rolled {}°: {fixing:?}",
                object.chords,
                object.rotation_degrees.x
            );
            assert_ne!(fixing, Fixing::Shackle);
            if let Fixing::Wrap(tube) = fixing {
                let top = chord_lines(std::slice::from_ref(&object))
                    .iter()
                    .map(|chord| chord.start.y)
                    .fold(f32::MIN, f32::max);
                assert!(
                    (tube.centre.y - top).abs() < 1e-4,
                    "the top chord: {tube:?}"
                );
            }
        }
    }

    /// A truss stood on end has no top chord, so the chain wraps whichever is nearest.
    #[test]
    fn a_truss_on_end_wraps_the_nearest_chord() {
        let mut upright = truss(4);
        upright.size = Vec3::new(0.29, 4.0, 0.29);
        let chords = chord_lines(std::slice::from_ref(&upright));
        let fixing = fixing_for(Vec3::new(0.16, 6.0, 0.12), -Vec3::Y, &chords);
        assert!(matches!(fixing, Fixing::Wrap(_)), "{fixing:?}");
    }

    /// A basket's sling never passes into either chord, its run under the truss lies below both,
    /// and its legs are symmetrical about the chain.
    #[test]
    fn the_basket_hugs_both_chords_and_runs_under_them() {
        let radius = 0.025;
        let first = Tube {
            centre: Vec3::new(0.0, 5.0, -0.12),
            axis: Vec3::X,
            radius,
        };
        let second = Tube {
            centre: Vec3::new(0.0, 5.0, 0.12),
            axis: Vec3::X,
            radius,
        };
        let mut shackle = Shackle {
            bolt: Vec3::ZERO,
            bolt_axis: Vec3::X,
            outward: -Vec3::Y,
        };
        let seat = Vec3::new(0.0, 5.3, 0.0);
        shackle.bolt = seat - (shackle.sling_seat() - shackle.bolt);
        let mut frame = FrameInstances::default();
        push_basket(&mut frame, &shackle, first, second);
        let hugging = radius + SLING_RADIUS;
        let middles: Vec<Vec3> = frame
            .meshes
            .iter()
            .flat_map(|(_, entries)| entries.iter())
            .map(|instance| {
                Vec3::from_array([
                    instance.model[3][0],
                    instance.model[3][1],
                    instance.model[3][2],
                ])
            })
            .collect();
        assert_eq!(middles.len(), 2 + BASKET_QUARTER_SEGMENTS * 2 + 1);
        for middle in &middles {
            for tube in [first, second] {
                let off_axis = (*middle - tube.centre).reject_from_normalized(tube.axis);
                assert!(
                    off_axis.length() > hugging - 1e-4,
                    "{middle} is inside {tube:?}"
                );
            }
        }
        let run = middles[middles.len() - 1];
        assert!(
            (run.y - (5.0 - hugging)).abs() < 2e-3,
            "under both chords: {run}"
        );
        // The legs mirror each other across the chain.
        let (left, right) = (middles[0], middles[1 + BASKET_QUARTER_SEGMENTS]);
        assert!(
            (left.y - right.y).abs() < 1e-4 && (left.z + right.z).abs() < 1e-4,
            "{left} {right}"
        );
        assert!(
            left.y < seat.y && left.y > 5.0,
            "the leg comes down to the chord: {left}"
        );
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
                assert!(shackle.sling_seat().dot(outward) > shackle.bolt.dot(outward));
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
