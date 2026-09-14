//! A rigging chain as it is hung: real hoist-chain links, a hoist body at one end and a steelflex
//! wrapped round the truss at the other.
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

/// Length of the shackle joining the chain's end link to the sling legs.
const SHACKLE: f32 = 0.06;
/// Radius of the steelflex wire rope.
const SLING_RADIUS: f32 = 0.004;
/// How far a chain end looks for a truss chord to wrap.
const SNAP_DISTANCE: f32 = 0.5;
/// The tube a steelflex wraps when there is no truss to wrap: 50 mm, like a truss chord.
const VIRTUAL_TUBE_RADIUS: f32 = 0.025;
/// Half the angle between the two sling legs where they meet at the shackle: 45° between them.
const LEG_HALF_ANGLE: f32 = std::f32::consts::PI / 8.0;
/// Tubes round the chord in one wrap.
const WRAP_SEGMENTS: usize = 10;
/// A clear saturated purple, sRGB #7B2FBE in linear light, so a sling reads at a glance.
const STEELFLEX: Vec3 = Vec3::new(0.1981, 0.0284, 0.5149);

/// How many links a chain `length` long is drawn with. The pitch stays the real one, so a longer
/// chain gets more links rather than longer ones.
pub(in crate::instances) fn link_count(length: f32) -> usize {
    (((length - (OUTER_LENGTH - PITCH)) / PITCH).round().max(1.0) as usize).min(MAX_LINKS)
}

/// A rigging chain hanging the height of its size, centred on its placement, with its hoist and
/// steelflex where its rig puts them. The fittings sit beyond the chain's ends.
pub(super) fn push_chain(
    frame: &mut FrameInstances,
    object: &SceneryObject,
    orientation: Quat,
    colour: Vec3,
    chords: &[ChordLine],
) {
    let length = object.size.y.max(0.1);
    let up = orientation * Vec3::Y;
    let across = orientation * Vec3::X;
    let through = orientation * Vec3::Z;
    let top = object.position + up * (length * 0.5);
    let bottom = object.position - up * (length * 0.5);
    push_links(frame, object, length, [up, across, through], colour);
    let rough = object.roughness;
    match object.detail.chain {
        ChainRig::Plain => {}
        ChainRig::MotorTop => {
            push_hoist(frame, top, up, orientation, colour, rough);
            push_steelflex(frame, bottom, -up, across, chords, colour, rough);
        }
        ChainRig::MotorBottom => {
            push_hoist(frame, bottom, -up, orientation, colour, rough);
            push_steelflex(frame, top, up, across, chords, colour, rough);
        }
    }
}

/// The links, centred along the chain, each turned a quarter to its neighbours.
fn push_links(
    frame: &mut FrameInstances,
    object: &SceneryObject,
    length: f32,
    [up, across, through]: [Vec3; 3],
    colour: Vec3,
) {
    let count = link_count(length);
    let radius = (OUTER_WIDTH - WIRE) * 0.5;
    let straight = (OUTER_LENGTH - WIRE) * 0.5 - radius;
    let wire = WIRE * 0.5;
    for index in 0..count {
        let offset = (index as f32 - (count - 1) as f32 * 0.5) * PITCH;
        let centre = object.position - up * offset;
        let side = if index % 2 == 0 { across } else { through };
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

/// The round tube a steelflex wraps: a truss chord, or a virtual tube where there is none.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Wrap {
    pub centre: Vec3,
    /// Unit direction of the tube being wrapped.
    pub axis: Vec3,
    /// Radius of the sling's centre line round the tube.
    pub radius: f32,
}

/// Where a steelflex hung off the chain end `end` wraps: round the nearest truss chord within
/// reach, or a 50 mm horizontal tube just beyond the end.
pub(super) fn wrap_for(end: Vec3, outward: Vec3, across: Vec3, chords: &[ChordLine]) -> Wrap {
    let nearest = chords
        .iter()
        .map(|chord| (chord, closest_on_segment(end, chord.start, chord.end)))
        .map(|(chord, point)| (chord, point, point.distance(end)))
        .filter(|(_, _, distance)| *distance <= SNAP_DISTANCE)
        .min_by(|a, b| a.2.total_cmp(&b.2));
    if let Some((chord, point, _)) = nearest {
        return Wrap {
            centre: point,
            axis: (chord.end - chord.start).normalize_or(Vec3::X),
            radius: chord.radius + SLING_RADIUS,
        };
    }
    let radius = VIRTUAL_TUBE_RADIUS + SLING_RADIUS;
    let meet = end + outward * SHACKLE;
    let mut axis = across - outward * across.dot(outward);
    axis.y = 0.0;
    Wrap {
        centre: meet + outward * (radius / LEG_HALF_ANGLE.sin()),
        axis: axis.normalize_or(Vec3::X),
        radius,
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

/// A steelflex at a chain end: a shackle on the end link, two legs from it to the wrap, and the
/// wrap round the tube in the plane across that tube.
fn push_steelflex(
    frame: &mut FrameInstances,
    end: Vec3,
    outward: Vec3,
    across: Vec3,
    chords: &[ChordLine],
    colour: Vec3,
    roughness: f32,
) {
    push_shackle(frame, end, outward, across, colour, roughness);
    let meet = end + outward * SHACKLE;
    let wrap = wrap_for(end, outward, across, chords);
    // The legs and the wrap share the plane across the tube, `toward` pointing at the shackle.
    let mut toward = meet - wrap.centre;
    toward -= wrap.axis * toward.dot(wrap.axis);
    let reach = toward.length();
    let toward = toward.normalize_or(-outward - wrap.axis * (-outward).dot(wrap.axis));
    let toward = toward.normalize_or(wrap.axis.any_orthonormal_vector());
    let sideways = wrap.axis.cross(toward);
    // The legs leave the wrap where they are tangent to it, which is what a sling pulled tight
    // round a tube does.
    let (along, beside) = if reach > wrap.radius {
        let cosine = wrap.radius / reach;
        (cosine, (1.0 - cosine * cosine).sqrt())
    } else {
        (0.0, 1.0)
    };
    for sign in [1.0, -1.0] {
        let tangent = wrap.centre + (toward * along + sideways * (beside * sign)) * wrap.radius;
        push_tube(frame, meet, tangent, SLING_RADIUS, STEELFLEX, 0.35);
    }
    let point = |step: usize| {
        let angle = std::f32::consts::TAU * step as f32 / WRAP_SEGMENTS as f32;
        wrap.centre + (toward * angle.cos() + sideways * angle.sin()) * wrap.radius
    };
    for step in 0..WRAP_SEGMENTS {
        push_tube(
            frame,
            point(step),
            point(step + 1),
            SLING_RADIUS,
            STEELFLEX,
            0.35,
        );
    }
}

/// A shackle where a chain is made fast: a short bow and its pin, pointing `outward`.
fn push_shackle(
    frame: &mut FrameInstances,
    at: Vec3,
    outward: Vec3,
    across: Vec3,
    colour: Vec3,
    roughness: f32,
) {
    let tip = at + outward * SHACKLE;
    for side in [0.02, -0.02] {
        push_tube(
            frame,
            at + across * side,
            tip + across * side,
            0.006,
            colour,
            roughness,
        );
    }
    push_tube(
        frame,
        tip + across * 0.025,
        tip - across * 0.025,
        0.007,
        colour,
        roughness,
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

    fn truss() -> SceneryObject {
        SceneryObject {
            position: Vec3::new(0.0, 5.0, 0.0),
            size: Vec3::new(4.0, 0.29, 0.29),
            kind: SceneryKind::Truss,
            chords: 4,
            ..SceneryObject::default()
        }
    }

    /// A steelflex below a truss wraps the nearest chord, centred on that chord's axis.
    #[test]
    fn a_steelflex_snaps_onto_the_nearest_truss_chord() {
        let chords = chord_lines(&[truss()]);
        assert_eq!(chords.len(), 4);
        let end = Vec3::new(0.7, 4.7, 0.05);
        let wrap = wrap_for(end, Vec3::Y, Vec3::X, &chords);
        let chord = chords
            .iter()
            .min_by(|a, b| {
                let distance =
                    |c: &ChordLine| closest_on_segment(end, c.start, c.end).distance(end);
                distance(a).total_cmp(&distance(b))
            })
            .unwrap();
        let on_axis = closest_on_segment(wrap.centre, chord.start, chord.end);
        assert!(on_axis.distance(wrap.centre) < 0.03, "{wrap:?}");
        assert!(wrap.centre.y < 5.0, "a bottom chord, not a top one");
        assert!((wrap.axis.dot(Vec3::X)).abs() > 0.99);
        assert!(wrap.radius > chord.radius);

        let far = wrap_for(Vec3::new(0.7, 3.0, 0.0), Vec3::Y, Vec3::X, &chords);
        assert!(
            far.centre.y < 3.2,
            "no truss in reach wraps a tube just beyond the end"
        );
        assert!(far.axis.y.abs() < 1e-5, "the virtual tube is horizontal");
    }
}
