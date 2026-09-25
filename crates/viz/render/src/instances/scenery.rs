//! The room the rig hangs in: trusses, drapes, staging, railings and the mirror ball.
//!
//! Scenery is drawn from the same procedural meshes a fixture proxy uses, so an operator gets a
//! recognisable venue without the show carrying geometry for it. It is structural rather than
//! live: none of it moves with a DMX frame, which is why it is rebuilt only when the scene
//! revision changes.

use super::{FrameInstances, FrameStyle, MeshInstance, MeshKind, Surface};
use glam::{Mat4, Quat, Vec3};
use std::collections::HashMap;
use viz_scene::{
    RiserFeet, Scene, SceneValues, SceneryKind, SceneryObject, euler_degrees, uuid::Uuid,
};

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
    let selected = selected_instances(scene, values, style);
    for object in &scene.scenery {
        push_object(
            frame,
            object,
            style,
            &chords,
            selected.get(&object.id).copied(),
        );
    }
    for (body, state) in scene.physics_scenery.iter().zip(&values.physics_frames) {
        let mut object = body.scenery.clone();
        object.position += Vec3::from_array(state.position_offset);
        let chosen = selected.get(&body.fixture_instance_id).copied();
        push_object(frame, &object, style, &chords, chosen);
    }
}

/// Physical instances of the fixtures the shared Architect/Patch selection names, with the ink
/// each is marked in: a member of a whole selected Venue group takes the group ink.
///
/// A generated Venue object is a fixture the scenery pass builds instead of a body, so the
/// selection — which names logical fixtures — reaches its drawn object through the instance id.
fn selected_instances(
    scene: &Scene,
    values: &SceneValues,
    style: &FrameStyle,
) -> HashMap<Uuid, Vec3> {
    if values.selected_fixtures.is_empty() {
        return HashMap::new();
    }
    let grouped = super::grouped_selection(scene, &values.selected_fixtures);
    scene
        .fixtures
        .iter()
        .filter(|fixture| values.selected_fixtures.contains(&fixture.fixture_id))
        .map(|fixture| {
            (
                fixture.instance_id,
                super::selection_ink(style, &grouped, fixture.fixture_id),
            )
        })
        .collect()
}

fn push_object(
    frame: &mut FrameInstances,
    object: &SceneryObject,
    style: &FrameStyle,
    chords: &[truss::ChordLine],
    // The ink the object is selected in, or none when it is not selected.
    selected: Option<Vec3>,
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
    let bounds = Mat4::from_scale_rotation_translation(object.size, orientation, object.position);
    if !style.scenery_surfaces {
        if let Some(ink) = selected {
            super::push_box_outline(frame, bounds, ink, 1.0);
        } else if object.kind != SceneryKind::Floor {
            super::push_box_outline(frame, bounds, style.faint_ink, 0.8);
        }
        return;
    }
    // A selected object keeps its own material; the mark is an additive cage just outside it,
    // exactly as a selected lamp gets, so the Visualizer shows what the Architect has selected.
    if let Some(ink) = selected {
        push_selection_cage(frame, object, orientation, ink);
    }
    match object.kind {
        SceneryKind::Truss => push_truss(frame, object, orientation, colour),
        SceneryKind::Curtain => push_curtain(frame, object, orientation, colour),
        SceneryKind::Railing => push_railing(frame, object, orientation, colour),
        SceneryKind::MirrorBall => push_mirror_ball(frame, object, orientation),
        SceneryKind::Chain => chain::push_chain(frame, object, orientation, colour, chords),
        SceneryKind::Riser if object.detail.feet == RiserFeet::Scissor => {
            riser::push_scissor_stage(frame, object, orientation, colour)
        }
        SceneryKind::Riser if object.detail.feet == RiserFeet::Fixed => {
            riser::push_fixed_legs(frame, object, orientation, colour)
        }
        SceneryKind::Stairs => riser::push_stairs(frame, object, orientation, colour),
        SceneryKind::Cylinder => push_primitive(frame, object, orientation, MeshKind::Cylinder),
        SceneryKind::Sphere => push_primitive(frame, object, orientation, MeshKind::Sphere),
        SceneryKind::FlightRack => equipment::push_flight_rack(frame, object, orientation, colour),
        SceneryKind::PaTop => equipment::push_pa_top(frame, object, orientation, colour),
        SceneryKind::LineArray => equipment::push_line_array(frame, object, orientation, colour),
        // A deck with nothing under it is still a staging deck: film-faced multiplex.
        SceneryKind::Riser => push_primitive_of(
            frame,
            object,
            orientation,
            MeshKind::Cube,
            Surface::Multiplex,
        ),
        SceneryKind::Floor | SceneryKind::Wall | SceneryKind::Prop | SceneryKind::Box => {
            push_primitive(frame, object, orientation, MeshKind::Cube)
        }
    }
}

/// The selection outline of a Venue object: its own box, opened by a small gap.
fn push_selection_cage(
    frame: &mut FrameInstances,
    object: &SceneryObject,
    orientation: Quat,
    selected_ink: Vec3,
) {
    const RELATIVE_GAP: f32 = 1.02;
    const MINIMUM_GAP_METRES: f32 = 0.04;
    let size = object.size * RELATIVE_GAP + Vec3::splat(MINIMUM_GAP_METRES);
    let cage = Mat4::from_scale_rotation_translation(size, orientation, object.position);
    super::push_box_outline(frame, cage, selected_ink, 1.0);
}

/// One unit mesh stretched to fill the object's size: a block, an upright cylinder or a ball.
///
/// The unit cube, cylinder and sphere are all one metre across and centred, and the cylinder
/// stands on `Y`, so scaling by the size makes each fill exactly the box its size describes.
fn push_primitive(
    frame: &mut FrameInstances,
    object: &SceneryObject,
    orientation: Quat,
    mesh: MeshKind,
) {
    push_primitive_of(frame, object, orientation, mesh, Surface::Plain);
}

/// [`push_primitive`], finished as `surface`.
fn push_primitive_of(
    frame: &mut FrameInstances,
    object: &SceneryObject,
    orientation: Quat,
    mesh: MeshKind,
    surface: Surface,
) {
    let model = Mat4::from_scale_rotation_translation(object.size, orientation, object.position);
    frame.mesh(mesh).push(
        MeshInstance::new(
            model,
            Vec3::from(object.colour),
            object.roughness,
            Vec3::ZERO,
            0.0,
        )
        .with_surface(surface),
    );
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
    push_tube_of(
        frame,
        from,
        to,
        radius,
        colour,
        roughness,
        0.55,
        Surface::Plain,
    );
}

/// One tube between two points, made of `surface` with the given metallic share.
#[allow(clippy::too_many_arguments)]
fn push_tube_of(
    frame: &mut FrameInstances,
    from: Vec3,
    to: Vec3,
    radius: f32,
    colour: Vec3,
    roughness: f32,
    metallic: f32,
    surface: Surface,
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
    frame.mesh(MeshKind::Cylinder).push(
        MeshInstance::new(model, colour, roughness, Vec3::ZERO, metallic).with_surface(surface),
    );
}

// A box drawn where a truss hangs tells an operator nothing. What they read a rig by is the
// number of chords, the bracing and where one piece couples to the next, so that is what is drawn.
mod truss;
use truss::push_truss;

// A chain reads by its links and what hangs at its ends; a stage element by the lift under it.
mod chain;
#[cfg(test)]
mod curtain_tests;
mod equipment;
#[cfg(test)]
mod primitive_tests;
mod riser;
#[cfg(test)]
mod selection_tests;
#[cfg(test)]
pub(super) use chain::link_count as chain_link_count;

/// How wide a curtain fold is built, as a share of the pitch between fold centres.
///
/// Above 1.0 on purpose: the folds have to overlap rather than abut. A fold is a cylinder, so its
/// cross-section is an ellipse — two of them spaced exactly one pitch apart meet at a single
/// tangent point at best, and the drape is see-through between every pair. Neighbouring folds sit
/// at alternating depths but share a centre in Z, so once their widths overlap the pair is solid
/// from a grazing angle as well as head-on.
const FOLD_OVERLAP: f32 = 1.15;

/// A drape, drawn as folds rather than a slab so it reads as fabric.
fn push_curtain(
    frame: &mut FrameInstances,
    object: &SceneryObject,
    orientation: Quat,
    colour: Vec3,
) {
    let size = object.size.max(Vec3::splat(0.02));
    let width = size.x;
    // Serge hung at fullness gathers into folds about 30 cm apart.
    let folds = ((width / 0.32).round() as usize).clamp(2, 120);
    let fold_width = width / folds as f32;
    let across = orientation * Vec3::X;
    let depth = size.z.max(0.08);
    for index in 0..folds {
        let offset = -width * 0.5 + fold_width * (index as f32 + 0.5);
        // Alternating depth is what makes a drape read as gathered rather than painted on.
        let bulge = if index % 2 == 0 { depth } else { depth * 0.55 };
        let model = Mat4::from_scale_rotation_translation(
            Vec3::new(fold_width * FOLD_OVERLAP, size.y, bulge),
            orientation,
            object.position + across * offset,
        );
        frame.mesh(MeshKind::Cylinder).push(
            MeshInstance::new(model, colour, 0.95, Vec3::ZERO, 0.0).with_surface(Surface::Fabric),
        );
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
/// A mirror ball on its chain.
///
/// A generated disco ball is as wide as the ball and as tall as the ball and its chain together, so
/// the chain is whatever the height leaves above the ball and runs up to the top of the box. A ball
/// placed no taller than it is wide — the modelled ball's size, and legacy mirror-ball scenery — is
/// drawn as it always was: the ball about its position on a short drop above it.
fn push_mirror_ball(frame: &mut FrameInstances, object: &SceneryObject, orientation: Quat) {
    let diameter = object.size.x.min(object.size.z).max(0.1);
    let chain = object.size.y - diameter;
    let (diameter, centre, drop) = if chain > 0.01 {
        let top = object.position + Vec3::Y * (object.size.y * 0.5);
        (diameter, top - Vec3::Y * (chain + diameter * 0.5), chain)
    } else {
        (object.size.max_element().max(0.1), object.position, 0.25)
    };
    // The chain it hangs from, so it reads as rigged rather than floating.
    push_tube(
        frame,
        centre + Vec3::Y * (diameter * 0.5),
        centre + Vec3::Y * (diameter * 0.5 + drop),
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
