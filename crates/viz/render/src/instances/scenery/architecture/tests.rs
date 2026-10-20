use super::*;
fn room() -> Vec<SceneryObject> {
    [
        (Vec3::new(0.0, 1.5, -3.0), 0.0, 8.0),
        (Vec3::new(4.0, 1.5, 0.0), 90.0, 6.0),
        (Vec3::new(0.0, 1.5, 3.0), 180.0, 8.0),
        (Vec3::new(-4.0, 1.5, 0.0), 270.0, 6.0),
    ]
    .into_iter()
    .map(|(position, angle, width)| SceneryObject {
        position,
        rotation_degrees: Vec3::new(0.0, angle, 0.0),
        size: Vec3::new(width, 3.0, 0.2),
        architecture: Some(VenueArchitecture::Wall {
            openings: vec![],
            ceiling_height_millimetres: Some(3000),
        }),
        ..Default::default()
    })
    .collect()
}
#[test]
fn outside_hides_near_walls_and_roof_but_keeps_every_physical_blocker() {
    let objects = room();
    assert_eq!(rooms(&objects).len(), 1);
    let mut inside_frame = FrameInstances::default();
    let mut outside_frame = FrameInstances::default();
    push_all(
        &mut inside_frame,
        &objects,
        &FrameStyle {
            camera_position: Some(Vec3::new(0.0, 1.6, 0.0)),
            ..Default::default()
        },
    );
    push_all(
        &mut outside_frame,
        &objects,
        &FrameStyle {
            camera_position: Some(Vec3::new(0.0, 5.0, 10.0)),
            ..Default::default()
        },
    );
    assert_eq!(
        inside_frame.curtain_blockers.len(),
        outside_frame.curtain_blockers.len()
    );
    assert!(
        inside_frame
            .meshes
            .iter()
            .flat_map(|(_, v)| v)
            .all(|i| i.surface[1] == 0.0)
    );
    let hidden = outside_frame
        .meshes
        .iter()
        .flat_map(|(_, v)| v)
        .filter(|i| i.surface[1] > 0.5)
        .count();
    assert!(hidden > 0);
    assert!(hidden < outside_frame.meshes.iter().map(|(_, v)| v.len()).sum());
}
#[test]
fn open_wall_chain_never_generates_roof_or_cutaway() {
    let mut objects = room();
    objects.pop();
    assert!(rooms(&objects).is_empty());
}

#[test]
fn openings_are_real_and_cutaway_keeps_identical_light_blockers_at_every_quality() {
    use bytemuck::Zeroable;
    use light_fixture::{VenueWindowShape, WallOpening, WallOpeningKind};
    let object = SceneryObject {
        size: Vec3::new(4.0, 3.0, 0.2),
        position: Vec3::new(0.0, 1.5, 0.0),
        architecture: Some(VenueArchitecture::Wall {
            openings: vec![WallOpening {
                id: uuid::Uuid::new_v4(),
                offset_millimetres: 1500,
                sill_millimetres: 1000,
                width_millimetres: 1000,
                height_millimetres: 1000,
                opening: WallOpeningKind::Window {
                    shape: VenueWindowShape::Round,
                },
            }],
            ceiling_height_millimetres: None,
        }),
        ..Default::default()
    };
    for quality in [
        viz_scene::RenderQuality::Draft,
        viz_scene::RenderQuality::Standard,
        viz_scene::RenderQuality::High,
        viz_scene::RenderQuality::Ultra,
    ] {
        let mut frame = FrameInstances::default();
        push(
            &mut frame,
            &object,
            &FrameStyle {
                quality,
                ..Default::default()
            },
            true,
        );
        let mut lights = [super::super::super::GpuLight {
            position_range: [0.0, 1.5, -3.0, 20.0],
            direction_cos_outer: [0.0, 0.0, 1.0, 0.1],
            ..Zeroable::zeroed()
        }];
        let packed = crate::renderer::curtain_occlusion::pack(&mut lights, &frame.curtain_blockers);
        let blocked = |point: Vec3| {
            packed.iter().skip(1).any(|b| {
                let p = point.extend(1.0);
                glam::Vec4::from_array(b.planes[4]).dot(p) > 0.0
                    && b.planes[..4]
                        .iter()
                        .all(|v| glam::Vec4::from_array(*v).dot(p) >= -1e-6)
            })
        };
        assert!(
            !blocked(Vec3::new(0.0, 1.5, 3.0)),
            "window centre must pass light at {quality:?}"
        );
        assert!(
            blocked(Vec3::new(2.0, 1.5, 3.0)),
            "wall must block light at {quality:?}"
        );
    }
}
