//! A small GLB 2.0 writer for meshes read from another format.
//!
//! Positions arrive in metres with Y up. Normals are worked out here, smooth across gentle bends
//! and split at sharp edges, because neither 3MF nor a plain OBJ reliably carries any and a CAD
//! box shaded with one normal per corner looks like a pillow.

use std::collections::BTreeMap;

/// One named, single-coloured triangle mesh.
#[derive(Debug, Clone, Default)]
pub struct Mesh {
    pub name: String,
    pub positions: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    /// Linear RGBA base colour; `None` keeps the renderer's neutral surface.
    pub colour: Option<[f32; 4]>,
}

/// Faces meeting at more than this angle keep separate normals: a hard edge.
const CREASE_COS: f32 = 0.766; // cos 40°

// Little-endian "JSON" and "BIN\0" chunk types.
const CHUNK_JSON: u32 = 0x4e4f_534a;
const CHUNK_BIN: u32 = 0x004e_4942;

/// Write the meshes as one GLB, refusing empty geometry, non-finite points and too many triangles.
pub fn build_glb(meshes: &[Mesh]) -> Result<Vec<u8>, String> {
    let meshes: Vec<&Mesh> = meshes
        .iter()
        .filter(|mesh| mesh.indices.len() >= 3)
        .collect();
    if meshes.is_empty() {
        return Err("the model contains no triangles".into());
    }
    let triangles: usize = meshes.iter().map(|mesh| mesh.indices.len() / 3).sum();
    if triangles > viz_scene::VENUE_MODEL_MAX_TRIANGLES {
        return Err(format!(
            "the model has {triangles} triangles, more than the {} a venue model may use",
            viz_scene::VENUE_MODEL_MAX_TRIANGLES
        ));
    }
    let mut document = GltfWriter::default();
    for mesh in meshes {
        if mesh
            .positions
            .iter()
            .flatten()
            .any(|value| !value.is_finite())
        {
            return Err(format!(
                "{} has a coordinate that is not a finite number",
                mesh_label(mesh)
            ));
        }
        if mesh
            .indices
            .iter()
            .any(|index| *index as usize >= mesh.positions.len())
        {
            return Err(format!(
                "{} has a face that refers to a missing vertex",
                mesh_label(mesh)
            ));
        }
        document.push_mesh(mesh);
    }
    Ok(document.finish())
}

fn mesh_label(mesh: &Mesh) -> String {
    if mesh.name.is_empty() {
        "a mesh".into()
    } else {
        format!("mesh \"{}\"", mesh.name)
    }
}

#[derive(Default)]
struct GltfWriter {
    binary: Vec<u8>,
    nodes: Vec<serde_json::Value>,
    meshes: Vec<serde_json::Value>,
    accessors: Vec<serde_json::Value>,
    views: Vec<serde_json::Value>,
    materials: Vec<serde_json::Value>,
    material_by_colour: BTreeMap<[u32; 4], usize>,
}

impl GltfWriter {
    fn push_mesh(&mut self, mesh: &Mesh) {
        let (positions, normals, indices) = split_normals(mesh);
        let (min, max) = bounds(&positions);
        let position = self.push_accessor(&positions, 34962, "VEC3", 5126, Some((min, max)));
        let normal = self.push_accessor(&normals, 34962, "VEC3", 5126, None);
        let index_bytes: Vec<u8> = indices
            .iter()
            .flat_map(|index| index.to_le_bytes())
            .collect();
        let view = self.push_view(&index_bytes, 34963);
        self.accessors.push(serde_json::json!({
            "bufferView": view, "componentType": 5125, "count": indices.len(), "type": "SCALAR"
        }));
        let mut primitive = serde_json::json!({
            "attributes": { "POSITION": position, "NORMAL": normal },
            "indices": self.accessors.len() - 1,
        });
        if let Some(colour) = mesh.colour {
            primitive["material"] = self.material(colour).into();
        }
        self.meshes
            .push(serde_json::json!({ "name": mesh.name, "primitives": [primitive] }));
        self.nodes
            .push(serde_json::json!({ "name": mesh.name, "mesh": self.meshes.len() - 1 }));
    }

    fn push_accessor(
        &mut self,
        values: &[[f32; 3]],
        target: u32,
        kind: &str,
        component: u32,
        bounds: Option<([f32; 3], [f32; 3])>,
    ) -> usize {
        let bytes: Vec<u8> = values
            .iter()
            .flatten()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        let view = self.push_view(&bytes, target);
        let mut accessor = serde_json::json!({
            "bufferView": view, "componentType": component, "count": values.len(), "type": kind
        });
        if let Some((min, max)) = bounds {
            accessor["min"] = serde_json::json!(min);
            accessor["max"] = serde_json::json!(max);
        }
        self.accessors.push(accessor);
        self.accessors.len() - 1
    }

    fn push_view(&mut self, bytes: &[u8], target: u32) -> usize {
        let offset = self.binary.len();
        self.binary.extend_from_slice(bytes);
        pad(&mut self.binary, 0);
        self.views.push(serde_json::json!({
            "buffer": 0, "byteOffset": offset, "byteLength": bytes.len(), "target": target
        }));
        self.views.len() - 1
    }

    fn material(&mut self, colour: [f32; 4]) -> usize {
        let key = colour.map(f32::to_bits);
        let next = self.materials.len();
        let index = *self.material_by_colour.entry(key).or_insert(next);
        if index == next {
            let mut material = serde_json::json!({
                "pbrMetallicRoughness": { "baseColorFactor": colour }
            });
            if colour[3] < 1.0 {
                material["alphaMode"] = "BLEND".into();
            }
            self.materials.push(material);
        }
        index
    }

    fn finish(self) -> Vec<u8> {
        let mut json = serde_json::json!({
            "asset": { "version": "2.0", "generator": "ToskLight Architect model import" },
            "scene": 0,
            "scenes": [{ "nodes": (0..self.nodes.len()).collect::<Vec<_>>() }],
            "nodes": self.nodes,
            "meshes": self.meshes,
            "accessors": self.accessors,
            "bufferViews": self.views,
            "buffers": [{ "byteLength": self.binary.len() }],
        });
        if !self.materials.is_empty() {
            json["materials"] = self.materials.into();
        }
        write_container(&json, &self.binary)
    }
}

/// A GLB container around a glTF JSON document and one binary chunk, which may be empty.
pub fn write_container(json: &serde_json::Value, binary: &[u8]) -> Vec<u8> {
    let mut json = json.to_string().into_bytes();
    pad(&mut json, b' ');
    let mut binary = binary.to_vec();
    pad(&mut binary, 0);
    let bin_chunk = if binary.is_empty() {
        0
    } else {
        8 + binary.len()
    };
    let total = 12 + 8 + json.len() + bin_chunk;
    let mut glb = Vec::with_capacity(total);
    glb.extend_from_slice(b"glTF");
    glb.extend_from_slice(&2_u32.to_le_bytes());
    glb.extend_from_slice(&(total as u32).to_le_bytes());
    glb.extend_from_slice(&(json.len() as u32).to_le_bytes());
    glb.extend_from_slice(&CHUNK_JSON.to_le_bytes());
    glb.extend_from_slice(&json);
    if !binary.is_empty() {
        glb.extend_from_slice(&(binary.len() as u32).to_le_bytes());
        glb.extend_from_slice(&CHUNK_BIN.to_le_bytes());
        glb.extend_from_slice(&binary);
    }
    glb
}

/// Pad to a four-byte boundary, as both GLB chunks and float accessors require.
pub fn pad(bytes: &mut Vec<u8>, filler: u8) {
    while !bytes.len().is_multiple_of(4) {
        bytes.push(filler);
    }
}

fn bounds(positions: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for position in positions {
        for axis in 0..3 {
            min[axis] = min[axis].min(position[axis]);
            max[axis] = max[axis].max(position[axis]);
        }
    }
    (min, max)
}

type Vec3 = [f32; 3];

fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normalized(a: Vec3) -> Vec3 {
    let length = dot(a, a).sqrt();
    if length > 1e-12 {
        [a[0] / length, a[1] / length, a[2] / length]
    } else {
        [0.0, 1.0, 0.0]
    }
}

/// Give every vertex one normal per group of faces around it that bend less than the crease angle.
///
/// Returns positions, normals and indices; a vertex on a hard edge is duplicated once per side.
fn split_normals(mesh: &Mesh) -> (Vec<Vec3>, Vec<Vec3>, Vec<u32>) {
    let faces: Vec<[usize; 3]> = mesh
        .indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|face| face.map(|index| index as usize))
        .collect();
    // Area-weighted face normals: the cross product's length is twice the area.
    let face_normals: Vec<Vec3> = faces
        .iter()
        .map(|[a, b, c]| {
            let origin = mesh.positions[*a];
            cross(
                sub(mesh.positions[*b], origin),
                sub(mesh.positions[*c], origin),
            )
        })
        .collect();
    let mut incident: Vec<Vec<(usize, usize)>> = vec![Vec::new(); mesh.positions.len()];
    for (face, corners) in faces.iter().enumerate() {
        for (corner, vertex) in corners.iter().enumerate() {
            incident[*vertex].push((face, corner));
        }
    }
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut corner_vertex = vec![[0_u32; 3]; faces.len()];
    for (vertex, around) in incident.iter().enumerate() {
        // Each group: the unit direction of its first face, and the summed weighted normal.
        let mut groups: Vec<(Vec3, Vec3, u32)> = Vec::new();
        for (face, corner) in around {
            let weighted = face_normals[*face];
            let direction = normalized(weighted);
            let slot = match groups
                .iter_mut()
                .find(|(first, _, _)| dot(*first, direction) >= CREASE_COS)
            {
                Some(group) => {
                    for (sum, value) in group.1.iter_mut().zip(weighted) {
                        *sum += value;
                    }
                    group.2
                }
                None => {
                    let slot = positions.len() as u32;
                    positions.push(mesh.positions[vertex]);
                    normals.push([0.0; 3]);
                    groups.push((direction, weighted, slot));
                    slot
                }
            };
            corner_vertex[*face][*corner] = slot;
        }
        for (_, sum, slot) in groups {
            normals[slot as usize] = normalized(sum);
        }
    }
    (
        positions,
        normals,
        corner_vertex.into_iter().flatten().collect(),
    )
}
