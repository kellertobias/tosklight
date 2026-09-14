//! A truss as it is built: chords, zig-zag faces at close to 45°, an end frame just inside each
//! end, a coupler receiver on every chord end, and the conical coupler that joins it to the next
//! piece.
//!
//! The proportions follow the square truss a rig is usually hung from (Global Truss F34, Prolyte
//! H30V): 290 mm outside, 50 mm chords, 20 mm braces, a node about every chord spacing so the
//! diagonals run near 45°, and no uprights between the end frames. Every face zig-zags from the
//! chord before it going round the section, so opposite faces run the other way, which is why a
//! side view of a real truss shows an X in every bay.
//! The 2D plan (`apps/viz-editor/src/cad/trussPlan.ts`) draws from the same rules, so both views of
//! one truss agree.

use super::super::{FrameInstances, MeshInstance, MeshKind};
use super::push_tube;
use glam::{Mat4, Quat, Vec3};
use viz_scene::SceneryObject;

/// The members of one truss section, in metres, derived from its outside size alone.
#[derive(Clone, Copy, Debug, PartialEq)]
struct TrussParts {
    chords: u8,
    /// Chord tube diameter.
    chord: f32,
    /// Brace tube diameter.
    brace: f32,
    /// Distance between neighbouring chord centres.
    spacing: f32,
    /// How far the coupler receiver reaches in from each end.
    receiver: f32,
    /// Where the end frame sits, measured in from each end.
    end_frame: f32,
}

impl TrussParts {
    fn new(section: f32, chords: u8) -> Self {
        let chords = chords.clamp(1, 4);
        let chord = if chords == 1 {
            section.clamp(0.02, 0.1)
        } else {
            (section * 0.17).clamp(0.025, 0.06)
        };
        let brace = chord * 0.4;
        Self {
            chords,
            chord,
            brace,
            spacing: (section - chord).max(chord),
            receiver: chord,
            end_frame: chord + brace * 0.5,
        }
    }

    /// Where each chord sits in the section, as (across, up), in order round the section so
    /// neighbours share a face.
    fn chord_offsets(&self) -> Vec<(f32, f32)> {
        let half = self.spacing * 0.5;
        match self.chords {
            1 => vec![(0.0, 0.0)],
            2 => vec![(0.0, half), (0.0, -half)],
            3 => vec![(0.0, half), (half, -half), (-half, -half)],
            _ => vec![(half, half), (-half, half), (-half, -half), (half, -half)],
        }
    }

    /// The faces as pairs of neighbouring chords, taken in order round the section.
    fn faces(&self) -> Vec<(usize, usize)> {
        let count = self.chords as usize;
        match count {
            0 | 1 => Vec::new(),
            2 => vec![(0, 1)],
            _ => (0..count).map(|face| (face, (face + 1) % count)).collect(),
        }
    }

    /// How many bays fit between the end frames of a piece `length` long: as many as keep the
    /// diagonals nearest 45°.
    fn bays(&self, length: f32) -> usize {
        let inner = length - self.end_frame * 2.0;
        if inner <= self.brace {
            return 0;
        }
        ((inner / self.spacing).round() as usize).clamp(1, 400)
    }
}

/// The longest axis runs the truss; the other two are its section, `up` staying vertical when it
/// can.
fn local_axes(size: Vec3) -> (Vec3, Vec3, Vec3) {
    if size.x >= size.y && size.x >= size.z {
        (Vec3::X, Vec3::Z, Vec3::Y)
    } else if size.z >= size.y {
        (Vec3::Z, Vec3::X, Vec3::Y)
    } else {
        (Vec3::Y, Vec3::X, Vec3::Z)
    }
}

/// One truss piece placed in the world: its members, where it starts along its run, and each
/// chord's offset from the run's centre line.
struct TrussLayout {
    parts: TrussParts,
    run: Vec3,
    length: f32,
    start: Vec3,
    end: Vec3,
    chords: Vec<Vec3>,
}

impl TrussLayout {
    fn new(object: &SceneryObject, orientation: Quat) -> Self {
        let size = object.size.max(Vec3::splat(0.02));
        let (run_axis, across_axis, up_axis) = local_axes(size);
        let length = size.dot(run_axis);
        let parts = TrussParts::new(size.dot(across_axis).max(size.dot(up_axis)), object.chords);
        let run = orientation * run_axis;
        let (across, up) = (orientation * across_axis, orientation * up_axis);
        let start = object.position - run * (length * 0.5);
        let chords = parts
            .chord_offsets()
            .into_iter()
            .map(|(a, u)| across * a + up * u)
            .collect();
        Self {
            parts,
            run,
            length,
            start,
            end: start + run * length,
            chords,
        }
    }
}

/// One truss chord as a line in the world, for rigging that wraps round it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ChordLine {
    pub start: Vec3,
    pub end: Vec3,
    /// Outside radius of the chord tube, in metres.
    pub radius: f32,
}

/// Every chord of every truss in `scenery`, drawn where [`push_truss`] draws it.
pub(super) fn chord_lines(scenery: &[SceneryObject]) -> Vec<ChordLine> {
    scenery
        .iter()
        .filter(|object| object.kind == viz_scene::SceneryKind::Truss)
        .flat_map(|object| {
            let layout =
                TrussLayout::new(object, viz_scene::euler_degrees(object.rotation_degrees));
            let radius = layout.parts.chord * 0.5;
            let (start, end) = (layout.start, layout.end);
            layout.chords.into_iter().map(move |offset| ChordLine {
                start: start + offset,
                end: end + offset,
                radius,
            })
        })
        .collect()
}

pub(super) fn push_truss(
    frame: &mut FrameInstances,
    object: &SceneryObject,
    orientation: Quat,
    colour: Vec3,
) {
    let TrussLayout {
        parts,
        run,
        length,
        start,
        end,
        chords,
    } = TrussLayout::new(object, orientation);
    let rough = object.roughness;

    for offset in &chords {
        push_tube(
            frame,
            start + *offset,
            end + *offset,
            parts.chord * 0.5,
            colour,
            rough,
        );
    }
    if chords.len() < 2 {
        return;
    }
    for offset in &chords {
        for (face, inward) in [(start, run), (end, -run)] {
            let at = face + *offset;
            let receiver = parts.chord * 0.6;
            push_tube(
                frame,
                at,
                at + inward * parts.receiver,
                receiver,
                colour,
                rough,
            );
            push_coupler(frame, at, run, parts.chord, rough);
        }
    }
    let faces = parts.faces();
    for along in [parts.end_frame, length - parts.end_frame] {
        let at = start + run * along;
        for &(first, second) in &faces {
            let (from, to) = (at + chords[first], at + chords[second]);
            push_tube(frame, from, to, parts.brace * 0.5, colour, rough);
        }
        if chords.len() == 4 {
            // A box's end frame carries one diagonal across it, as the end elevation shows.
            let (from, to) = (at + chords[0], at + chords[2]);
            push_tube(frame, from, to, parts.brace * 0.5, colour, rough);
        }
    }
    let bays = parts.bays(length);
    if bays == 0 {
        return;
    }
    let bay = (length - parts.end_frame * 2.0) / bays as f32;
    for (first, second) in faces {
        for index in 0..bays {
            let near = start + run * (parts.end_frame + bay * index as f32);
            let far = near + run * bay;
            let diagonals: &[bool] = if object.detail.deco {
                // Deco truss crosses its diagonals in every bay.
                &[true, false]
            } else {
                &[index % 2 == 0]
            };
            for &forward in diagonals {
                let (from, to) = if forward {
                    (near + chords[first], far + chords[second])
                } else {
                    (near + chords[second], far + chords[first])
                };
                push_tube(frame, from, to, parts.brace * 0.5, colour, rough);
            }
        }
    }
}

/// The conical coupler joining this piece to the next: an egg centred on the chord end, half in
/// this piece's receiver and half in the neighbour's.
fn push_coupler(frame: &mut FrameInstances, at: Vec3, run: Vec3, chord: f32, roughness: f32) {
    let model = Mat4::from_scale_rotation_translation(
        Vec3::new(chord * 0.7, chord * 1.6, chord * 0.7),
        Quat::from_rotation_arc(Vec3::Y, run.normalize_or(Vec3::X)),
        at,
    );
    frame.mesh(MeshKind::Sphere).push(MeshInstance::new(
        model,
        Vec3::splat(0.72),
        roughness.min(0.35),
        Vec3::ZERO,
        0.9,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 290 mm truss braces at 45° like the manufacturer drawings: a node about every 240 mm.
    #[test]
    fn a_standard_section_braces_close_to_forty_five_degrees() {
        for (section, length) in [(0.29, 3.0), (0.34, 2.0), (0.52, 4.0), (0.29, 0.5)] {
            let parts = TrussParts::new(section, 4);
            let bays = parts.bays(length);
            let bay = (length - parts.end_frame * 2.0) / bays as f32;
            let degrees = (parts.spacing / bay).atan().to_degrees();
            assert!(
                (38.0..=52.0).contains(&degrees),
                "{section} m section, {length} m long: {degrees}°"
            );
        }
        let f34 = TrussParts::new(0.29, 4);
        assert!((f34.chord - 0.049).abs() < 0.002, "{}", f34.chord);
        assert!((f34.brace - 0.02).abs() < 0.001, "{}", f34.brace);
    }

    /// Opposite faces of a box run their first diagonal the other way, so a side view crosses.
    #[test]
    fn opposite_faces_of_a_box_zig_zag_the_other_way() {
        let parts = TrussParts::new(0.29, 4);
        let chords = parts.chord_offsets();
        let faces = parts.faces();
        // The first diagonal climbs from the face's first chord to its second.
        let rise = |(first, second): (usize, usize)| chords[second].1 - chords[first].1;
        let slant = |(first, second): (usize, usize)| chords[second].0 - chords[first].0;
        assert!(rise(faces[1]) * rise(faces[3]) < 0.0, "the two sides cross");
        assert!(
            slant(faces[0]) * slant(faces[2]) < 0.0,
            "top and bottom cross"
        );
        assert_eq!(TrussParts::new(0.29, 2).faces().len(), 1);
        assert!(TrussParts::new(0.05, 1).faces().is_empty());
    }
}
