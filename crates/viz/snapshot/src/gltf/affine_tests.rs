use super::*;
use glam::{Mat4, Vec3, Vec4};
use viz_render::{GeometryInstance, GeometryMesh};

fn geometry(transform: Mat4) -> SceneGeometry {
    SceneGeometry {
        meshes: vec![GeometryMesh {
            name: "roof triangle".into(),
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            indices: vec![0, 1, 2],
        }],
        instances: vec![GeometryInstance {
            mesh: 0,
            transform: transform.to_cols_array_2d(),
            base_colour: [0.2; 3],
            roughness: 0.8,
            metallic: 0.0,
            emissive: [0.0; 3],
        }],
    }
}
fn chunks(bytes: &[u8]) -> (Value, &[u8]) {
    let n = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    (
        serde_json::from_slice(&bytes[20..20 + n]).unwrap(),
        &bytes[28 + n..],
    )
}
fn vectors(doc: &Value, bin: &[u8], accessor: &Value) -> Vec<Vec3> {
    let a = &doc["accessors"][accessor.as_u64().unwrap() as usize];
    let v = &doc["bufferViews"][a["bufferView"].as_u64().unwrap() as usize];
    let offset = v["byteOffset"].as_u64().unwrap() as usize;
    (0..a["count"].as_u64().unwrap() as usize)
        .map(|i| {
            Vec3::from_array(std::array::from_fn(|axis| {
                f32::from_le_bytes(
                    bin[offset + i * 12 + axis * 4..offset + i * 12 + axis * 4 + 4]
                        .try_into()
                        .unwrap(),
                )
            }))
        })
        .collect()
}
#[test]
fn sheared_roof_is_baked_before_importer_trs_decomposition() {
    let matrix = Mat4::from_cols(
        Vec4::new(-24.0, 0.0, 30.0, 0.0),
        Vec4::new(-24.0, 0.0, 0.0, 0.0),
        Vec4::new(0.0, -1.0, 0.0, 0.0),
        Vec4::new(12.0, 8.0, -8.0, 1.0),
    );
    let mut names = HashMap::new();
    names.insert(0, "frozen-roof".into());
    let glb = write_glb_with_names(&geometry(matrix), "test", &names);
    let (doc, bin) = chunks(&glb);
    assert_eq!(
        doc["nodes"][0]["matrix"],
        json!(flatten(Mat4::IDENTITY.to_cols_array_2d()))
    );
    assert_eq!(doc["nodes"][0]["name"], "frozen-roof");
    let attributes = &doc["meshes"][doc["nodes"][0]["mesh"].as_u64().unwrap() as usize]["primitives"]
        [0]["attributes"];
    let points = vectors(&doc, bin, &attributes["POSITION"]);
    assert_eq!(
        points,
        vec![
            Vec3::new(12.0, 8.0, -8.0),
            Vec3::new(-12.0, 8.0, 22.0),
            Vec3::new(-12.0, 8.0, -8.0)
        ]
    );
    for normal in vectors(&doc, bin, &attributes["NORMAL"]) {
        assert!((normal - Vec3::NEG_Y).length() < 1e-6);
    }
}
#[test]
fn ordinary_rotated_scaled_placements_keep_shared_mesh_material_and_names() {
    let first = Mat4::from_scale_rotation_translation(
        Vec3::new(2.0, 3.0, 4.0),
        glam::Quat::from_rotation_y(0.7),
        Vec3::X,
    );
    let mut scene = geometry(first);
    let mut moved = scene.instances[0];
    moved.transform = Mat4::from_translation(Vec3::Y).to_cols_array_2d();
    scene.instances.push(moved);
    let mut names = HashMap::new();
    names.insert(1, "tosklight-projector-lens-test".into());
    let glb = write_glb_with_names(&scene, "test", &names);
    let (doc, _) = chunks(&glb);
    assert_eq!(doc["meshes"].as_array().unwrap().len(), 1);
    assert_eq!(doc["materials"].as_array().unwrap().len(), 1);
    assert_eq!(
        doc["nodes"][0]["matrix"],
        json!(flatten(first.to_cols_array_2d()))
    );
    assert_eq!(doc["nodes"][1]["name"], "tosklight-projector-lens-test");
}
#[test]
fn singular_sheared_placement_has_finite_positions_and_normals() {
    let matrix = Mat4::from_cols(
        Vec4::new(1.0, 0.0, 0.0, 0.0),
        Vec4::new(2.0, 0.0, 0.0, 0.0),
        Vec4::ZERO,
        Vec4::W,
    );
    let glb = write_glb(&geometry(matrix), "test");
    let (doc, bin) = chunks(&glb);
    let attributes = &doc["meshes"][doc["nodes"][0]["mesh"].as_u64().unwrap() as usize]["primitives"]
        [0]["attributes"];
    for kind in ["POSITION", "NORMAL"] {
        for value in vectors(&doc, bin, &attributes[kind]) {
            assert!(value.is_finite());
        }
    }
    assert_eq!(
        doc["nodes"][0]["matrix"],
        json!(flatten(Mat4::IDENTITY.to_cols_array_2d()))
    );
}
