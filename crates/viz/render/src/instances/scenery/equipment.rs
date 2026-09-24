//! Generated stage equipment: a flight-case rack, a PA top and a hanging line array.
//!
//! Each is built from its placed size, and each reads one count off its height the way it is
//! bought: a rack by the 19-inch units it holds, a line array by the elements hung under its
//! frame, and a PA top by whether it stands on a pole. The plan reads the same counts with the
//! same measures (see `apps/viz-editor/src/cad/equipmentPlan.ts`), so the two never disagree.

use super::super::{FrameInstances, MeshInstance, MeshKind};
use super::push_tube;
use glam::{Mat4, Quat, Vec3};
use viz_scene::SceneryObject;

/// One 19-inch rack unit, and the lid and base a rack case adds to the units it holds.
pub const RACK_UNIT: f32 = 0.04445;
pub const RACK_CASE: f32 = 0.12;
/// A line array's flying frame, and the height of one element hung under it.
pub const LINE_ARRAY_FRAME: f32 = 0.1;
pub const LINE_ARRAY_ELEMENT: f32 = 0.25;
/// A PA top cabinet's own height: anything the object is placed taller than this is its pole.
pub const PA_CABINET: f32 = 0.6;

/// How many rack units a case of this height holds.
pub fn rack_units(height: f32) -> usize {
    (((height - RACK_CASE) / RACK_UNIT).round() as usize).clamp(1, 48)
}

/// How many elements a line array of this height hangs.
pub fn line_array_elements(height: f32) -> usize {
    (((height - LINE_ARRAY_FRAME) / LINE_ARRAY_ELEMENT).round() as usize).clamp(1, 24)
}

/// How long a PA top's pole stand is, or none when it stands on its own cabinet.
pub fn pa_pole(height: f32) -> Option<f32> {
    let pole = height - PA_CABINET;
    (pole > 0.05).then_some(pole)
}

/// The panel and hardware tones drawn against a black case.
const PANEL: Vec3 = Vec3::new(0.09, 0.09, 0.1);
const METAL: Vec3 = Vec3::new(0.55, 0.56, 0.58);

fn push_block(
    frame: &mut FrameInstances,
    orientation: Quat,
    centre: Vec3,
    size: Vec3,
    colour: Vec3,
    roughness: f32,
) {
    frame.mesh(MeshKind::Cube).push(MeshInstance::new(
        Mat4::from_scale_rotation_translation(size.max(Vec3::splat(0.002)), orientation, centre),
        colour,
        roughness,
        Vec3::ZERO,
        0.0,
    ));
}

/// A flight-case rack: the case, and on its front a panel line at every unit it holds, so the
/// count reads from across the room.
pub(super) fn push_flight_rack(
    frame: &mut FrameInstances,
    object: &SceneryObject,
    orientation: Quat,
    colour: Vec3,
) {
    let size = object.size.max(Vec3::splat(0.05));
    push_block(frame, orientation, object.position, size, colour, 0.7);
    let up = orientation * Vec3::Y;
    let front = orientation * Vec3::Z;
    let units = rack_units(size.y);
    let bottom = object.position - up * (size.y * 0.5) + up * (RACK_CASE * 0.5);
    let face = front * (size.z * 0.5 + 0.002);
    for unit in 0..units {
        let centre = bottom + up * (RACK_UNIT * (unit as f32 + 0.5)) + face;
        push_block(
            frame,
            orientation,
            centre,
            Vec3::new(size.x * 0.82, RACK_UNIT * 0.86, 0.004),
            PANEL,
            0.5,
        );
    }
}

/// A PA top: its cabinet at the top of the placed height with a grille on its front, on a pole
/// stand and three feet when it is placed taller than the cabinet.
pub(super) fn push_pa_top(
    frame: &mut FrameInstances,
    object: &SceneryObject,
    orientation: Quat,
    colour: Vec3,
) {
    let size = object.size.max(Vec3::splat(0.05));
    let up = orientation * Vec3::Y;
    let front = orientation * Vec3::Z;
    let floor = object.position - up * (size.y * 0.5);
    let cabinet = size.y.min(PA_CABINET);
    let centre = floor + up * (size.y - cabinet * 0.5);
    push_block(
        frame,
        orientation,
        centre,
        Vec3::new(size.x, cabinet, size.z),
        colour,
        0.6,
    );
    push_block(
        frame,
        orientation,
        centre + front * (size.z * 0.5 + 0.002),
        Vec3::new(size.x * 0.86, cabinet * 0.86, 0.004),
        PANEL,
        0.9,
    );
    let Some(pole) = pa_pole(size.y) else {
        return;
    };
    push_tube(frame, floor, floor + up * pole, 0.018, METAL, 0.4);
    // A tripod's legs spread from a collar a quarter of the way up.
    let collar = floor + up * (pole * 0.25).min(0.4);
    let spread = (pole * 0.35).clamp(0.25, 0.6);
    for turn in 0..3 {
        let angle = std::f32::consts::TAU * turn as f32 / 3.0;
        let out = orientation * Vec3::new(angle.cos(), 0.0, angle.sin()) * spread;
        push_tube(frame, collar, floor + out, 0.012, METAL, 0.4);
    }
}

/// A hanging line array: the flying frame at the top and its elements under it, each a cabinet
/// with a grille, so the count reads from across the room.
pub(super) fn push_line_array(
    frame: &mut FrameInstances,
    object: &SceneryObject,
    orientation: Quat,
    colour: Vec3,
) {
    let size = object.size.max(Vec3::splat(0.05));
    let up = orientation * Vec3::Y;
    let front = orientation * Vec3::Z;
    let top = object.position + up * (size.y * 0.5);
    push_block(
        frame,
        orientation,
        top - up * (LINE_ARRAY_FRAME * 0.5),
        Vec3::new(size.x * 1.04, LINE_ARRAY_FRAME * 0.6, size.z * 0.9),
        METAL,
        0.4,
    );
    let elements = line_array_elements(size.y);
    let element = (size.y - LINE_ARRAY_FRAME) / elements as f32;
    for index in 0..elements {
        let centre = top - up * (LINE_ARRAY_FRAME + element * (index as f32 + 0.5));
        push_block(
            frame,
            orientation,
            centre,
            Vec3::new(size.x, element * 0.94, size.z),
            colour,
            0.6,
        );
        push_block(
            frame,
            orientation,
            centre + front * (size.z * 0.5 + 0.002),
            Vec3::new(size.x * 0.9, element * 0.7, 0.004),
            PANEL,
            0.9,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_count_is_read_off_the_height_the_way_the_equipment_is_bought() {
        assert_eq!(rack_units(RACK_CASE + RACK_UNIT * 6.0), 6);
        assert_eq!(rack_units(RACK_CASE + RACK_UNIT * 12.2), 12);
        assert_eq!(rack_units(0.0), 1);
        assert_eq!(
            line_array_elements(LINE_ARRAY_FRAME + LINE_ARRAY_ELEMENT * 8.0),
            8
        );
        assert_eq!(line_array_elements(100.0), 24);
        assert_eq!(pa_pole(PA_CABINET), None);
        assert!((pa_pole(PA_CABINET + 1.2).unwrap() - 1.2).abs() < 1e-5);
    }
}
