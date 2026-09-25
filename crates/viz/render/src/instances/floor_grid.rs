//! The reference grid on the ground plane.

use super::{FLOOR_HEIGHT, FrameInstances, FrameStyle};
use glam::Vec3;
use viz_scene::Scene;

/// The reference grid on the ground plane.
///
/// Lines on the floor, not a floor. A filled plane is a surface: it takes light, it hides whatever
/// is under it, and it turns the bottom of the picture into a large flat area competing with the
/// rig for attention. What an operator actually wants from it is a sense of scale and of where the
/// centre line is, which is what a grid of dark lines gives without being lit at all.
pub(super) fn push_floor_grid(frame: &mut FrameInstances, scene: &Scene, style: &FrameStyle) {
    /// Metres between lines. A metre is the unit a rig is measured and marked out in.
    const SPACING: f32 = 1.0;
    /// How far past the rig the grid runs, so it never stops at the edge of the fixtures.
    const MARGIN: f32 = 4.0;
    /// The largest grid worth drawing, so an accidentally enormous scene cannot fill the buffer.
    const MAX_LINES: i32 = 200;

    let bounds = scene.bounds;
    let (min_x, max_x, min_z, max_z) = if bounds.is_empty() {
        (-8.0, 8.0, -8.0, 8.0)
    } else {
        (
            bounds.min.x - MARGIN,
            bounds.max.x + MARGIN,
            bounds.min.z - MARGIN,
            bounds.max.z + MARGIN,
        )
    };
    let first = |value: f32| (value / SPACING).floor() as i32;
    let last = |value: f32| (value / SPACING).ceil() as i32;
    let (x0, x1) = (first(min_x), last(max_x));
    let (z0, z1) = (first(min_z), last(max_z));
    if x1 - x0 > MAX_LINES || z1 - z0 > MAX_LINES {
        return;
    }

    // Dark, and only just visible. The grid is a reference the eye can find when it looks for it
    // and ignore when it does not; a bright one draws attention away from the only thing on the
    // stage that is supposed to be bright.
    // These were chosen while every line was multiplied by the glow the lit views use. The glow now
    // belongs to the views that simulate light, so the grid carries its own weight.
    let line = (style.faint_ink * 0.09).extend(1.0);
    // The centre lines are the ones an operator counts from, so they are drawn a little stronger.
    let centre = (style.faint_ink * 0.30).extend(1.0);
    let y = FLOOR_HEIGHT + 0.002;

    for step in x0..=x1 {
        let x = step as f32 * SPACING;
        let colour = if step == 0 { centre } else { line };
        frame.line(
            Vec3::new(x, y, min_z),
            Vec3::new(x, y, max_z),
            colour,
            colour,
        );
    }
    for step in z0..=z1 {
        let z = step as f32 * SPACING;
        let colour = if step == 0 { centre } else { line };
        frame.line(
            Vec3::new(min_x, y, z),
            Vec3::new(max_x, y, z),
            colour,
            colour,
        );
    }
}
