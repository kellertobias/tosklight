//! The room the rig hangs in: trusses, drapes, staging, railings and the mirror ball.
//!
//! Scenery is drawn from the same procedural meshes a fixture proxy uses, so an operator gets a
//! recognisable venue without the show carrying geometry for it. It is structural rather than
//! live: none of it moves with a DMX frame, which is why it is rebuilt only when the scene
//! revision changes.

use super::{FrameInstances, FrameStyle, MeshInstance, MeshKind};
use glam::{Mat4, Quat, Vec3};
use viz_scene::{Scene, SceneValues, SceneryKind, SceneryObject, euler_degrees};

pub(super) fn push_scenery(
    frame: &mut FrameInstances,
    scene: &Scene,
    values: &SceneValues,
    style: &FrameStyle,
) {
    // A chain end is made fast to the nearest truss chord, by a steelflex or a pipe clamp as that
    // chord's truss calls for, so a chain needs every chord in the room.
    let chords = if scene
        .scenery
        .iter()
        .any(|object| object.kind == SceneryKind::Chain)
    {
        truss::chord_lines(&scene.scenery)
    } else {
        Vec::new()
    };
    for object in &scene.scenery {
        push_object(frame, object, style, &chords);
    }
    for (body, state) in scene.physics_scenery.iter().zip(&values.physics_frames) {
        let mut object = body.scenery.clone();
        object.position += Vec3::from_array(state.position_offset);
        push_object(frame, &object, style, &chords);
    }
}

fn push_object(
    frame: &mut FrameInstances,
    object: &SceneryObject,
    style: &FrameStyle,
    chords: &[truss::ChordLine],
) {
    // Not every view draws every kind. A lines view keeps what the rig is arranged around and
    // drops the rigging and the soft goods, which would only stand between the operator and
    // the lamps hanging off them.
    if !(style.scenery)(object.kind) {
        return;
    }
    let orientation = euler_degrees(object.rotation_degrees);
    let colour = Vec3::from(object.colour);
    // An outline view draws every object it keeps as the outline of its own box, for the same
    // reason a fixture is one: nothing here is lit, so a solid is a black shape in a black
    // room. The stage floor is the exception — the ground already has the grid on it, and a
    // box around the ground is a box around everything.
    if !style.scenery_surfaces {
        if object.kind != SceneryKind::Floor {
            super::push_box_outline(
                frame,
                Mat4::from_scale_rotation_translation(object.size, orientation, object.position),
                style.faint_ink,
                0.8,
            );
        }
        return;
    }
    match object.kind {
        SceneryKind::Truss => push_truss(frame, object, orientation, colour),
        SceneryKind::Curtain => push_curtain(frame, object, orientation, colour),
        SceneryKind::Railing => push_railing(frame, object, orientation, colour),
        SceneryKind::MirrorBall => push_mirror_ball(frame, object, orientation),
        SceneryKind::Chain => chain::push_chain(frame, object, orientation, colour, chords),
        SceneryKind::Riser if object.detail.scissor_lift => {
            riser::push_scissor_stage(frame, object, orientation, colour)
        }
        SceneryKind::Floor | SceneryKind::Wall | SceneryKind::Riser | SceneryKind::Prop => {
            let model =
                Mat4::from_scale_rotation_translation(object.size, orientation, object.position);
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

// A box drawn where a truss hangs tells an operator nothing. What they read a rig by is the
// number of chords, the bracing and where one piece couples to the next, so that is what is drawn.
mod truss;
use truss::push_truss;

// A chain reads by its links and what hangs at its ends; a stage element by the lift under it.
mod chain;
mod riser;
#[cfg(test)]
pub(super) use chain::link_count as chain_link_count;

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
