//! A box, a cylinder and a ball are one unit mesh each, stretched to fill their size.
use super::*;
use crate::instances::build;

fn primitive(kind: SceneryKind) -> SceneryObject {
    SceneryObject {
        position: Vec3::new(1.0, 2.0, 3.0),
        size: Vec3::new(2.0, 3.0, 4.0),
        colour: [0.8, 0.1, 0.2],
        roughness: 0.7,
        kind,
        ..SceneryObject::default()
    }
}

#[test]
fn each_primitive_is_its_own_mesh_at_its_size_and_colour() {
    for (kind, mesh) in [
        (SceneryKind::Box, MeshKind::Cube),
        (SceneryKind::Cylinder, MeshKind::Cylinder),
        (SceneryKind::Sphere, MeshKind::Sphere),
    ] {
        let mut scene = Scene::default();
        scene.scenery.push(primitive(kind));
        let frame = build(&scene, &SceneValues::default(), &FrameStyle::default());
        let drawn: Vec<_> = frame
            .meshes
            .iter()
            .flat_map(|(each, entries)| entries.iter().map(move |entry| (*each, entry)))
            .filter(|(_, entry)| entry.base_colour[..3] == [0.8, 0.1, 0.2])
            .collect();
        assert_eq!(drawn.len(), 1, "{kind:?} is one mesh");
        let (drawn_mesh, instance) = drawn[0];
        assert_eq!(drawn_mesh, mesh, "{kind:?}");
        let (scale, _, translation) =
            Mat4::from_cols_array_2d(&instance.model).to_scale_rotation_translation();
        assert!(
            (scale - Vec3::new(2.0, 3.0, 4.0)).length() < 1e-4,
            "{kind:?} {scale}"
        );
        assert!((translation - Vec3::new(1.0, 2.0, 3.0)).length() < 1e-4);
        assert!((instance.base_colour[3] - 0.7).abs() < 1e-6);
    }
}

/// A lines view keeps the primitives, as it keeps every other prop the rig is arranged around.
#[test]
fn a_lines_view_keeps_the_primitives() {
    for kind in [SceneryKind::Box, SceneryKind::Cylinder, SceneryKind::Sphere] {
        assert!(viz_scene::ViewMode::Lines3d.draws_scenery(kind), "{kind:?}");
    }
}
