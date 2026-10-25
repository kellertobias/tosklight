use super::*;
use crate::{FrameStyle, build_instances};
use viz_scene::{CrowdDensity, SceneValues};

fn scene(posture: CrowdPosture) -> Scene {
    Scene {
        crowds: vec![CrowdArea {
            id: viz_scene::uuid::Uuid::nil(),
            name: "Audience".into(),
            position: Vec3::new(1.0, 0.0, -2.0),
            rotation_degrees: Vec3::new(0.0, 31.0, 0.0),
            width_metres: 10.0,
            depth_metres: 10.0,
            posture,
            density: CrowdDensity::Medium,
            seed: 42,
        }],
        ..Scene::default()
    }
}
#[test]
fn detail_preserves_population_and_exact_placements_and_has_no_unlit_emission() {
    for posture in [
        CrowdPosture::Sitting,
        CrowdPosture::StandingStill,
        CrowdPosture::Dancing,
    ] {
        let scene = scene(posture);
        let mut style = FrameStyle {
            quality: RenderQuality::Ultra,
            crowd_amount: 0.5,
            crowd_person_budget: 64,
            ..Default::default()
        };
        let simple = build_instances(&scene, &SceneValues::default(), &style);
        style.detailed_people = true;
        let detail = build_instances(&scene, &SceneValues::default(), &style);
        assert_eq!(
            (
                simple.crowd_authored,
                simple.crowd_requested,
                simple.crowd_drawn
            ),
            (
                detail.crowd_authored,
                detail.crowd_requested,
                detail.crowd_drawn
            )
        );
        let old = &simple
            .meshes
            .iter()
            .find(|(kind, _)| *kind == MeshKind::CrowdPerson)
            .unwrap()
            .1;
        let skin = &detail
            .meshes
            .iter()
            .find(|(kind, _)| {
                *kind == MeshKind::DetailedPeopleSkin(posture == CrowdPosture::Sitting)
            })
            .unwrap()
            .1;
        assert_eq!(old.len(), skin.len());
        for (simple, detailed) in old.iter().zip(skin) {
            assert_eq!(simple.model, detailed.model);
        }
        assert!(
            !detail.meshes.iter().any(|(kind, _)| matches!(
                kind,
                MeshKind::CrowdPerson | MeshKind::CrowdPersonOutline
            ))
        );
        for (_, instances) in &detail.meshes {
            for instance in instances {
                assert_eq!(instance.emissive, [0.0; 4]);
            }
        }
        style.crowd_amount = 0.0;
        assert_eq!(
            build_instances(&scene, &SceneValues::default(), &style).crowd_drawn,
            0
        );
    }
}
#[test]
fn detail_is_ignored_below_ultra_and_meshes_are_shared_by_native_and_export() {
    let scene = scene(CrowdPosture::StandingStill);
    for quality in [
        RenderQuality::Draft,
        RenderQuality::Standard,
        RenderQuality::High,
    ] {
        let style = FrameStyle {
            quality,
            detailed_people: true,
            ..Default::default()
        };
        let frame = build_instances(&scene, &SceneValues::default(), &style);
        assert!(!frame.meshes.iter().any(|(kind, _)| matches!(
            kind,
            MeshKind::DetailedPeopleSkin(_)
                | MeshKind::DetailedPeopleTop(_)
                | MeshKind::DetailedPeopleBottom(_)
        )));
    }
    let style = FrameStyle {
        quality: RenderQuality::Ultra,
        detailed_people: true,
        ..Default::default()
    };
    let geometry = crate::scene_geometry_with_style(&scene, &SceneValues::default(), &style);
    assert!(
        geometry
            .meshes
            .iter()
            .any(|mesh| mesh.name == "detailed-people-skin")
    );
    assert!(
        geometry
            .meshes
            .iter()
            .all(|mesh| !mesh.name.contains("outline"))
    );
    let again = crate::scene_geometry_with_style(&scene, &SceneValues::default(), &style);
    assert_eq!(
        geometry.instances[0].transform,
        again.instances[0].transform
    );
    assert_eq!(
        geometry.instances[0].base_colour,
        again.instances[0].base_colour
    );
}
