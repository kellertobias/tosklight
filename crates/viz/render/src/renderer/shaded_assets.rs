//! Content identity for shaded model/projection meshes, bounded to the current scene's assets.
use crate::{
    instances::MeshKind,
    mesh::{MeshData, Vertex},
};
use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    hash::Hasher,
};
use viz_scene::Scene;

#[derive(Default)]
pub(super) struct AssetCache {
    fingerprints: HashMap<MeshKind, u64>,
}

pub(super) struct AssetChanges {
    pub changed: Vec<MeshKind>,
    pub removed: Vec<MeshKind>,
}

impl AssetCache {
    pub fn reconcile(&mut self, scene: &Scene) -> AssetChanges {
        let mut next = HashMap::new();
        for (model_index, model) in scene.models.iter().enumerate() {
            for part_index in 0..model.parts.len() {
                let kind = MeshKind::ModelPart(model_index as u32, part_index as u32);
                if let Some(asset) = asset(scene, kind).filter(Asset::drawable) {
                    next.insert(kind, asset.fingerprint());
                }
            }
        }
        for index in 0..scene.plan_artwork.len() {
            let kind = MeshKind::PlanArtwork(index as u32);
            if let Some(asset) = asset(scene, kind).filter(Asset::drawable) {
                next.insert(kind, asset.fingerprint());
            }
        }
        let removed = self
            .fingerprints
            .keys()
            .filter(|kind| !next.contains_key(kind))
            .copied()
            .collect();
        let changed = next
            .iter()
            .filter(|(kind, fingerprint)| self.fingerprints.get(kind) != Some(fingerprint))
            .map(|(kind, _)| *kind)
            .collect();
        self.fingerprints = next;
        AssetChanges { changed, removed }
    }
}

struct Asset<'a> {
    positions: &'a [[f32; 3]],
    normals: &'a [[f32; 3]],
    indices: &'a [u32],
}

impl Asset<'_> {
    fn drawable(&self) -> bool {
        !self.positions.is_empty() && !self.normals.is_empty() && !self.indices.is_empty()
    }

    fn fingerprint(&self) -> u64 {
        let mut hash = DefaultHasher::new();
        for bytes in [
            bytemuck::cast_slice(self.positions),
            bytemuck::cast_slice(self.normals),
            bytemuck::cast_slice(self.indices),
        ] {
            hash.write_usize(bytes.len());
            hash.write(bytes);
        }
        hash.finish()
    }

    fn mesh(&self) -> MeshData {
        MeshData {
            vertices: self
                .positions
                .iter()
                .zip(self.normals)
                .map(|(position, normal)| Vertex {
                    position: *position,
                    normal: *normal,
                    uv: [0.0; 2],
                })
                .collect(),
            indices: self.indices.to_vec(),
        }
    }
}

fn asset(scene: &Scene, kind: MeshKind) -> Option<Asset<'_>> {
    match kind {
        MeshKind::ModelPart(model, part) => {
            let part = scene.models.get(model as usize)?.parts.get(part as usize)?;
            Some(Asset {
                positions: &part.positions,
                normals: &part.normals,
                indices: &part.indices,
            })
        }
        MeshKind::PlanArtwork(index) => {
            let artwork = scene.plan_artwork.get(index as usize)?;
            Some(Asset {
                positions: &artwork.vertices,
                normals: &artwork.normals,
                indices: &artwork.indices,
            })
        }
        _ => None,
    }
}

pub(super) fn mesh_data(scene: &Scene, kind: MeshKind) -> Option<MeshData> {
    asset(scene, kind)
        .filter(Asset::drawable)
        .map(|asset| asset.mesh())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scene() -> Scene {
        let mut scene = Scene::default();
        scene.models.push(viz_scene::FixtureModel {
            parts: vec![viz_scene::ModelPart {
                positions: vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                normals: vec![[0.0, 0.0, 1.0]; 3],
                indices: vec![0, 1, 2],
                ..Default::default()
            }],
            ..Default::default()
        });
        scene.plan_artwork.push(viz_scene::PlanArtwork {
            view: viz_scene::ProjectionView::Top,
            vertices: vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 1.0, 0.0]; 3],
            indices: vec![0, 1, 2],
            lines: Vec::new(),
        });
        scene
    }

    #[test]
    fn unchanged_geometry_reuses_meshes_across_unrelated_show_revisions() {
        let mut scene = scene();
        let mut cache = AssetCache::default();
        assert_eq!(cache.reconcile(&scene).changed.len(), 2);
        scene.revision += 1;
        scene.models[0].parts[0].colour = [1.0; 3];
        let changes = cache.reconcile(&scene);
        assert!(changes.changed.is_empty() && changes.removed.is_empty());
    }

    #[test]
    fn model_replacement_preserves_normals_and_invalidates_each_geometry_component() {
        let mut scene = scene();
        let mut cache = AssetCache::default();
        cache.reconcile(&scene);
        let kind = MeshKind::ModelPart(0, 0);
        scene.models[0].parts[0].positions[0][0] = 0.5;
        assert_eq!(cache.reconcile(&scene).changed, vec![kind]);
        scene.models[0].parts[0].normals[0] = [1.0, 0.0, 0.0];
        assert_eq!(cache.reconcile(&scene).changed, vec![kind]);
        assert_eq!(
            mesh_data(&scene, kind).unwrap().vertices[0].normal,
            [1.0, 0.0, 0.0]
        );
        scene.models[0].parts[0].indices.reverse();
        assert_eq!(cache.reconcile(&scene).changed, vec![kind]);
        assert_eq!(scene.revision, 0);
    }

    #[test]
    fn projection_replacement_and_removed_slots_cannot_keep_old_geometry() {
        let mut scene = scene();
        let mut cache = AssetCache::default();
        cache.reconcile(&scene);
        let kind = MeshKind::PlanArtwork(0);
        scene.plan_artwork[0].vertices[0][0] = 0.5;
        assert_eq!(cache.reconcile(&scene).changed, vec![kind]);
        scene.plan_artwork[0].normals[0] = [0.0, 0.0, 1.0];
        assert_eq!(cache.reconcile(&scene).changed, vec![kind]);
        scene.plan_artwork.clear();
        assert_eq!(cache.reconcile(&scene).removed, vec![kind]);
        scene.models.clear();
        assert_eq!(
            cache.reconcile(&scene).removed,
            vec![MeshKind::ModelPart(0, 0)]
        );
        assert!(cache.fingerprints.is_empty());
    }

    #[test]
    fn emptied_model_slot_is_removed_instead_of_drawing_the_previous_asset() {
        let mut scene = scene();
        let mut cache = AssetCache::default();
        cache.reconcile(&scene);
        scene.models[0].parts[0].positions.clear();
        assert_eq!(
            cache.reconcile(&scene).removed,
            vec![MeshKind::ModelPart(0, 0)]
        );
        assert!(mesh_data(&scene, MeshKind::ModelPart(0, 0)).is_none());
    }
}
