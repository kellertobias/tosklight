//! Import tests. Every model here is a `.glb` assembled byte by byte in the test, so the importer
//! is proven against the container format rather than against a fixture file nobody can read.

use super::*;
use serde_json::json;

/// Packs a JSON document and a binary buffer into a glTF 2.0 binary.
pub(super) fn glb(document: &Value, bin: &[u8]) -> Vec<u8> {
    let mut json = serde_json::to_vec(document).unwrap();
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let mut bin = bin.to_vec();
    while !bin.len().is_multiple_of(4) {
        bin.push(0);
    }
    let total = 12 + 8 + json.len() + if bin.is_empty() { 0 } else { 8 + bin.len() };
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2_u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(&JSON_CHUNK.to_le_bytes());
    out.extend_from_slice(&json);
    if !bin.is_empty() {
        out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        out.extend_from_slice(&BIN_CHUNK.to_le_bytes());
        out.extend_from_slice(&bin);
    }
    out
}

fn bytes_of_f32(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// A 2×2 quad in the XY plane facing +Z, offset to (10, 20, 0), with UVs and u16 indices.
/// `with_uvs` false leaves TEXCOORD_0 out; `node` wraps the mesh in a scene graph.
fn quad(with_uvs: bool, node: Option<Value>) -> Vec<u8> {
    let positions = bytes_of_f32(&[
        9.0, 19.0, 0.0, 11.0, 19.0, 0.0, 11.0, 21.0, 0.0, 9.0, 21.0, 0.0,
    ]);
    let uvs = bytes_of_f32(&[0.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0]);
    let indices: Vec<u8> = [0_u16, 1, 2, 0, 2, 3]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    let mut bin = positions.clone();
    bin.extend_from_slice(&uvs);
    bin.extend_from_slice(&indices);
    let mut attributes = json!({ "POSITION": 0 });
    if with_uvs {
        attributes["TEXCOORD_0"] = json!(1);
    }
    let mut document = json!({
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
        "meshes": [{ "name": "Screen", "primitives": [{ "attributes": attributes, "indices": 2 }] }]
    });
    if let Some(node) = node {
        document["nodes"] = json!([node]);
        document["scenes"] = json!([{ "nodes": [0] }]);
        document["scene"] = json!(0);
    }
    glb(&document, &bin)
}

fn radius(geometry: &ModelGeometry) -> f32 {
    geometry
        .vertices
        .iter()
        .map(|v| v.position.iter().map(|p| p * p).sum::<f32>().sqrt())
        .fold(0.0, f32::max)
}

#[test]
fn a_textured_quad_imports_centred_in_the_unit_sphere_with_computed_normals() {
    let geometry = import_glb(&quad(true, None)).unwrap();
    assert_eq!(geometry.vertices.len(), 4);
    assert_eq!(geometry.triangle_count(), 2);
    assert!((radius(&geometry) - 1.0).abs() < 1e-5);
    let half = std::f32::consts::FRAC_1_SQRT_2;
    let first = geometry.vertices[0];
    assert!((first.position[0] + half).abs() < 1e-5, "{first:?}");
    assert!((first.position[1] + half).abs() < 1e-5, "{first:?}");
    assert_eq!(first.uv, [0.0, 1.0]);
    for vertex in &geometry.vertices {
        assert!(
            (vertex.normal[2] - 1.0).abs() < 1e-5,
            "a counter-clockwise quad faces +Z: {vertex:?}"
        );
    }
}

#[test]
fn node_transforms_are_baked_in_before_normalization() {
    // A quarter turn about Y: the quad now faces +X, and still fits the unit sphere.
    let half = std::f32::consts::FRAC_1_SQRT_2;
    let geometry = import_glb(&quad(
        true,
        Some(json!({ "mesh": 0, "rotation": [0.0, half, 0.0, half], "scale": [3.0, 3.0, 3.0] })),
    ))
    .unwrap();
    assert!((radius(&geometry) - 1.0).abs() < 1e-5);
    for vertex in &geometry.vertices {
        assert!(vertex.position[2].abs() > 0.5 || vertex.position[0].abs() < 1e-4);
        assert!((vertex.normal[0] - 1.0).abs() < 1e-4, "{vertex:?}");
    }
}

#[test]
fn a_model_without_texture_coordinates_is_refused_with_an_actionable_error() {
    let error = import_glb(&quad(false, None)).unwrap_err();
    assert_eq!(
        error,
        ModelImportError::NoTextureCoordinates("Screen".into())
    );
    assert_eq!(error.code(), "model-missing-texture-coordinates");
    assert!(error.to_string().contains("UV-unwrap"), "{error}");
}

#[test]
fn files_that_are_not_glb_2_are_refused() {
    assert_eq!(import_glb(b"{\"asset\":{}}"), Err(ModelImportError::NotGlb));
    let mut version_one = quad(true, None);
    version_one[4] = 1;
    assert_eq!(
        import_glb(&version_one),
        Err(ModelImportError::UnsupportedVersion(1))
    );
    let whole = quad(true, None);
    assert!(matches!(
        import_glb(&whole[..whole.len() - 10]),
        Err(ModelImportError::Damaged(_))
    ));
}

#[test]
fn external_buffers_are_refused() {
    let document = json!({
        "asset": { "version": "2.0" },
        "buffers": [{ "byteLength": 48, "uri": "quad.bin" }],
        "bufferViews": [{ "buffer": 0, "byteLength": 48 }],
        "accessors": [
            { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3" },
            { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC2" }
        ],
        "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0, "TEXCOORD_0": 1 } }] }]
    });
    let error = import_glb(&glb(&document, &[])).unwrap_err();
    assert_eq!(error.code(), "model-external-data");
}

#[test]
fn strips_and_normalized_integer_uvs_are_read() {
    let positions = bytes_of_f32(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0]);
    let mut bin = positions;
    bin.extend_from_slice(&[0, 255, 255, 255, 0, 0, 255, 0]);
    let document = json!({
        "asset": { "version": "2.0" },
        "buffers": [{ "byteLength": bin.len() }],
        "bufferViews": [
            { "buffer": 0, "byteLength": 48 },
            { "buffer": 0, "byteOffset": 48, "byteLength": 8 }
        ],
        "accessors": [
            { "bufferView": 0, "componentType": 5126, "count": 4, "type": "VEC3" },
            { "bufferView": 1, "componentType": 5121, "normalized": true, "count": 4, "type": "VEC2" }
        ],
        "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0, "TEXCOORD_0": 1 }, "mode": 5 }] }]
    });
    let geometry = import_glb(&glb(&document, &bin)).unwrap();
    assert_eq!(geometry.triangle_count(), 2);
    assert_eq!(geometry.vertices[1].uv, [1.0, 1.0]);
    assert_eq!(geometry.vertices[2].uv, [0.0, 0.0]);
}

#[test]
fn points_and_lines_are_refused() {
    let positions = bytes_of_f32(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
    let mut bin = positions;
    bin.extend_from_slice(&bytes_of_f32(&[0.0; 6]));
    let document = json!({
        "asset": { "version": "2.0" },
        "buffers": [{ "byteLength": bin.len() }],
        "bufferViews": [
            { "buffer": 0, "byteLength": 36 },
            { "buffer": 0, "byteOffset": 36, "byteLength": 24 }
        ],
        "accessors": [
            { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3" },
            { "bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC2" }
        ],
        "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0, "TEXCOORD_0": 1 }, "mode": 1 }] }]
    });
    assert_eq!(
        import_glb(&glb(&document, &bin)).unwrap_err().code(),
        "model-unsupported"
    );
}

#[test]
fn a_file_with_no_meshes_is_refused() {
    let document = json!({ "asset": { "version": "2.0" } });
    assert_eq!(
        import_glb(&glb(&document, &[])),
        Err(ModelImportError::NoMeshes)
    );
}

#[test]
fn the_store_writes_loads_and_removes_models_under_the_hidden_directory() {
    let root = std::env::temp_dir()
        .join("media-model-store")
        .join(format!("{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let store = ModelStore::new(&root);
    let file = media_domain::ModelLibrary::stored_file_name(3);
    store.store(&file, &quad(true, None)).unwrap();
    assert!(root.join(MODEL_DIRECTORY).join(&file).is_file());
    assert_eq!(store.load(&file).unwrap().triangle_count(), 2);

    store.store(&file, b"not a model").unwrap();
    let error = store.load(&file).unwrap_err();
    assert!(error.contains("glTF"), "{error}");

    store.remove(&file).unwrap();
    store.remove(&file).unwrap();
    assert!(store.load(&file).unwrap_err().contains("cannot read"));
    assert!(store.store("../escape.glb", b"x").is_err());
    let _ = std::fs::remove_dir_all(&root);
}
