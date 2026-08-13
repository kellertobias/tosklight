//! The current picture as plain geometry and plain lights.
//!
//! The render core exists to put an image on a surface, and everything it builds per frame is
//! shaped for a shader: packed instance arrays, cosines instead of angles, one light per lit cell.
//! Something that has to hand the same picture to another application — a modelling package, a
//! renderer that is not this one — needs those two things and none of the packing.
//!
//! This is that seam, and it is deliberately narrow: two functions returning plain data, no `wgpu`
//! type in sight. It reuses the per-frame builders rather than describing the scene a second time,
//! so an exported rig is the rig on screen and cannot drift from it.

use crate::instances::{self, FrameStyle, MeshKind};
use crate::mesh;
use viz_scene::{Scene, SceneValues};

/// One triangle mesh, in metres, in the space its instances are placed in.
#[derive(Clone, Debug, Default)]
pub struct GeometryMesh {
    pub name: String,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

/// One placement of a [`GeometryMesh`], with the surface it is drawn with.
#[derive(Clone, Copy, Debug)]
pub struct GeometryInstance {
    /// Index into [`SceneGeometry::meshes`].
    pub mesh: u32,
    /// Column-major model matrix, world space, metres.
    pub transform: [[f32; 4]; 4],
    /// Linear base colour.
    pub base_colour: [f32; 3],
    pub roughness: f32,
    pub metallic: f32,
    /// Linear emissive radiance. A lit lamp face carries its own light here and can exceed one.
    pub emissive: [f32; 3],
}

/// Every solid thing in the scene, placed where this value frame put it.
#[derive(Clone, Debug, Default)]
pub struct SceneGeometry {
    pub meshes: Vec<GeometryMesh>,
    pub instances: Vec<GeometryInstance>,
}

impl SceneGeometry {
    pub fn triangle_count(&self) -> usize {
        self.instances
            .iter()
            .filter_map(|instance| self.meshes.get(instance.mesh as usize))
            .map(|mesh| mesh.indices.len() / 3)
            .sum()
    }
}

/// Flatten the current frame into meshes and placements.
///
/// Beams and aim lines are left out. They are how *this* renderer draws light through haze, and a
/// package that has its own volumetrics wants the lights, not a picture of them.
pub fn scene_geometry(scene: &Scene, values: &SceneValues) -> SceneGeometry {
    let style = FrameStyle {
        draw_beams: false,
        draw_aim_lines: false,
        plot: false,
        ..FrameStyle::default()
    };
    let frame = instances::build(scene, values, &style);
    let mut geometry = SceneGeometry::default();
    for (kind, placements) in &frame.meshes {
        if placements.is_empty() {
            continue;
        }
        let Some(mesh) = mesh_for(*kind, scene) else {
            continue;
        };
        let index = geometry.meshes.len() as u32;
        geometry.meshes.push(mesh);
        for placement in placements {
            geometry.instances.push(GeometryInstance {
                mesh: index,
                transform: placement.model,
                base_colour: [
                    placement.base_colour[0],
                    placement.base_colour[1],
                    placement.base_colour[2],
                ],
                roughness: placement.base_colour[3],
                metallic: placement.emissive[3],
                emissive: [
                    placement.emissive[0],
                    placement.emissive[1],
                    placement.emissive[2],
                ],
            });
        }
    }
    geometry
}

/// The triangles one mesh kind draws: a procedural proxy, or one part of a library model.
fn mesh_for(kind: MeshKind, scene: &Scene) -> Option<GeometryMesh> {
    if let MeshKind::ModelPart(model_index, part_index) = kind {
        let part = scene
            .models
            .get(model_index as usize)?
            .parts
            .get(part_index as usize)?;
        if part.positions.is_empty() || part.indices.is_empty() {
            return None;
        }
        return Some(GeometryMesh {
            name: if part.name.is_empty() {
                format!("model {model_index} part {part_index}")
            } else {
                part.name.clone()
            },
            positions: part.positions.clone(),
            normals: part.normals.clone(),
            indices: part.indices.clone(),
        });
    }
    if let MeshKind::PlanArtwork(artwork_index) = kind {
        let artwork = scene.plan_artwork.get(artwork_index as usize)?;
        return Some(GeometryMesh {
            name: format!("plan artwork {artwork_index}"),
            positions: artwork.vertices.clone(),
            normals: artwork.normals.clone(),
            indices: artwork.indices.clone(),
        });
    }
    let (name, data) = mesh::procedural(kind)?;
    Some(GeometryMesh {
        name: name.to_owned(),
        positions: data.vertices.iter().map(|vertex| vertex.position).collect(),
        normals: data.vertices.iter().map(|vertex| vertex.normal).collect(),
        indices: data.indices,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instances::SemanticLight;

    fn demo() -> (Scene, SceneValues) {
        let nil = viz_scene::uuid::Uuid::nil();
        let mut scene = Scene::default();
        scene.fixtures.push(viz_scene::FixtureInstance {
            instance_id: nil,
            fixture_id: nil,
            name: "Spot".into(),
            number: Some(1),
            position: viz_scene::glam::Vec3::new(0.0, 6.0, 0.0),
            rotation_degrees: viz_scene::glam::Vec3::ZERO,
            bracket_degrees: 0.0,
            shaper_degrees: None,
            installed_colour: [1.0; 3],
            installed_shaper_angles_degrees: [0.0; 4],
            body: viz_scene::FixtureBody {
                size: viz_scene::glam::Vec3::new(0.3, 0.5, 0.3),
                kind: viz_scene::BodyKind::MovingHead,
            },
            patched: true,
            address: Some((1, 1)),
            model: None,
            fallback: None,
        });
        scene.emitters.push(viz_scene::EmitterInstance {
            fixture_index: 0,
            head_index: 0,
            label: "Main".into(),
            local_origin: viz_scene::glam::Vec3::ZERO,
            tilt_pivot: viz_scene::glam::Vec3::ZERO,
            local_orientation_degrees: viz_scene::glam::Vec3::ZERO,
            pan: None,
            tilt: None,
            beam_angle_degrees: 10.0,
            field_angle_degrees: 30.0,
            optics: viz_scene::EmitterOptics::default(),
            kind: viz_scene::EmitterKind::Beam,
            laser: None,
            effect: None,
            live_shaper_angle_roles: [false; 4],
            shaper_roles: [false; 4],
            live_shaper_rotation_role: false,
            cells: viz_scene::EmitterLayoutCells::single(),
        });
        scene.recompute_bounds();
        let mut values = SceneValues::default();
        values.resize(1);
        values.emitters[0].intensity = 1.0;
        (scene, values)
    }

    #[test]
    fn a_fixture_exports_the_triangles_it_is_drawn_with() {
        let (scene, values) = demo();
        let geometry = scene_geometry(&scene, &values);
        assert!(!geometry.meshes.is_empty(), "nothing to hand over");
        assert!(geometry.triangle_count() > 0);
        assert!(
            geometry
                .instances
                .iter()
                .any(|instance| instance.emissive.iter().any(|channel| *channel > 0.5)),
            "a lit lamp face carries its own light"
        );
    }

    /// The two exports are built from the same per-frame code as the picture. This is what stops
    /// one of them being quietly changed without the other.
    #[test]
    fn the_exported_lights_match_the_lights_the_frame_uploads() {
        let (scene, values) = demo();
        let frame = instances::build(&scene, &values, &FrameStyle::default());
        let exported: Vec<SemanticLight> = instances::semantic_lights(&scene, &values);
        assert_eq!(exported.len(), frame.lights.len());
        for (light, gpu) in exported.iter().zip(frame.lights.iter()) {
            assert!((light.origin.x - gpu.position_range[0]).abs() < 1e-5);
            assert!((light.origin.y - gpu.position_range[1]).abs() < 1e-5);
            assert!((light.origin.z - gpu.position_range[2]).abs() < 1e-5);
            assert!((light.outer_half_angle.cos() - gpu.direction_cos_outer[3]).abs() < 1e-5);
            assert!((light.inner_half_angle.cos() - gpu.params[0]).abs() < 1e-5);
        }
    }
}
