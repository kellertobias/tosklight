//! Exact native placement identities for capture-only, per-emitter shadow linking.
use super::*;
use std::collections::{HashMap, HashSet};
use viz_render::SceneGeometry;

pub(super) fn projector_lens_names(
    geometry: &SceneGeometry,
    media: &ImageMedia,
) -> Result<HashMap<usize, String>, UltimateError> {
    let mut names = HashMap::new();
    let mut identities = HashSet::new();
    for projector in &media.projectors {
        if !identities.insert(projector.id) {
            return Err(UltimateError::Failed(
                "Repeated frozen projector identity".into(),
            ));
        }
        let mut matching = geometry
            .instances
            .iter()
            .enumerate()
            .filter(|(_, instance)| {
                geometry
                    .meshes
                    .get(instance.mesh as usize)
                    .is_some_and(|mesh| mesh.name == "lens")
                    && instance.transform == projector.native_lens_model
            });
        let Some((index, _)) = matching.next() else {
            return Err(UltimateError::Failed(
                "Frozen projector lens placement is missing from real geometry".into(),
            ));
        };
        if matching.next().is_some() || names.insert(index, projector.lens_node.clone()).is_some() {
            return Err(UltimateError::Failed(
                "Frozen projector lens identity is ambiguous; choose Ultra".into(),
            ));
        }
    }
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;
    use viz_scene::{MediaProjector, uuid::Uuid};
    #[test]
    fn exact_lens_names_are_one_to_one_and_other_occluders_are_preserved() {
        let mut scene = Scene::default();
        for (id, x) in [(1, -2.0), (2, 2.0)] {
            scene.media_projectors.push(MediaProjector {
                id: Uuid::from_u128(id),
                surface_id: Uuid::nil(),
                name: "not an identity".into(),
                position: Vec3::new(x, 1.0, 3.0),
                rotation_degrees: Vec3::new(0.0, 20.0, 3.0),
                cone_length_metres: 8.0,
                spill: 1.0,
            });
        }
        let style = FrameStyle {
            media_content: false,
            ..Default::default()
        };
        let frame = viz_render::build_instances(&scene, &SceneValues::default(), &style);
        let media = ImageMedia::build(&scene, &frame, &FrozenMedia::default()).unwrap();
        let geometry =
            viz_render::scene_geometry_with_style(&scene, &SceneValues::default(), &style);
        let names = projector_lens_names(&geometry, &media).unwrap();
        assert_eq!(names.len(), 2);
        let legacy = crate::gltf::write_glb(&geometry, "test");
        let named = crate::gltf::write_glb_with_names(&geometry, "test", &names);
        let parse = |bytes: &[u8]| {
            let len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
            serde_json::from_slice::<serde_json::Value>(&bytes[20..20 + len]).unwrap()
        };
        let before = parse(&legacy);
        let after = parse(&named);
        assert_eq!(before["meshes"], after["meshes"]);
        assert_eq!(before["materials"], after["materials"]);
        assert_eq!(before["accessors"], after["accessors"]);
        let old = before["nodes"].as_array().unwrap();
        let new = after["nodes"].as_array().unwrap();
        assert_eq!(old.len(), new.len());
        for (index, (a, b)) in old.iter().zip(new).enumerate() {
            assert_eq!(a["mesh"], b["mesh"]);
            assert_eq!(a["matrix"], b["matrix"]);
            assert_eq!(b["name"].as_str(), names.get(&index).map(String::as_str));
        }
        let mut ambiguous = geometry;
        ambiguous
            .instances
            .push(ambiguous.instances[*names.keys().next().unwrap()]);
        assert!(projector_lens_names(&ambiguous, &media).is_err());
    }
}
