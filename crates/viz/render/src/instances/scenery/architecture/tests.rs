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

#[test]
fn open_door_centre_passes_light_but_actual_leaf_blocks_it() {
    use bytemuck::Zeroable;
    use light_fixture::{
        VenueDoorHinge, VenueDoorLeaves, VenueDoorSwing, WallOpening, WallOpeningKind,
    };
    for swing in [VenueDoorSwing::Inward, VenueDoorSwing::Outward] {
        for leaves in [VenueDoorLeaves::Single, VenueDoorLeaves::Double] {
            let object = SceneryObject {
                size: Vec3::new(4.0, 3.0, 0.2),
                position: Vec3::new(0.0, 1.5, 0.0),
                architecture: Some(VenueArchitecture::Wall {
                    openings: vec![WallOpening {
                        id: uuid::Uuid::new_v4(),
                        offset_millimetres: 1000,
                        sill_millimetres: 0,
                        width_millimetres: 1600,
                        height_millimetres: 2100,
                        opening: WallOpeningKind::Door {
                            leaves,
                            swing,
                            hinge: VenueDoorHinge::Left,
                        },
                    }],
                    ceiling_height_millimetres: None,
                }),
                ..Default::default()
            };
            let mut frame = FrameInstances::default();
            push(&mut frame, &object, &FrameStyle::default(), true);
            let blocked = |origin: Vec3, destination: Vec3| {
                let direction = (destination - origin).normalize();
                let mut light = [super::super::super::GpuLight {
                    position_range: origin.extend(20.0).to_array(),
                    direction_cos_outer: direction.extend(0.1).to_array(),
                    ..Zeroable::zeroed()
                }];
                let packed =
                    crate::renderer::curtain_occlusion::pack(&mut light, &frame.curtain_blockers);
                let p = destination.extend(1.0);
                packed.iter().skip(1).any(|b| {
                    glam::Vec4::from_array(b.planes[4]).dot(p) > 0.0
                        && b.planes[..4]
                            .iter()
                            .all(|plane| glam::Vec4::from_array(*plane).dot(p) >= -1e-6)
                })
            };
            assert!(
                !blocked(Vec3::new(-0.2, 1.0, -3.0), Vec3::new(-0.2, 1.0, 3.0)),
                "open centre stays clear: {leaves:?}, {swing:?}"
            );
            let z = if swing == VenueDoorSwing::Inward {
                0.4
            } else {
                -0.4
            };
            assert!(
                blocked(Vec3::new(-2.0, 1.0, z), Vec3::new(0.0, 1.0, z)),
                "open leaf remains physical: {leaves:?}, {swing:?}"
            );
        }
    }
}

#[test]
fn stair_cutouts_are_open_in_visible_slab_and_physical_blockers() {
    use bytemuck::Zeroable;
    let stair = SceneryObject {
        size: Vec3::new(1.2, 3.0, 4.5),
        position: Vec3::new(0.0, 1.5, 0.0),
        architecture: Some(VenueArchitecture::Stairs { steps: 16 }),
        ..Default::default()
    };
    let slab = SceneryObject {
        size: Vec3::new(8.0, 0.2, 8.0),
        architecture: Some(VenueArchitecture::Slab {
            purpose: VenueSlabPurpose::Balcony,
            outline_millimetres: vec![
                light_fixture::VenuePlanPoint { x: -4000, y: -4000 },
                light_fixture::VenuePlanPoint { x: 4000, y: -4000 },
                light_fixture::VenuePlanPoint { x: 4000, y: 4000 },
                light_fixture::VenuePlanPoint { x: -4000, y: 4000 },
            ],
            elevation_millimetres: 1500,
            thickness_millimetres: 200,
        }),
        ..Default::default()
    };
    let mut frame = FrameInstances::default();
    push_all(&mut frame, &[stair, slab], &FrameStyle::default());
    let blocked = |x: f32| {
        let mut lights = [super::super::super::GpuLight {
            position_range: [x, 1.0, -1.8, 20.0],
            direction_cos_outer: [0.0, 1.0, 0.0, 0.1],
            ..Zeroable::zeroed()
        }];
        let packed = crate::renderer::curtain_occlusion::pack(&mut lights, &frame.curtain_blockers);
        let point = Vec3::new(x, 2.5, -1.8).extend(1.0);
        packed.iter().skip(1).any(|b| {
            glam::Vec4::from_array(b.planes[4]).dot(point) > 0.0
                && b.planes[..4]
                    .iter()
                    .all(|plane| glam::Vec4::from_array(*plane).dot(point) >= -1e-6)
        })
    };
    assert!(
        !blocked(0.0),
        "true stairwell aperture must pass light above the low steps"
    );
    assert!(
        blocked(2.0),
        "the floor beside the stairwell must remain a physical blocker"
    );
}

#[test]
fn legacy_floor_conversion_keeps_support_floor_and_cuts_the_reached_floor_only() {
    let stairs = SceneryObject {
        size: Vec3::new(1.2, 3.0, 4.5),
        position: Vec3::new(0.0, 1.5, 0.0),
        architecture: Some(VenueArchitecture::Stairs { steps: 16 }),
        ..Default::default()
    };
    let support = SceneryObject {
        kind: SceneryKind::Floor,
        size: Vec3::new(8.0, 0.2, 8.0),
        position: Vec3::new(0.0, -0.1, 0.0),
        ..Default::default()
    };
    let upper = SceneryObject {
        kind: SceneryKind::Floor,
        size: Vec3::new(8.0, 0.2, 8.0),
        position: Vec3::new(0.0, 1.6, 0.0),
        ..Default::default()
    };
    let distant = SceneryObject {
        position: Vec3::new(20.0, 1.6, 0.0),
        ..upper.clone()
    };
    let mut objects = [stairs, support, upper, distant];
    apply_floor_cutouts(&mut objects);
    assert!(objects[1].architecture.is_none());
    assert!(matches!(
        objects[2].architecture,
        Some(VenueArchitecture::Slab {
            purpose: VenueSlabPurpose::Floor,
            ..
        })
    ));
    assert!(objects[3].architecture.is_none());
}

#[test]
fn joined_corner_blocks_diagonal_light_with_camera_cutaway_at_every_quality() {
    use bytemuck::Zeroable;
    let objects = room();
    for quality in [
        viz_scene::RenderQuality::Draft,
        viz_scene::RenderQuality::Standard,
        viz_scene::RenderQuality::High,
        viz_scene::RenderQuality::Ultra,
    ] {
        let mut previous = None;
        for camera in [Vec3::new(0.0, 1.5, 0.0), Vec3::new(8.0, 6.0, 8.0)] {
            let mut frame = FrameInstances::default();
            push_all(
                &mut frame,
                &objects,
                &FrameStyle {
                    quality,
                    camera_position: Some(camera),
                    ..Default::default()
                },
            );
            let bytes = bytemuck::cast_slice::<_, u8>(&frame.curtain_blockers).to_vec();
            if let Some(previous) = previous {
                assert_eq!(
                    bytes, previous,
                    "camera must not remove joined wall blockers"
                );
            }
            previous = Some(bytes);
            let mut lights = [super::super::super::GpuLight {
                position_range: [6.0, 1.5, 5.0, 20.0],
                direction_cos_outer: [-0.707, 0.0, -0.707, 0.01],
                ..Zeroable::zeroed()
            }];
            let packed =
                crate::renderer::curtain_occlusion::pack(&mut lights, &frame.curtain_blockers);
            let p = Vec3::new(2.0, 1.5, 1.0).extend(1.0);
            assert!(
                packed
                    .iter()
                    .skip(1)
                    .any(|b| glam::Vec4::from_array(b.planes[4]).dot(p) > 0.0
                        && b.planes[..4]
                            .iter()
                            .all(|plane| glam::Vec4::from_array(*plane).dot(p) >= -1e-6)),
                "diagonal light must not leak through room corner at {quality:?}"
            );
        }
    }
}
