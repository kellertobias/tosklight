//! The 3D model store: importing glTF 2.0 binary (`.glb`) models and keeping them on disk.
//!
//! An operator uploads one self-contained `.glb` per model slot. Import reads every triangle mesh
//! reachable from the default scene, bakes node transforms into the vertices, and requires
//! positions and the first set of texture coordinates — the layer image is wrapped onto the model
//! through those coordinates, so a model without them has nowhere to put the picture. Normals are
//! read when present and computed from the faces when not. The finished mesh is normalized into
//! the unit sphere around the origin so every model frames the same way on the fixed camera.
//!
//! The original file is kept as uploaded, so a later build that reads more of glTF can import it
//! again; the store never keeps a derived format that would need its own migration.

use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use media_domain::model_projection::{IDENTITY, Matrix4, multiply};
use media_domain::{ModelGeometry, ModelGeometryError, ModelVertex};
use serde_json::Value;

/// The largest `.glb` the store accepts.
pub const MAX_MODEL_BYTES: usize = 256 * 1024 * 1024;

/// The hidden directory inside the library root that holds stored models. Dotted, so catalog
/// discovery treats it as the library's own bookkeeping rather than media.
pub const MODEL_DIRECTORY: &str = ".models";

const JSON_CHUNK: u32 = 0x4E4F_534A;
const BIN_CHUNK: u32 = 0x004E_4942;
const MAX_NODE_DEPTH: usize = 64;

/// Why a file cannot become a model. Every message tells the operator what to do about it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ModelImportError {
    #[error("this is not a glTF binary file; export the model as glTF 2.0 Binary (.glb)")]
    NotGlb,
    #[error("this .glb is glTF version {0}; export it again as glTF 2.0 Binary")]
    UnsupportedVersion(u32),
    #[error("the .glb file is truncated or damaged ({0}); export it again")]
    Damaged(String),
    #[error(
        "the model refers to external data ({0}); export a self-contained glTF Binary (.glb) \
         with embedded buffers"
    )]
    ExternalData(String),
    #[error(
        "the model uses {0}, which the Media Server cannot read; triangulate the meshes and export again"
    )]
    Unsupported(String),
    #[error("the model contains no triangle meshes")]
    NoMeshes,
    #[error(
        "mesh \"{0}\" has no texture coordinates (TEXCOORD_0); UV-unwrap it in your 3D tool and \
         export again so the layer image has somewhere to go"
    )]
    NoTextureCoordinates(String),
    #[error("the model is too large: {0}")]
    TooLarge(String),
    #[error("{0}")]
    Geometry(#[from] ModelGeometryError),
}

impl ModelImportError {
    /// A stable, machine-readable code for the API.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::NotGlb => "model-not-glb",
            Self::UnsupportedVersion(_) => "model-unsupported-version",
            Self::Damaged(_) => "model-damaged",
            Self::ExternalData(_) => "model-external-data",
            Self::Unsupported(_) => "model-unsupported",
            Self::NoMeshes => "model-no-meshes",
            Self::NoTextureCoordinates(_) => "model-missing-texture-coordinates",
            Self::TooLarge(_) => "model-too-large",
            Self::Geometry(_) => "model-invalid-geometry",
        }
    }
}

fn damaged(detail: impl Into<String>) -> ModelImportError {
    ModelImportError::Damaged(detail.into())
}

/// Imports a `.glb` into a normalized, drawable mesh.
pub fn import_glb(bytes: &[u8]) -> Result<ModelGeometry, ModelImportError> {
    if bytes.len() > MAX_MODEL_BYTES {
        return Err(ModelImportError::TooLarge(format!(
            "the file is larger than {} MiB",
            MAX_MODEL_BYTES / (1024 * 1024)
        )));
    }
    let (json, bin) = chunks(bytes)?;
    let document = Document { json: &json, bin };
    let mut builder = Builder::default();
    for (mesh, matrix) in document.mesh_nodes()? {
        document.append_mesh(mesh, &matrix, &mut builder)?;
    }
    if builder.geometry.indices.is_empty() {
        return Err(ModelImportError::NoMeshes);
    }
    Ok(builder.geometry.normalized()?)
}

fn read_u32(bytes: &[u8], at: usize) -> Option<u32> {
    bytes
        .get(at..at + 4)
        .map(|b| u32::from_le_bytes(b.try_into().expect("four bytes")))
}

fn chunks(bytes: &[u8]) -> Result<(Value, Option<&[u8]>), ModelImportError> {
    if bytes.len() < 12 || &bytes[..4] != b"glTF" {
        return Err(ModelImportError::NotGlb);
    }
    let version = read_u32(bytes, 4).expect("checked length");
    if version != 2 {
        return Err(ModelImportError::UnsupportedVersion(version));
    }
    let declared = read_u32(bytes, 8).expect("checked length") as usize;
    if declared > bytes.len() {
        return Err(damaged("the file is shorter than its header declares"));
    }
    let bytes = &bytes[..declared];
    let mut cursor = 12;
    let mut json = None;
    let mut bin = None;
    while cursor + 8 <= bytes.len() {
        let length = read_u32(bytes, cursor).expect("checked") as usize;
        let kind = read_u32(bytes, cursor + 4).expect("checked");
        let start = cursor + 8;
        let end = start
            .checked_add(length)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| damaged("a chunk runs past the end of the file"))?;
        match kind {
            JSON_CHUNK if json.is_none() => json = Some(&bytes[start..end]),
            BIN_CHUNK if bin.is_none() => bin = Some(&bytes[start..end]),
            _ => {}
        }
        cursor = end;
    }
    let json = json.ok_or_else(|| damaged("there is no JSON chunk"))?;
    let json = serde_json::from_slice(json)
        .map_err(|error| damaged(format!("the JSON chunk is not valid JSON: {error}")))?;
    Ok((json, bin))
}

#[derive(Default)]
struct Builder {
    geometry: ModelGeometry,
}

struct Document<'a> {
    json: &'a Value,
    bin: Option<&'a [u8]>,
}

fn array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn index_of(value: &Value, key: &str) -> Option<usize> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .map(|index| index as usize)
}

fn floats<const N: usize>(value: &Value, key: &str) -> Option<[f32; N]> {
    let values = value.get(key)?.as_array()?;
    if values.len() != N {
        return None;
    }
    let mut out = [0.0; N];
    for (slot, value) in out.iter_mut().zip(values) {
        *slot = value.as_f64()? as f32;
    }
    Some(out)
}

fn node_matrix(node: &Value) -> Matrix4 {
    if let Some(values) = floats::<16>(node, "matrix") {
        let mut matrix = [[0.0; 4]; 4];
        for (index, value) in values.iter().enumerate() {
            matrix[index / 4][index % 4] = *value;
        }
        return matrix;
    }
    let [tx, ty, tz] = floats::<3>(node, "translation").unwrap_or([0.0; 3]);
    let [x, y, z, w] = floats::<4>(node, "rotation").unwrap_or([0.0, 0.0, 0.0, 1.0]);
    let [sx, sy, sz] = floats::<3>(node, "scale").unwrap_or([1.0; 3]);
    let rotation = [
        [
            1.0 - 2.0 * (y * y + z * z),
            2.0 * (x * y + z * w),
            2.0 * (x * z - y * w),
            0.0,
        ],
        [
            2.0 * (x * y - z * w),
            1.0 - 2.0 * (x * x + z * z),
            2.0 * (y * z + x * w),
            0.0,
        ],
        [
            2.0 * (x * z + y * w),
            2.0 * (y * z - x * w),
            1.0 - 2.0 * (x * x + y * y),
            0.0,
        ],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let scale = [
        [sx, 0.0, 0.0, 0.0],
        [0.0, sy, 0.0, 0.0],
        [0.0, 0.0, sz, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let mut translation = IDENTITY;
    translation[3] = [tx, ty, tz, 1.0];
    multiply(&translation, &multiply(&rotation, &scale))
}

/// The cofactor matrix of the upper 3×3: the inverse transpose up to a positive or negative
/// scale factor, which normalization removes. The sign is corrected so mirrored nodes keep
/// outward normals.
fn normal_matrix(matrix: &Matrix4) -> [[f32; 3]; 3] {
    let m = |column: usize, row: usize| matrix[column][row];
    let mut cofactor = [[0.0; 3]; 3];
    for (column, out) in cofactor.iter_mut().enumerate() {
        for (row, value) in out.iter_mut().enumerate() {
            let (c1, c2) = ((column + 1) % 3, (column + 2) % 3);
            let (r1, r2) = ((row + 1) % 3, (row + 2) % 3);
            *value = m(c1, r1) * m(c2, r2) - m(c2, r1) * m(c1, r2);
        }
    }
    let determinant: f32 = (0..3).map(|row| m(0, row) * cofactor[0][row]).sum();
    if determinant < 0.0 {
        for column in &mut cofactor {
            for value in column {
                *value = -*value;
            }
        }
    }
    cofactor
}

impl Document<'_> {
    /// Every mesh instance in the default scene, as `(mesh index, world matrix)`.
    fn mesh_nodes(&self) -> Result<Vec<(usize, Matrix4)>, ModelImportError> {
        let nodes = array(self.json, "nodes");
        let scenes = array(self.json, "scenes");
        let roots: Vec<usize> =
            if let Some(scene) = scenes.get(index_of(self.json, "scene").unwrap_or(0)) {
                array(scene, "nodes")
                    .iter()
                    .filter_map(|v| v.as_u64().map(|i| i as usize))
                    .collect()
            } else {
                let mut child = vec![false; nodes.len()];
                for node in nodes {
                    for index in array(node, "children").iter().filter_map(Value::as_u64) {
                        if let Some(flag) = child.get_mut(index as usize) {
                            *flag = true;
                        }
                    }
                }
                (0..nodes.len()).filter(|index| !child[*index]).collect()
            };
        let mut found = Vec::new();
        let mut stack: Vec<(usize, Matrix4, usize)> = roots
            .into_iter()
            .rev()
            .map(|root| (root, IDENTITY, 0))
            .collect();
        let mut visited = 0_usize;
        while let Some((index, parent, depth)) = stack.pop() {
            let node = nodes
                .get(index)
                .ok_or_else(|| damaged(format!("node {index} does not exist")))?;
            visited += 1;
            if depth > MAX_NODE_DEPTH || visited > nodes.len().saturating_mul(4).max(1) {
                return Err(damaged("the node hierarchy loops or is nested too deeply"));
            }
            let world = multiply(&parent, &node_matrix(node));
            if node.get("mesh").is_some() {
                let mesh = index_of(node, "mesh").ok_or_else(|| {
                    damaged(format!("node {index} has an invalid mesh reference"))
                })?;
                found.push((mesh, world));
            }
            for child in array(node, "children")
                .iter()
                .rev()
                .filter_map(Value::as_u64)
            {
                stack.push((child as usize, world, depth + 1));
            }
        }
        if found.is_empty() && nodes.is_empty() {
            // A file with meshes but no scene graph: every mesh at the origin.
            return Ok((0..array(self.json, "meshes").len())
                .map(|mesh| (mesh, IDENTITY))
                .collect());
        }
        Ok(found)
    }

    fn append_mesh(
        &self,
        mesh_index: usize,
        matrix: &Matrix4,
        builder: &mut Builder,
    ) -> Result<(), ModelImportError> {
        let mesh = array(self.json, "meshes")
            .get(mesh_index)
            .ok_or_else(|| damaged(format!("mesh {mesh_index} does not exist")))?;
        let name = mesh
            .get("name")
            .and_then(Value::as_str)
            .map_or_else(|| format!("#{mesh_index}"), str::to_owned);
        let normals = normal_matrix(matrix);
        for primitive in array(mesh, "primitives") {
            let mode = primitive.get("mode").and_then(Value::as_u64).unwrap_or(4);
            if !matches!(mode, 4..=6) {
                return Err(ModelImportError::Unsupported(format!(
                    "points or lines in mesh \"{name}\""
                )));
            }
            if primitive
                .get("extensions")
                .is_some_and(|extensions| extensions.get("KHR_draco_mesh_compression").is_some())
            {
                return Err(ModelImportError::Unsupported(format!(
                    "Draco mesh compression in mesh \"{name}\""
                )));
            }
            let attributes = primitive.get("attributes").ok_or_else(|| {
                damaged(format!(
                    "mesh \"{name}\" has a primitive without attributes"
                ))
            })?;
            let position = index_of(attributes, "POSITION").ok_or_else(|| {
                damaged(format!("mesh \"{name}\" has a primitive without positions"))
            })?;
            let texcoord = index_of(attributes, "TEXCOORD_0")
                .ok_or_else(|| ModelImportError::NoTextureCoordinates(name.clone()))?;
            let positions = self.read(position, 3)?;
            let uvs = self.read(texcoord, 2)?;
            let normal_values = index_of(attributes, "NORMAL")
                .map(|accessor| self.read(accessor, 3))
                .transpose()?;
            let count = positions.len() / 3;
            if uvs.len() / 2 != count
                || normal_values.as_ref().is_some_and(|n| n.len() / 3 != count)
            {
                return Err(damaged(format!(
                    "mesh \"{name}\" has attributes of different lengths"
                )));
            }
            let base = builder.geometry.vertices.len();
            if base + count > media_domain::model_library::MAX_MODEL_VERTICES {
                return Err(ModelImportError::TooLarge(format!(
                    "more than {} vertices",
                    media_domain::model_library::MAX_MODEL_VERTICES
                )));
            }
            let local: Vec<u32> = match index_of(primitive, "indices") {
                Some(accessor) => self.read_indices(accessor)?,
                None => (0..count as u32).collect(),
            };
            if local.iter().any(|index| *index as usize >= count) {
                return Err(damaged(format!(
                    "mesh \"{name}\" refers to a vertex it does not contain"
                )));
            }
            let triangles = triangulate(&local, mode);

            let first_vertex = builder.geometry.vertices.len();
            for vertex in 0..count {
                let p = &positions[vertex * 3..vertex * 3 + 3];
                let world = [0, 1, 2].map(|row| {
                    matrix[0][row] * p[0]
                        + matrix[1][row] * p[1]
                        + matrix[2][row] * p[2]
                        + matrix[3][row]
                });
                let normal = normal_values.as_ref().map_or([0.0; 3], |n| {
                    let n = &n[vertex * 3..vertex * 3 + 3];
                    [0, 1, 2].map(|row| {
                        normals[0][row] * n[0] + normals[1][row] * n[1] + normals[2][row] * n[2]
                    })
                });
                builder.geometry.vertices.push(ModelVertex {
                    position: world,
                    normal,
                    uv: [uvs[vertex * 2], uvs[vertex * 2 + 1]],
                });
            }
            if normal_values.is_none() {
                accumulate_face_normals(&mut builder.geometry.vertices[first_vertex..], &triangles);
            }
            builder
                .geometry
                .indices
                .extend(triangles.iter().map(|index| index + base as u32));
        }
        Ok(())
    }

    /// Reads a float accessor with `components` per element, applying integer normalization.
    fn read(&self, accessor: usize, components: usize) -> Result<Vec<f32>, ModelImportError> {
        let view = self.accessor(accessor)?;
        if view.components != components {
            return Err(damaged(format!(
                "accessor {accessor} has {} components where {components} are required",
                view.components
            )));
        }
        let mut out = Vec::with_capacity(view.count * components);
        view.for_each(|bytes, component_type, normalized| {
            out.push(component_value(bytes, component_type, normalized));
        });
        Ok(out)
    }

    fn read_indices(&self, accessor: usize) -> Result<Vec<u32>, ModelImportError> {
        let view = self.accessor(accessor)?;
        if view.components != 1 || !matches!(view.component_type, 5121 | 5123 | 5125) {
            return Err(damaged(format!(
                "accessor {accessor} is not an unsigned integer index list"
            )));
        }
        let mut out = Vec::with_capacity(view.count);
        view.for_each(|bytes, component_type, _| {
            out.push(match component_type {
                5121 => u32::from(bytes[0]),
                5123 => u32::from(u16::from_le_bytes([bytes[0], bytes[1]])),
                _ => u32::from_le_bytes(bytes.try_into().expect("four bytes")),
            });
        });
        Ok(out)
    }

    fn accessor(&self, index: usize) -> Result<Accessor<'_>, ModelImportError> {
        let accessor = array(self.json, "accessors")
            .get(index)
            .ok_or_else(|| damaged(format!("accessor {index} does not exist")))?;
        if accessor.get("sparse").is_some() {
            return Err(ModelImportError::Unsupported("sparse accessors".to_owned()));
        }
        let component_type = accessor
            .get("componentType")
            .and_then(Value::as_u64)
            .ok_or_else(|| damaged(format!("accessor {index} has no component type")))?;
        let component_size = match component_type {
            5120 | 5121 => 1,
            5122 | 5123 => 2,
            5125 | 5126 => 4,
            other => {
                return Err(damaged(format!(
                    "accessor {index} has component type {other}"
                )));
            }
        };
        let components = match accessor.get("type").and_then(Value::as_str) {
            Some("SCALAR") => 1,
            Some("VEC2") => 2,
            Some("VEC3") => 3,
            Some("VEC4") => 4,
            Some("MAT4") => 16,
            _ => {
                return Err(damaged(format!(
                    "accessor {index} has an unknown element type"
                )));
            }
        };
        let count = index_of(accessor, "count")
            .ok_or_else(|| damaged(format!("accessor {index} has no count")))?;
        if count > media_domain::model_library::MAX_MODEL_VERTICES * 6 {
            return Err(ModelImportError::TooLarge(format!(
                "accessor {index} holds {count} elements"
            )));
        }
        let element = component_size * components;
        let normalized = accessor
            .get("normalized")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let Some(view_index) = index_of(accessor, "bufferView") else {
            return Ok(Accessor {
                data: None,
                stride: element,
                count,
                components,
                component_type: component_type as u32,
                component_size,
                normalized,
            });
        };
        let view = array(self.json, "bufferViews")
            .get(view_index)
            .ok_or_else(|| damaged(format!("buffer view {view_index} does not exist")))?;
        let buffer_index = index_of(view, "buffer").unwrap_or(0);
        let buffer = array(self.json, "buffers")
            .get(buffer_index)
            .ok_or_else(|| damaged(format!("buffer {buffer_index} does not exist")))?;
        if let Some(uri) = buffer.get("uri").and_then(Value::as_str) {
            let shown: String = uri.chars().take(40).collect();
            return Err(ModelImportError::ExternalData(format!(
                "buffer \"{shown}\""
            )));
        }
        if buffer_index != 0 {
            return Err(damaged("only the embedded binary buffer can hold data"));
        }
        let bin = self
            .bin
            .ok_or_else(|| damaged("the file has no binary chunk for its buffer"))?;
        let view_offset = index_of(view, "byteOffset").unwrap_or(0);
        let view_length = index_of(view, "byteLength")
            .ok_or_else(|| damaged(format!("buffer view {view_index} has no length")))?;
        let view_bytes = view_offset
            .checked_add(view_length)
            .and_then(|end| bin.get(view_offset..end))
            .ok_or_else(|| damaged(format!("buffer view {view_index} lies outside the buffer")))?;
        let stride = index_of(view, "byteStride").unwrap_or(element).max(element);
        let offset = index_of(accessor, "byteOffset").unwrap_or(0);
        let needed = if count == 0 {
            0
        } else {
            stride
                .checked_mul(count - 1)
                .and_then(|v| v.checked_add(element))
                .ok_or_else(|| damaged(format!("accessor {index} is too long")))?
        };
        let data = offset
            .checked_add(needed)
            .and_then(|end| view_bytes.get(offset..end))
            .ok_or_else(|| damaged(format!("accessor {index} runs past its buffer view")))?;
        Ok(Accessor {
            data: Some(data),
            stride,
            count,
            components,
            component_type: component_type as u32,
            component_size,
            normalized,
        })
    }
}

struct Accessor<'a> {
    /// `None` for an accessor with no buffer view, which glTF defines as all zeros.
    data: Option<&'a [u8]>,
    stride: usize,
    count: usize,
    components: usize,
    component_type: u32,
    component_size: usize,
    normalized: bool,
}

impl Accessor<'_> {
    fn for_each(&self, mut visit: impl FnMut(&[u8], u32, bool)) {
        let zeros = [0_u8; 4];
        for element in 0..self.count {
            for component in 0..self.components {
                let bytes = match self.data {
                    Some(data) => {
                        let start = element * self.stride + component * self.component_size;
                        &data[start..start + self.component_size]
                    }
                    None => &zeros[..self.component_size],
                };
                visit(bytes, self.component_type, self.normalized);
            }
        }
    }
}

fn component_value(bytes: &[u8], component_type: u32, normalized: bool) -> f32 {
    let (value, scale) = match component_type {
        5120 => (f32::from(bytes[0] as i8), 127.0),
        5121 => (f32::from(bytes[0]), 255.0),
        5122 => (f32::from(i16::from_le_bytes([bytes[0], bytes[1]])), 32767.0),
        5123 => (f32::from(u16::from_le_bytes([bytes[0], bytes[1]])), 65535.0),
        5125 => (
            u32::from_le_bytes(bytes.try_into().expect("four bytes")) as f32,
            4_294_967_295.0,
        ),
        _ => return f32::from_le_bytes(bytes.try_into().expect("four bytes")),
    };
    if normalized {
        (value / scale).max(-1.0)
    } else {
        value
    }
}

/// Triangle strips and fans become a plain triangle list.
fn triangulate(indices: &[u32], mode: u64) -> Vec<u32> {
    match mode {
        5 => (2..indices.len())
            .flat_map(|i| {
                if i % 2 == 0 {
                    [indices[i - 2], indices[i - 1], indices[i]]
                } else {
                    [indices[i - 1], indices[i - 2], indices[i]]
                }
            })
            .collect(),
        6 => (2..indices.len())
            .flat_map(|i| [indices[0], indices[i - 1], indices[i]])
            .collect(),
        _ => indices[..indices.len() / 3 * 3].to_vec(),
    }
}

fn accumulate_face_normals(vertices: &mut [ModelVertex], triangles: &[u32]) {
    for triangle in triangles.as_chunks::<3>().0 {
        let [a, b, c] = [0, 1, 2].map(|i| vertices[triangle[i] as usize].position);
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let face = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        for &index in triangle {
            let normal = &mut vertices[index as usize].normal;
            for axis in 0..3 {
                normal[axis] += face[axis];
            }
        }
    }
}

/// A 2×2 textured quad facing +Z as a complete `.glb`: the smallest model the importer accepts.
/// For tests in crates that exercise the store without a model file of their own.
#[doc(hidden)]
pub fn sample_textured_quad() -> Vec<u8> {
    let floats = |values: &[f32]| {
        values
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<_>>()
    };
    let mut bin = floats(&[
        -1.0, -1.0, 0.0, 1.0, -1.0, 0.0, 1.0, 1.0, 0.0, -1.0, 1.0, 0.0,
    ]);
    bin.extend(floats(&[0.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0]));
    bin.extend([0_u16, 1, 2, 0, 2, 3].iter().flat_map(|v| v.to_le_bytes()));
    bin.extend([0, 0]);
    let mut json = serde_json::json!({
        "asset": { "version": "2.0" },
        "buffers": [{ "byteLength": bin.len() }],
        "bufferViews": [
            { "buffer": 0, "byteOffset": 0, "byteLength": 48 },
            { "buffer": 0, "byteOffset": 48, "byteLength": 32 },
            { "buffer": 0, "byteOffset": 80, "byteLength": 12 }
        ],
        "accessors": [
            { "bufferView": 0, "componentType": 5126, "count": 4, "type": "VEC3" },
            { "bufferView": 1, "componentType": 5126, "count": 4, "type": "VEC2" },
            { "bufferView": 2, "componentType": 5123, "count": 6, "type": "SCALAR" }
        ],
        "meshes": [{ "name": "Quad", "primitives": [{ "attributes": { "POSITION": 0, "TEXCOORD_0": 1 }, "indices": 2 }] }]
    })
    .to_string()
    .into_bytes();
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let total = 12 + 8 + json.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2_u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(&JSON_CHUNK.to_le_bytes());
    out.extend_from_slice(&json);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(&BIN_CHUNK.to_le_bytes());
    out.extend_from_slice(&bin);
    out
}

/// Stored model files inside one library root.
#[derive(Debug, Clone)]
pub struct ModelStore {
    directory: PathBuf,
}

impl ModelStore {
    pub fn new(library_root: &Path) -> Self {
        Self {
            directory: library_root.join(MODEL_DIRECTORY),
        }
    }

    fn path(&self, file: &str) -> io::Result<PathBuf> {
        let plain = !file.is_empty()
            && !file.starts_with('.')
            && file
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
        if plain {
            Ok(self.directory.join(file))
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("\"{file}\" is not a stored model file name"),
            ))
        }
    }

    /// Writes a model file atomically: a crash mid-write leaves the previous model in place.
    pub fn store(&self, file: &str, bytes: &[u8]) -> io::Result<()> {
        let destination = self.path(file)?;
        std::fs::create_dir_all(&self.directory)?;
        let temporary = self.directory.join(format!(".{file}.partial"));
        {
            let mut output = std::fs::File::create(&temporary)?;
            output.write_all(bytes)?;
            output.sync_all()?;
        }
        std::fs::rename(&temporary, &destination).inspect_err(|_| {
            let _ = std::fs::remove_file(&temporary);
        })
    }

    /// Reads and imports a stored model.
    pub fn load(&self, file: &str) -> Result<ModelGeometry, String> {
        let path = self.path(file).map_err(|error| error.to_string())?;
        let bytes = std::fs::read(&path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        import_glb(&bytes).map_err(|error| format!("{}: {error}", path.display()))
    }

    /// Removes a stored model. A file that is already gone is not an error.
    pub fn remove(&self, file: &str) -> io::Result<()> {
        match std::fs::remove_file(self.path(file)?) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
#[path = "models_tests.rs"]
mod tests;
