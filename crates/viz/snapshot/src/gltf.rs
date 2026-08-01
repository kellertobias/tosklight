//! Writing the captured geometry as one self-contained glTF 2.0 binary file.
//!
//! glTF is the format every modelling package reads without a plug-in, and a `.glb` is the whole
//! rig in one file with nothing to lose on the way. The writer is deliberately small: triangles,
//! normals, one material per distinct surface, and a node per placement. No textures, no
//! animation, no cameras and no lights — the lights are a lighting rig's real content and go in
//! the snapshot document beside this file, where they can be described as optics rather than as
//! whatever a punctual-light extension happens to support.

use serde_json::{Value, json};
use std::collections::HashMap;
use viz_render::SceneGeometry;

/// glTF component and target constants, spelled once.
const FLOAT: u32 = 5126;
const UNSIGNED_INT: u32 = 5125;
const ARRAY_BUFFER: u32 = 34962;
const ELEMENT_ARRAY_BUFFER: u32 = 34963;
const TRIANGLES: u32 = 4;

/// Colour channels are quantised before materials are compared, so a rig of nominally identical
/// lamps does not produce one material per lamp because of arithmetic noise.
const COLOUR_STEPS: f32 = 2048.0;

#[derive(Default)]
struct Binary {
    bytes: Vec<u8>,
    views: Vec<Value>,
    accessors: Vec<Value>,
}

impl Binary {
    /// Append a buffer view over `data`, padded to the four-byte alignment glTF requires.
    fn view(&mut self, data: &[u8], target: u32) -> usize {
        while !self.bytes.len().is_multiple_of(4) {
            self.bytes.push(0);
        }
        let offset = self.bytes.len();
        self.bytes.extend_from_slice(data);
        self.views.push(json!({
            "buffer": 0,
            "byteOffset": offset,
            "byteLength": data.len(),
            "target": target,
        }));
        self.views.len() - 1
    }

    fn vec3(&mut self, values: &[[f32; 3]], with_bounds: bool) -> usize {
        let mut bytes = Vec::with_capacity(values.len() * 12);
        for value in values {
            for channel in value {
                bytes.extend_from_slice(&channel.to_le_bytes());
            }
        }
        let view = self.view(&bytes, ARRAY_BUFFER);
        let mut accessor = json!({
            "bufferView": view,
            "componentType": FLOAT,
            "count": values.len(),
            "type": "VEC3",
        });
        if with_bounds {
            // The specification requires bounds on `POSITION`, and every importer uses them to
            // frame what it just opened.
            let (min, max) = bounds(values);
            accessor["min"] = json!(min);
            accessor["max"] = json!(max);
        }
        self.accessors.push(accessor);
        self.accessors.len() - 1
    }

    fn indices(&mut self, values: &[u32]) -> usize {
        let mut bytes = Vec::with_capacity(values.len() * 4);
        for value in values {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        let view = self.view(&bytes, ELEMENT_ARRAY_BUFFER);
        self.accessors.push(json!({
            "bufferView": view,
            "componentType": UNSIGNED_INT,
            "count": values.len(),
            "type": "SCALAR",
        }));
        self.accessors.len() - 1
    }
}

fn bounds(values: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for value in values {
        for axis in 0..3 {
            min[axis] = min[axis].min(value[axis]);
            max[axis] = max[axis].max(value[axis]);
        }
    }
    if values.is_empty() {
        return ([0.0; 3], [0.0; 3]);
    }
    (min, max)
}

/// One distinct surface. Instances that share one are drawn with one material.
#[derive(Eq, Hash, PartialEq)]
struct SurfaceKey {
    colour: [u32; 3],
    roughness: u32,
    metallic: u32,
    emissive: [u32; 3],
}

fn quantise(value: f32) -> u32 {
    (value.clamp(0.0, 64.0) * COLOUR_STEPS) as u32
}

fn surface_key(instance: &viz_render::GeometryInstance) -> SurfaceKey {
    SurfaceKey {
        colour: instance.base_colour.map(quantise),
        roughness: quantise(instance.roughness),
        metallic: quantise(instance.metallic),
        emissive: instance.emissive.map(quantise),
    }
}

/// Build the material for one surface.
///
/// A lit lamp face is brighter than a material's emissive factor is allowed to be — the factor is
/// a colour, capped at one — so the level is carried in the emissive-strength extension and the
/// factor keeps only the hue. An importer without that extension still gets the right colour at
/// unit strength rather than a black lamp.
fn material(instance: &viz_render::GeometryInstance) -> Value {
    let mut material = json!({
        "pbrMetallicRoughness": {
            "baseColorFactor": [
                instance.base_colour[0],
                instance.base_colour[1],
                instance.base_colour[2],
                1.0,
            ],
            "metallicFactor": instance.metallic.clamp(0.0, 1.0),
            "roughnessFactor": instance.roughness.clamp(0.0, 1.0),
        },
    });
    let peak = instance
        .emissive
        .iter()
        .fold(0.0_f32, |peak, channel| peak.max(*channel));
    if peak > 1e-4 {
        let scale = peak.max(1.0);
        material["emissiveFactor"] = json!([
            (instance.emissive[0] / scale).clamp(0.0, 1.0),
            (instance.emissive[1] / scale).clamp(0.0, 1.0),
            (instance.emissive[2] / scale).clamp(0.0, 1.0),
        ]);
        if scale > 1.0 {
            material["extensions"] = json!({
                "KHR_materials_emissive_strength": { "emissiveStrength": scale },
            });
        }
    }
    material
}

/// Write `geometry` as a self-contained GLB.
pub fn write_glb(geometry: &SceneGeometry, generator: &str) -> Vec<u8> {
    let mut binary = Binary::default();
    let mut materials: Vec<Value> = Vec::new();
    let mut material_indices: HashMap<SurfaceKey, usize> = HashMap::new();
    // One glTF mesh per (geometry, surface) pair: the triangles are uploaded once and the pairs
    // reference the same accessors, so drawing one proxy in twenty colours costs twenty small
    // records rather than twenty copies of the geometry.
    let mut meshes: Vec<Value> = Vec::new();
    let mut mesh_indices: HashMap<(u32, usize), usize> = HashMap::new();
    let mut primitives: Vec<Option<(usize, usize, usize)>> = Vec::new();
    let mut nodes: Vec<Value> = Vec::new();

    for mesh in &geometry.meshes {
        if mesh.indices.is_empty() || mesh.positions.is_empty() {
            primitives.push(None);
            continue;
        }
        let position = binary.vec3(&mesh.positions, true);
        let normal = if mesh.normals.len() == mesh.positions.len() {
            binary.vec3(&mesh.normals, false)
        } else {
            // A part with no usable normals is still geometry; a package can shade it flat.
            position
        };
        let indices = binary.indices(&mesh.indices);
        primitives.push(Some((position, normal, indices)));
    }

    for instance in &geometry.instances {
        let Some(Some((position, normal, indices))) =
            primitives.get(instance.mesh as usize).copied()
        else {
            continue;
        };
        let key = surface_key(instance);
        let material_index = match material_indices.get(&key) {
            Some(index) => *index,
            None => {
                materials.push(material(instance));
                let index = materials.len() - 1;
                material_indices.insert(key, index);
                index
            }
        };
        let mesh_index = match mesh_indices.get(&(instance.mesh, material_index)) {
            Some(index) => *index,
            None => {
                let name = geometry
                    .meshes
                    .get(instance.mesh as usize)
                    .map(|mesh| mesh.name.clone())
                    .unwrap_or_default();
                meshes.push(json!({
                    "name": name,
                    "primitives": [{
                        "attributes": { "POSITION": position, "NORMAL": normal },
                        "indices": indices,
                        "material": material_index,
                        "mode": TRIANGLES,
                    }],
                }));
                let index = meshes.len() - 1;
                mesh_indices.insert((instance.mesh, material_index), index);
                index
            }
        };
        nodes.push(json!({
            "mesh": mesh_index,
            "matrix": flatten(instance.transform),
        }));
    }

    let roots: Vec<usize> = (0..nodes.len()).collect();
    let mut document = json!({
        "asset": { "version": "2.0", "generator": generator },
        "scene": 0,
        "scenes": [{ "name": "Rig", "nodes": roots }],
        "nodes": nodes,
        "meshes": meshes,
        "materials": materials,
        "accessors": binary.accessors,
        "bufferViews": binary.views,
        "buffers": [{ "byteLength": binary.bytes.len() }],
    });
    if materials
        .iter()
        .any(|material| material.get("extensions").is_some())
    {
        document["extensionsUsed"] = json!(["KHR_materials_emissive_strength"]);
    }

    container(&document.to_string(), &binary.bytes)
}

fn flatten(matrix: [[f32; 4]; 4]) -> Vec<f32> {
    matrix.iter().flat_map(|column| *column).collect()
}

/// Wrap a JSON document and its binary buffer in the GLB container.
fn container(json: &str, binary: &[u8]) -> Vec<u8> {
    // Both chunks are padded to four bytes: JSON with spaces so it stays parseable, binary with
    // zeroes. A reader is entitled to reject a file that is not.
    let mut json_chunk = json.as_bytes().to_vec();
    while !json_chunk.len().is_multiple_of(4) {
        json_chunk.push(b' ');
    }
    let mut binary_chunk = binary.to_vec();
    while !binary_chunk.len().is_multiple_of(4) {
        binary_chunk.push(0);
    }

    let length = 12
        + 8
        + json_chunk.len()
        + if binary_chunk.is_empty() { 0 } else { 8 }
        + binary_chunk.len();
    let mut glb = Vec::with_capacity(length);
    glb.extend_from_slice(&0x4654_6c67_u32.to_le_bytes()); // "glTF"
    glb.extend_from_slice(&2_u32.to_le_bytes());
    glb.extend_from_slice(&(length as u32).to_le_bytes());
    glb.extend_from_slice(&(json_chunk.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x4e4f_534a_u32.to_le_bytes()); // "JSON"
    glb.extend_from_slice(&json_chunk);
    if !binary_chunk.is_empty() {
        glb.extend_from_slice(&(binary_chunk.len() as u32).to_le_bytes());
        glb.extend_from_slice(&0x004e_4942_u32.to_le_bytes()); // "BIN\0"
        glb.extend_from_slice(&binary_chunk);
    }
    glb
}

#[cfg(test)]
mod tests {
    use super::*;
    use viz_render::{GeometryInstance, GeometryMesh};

    fn triangle(colour: [f32; 3], emissive: [f32; 3]) -> SceneGeometry {
        SceneGeometry {
            meshes: vec![GeometryMesh {
                name: "triangle".into(),
                positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 2.0, 0.0]],
                normals: vec![[0.0, 0.0, 1.0]; 3],
                indices: vec![0, 1, 2],
            }],
            instances: vec![GeometryInstance {
                mesh: 0,
                transform: glam::Mat4::IDENTITY.to_cols_array_2d(),
                base_colour: colour,
                roughness: 0.5,
                metallic: 0.0,
                emissive,
            }],
        }
    }

    /// Parse a GLB back into its two chunks the way a reader has to.
    fn split(glb: &[u8]) -> (Value, Vec<u8>) {
        assert_eq!(&glb[0..4], b"glTF");
        assert_eq!(
            u32::from_le_bytes(glb[8..12].try_into().unwrap()) as usize,
            glb.len()
        );
        let json_length = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        assert_eq!(&glb[16..20], b"JSON");
        let json: Value = serde_json::from_slice(&glb[20..20 + json_length]).expect("valid JSON");
        let rest = &glb[20 + json_length..];
        let binary_length = u32::from_le_bytes(rest[0..4].try_into().unwrap()) as usize;
        assert_eq!(&rest[4..8], b"BIN\0");
        (json, rest[8..8 + binary_length].to_vec())
    }

    #[test]
    fn the_container_is_a_readable_glb() {
        let glb = write_glb(&triangle([0.4, 0.4, 0.4], [0.0; 3]), "test");
        assert_eq!(glb.len() % 4, 0, "a chunked container stays aligned");
        let (json, binary) = split(&glb);
        assert_eq!(json["asset"]["version"], "2.0");
        assert_eq!(json["nodes"].as_array().unwrap().len(), 1);
        assert_eq!(
            json["buffers"][0]["byteLength"].as_u64().unwrap() as usize,
            binary.len()
        );
        // Three positions, three normals, three indices.
        assert_eq!(binary.len(), 3 * 12 + 3 * 12 + 3 * 4);
        assert_eq!(json["accessors"][0]["min"], json!([0.0, 0.0, 0.0]));
        assert_eq!(json["accessors"][0]["max"], json!([1.0, 2.0, 0.0]));
    }

    #[test]
    fn identical_surfaces_share_one_material_and_one_mesh() {
        let mut geometry = triangle([0.4, 0.4, 0.4], [0.0; 3]);
        let first = geometry.instances[0];
        geometry.instances.push(first);
        let mut moved = first;
        moved.transform = glam::Mat4::from_translation(glam::Vec3::X).to_cols_array_2d();
        geometry.instances.push(moved);
        let (json, _) = split(&write_glb(&geometry, "test"));
        assert_eq!(json["nodes"].as_array().unwrap().len(), 3);
        assert_eq!(json["materials"].as_array().unwrap().len(), 1);
        assert_eq!(json["meshes"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn a_lit_lamp_face_keeps_its_hue_and_carries_its_level_as_strength() {
        // A lamp face at nine times over is what the renderer hands over for a lamp at full. The
        // factor is a colour and cannot say that, so the level has to survive somewhere else.
        let glb = write_glb(&triangle([0.05, 0.0, 0.0], [9.0, 4.5, 0.0]), "test");
        let (json, _) = split(&glb);
        let material = &json["materials"][0];
        assert_eq!(material["emissiveFactor"], json!([1.0, 0.5, 0.0]));
        assert_eq!(
            material["extensions"]["KHR_materials_emissive_strength"]["emissiveStrength"],
            json!(9.0)
        );
        assert_eq!(
            json["extensionsUsed"],
            json!(["KHR_materials_emissive_strength"])
        );
    }

    #[test]
    fn a_geometry_with_nothing_in_it_still_writes_a_readable_file() {
        let glb = write_glb(&SceneGeometry::default(), "test");
        assert_eq!(&glb[0..4], b"glTF");
        let json_length = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        let json: Value = serde_json::from_slice(&glb[20..20 + json_length]).expect("valid JSON");
        assert_eq!(json["nodes"].as_array().unwrap().len(), 0);
    }
}
