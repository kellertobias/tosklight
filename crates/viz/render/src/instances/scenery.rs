//! The room the rig hangs in: trusses, drapes, staging, railings and the mirror ball.
//!
//! Scenery is drawn from the same procedural meshes a fixture proxy uses, so an operator gets a
//! recognisable venue without the show carrying geometry for it. It is structural rather than
//! live: none of it moves with a DMX frame, which is why it is rebuilt only when the scene
//! revision changes.

use super::{FrameInstances, FrameStyle, MeshInstance, MeshKind};
use glam::{Mat4, Quat, Vec3};
use viz_scene::{Scene, SceneryKind, SceneryObject, euler_degrees};

pub(super) fn push_scenery(frame: &mut FrameInstances, scene: &Scene, style: &FrameStyle) {
    for object in &scene.scenery {
        // Not every view draws every kind. A lines view keeps what the rig is arranged around and
        // drops the rigging and the soft goods, which would only stand between the operator and
        // the lamps hanging off them.
        if !(style.scenery)(object.kind) {
            continue;
        }
        let orientation = euler_degrees(object.rotation_degrees);
        let colour = Vec3::from(object.colour);
        // An outline view draws every object it keeps as the outline of its own box, for the same
        // reason a fixture is one: nothing here is lit, so a solid is a black shape in a black
        // room. The stage floor is the exception — the ground already has the grid on it, and a
        // box around the ground is a box around everything.
        if !style.fixture_models {
            if object.kind != SceneryKind::Floor {
                super::push_box_outline(
                    frame,
                    Mat4::from_scale_rotation_translation(
                        object.size,
                        orientation,
                        object.position,
                    ),
                    style.faint_ink,
                    0.8,
                );
            }
            continue;
        }
        match object.kind {
            SceneryKind::Truss => push_truss(frame, object, orientation, colour),
            SceneryKind::Curtain => push_curtain(frame, object, orientation, colour),
            SceneryKind::Railing => push_railing(frame, object, orientation, colour),
            SceneryKind::MirrorBall => push_mirror_ball(frame, object, orientation),
            SceneryKind::Floor | SceneryKind::Wall | SceneryKind::Riser | SceneryKind::Prop => {
                let model = Mat4::from_scale_rotation_translation(
                    object.size,
                    orientation,
                    object.position,
                );
                frame.mesh(MeshKind::Cube).push(MeshInstance::new(
                    model,
                    colour,
                    object.roughness,
                    Vec3::ZERO,
                    0.0,
                ));
            }
        }
    }
}

/// One tube between two points, for a truss chord or a brace.
fn push_tube(
    frame: &mut FrameInstances,
    from: Vec3,
    to: Vec3,
    radius: f32,
    colour: Vec3,
    roughness: f32,
) {
    let axis = to - from;
    let length = axis.length();
    if length < 1e-4 {
        return;
    }
    // The unit cylinder stands on `Y`, so it is turned onto the run between the two points.
    let rotation = Quat::from_rotation_arc(Vec3::Y, axis / length);
    let model = Mat4::from_scale_rotation_translation(
        Vec3::new(radius * 2.0, length, radius * 2.0),
        rotation,
        (from + to) * 0.5,
    );
    frame.mesh(MeshKind::Cylinder).push(MeshInstance::new(
        model,
        colour,
        roughness,
        Vec3::ZERO,
        0.55,
    ));
}

/// A truss as it is actually built: chords running the length, with bracing between them.
///
/// A box drawn where a truss hangs tells an operator nothing. What they read a rig by is the
/// number of chords and the ladder of bracing, so that is what is drawn.
fn push_truss(frame: &mut FrameInstances, object: &SceneryObject, orientation: Quat, colour: Vec3) {
    let size = object.size.max(Vec3::splat(0.02));
    // The run is the longest axis; the other two give the cross-section.
    let (run_axis, cross) = if size.x >= size.y && size.x >= size.z {
        (Vec3::X, Vec3::new(size.y, size.z, 0.0))
    } else if size.y >= size.z {
        (Vec3::Y, Vec3::new(size.x, size.z, 0.0))
    } else {
        (Vec3::Z, Vec3::new(size.x, size.y, 0.0))
    };
    let length = (size * run_axis.abs()).max_element();
    let run = orientation * run_axis;
    let (across, up) = truss_cross_axes(run_axis, orientation);
    let half_across = (cross.x * 0.5).max(0.02);
    let half_up = (cross.y * 0.5).max(0.02);
    let centre = object.position;
    let start = centre - run * (length * 0.5);

    // Where the chords sit in the cross-section.
    let chords: Vec<Vec3> = match object.chords.clamp(1, 4) {
        1 => vec![Vec3::ZERO],
        2 => vec![up * half_up, -up * half_up],
        3 => vec![
            up * half_up,
            -up * half_up + across * half_across,
            -up * half_up - across * half_across,
        ],
        _ => vec![
            up * half_up + across * half_across,
            up * half_up - across * half_across,
            -up * half_up + across * half_across,
            -up * half_up - across * half_across,
        ],
    };
    let chord_radius = (half_across.min(half_up) * 0.28).clamp(0.012, 0.06);
    let brace_radius = chord_radius * 0.6;

    for offset in &chords {
        push_tube(
            frame,
            start + *offset,
            start + run * length + *offset,
            chord_radius,
            colour,
            object.roughness,
        );
    }
    if chords.len() < 2 {
        return;
    }
    // Bracing: a zig-zag between neighbouring chords, in bays about a third of a metre long.
    let bays = ((length / 0.34).round() as usize).clamp(1, 160);
    let bay = length / bays as f32;
    for index in 0..chords.len() {
        let first = chords[index];
        let second = chords[(index + 1) % chords.len()];
        if chords.len() == 2 && index == 1 {
            break;
        }
        for bay_index in 0..bays {
            let near = start + run * (bay * bay_index as f32);
            let far = start + run * (bay * (bay_index + 1) as f32);
            let (from, to) = if bay_index % 2 == 0 {
                (near + first, far + second)
            } else {
                (near + second, far + first)
            };
            push_tube(frame, from, to, brace_radius, colour, object.roughness);
            // An upright at each node keeps the bays square, the way a truss is welded.
            push_tube(
                frame,
                near + first,
                near + second,
                brace_radius,
                colour,
                object.roughness,
            );
        }
    }
}

/// The two cross-section axes for a truss running along `run_axis`.
fn truss_cross_axes(run_axis: Vec3, orientation: Quat) -> (Vec3, Vec3) {
    let across = if run_axis == Vec3::Y {
        Vec3::X
    } else {
        Vec3::Y
    };
    let other = run_axis.cross(across).normalize_or(Vec3::Z);
    (orientation * other, orientation * across)
}

/// A drape, drawn as folds rather than a slab so it reads as fabric.
fn push_curtain(
    frame: &mut FrameInstances,
    object: &SceneryObject,
    orientation: Quat,
    colour: Vec3,
) {
    let size = object.size.max(Vec3::splat(0.02));
    let width = size.x;
    let folds = ((width / 0.45).round() as usize).clamp(2, 80);
    let fold_width = width / folds as f32;
    let across = orientation * Vec3::X;
    let depth = size.z.max(0.08);
    for index in 0..folds {
        let offset = -width * 0.5 + fold_width * (index as f32 + 0.5);
        // Alternating depth is what makes a drape read as gathered rather than painted on.
        let bulge = if index % 2 == 0 { depth } else { depth * 0.45 };
        let model = Mat4::from_scale_rotation_translation(
            Vec3::new(fold_width * 0.92, size.y, bulge),
            orientation,
            object.position + across * offset,
        );
        frame.mesh(MeshKind::Cylinder).push(MeshInstance::new(
            model,
            colour,
            0.95,
            Vec3::ZERO,
            0.0,
        ));
    }
}

/// A handrail: posts at intervals with a top rail and a knee rail.
fn push_railing(
    frame: &mut FrameInstances,
    object: &SceneryObject,
    orientation: Quat,
    colour: Vec3,
) {
    let size = object.size.max(Vec3::splat(0.02));
    let length = size.x.max(size.z);
    let along = orientation * if size.x >= size.z { Vec3::X } else { Vec3::Z };
    let height = size.y.max(0.3);
    let base = object.position - Vec3::Y * height * 0.5;
    let posts = ((length / 1.2).round() as usize).clamp(2, 40);
    for index in 0..=posts {
        let offset = -length * 0.5 + length * index as f32 / posts as f32;
        let foot = base + along * offset;
        push_tube(frame, foot, foot + Vec3::Y * height, 0.02, colour, 0.4);
    }
    for rail in [height, height * 0.55] {
        push_tube(
            frame,
            base + along * (-length * 0.5) + Vec3::Y * rail,
            base + along * (length * 0.5) + Vec3::Y * rail,
            0.022,
            colour,
            0.4,
        );
    }
}

/// A mirror ball: a sphere of facets that throws the light back.
fn push_mirror_ball(frame: &mut FrameInstances, object: &SceneryObject, orientation: Quat) {
    let diameter = object.size.max_element().max(0.1);
    let centre = object.position;
    // The hanging point, so it reads as rigged rather than floating.
    push_tube(
        frame,
        centre + Vec3::Y * (diameter * 0.5),
        centre + Vec3::Y * (diameter * 0.5 + 0.25),
        0.008,
        Vec3::splat(0.2),
        0.4,
    );
    let model = Mat4::from_scale_rotation_translation(Vec3::splat(diameter), orientation, centre);
    // Mirrored glass: fully metallic and almost perfectly smooth, so every beam that lands on it
    // comes back as a highlight instead of a matt patch.
    frame.mesh(MeshKind::Sphere).push(MeshInstance::new(
        model,
        Vec3::splat(0.92),
        0.04,
        Vec3::ZERO,
        1.0,
    ));
}
