use super::*;
use light_fixture::{VenueWindowShape, WallOpening, WallOpeningKind};

fn walls() -> Vec<VenueWallPlacement> {
    [
        (1, [0.0, 1500.0, 0.0], 0.0),
        (2, [2000.0, 1500.0, -2000.0], 90.0),
    ]
    .into_iter()
    .map(|(id, position, yaw)| VenueWallPlacement {
        id: Uuid::from_u128(id),
        size_millimetres: [4000.0, 3000.0, 200.0],
        local_to_world: Mat4::from_rotation_translation(
            glam::Quat::from_rotation_y(f32::to_radians(yaw)),
            Vec3::from_array(position),
        )
        .to_cols_array_2d()
        .map(|column| column.map(f64::from)),
        openings: vec![],
    })
    .collect()
}
fn opening() -> WallOpening {
    WallOpening {
        id: Uuid::from_u128(30),
        offset_millimetres: 500,
        sill_millimetres: 900,
        width_millimetres: 800,
        height_millimetres: 1000,
        opening: WallOpeningKind::Window {
            shape: VenueWindowShape::Rectangular,
        },
    }
}

#[test]
fn repeated_frames_and_reordered_physical_walls_share_one_mesh_generation() {
    let mut cache = Cache::default();
    let source = walls();
    let first = cache.resolve(source.clone());
    assert_eq!(
        first.len(),
        2,
        "exercise actual joined meshes, not an empty cache"
    );
    let mut reversed = source.clone();
    reversed.reverse();
    assert!(Arc::ptr_eq(&first, &cache.resolve(reversed)));
    for _ in 0..100 {
        assert!(Arc::ptr_eq(&first, &cache.resolve(source.clone())));
    }
    assert_eq!(cache.builds, 1);
}

#[test]
fn size_opening_and_effective_neighbour_pose_each_invalidate_meshes() {
    let mut cache = Cache::default();
    let mut source = walls();
    let mut previous = cache.resolve(source.clone());
    source[0].size_millimetres[2] = 300.0;
    let resized = cache.resolve(source.clone());
    assert!(!Arc::ptr_eq(&previous, &resized));
    previous = resized;
    source[0].openings.push(opening());
    let opened = cache.resolve(source.clone());
    assert!(!Arc::ptr_eq(&previous, &opened));
    previous = opened;
    source[0].openings[0].width_millimetres += 100;
    let edited = cache.resolve(source.clone());
    assert!(!Arc::ptr_eq(&previous, &edited));
    previous = edited;
    source[1].local_to_world[3][0] += 1000.0;
    let moved = cache.resolve(source.clone());
    assert!(!Arc::ptr_eq(&previous, &moved));
    assert!(
        moved.is_empty(),
        "disconnected walls must restore ordinary geometry"
    );
    assert!(Arc::ptr_eq(&moved, &cache.resolve(source)));
    assert_eq!(cache.builds, 5);
}

#[test]
fn latest_only_cache_releases_old_generations_and_rebuilds_after_undo_or_delete() {
    let mut cache = Cache::default();
    let source = walls();
    let first = cache.resolve(source.clone());
    let expired = Arc::downgrade(&first);
    drop(first);
    let mut changed = source.clone();
    changed[1].local_to_world[3][2] -= 500.0;
    cache.resolve(changed);
    assert!(
        expired.upgrade().is_none(),
        "cache must not retain old drag generations"
    );
    let restored = cache.resolve(source);
    assert_eq!(restored.len(), 2);
    assert_eq!(cache.builds, 3);
    let empty = cache.resolve(vec![]);
    assert!(empty.is_empty());
    assert!(Arc::ptr_eq(&empty, &cache.resolve(vec![])));
    assert!(cache.walls.is_empty());
    assert_eq!(cache.builds, 4);
}

#[test]
fn material_name_and_ceiling_metadata_do_not_rebuild_physical_wall_meshes() {
    CACHE.with_borrow_mut(|cache| *cache = Cache::default());
    let mut objects: Vec<_> = walls()
        .iter()
        .map(|wall| SceneryObject {
            id: wall.id,
            size: Vec3::from_array(wall.size_millimetres.map(|v| (v * 0.001) as f32)),
            position: Vec3::new(
                wall.local_to_world[3][0] as f32 * 0.001,
                1.5,
                wall.local_to_world[3][2] as f32 * 0.001,
            ),
            rotation_degrees: if wall.id == Uuid::from_u128(2) {
                Vec3::new(0.0, 90.0, 0.0)
            } else {
                Vec3::ZERO
            },
            architecture: Some(VenueArchitecture::Wall {
                openings: vec![],
                ceiling_height_millimetres: None,
            }),
            ..Default::default()
        })
        .collect();
    let first = resolve(&objects);
    assert_eq!(first.len(), 2);
    objects[0].colour = [0.1, 0.2, 0.3];
    objects[0].roughness = 0.9;
    objects[0].name = "Renamed wall".into();
    objects[0].architecture = Some(VenueArchitecture::Wall {
        openings: vec![],
        ceiling_height_millimetres: Some(4000),
    });
    objects.reverse();
    assert!(Arc::ptr_eq(&first, &resolve(&objects)));
    assert_eq!(CACHE.with_borrow(|cache| cache.builds), 1);
}
