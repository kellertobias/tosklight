use super::*;
use viz_scene::{CrowdArea, CrowdDensity, CrowdPosture, RenderQuality};
fn json(image: &ImageManifest) -> serde_json::Value {
    let size = u32::from_le_bytes(image.geometry[12..16].try_into().unwrap()) as usize;
    serde_json::from_slice(&image.geometry[20..20 + size]).unwrap()
}
#[test]
fn draft_requested_detail_reaches_actual_ultra_blender_geometry_without_changing_population() {
    let scene = Scene {
        crowds: vec![CrowdArea {
            id: viz_scene::uuid::Uuid::nil(),
            name: "Small audience".into(),
            position: Vec3::ZERO,
            rotation_degrees: Vec3::ZERO,
            width_metres: 2.0,
            depth_metres: 2.0,
            posture: CrowdPosture::StandingStill,
            density: CrowdDensity::Sparse,
            seed: 42,
        }],
        ..Default::default()
    };
    let mut view = ViewConfiguration {
        quality: RenderQuality::Draft,
        ..Default::default()
    };
    let values = SceneValues::default();
    let media = FrozenMedia::default();
    let simple =
        ImageManifest::build_with_crowd_amount(&scene, &values, &view, 1.0, &media, 0.5).unwrap();
    view.detailed_people = true;
    let detailed =
        ImageManifest::build_with_crowd_amount(&scene, &values, &view, 1.0, &media, 0.5).unwrap();
    assert!(!simple.detailed_people && detailed.detailed_people);
    assert_eq!(detailed.crowd_amount, simple.crowd_amount);
    assert_eq!(view.quality, RenderQuality::Draft);
    let off = json(&simple);
    let on = json(&detailed);
    assert!(
        off["meshes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["name"] == "crowd-person")
    );
    assert!(
        on["meshes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["name"] == "detailed-people-skin")
    );
    assert!(
        !on["meshes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["name"] == "crowd-person")
    );
    for material in on["materials"].as_array().unwrap() {
        assert!(
            material.get("emissiveFactor").is_none(),
            "people have no fake unlit emission"
        );
    }
}
