//! A stage element as it is built: a deck on top and what raises it under that.
//!
//! A deck on a scissor lift has a base frame on the floor and arms crossing in an X on both long
//! sides with a cross tube through their pivot. A high deck gets more scissor stages stacked on
//! each other, so no arm runs steeper than a real lift's.
//!
//! A deck on regular feet has one leg under each corner instead, the way a staging deck is built:
//! the legs are the rise and the deck's top is its own thickness above them.
//!
//! A flight of stairs climbs to the same height in fixed rises, with a rail up each side when it
//! carries them, and stands on its own steps rather than on anything under it.

use super::super::{FrameInstances, MeshInstance, MeshKind};
use super::push_tube;
use glam::{Mat4, Quat, Vec3};
use viz_scene::SceneryObject;

/// The thickest a deck is drawn; a low rise gets a proportionally thinner one.
const DECK: f32 = 0.1;
/// Section of the base frame's rails.
const RAIL: f32 = 0.05;
/// Radius of a scissor arm and a pivot cross tube.
const ARM_RADIUS: f32 = 0.02;
/// The steepest an arm runs from the horizontal.
const MAX_ARM_DEGREES: f32 = 40.0;
/// Painted steel for the lift, the frame and the legs.
const STEEL: Vec3 = Vec3::new(0.05, 0.05, 0.055);
/// The top a deck on fixed legs is built with; a very low deck gets a proportionally thinner one.
const TOP: f32 = 0.04;
/// Section of a fixed leg, as a staging leg is made.
const LEG: f32 = 0.06;

/// How many scissor stages lift a deck `rise` metres with arms running `run` metres, so no arm is
/// steeper than [`MAX_ARM_DEGREES`].
pub(super) fn scissor_stages(rise: f32, run: f32) -> usize {
    let per_stage = run.max(0.01) * MAX_ARM_DEGREES.to_radians().tan();
    ((rise / per_stage).ceil() as usize).clamp(1, 32)
}

/// A deck standing on one fixed leg under each corner, raised to the element's height.
///
/// A leg stands under a corner with its outer faces flush with the deck's edges, so it is under
/// the deck rather than beside it, and it carries the top from the floor to its underside: the
/// deck's surface is exactly the height the element is placed at.
pub(super) fn push_fixed_legs(
    frame: &mut FrameInstances,
    object: &SceneryObject,
    orientation: Quat,
    colour: Vec3,
) {
    let size = object.size.max(Vec3::splat(0.02));
    let height = size.y;
    let up = orientation * Vec3::Y;
    let along = orientation * Vec3::X;
    let beside = orientation * Vec3::Z;
    let floor = object.position - up * (height * 0.5);
    let cube = |frame: &mut FrameInstances, scale: Vec3, centre: Vec3, colour, rough, metal| {
        frame.mesh(MeshKind::Cube).push(MeshInstance::new(
            Mat4::from_scale_rotation_translation(scale, orientation, centre),
            colour,
            rough,
            Vec3::ZERO,
            metal,
        ));
    };

    let top = TOP.min(height * 0.4);
    cube(
        frame,
        Vec3::new(size.x, top, size.z),
        floor + up * (height - top * 0.5),
        colour,
        object.roughness,
        0.0,
    );

    let leg = LEG.min(size.x * 0.4).min(size.z * 0.4);
    let rise = height - top;
    if rise <= 0.0 {
        return;
    }
    for x in [1.0, -1.0] {
        for z in [1.0, -1.0] {
            let corner = floor
                + along * ((size.x - leg) * 0.5 * x)
                + beside * ((size.z - leg) * 0.5 * z)
                + up * (rise * 0.5);
            cube(frame, Vec3::new(leg, rise, leg), corner, STEEL, 0.5, 0.6);
        }
    }
}

pub(super) fn push_scissor_stage(
    frame: &mut FrameInstances,
    object: &SceneryObject,
    orientation: Quat,
    colour: Vec3,
) {
    let size = object.size.max(Vec3::splat(0.02));
    let height = size.y;
    let long_x = size.x >= size.z;
    let (length, depth) = if long_x {
        (size.x, size.z)
    } else {
        (size.z, size.x)
    };
    let up = orientation * Vec3::Y;
    let along = orientation * if long_x { Vec3::X } else { Vec3::Z };
    let beside = orientation * if long_x { Vec3::Z } else { Vec3::X };
    let floor = object.position - up * (height * 0.5);
    let cube = |frame: &mut FrameInstances, scale: Vec3, centre: Vec3, colour, rough, metal| {
        frame.mesh(MeshKind::Cube).push(MeshInstance::new(
            Mat4::from_scale_rotation_translation(scale, orientation, centre),
            colour,
            rough,
            Vec3::ZERO,
            metal,
        ));
    };

    let deck = (height * 0.25).min(DECK);
    cube(
        frame,
        Vec3::new(size.x, deck, size.z),
        floor + up * (height - deck * 0.5),
        colour,
        object.roughness,
        0.0,
    );

    let rail = (height * 0.1).min(RAIL);
    let width = RAIL.min(depth * 0.1).min(length * 0.1);
    let local = |run: f32, across: f32| {
        if long_x {
            Vec3::new(run, rail, across)
        } else {
            Vec3::new(across, rail, run)
        }
    };
    let lift = floor + up * (rail * 0.5);
    for sign in [1.0, -1.0] {
        let side = lift + beside * ((depth - width) * 0.5 * sign);
        cube(frame, local(length, width), side, STEEL, 0.5, 0.6);
        let end = lift + along * ((length - width) * 0.5 * sign);
        cube(frame, local(width, depth), end, STEEL, 0.5, 0.6);
    }

    let low = rail;
    let high = height - deck;
    let rise = high - low;
    if rise < ARM_RADIUS * 2.0 {
        return;
    }
    // The arms reach almost to the deck's ends, as a lift under a whole deck does; their ends and
    // thickness sit inside the deck and the base rails, which hide them.
    let half_run = (length * 0.5 - width - ARM_RADIUS).max(length * 0.2);
    let stages = scissor_stages(rise, half_run * 2.0);
    let stage = rise / stages as f32;
    let offset = (depth * 0.5 - width * 1.5).max(0.0);
    for index in 0..stages {
        let (bottom, top) = (low + stage * index as f32, low + stage * (index + 1) as f32);
        for sign in [1.0, -1.0] {
            let side = floor + beside * (offset * sign);
            let at = |run: f32, lift: f32| side + along * run + up * lift;
            push_tube(
                frame,
                at(-half_run, bottom),
                at(half_run, top),
                ARM_RADIUS,
                STEEL,
                0.5,
            );
            push_tube(
                frame,
                at(-half_run, top),
                at(half_run, bottom),
                ARM_RADIUS,
                STEEL,
                0.5,
            );
        }
        let pivot = floor + up * ((bottom + top) * 0.5);
        push_tube(
            frame,
            pivot + beside * offset,
            pivot - beside * offset,
            ARM_RADIUS,
            STEEL,
            0.5,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tall lift stacks stages rather than standing its arms up.
    #[test]
    fn no_scissor_arm_runs_steeper_than_forty_degrees() {
        for (rise, run) in [(0.3, 1.8), (1.0, 1.8), (2.5, 0.8), (0.05, 1.0)] {
            let stages = scissor_stages(rise, run);
            let degrees = (rise / stages as f32 / run).atan().to_degrees();
            assert!(degrees <= 40.0 + 1e-3, "{rise} m over {run} m: {degrees}°");
        }
        assert_eq!(scissor_stages(0.5, 1.8), 1);
        assert!(scissor_stages(2.5, 0.8) > 3);
    }
}

/// A stair's tread depth and rise, as a set of stage steps is built: 200 mm up, 280 mm along.
const STEP_RISE: f32 = 0.2;
const STEP_RUN: f32 = 0.28;
/// A handrail's post and rail sections, and how high the rail stands above the nosing.
const RAIL_RADIUS: f32 = 0.02;
const RAIL_HEIGHT: f32 = 0.9;

/// A flight of stairs: treads climbing the run, and a rail up each side when it carries them.
///
/// The steps climb along the flight's longer horizontal side, from the floor at the low end to
/// the top at the high end, so a flight set to a deck's height meets that deck's surface. The
/// number of steps follows the height rather than the depth, because a set of stage steps is made
/// in fixed rises and what changes is how many of them there are.
pub(super) fn push_stairs(
    frame: &mut FrameInstances,
    object: &SceneryObject,
    orientation: Quat,
    colour: Vec3,
) {
    let size = object.size.max(Vec3::splat(0.02));
    let height = size.y;
    let long_x = size.x >= size.z;
    let (run, width) = if long_x {
        (size.x, size.z)
    } else {
        (size.z, size.x)
    };
    let up = orientation * Vec3::Y;
    let along = orientation * if long_x { Vec3::X } else { Vec3::Z };
    let floor = object.position - up * (height * 0.5);
    let steps = ((height / STEP_RISE).round() as usize).clamp(1, 24);
    let rise = height / steps as f32;
    // The treads share the run between them, however deep the flight was placed.
    let tread = run / steps as f32;
    for index in 0..steps {
        // Each step is a block from the floor to its own nosing, so the flight reads as solid.
        let top = rise * (index + 1) as f32;
        let centre = floor + along * (-run * 0.5 + tread * (index as f32 + 0.5)) + up * (top * 0.5);
        frame.mesh(MeshKind::Cube).push(MeshInstance::new(
            Mat4::from_scale_rotation_translation(
                Vec3::new(
                    if long_x { tread } else { width },
                    top,
                    if long_x { width } else { tread },
                ),
                orientation,
                centre,
            ),
            colour,
            object.roughness,
            Vec3::ZERO,
            0.0,
        ));
    }
    // A rail up each chosen side, its posts standing on the nosings and its rail following the
    // climb. Left and right are as seen climbing: up the flight, with the steps rising ahead.
    let left = up.cross(along).normalize_or_zero();
    let sides = [
        (object.detail.handrails.left, 1.0),
        (object.detail.handrails.right, -1.0),
    ];
    for (railed, side) in sides {
        if !railed {
            continue;
        }
        let edge = left * (width * 0.5 - RAIL_RADIUS) * side;
        let nosing = |index: usize| {
            floor + edge + along * (-run * 0.5 + tread * index as f32) + up * (rise * index as f32)
        };
        for index in 0..=steps {
            let foot = nosing(index);
            push_tube(
                frame,
                foot,
                foot + up * RAIL_HEIGHT,
                RAIL_RADIUS,
                STEEL,
                0.45,
            );
        }
        push_tube(
            frame,
            nosing(0) + up * RAIL_HEIGHT,
            nosing(steps) + up * RAIL_HEIGHT,
            RAIL_RADIUS,
            STEEL,
            0.45,
        );
    }
}
