//! Line-list guides: lit aim lines, dotted aim guides and box outlines.

use super::{BEAM_THROW_METRES, EmitterPose, FrameInstances, LineVertex, beam_length};
use glam::{Mat4, Vec3};

pub(super) fn push_aim_line(
    frame: &mut FrameInstances,
    origin: Vec3,
    pose: EmitterPose,
    intensity: f32,
    colour: Vec3,
) {
    let end =
        origin + pose.direction * beam_length(origin, pose.direction).min(BEAM_THROW_METRES * 0.55);
    /*
     * How bright the line is drawn, from the level the fixture is at.
     *
     * Curved rather than proportional, and with almost no floor. Nothing in this view is
     * tonemapped, so a line drawn at its literal level reads far brighter than the level is: half
     * and full looked nearly the same, and a lamp at one percent looked like a lamp that was on.
     * The curve puts the visible difference where an operator is working — half against full is a
     * real step now — and lets one percent be the barely-there line it should be.
     */
    let level = intensity.clamp(0.0, 1.0).powf(1.9).max(0.02);
    let near = colour.extend(level);
    let far = colour.extend(0.0);
    frame.lines.push(LineVertex {
        position: origin.to_array(),
        _pad: 0.0,
        colour: near.to_array(),
    });
    frame.lines.push(LineVertex {
        position: end.to_array(),
        _pad: 0.0,
        colour: far.to_array(),
    });
}

/// The twelve edges of a unit cube carried through `transform`.
///
/// What an outline view is made of. A rig drawn as outlines stays readable however many fixtures
/// are in it, because an outline hides nothing behind it: two heads at the same depth are both
/// still visible, which is exactly what a solid box in an unlit room cannot manage.
pub(crate) fn push_box_outline(
    frame: &mut FrameInstances,
    transform: Mat4,
    ink: Vec3,
    opacity: f32,
) {
    const CORNERS: [Vec3; 8] = [
        Vec3::new(-0.5, -0.5, -0.5),
        Vec3::new(0.5, -0.5, -0.5),
        Vec3::new(0.5, -0.5, 0.5),
        Vec3::new(-0.5, -0.5, 0.5),
        Vec3::new(-0.5, 0.5, -0.5),
        Vec3::new(0.5, 0.5, -0.5),
        Vec3::new(0.5, 0.5, 0.5),
        Vec3::new(-0.5, 0.5, 0.5),
    ];
    const EDGES: [(usize, usize); 12] = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 4),
        (0, 4),
        (1, 5),
        (2, 6),
        (3, 7),
    ];
    let colour = ink.extend(opacity);
    let world: Vec<Vec3> = CORNERS
        .iter()
        .map(|corner| transform.transform_point3(*corner))
        .collect();
    for (from, to) in EDGES {
        frame.line(world[from], world[to], colour, colour);
    }
}

/// The dotted line showing where an emitter is aimed, lit or not.
///
/// Dotted rather than solid, and drawn in the faint ink, so it never reads as light: a solid line
/// down the aim of every dark lamp in a rig is a picture of a hundred beams that are not on. It is
/// dashed by emitting the dashes as separate segments, which is what a line list can express — a
/// stipple pattern would be a whole pipeline for one kind of line.
pub(super) fn push_aim_guide(
    frame: &mut FrameInstances,
    origin: Vec3,
    pose: EmitterPose,
    ink: Vec3,
) {
    /// Length of one dash and of the gap after it, in metres.
    const DASH: f32 = 0.22;
    const GAP: f32 = 0.28;
    /// Most dashes worth drawing down one guide, so a long throw cannot flood the line buffer.
    const MAX_DASHES: usize = 48;

    let reach = beam_length(origin, pose.direction).min(BEAM_THROW_METRES * 0.55);
    if reach <= DASH {
        return;
    }
    let colour = (ink * 1.3).extend(0.85);
    // Fading out along the throw keeps the far end of a long guide from cluttering the picture,
    // and reads the way an aim does: certain at the lamp, less so where it lands.
    let mut travelled = 0.0;
    let mut drawn = 0;
    while travelled < reach && drawn < MAX_DASHES {
        let start = travelled;
        let end = (travelled + DASH).min(reach);
        let fade = |along: f32| colour * Vec3::ONE.extend(1.0 - (along / reach) * 0.75);
        frame.line(
            origin + pose.direction * start,
            origin + pose.direction * end,
            fade(start),
            fade(end),
        );
        travelled = end + GAP;
        drawn += 1;
    }
}
