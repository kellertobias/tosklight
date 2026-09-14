use super::*;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::collections::HashMap;
use std::io::Write as _;

/// Siblings a test model may name, by the relative path it spells.
fn siblings(files: &[(&str, &[u8])]) -> impl Fn(&str) -> Result<Vec<u8>, String> + use<> {
    let files: HashMap<String, Vec<u8>> = files
        .iter()
        .map(|(name, bytes)| (name.to_string(), bytes.to_vec()))
        .collect();
    move |name| {
        files
            .get(name)
            .cloned()
            .ok_or_else(|| "No such file or directory".to_owned())
    }
}

fn no_siblings(name: &str) -> Result<Vec<u8>, String> {
    Err(format!("{name} was not expected"))
}

/// Every converted model must be a GLB the show and every renderer accept.
fn read(glb: &[u8]) -> viz_scene::FixtureModel {
    light_fixture::validate_glb_model(glb).expect("a self-contained GLB");
    viz_scene::read_glb_with_limit(glb, viz_scene::VENUE_MODEL_MAX_TRIANGLES).expect("readable")
}

fn bounds(model: &viz_scene::FixtureModel) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for position in model.parts.iter().flat_map(|part| &part.positions) {
        for axis in 0..3 {
            min[axis] = min[axis].min(position[axis]);
            max[axis] = max[axis].max(position[axis]);
        }
    }
    (min, max)
}

fn assert_near(actual: [f32; 3], expected: [f32; 3]) {
    for axis in 0..3 {
        assert!(
            (actual[axis] - expected[axis]).abs() < 1e-4,
            "{actual:?} is not {expected:?}"
        );
    }
}

#[test]
fn an_obj_with_two_triangles_takes_its_colour_from_the_mtl() {
    let obj = b"mtllib stage.mtl\no Riser\nv 0 0 0\nv 4 0 0\nv 4 1 2\nv 0 1 2\nusemtl Black\nf 1 2 3\nf -4 -2 -1\n";
    let mtl = b"# riser paint\nnewmtl Black\nKd 0.1 0.2 0.3\nd 1\n";
    let glb = to_glb("Stage.obj", obj, siblings(&[("stage.mtl", mtl)])).expect("converted");
    let model = read(&glb);
    assert_eq!(model.triangle_count(), 2);
    assert_eq!(model.parts.len(), 1);
    assert_eq!(model.parts[0].name, "Riser");
    assert_near(model.parts[0].colour, [0.1, 0.2, 0.3]);
    assert_near(bounds(&model).1, [4.0, 1.0, 2.0]);
}

#[test]
fn an_obj_quad_with_texture_and_normal_indices_is_split_into_two_triangles() {
    let obj = b"v 0 0 0\nv 2 0 0\nv 2 0 3\nv 0 0 3\nvt 0 0\nvn 0 1 0\nf 1/1/1 2/1/1 3/1/1 4/1/1\n";
    let model = read(&to_glb("floor.OBJ", obj, no_siblings).expect("converted"));
    assert_eq!(model.triangle_count(), 2);
    assert_near(bounds(&model).0, [0.0, 0.0, 0.0]);
    assert_near(bounds(&model).1, [2.0, 0.0, 3.0]);
    // A flat floor keeps one shared upward normal: the quad's diagonal is not a hard edge.
    assert_eq!(model.parts[0].positions.len(), 4);
    let normal = model.parts[0].normals[0];
    assert!(normal[1].abs() > 0.99, "{normal:?}");
}

fn triangle_gltf(buffer_uri: &str) -> Vec<u8> {
    serde_json::json!({
        "asset": { "version": "2.0" },
        "scenes": [{ "nodes": [0] }],
        "nodes": [{ "name": "Wall", "mesh": 0 }],
        "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0 }, "material": 0 }] }],
        "materials": [{ "pbrMetallicRoughness": { "baseColorFactor": [0.5, 0.25, 0.125, 1.0] } }],
        "accessors": [{ "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
                        "min": [0, 0, 0], "max": [6, 3, 0] }],
        "bufferViews": [{ "buffer": 0, "byteLength": 36 }],
        "buffers": [{ "uri": buffer_uri, "byteLength": 36 }]
    })
    .to_string()
    .into_bytes()
}

fn triangle_positions() -> Vec<u8> {
    [0.0_f32, 0.0, 0.0, 6.0, 0.0, 0.0, 0.0, 3.0, 0.0]
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

#[test]
fn a_gltf_with_a_data_uri_buffer_becomes_one_glb_keeping_its_material() {
    let uri = format!(
        "data:application/octet-stream;base64,{}",
        STANDARD.encode(triangle_positions())
    );
    let model = read(&to_glb("wall.gltf", &triangle_gltf(&uri), no_siblings).expect("converted"));
    assert_eq!(model.triangle_count(), 1);
    assert_eq!(model.parts[0].name, "Wall");
    assert_near(model.parts[0].colour, [0.5, 0.25, 0.125]);
    assert_near(bounds(&model).1, [6.0, 3.0, 0.0]);
}

#[test]
fn a_gltf_with_a_sibling_bin_and_texture_packs_both_into_the_glb() {
    let mut document: serde_json::Value =
        serde_json::from_slice(&triangle_gltf("wall%20data.bin")).expect("json");
    document["images"] = serde_json::json!([{ "uri": "paint.png" }]);
    let gltf = document.to_string().into_bytes();
    let png = b"\x89PNG\r\n\x1a\nnot really";
    let positions = triangle_positions();
    let files = siblings(&[("wall data.bin", &positions), ("paint.png", png)]);
    let glb = to_glb("wall.gltf", &gltf, files).expect("converted");
    let model = read(&glb);
    assert_eq!(model.triangle_count(), 1);
    let json_length = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    let json: serde_json::Value = serde_json::from_slice(&glb[20..20 + json_length]).unwrap();
    assert_eq!(json["images"][0]["mimeType"], "image/png");
    assert!(json["images"][0].get("uri").is_none());
    assert_eq!(json["buffers"].as_array().unwrap().len(), 1);
}

#[test]
fn a_gltf_missing_its_bin_is_refused_naming_the_file() {
    let error =
        to_glb("wall.gltf", &triangle_gltf("wall.bin"), siblings(&[])).expect_err("refused");
    assert_eq!(error.file, "wall.gltf");
    assert!(
        error.message.contains("buffer wall.bin must be beside"),
        "{error}"
    );
}

#[test]
fn a_gltf_needing_draco_is_refused_with_the_extension_named() {
    let mut document: serde_json::Value =
        serde_json::from_slice(&triangle_gltf("wall.bin")).expect("json");
    document["extensionsRequired"] = serde_json::json!(["KHR_draco_mesh_compression"]);
    let error =
        to_glb("wall.gltf", document.to_string().as_bytes(), no_siblings).expect_err("refused");
    assert!(
        error.message.contains("KHR_draco_mesh_compression"),
        "{error}"
    );
}

/// A 3MF package holding `model` as its root part.
fn three_mf(model: &str) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("_rels/.rels", options).unwrap();
    zip.write_all(br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Target="/3D/3dmodel.model" Id="rel0" Type="http://schemas.microsoft.com/3dmanufacturing/2013/01/3dmodel"/></Relationships>"#).unwrap();
    zip.start_file("3D/3dmodel.model", options).unwrap();
    zip.write_all(model.as_bytes()).unwrap();
    zip.finish().unwrap().into_inner()
}

#[test]
fn a_3mf_in_millimetres_is_placed_by_its_build_transform_in_metres_and_y_up() {
    // A 4 m wide, 2 m deep and 6 m tall triangle, lifted 1 m along X by its build item.
    let model = r##"<?xml version="1.0" encoding="UTF-8"?>
<model unit="millimeter" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02">
  <resources>
    <basematerials id="1"><base name="Red" displaycolor="#FF0000"/></basematerials>
    <object id="2" name="Tower" type="model" pid="1" pindex="0">
      <mesh>
        <vertices>
          <vertex x="0" y="0" z="0"/><vertex x="4000" y="0" z="0"/><vertex x="0" y="2000" z="6000"/>
        </vertices>
        <triangles><triangle v1="0" v2="1" v3="2"/></triangles>
      </mesh>
    </object>
    <object id="3" name="Assembly"><components><component objectid="2"/></components></object>
  </resources>
  <build><item objectid="3" transform="1 0 0 0 1 0 0 0 1 1000 0 0"/></build>
</model>"##;
    let converted = read(&to_glb("tower.3mf", &three_mf(model), no_siblings).expect("converted"));
    assert_eq!(converted.triangle_count(), 1);
    assert_eq!(converted.parts[0].name, "Tower");
    assert_near(converted.parts[0].colour, [1.0, 0.0, 0.0]);
    let (min, max) = bounds(&converted);
    // 3MF Z (height) is glTF Y; 3MF Y (depth, away from the viewer) is glTF -Z.
    assert_near(min, [1.0, 0.0, -2.0]);
    assert_near(max, [5.0, 6.0, 0.0]);
    assert_near(converted.extent.to_array(), [2.0, 3.0, 1.0]);
}

#[test]
fn a_3mf_component_is_moved_before_its_build_item_turns_it() {
    // The component shifts the part 1 m along X; the item then turns everything 90° about Z,
    // which carries +X onto +Y — and 3MF +Y is glTF -Z.
    let model = r#"<model unit="meter"><resources>
    <object id="1"><mesh><vertices>
      <vertex x="0" y="0" z="0"/><vertex x="0.5" y="0" z="0"/><vertex x="0" y="0" z="2"/>
    </vertices><triangles><triangle v1="0" v2="1" v3="2"/></triangles></mesh></object>
    <object id="2"><components><component objectid="1" transform="1 0 0 0 1 0 0 0 1 1 0 0"/></components></object>
  </resources><build><item objectid="2" transform="0 1 0 -1 0 0 0 0 1 0 0 0"/></build></model>"#;
    let converted = read(&to_glb("turned.3mf", &three_mf(model), no_siblings).expect("converted"));
    let (min, max) = bounds(&converted);
    assert_near(min, [0.0, 0.0, -1.5]);
    assert_near(max, [0.0, 2.0, -1.0]);
}

#[test]
fn an_unknown_extension_is_refused_with_the_formats_that_load() {
    let error = to_glb("hall.fbx", b"Kaydara", no_siblings).expect_err("refused");
    assert_eq!(error.file, "hall.fbx");
    assert!(error.message.contains(".fbx is not"), "{error}");
    assert!(error.message.contains(".glb, .gltf, .3mf, .obj"), "{error}");
}

#[test]
fn an_obj_whose_material_library_is_missing_is_refused_naming_it() {
    let obj = b"mtllib lost.mtl\nv 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n";
    let error = to_glb("set.obj", obj, siblings(&[])).expect_err("refused");
    assert!(
        error
            .to_string()
            .starts_with("set.obj: material library lost.mtl"),
        "{error}"
    );
}

#[test]
fn a_model_with_no_faces_is_refused_as_empty() {
    let obj = b"v 0 0 0\nv 1 0 0\nv 0 1 0\n";
    let error = to_glb("points.obj", obj, no_siblings).expect_err("refused");
    assert!(error.message.contains("no triangles"), "{error}");
    let empty = r#"<model unit="meter"><resources><object id="1"><mesh><vertices/><triangles/></mesh></object></resources><build><item objectid="1"/></build></model>"#;
    let error = to_glb("empty.3mf", &three_mf(empty), no_siblings).expect_err("refused");
    assert!(error.message.contains("no triangles"), "{error}");
}

#[test]
fn non_finite_coordinates_and_dangling_faces_are_refused() {
    let error = to_glb("bad.obj", b"v 0 nan 0\n", no_siblings).expect_err("refused");
    assert!(error.message.contains("not a finite number"), "{error}");
    let error = to_glb("bad.obj", b"v 0 0 0\nf 1 2 3\n", no_siblings).expect_err("refused");
    assert!(error.message.contains("not defined"), "{error}");
}

#[test]
fn a_glb_passes_through_unchanged() {
    let bytes = b"glTF anything".to_vec();
    assert_eq!(
        to_glb("hall.GLB", &bytes, no_siblings).expect("kept"),
        bytes
    );
}
