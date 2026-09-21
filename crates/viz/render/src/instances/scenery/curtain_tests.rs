//! A curtain has to be opaque: its folds overlap rather than abut.
use super::*;
use crate::instances::build;

fn curtain(width: f32) -> SceneryObject {
    SceneryObject {
        position: Vec3::new(0.0, 1.5, 0.0),
        size: Vec3::new(width, 3.0, 0.3),
        colour: [0.1, 0.2, 0.9],
        roughness: 0.95,
        kind: SceneryKind::Curtain,
        ..SceneryObject::default()
    }
}

/// Every fold, left to right: the X its centre stands at, and how wide it is built.
fn folds(width: f32) -> Vec<(f32, f32)> {
    let mut scene = Scene::default();
    scene.scenery.push(curtain(width));
    let frame = build(&scene, &SceneValues::default(), &FrameStyle::default());
    let mut placed: Vec<(f32, f32)> = frame
        .meshes
        .iter()
        .filter(|(kind, _)| *kind == MeshKind::Cylinder)
        .flat_map(|(_, entries)| entries.iter())
        .map(|entry| {
            let (scale, _, translation) =
                Mat4::from_cols_array_2d(&entry.model).to_scale_rotation_translation();
            (translation.x, scale.x)
        })
        .collect();
    placed.sort_by(|a, b| a.0.total_cmp(&b.0));
    placed
}

#[test]
fn neighbouring_folds_overlap_at_every_width() {
    // Narrow, default and wide: the fold count is clamped at both ends, so the pitch differs.
    for width in [0.9_f32, 4.0, 24.0] {
        let placed = folds(width);
        assert!(placed.len() >= 2, "{width} m curtain has folds");
        for pair in placed.windows(2) {
            let [(left_x, left_width), (right_x, right_width)] = [pair[0], pair[1]];
            let reach = left_width / 2.0 + right_width / 2.0;
            let apart = right_x - left_x;
            assert!(
                reach > apart,
                "{width} m curtain: folds at {left_x} and {right_x} only reach {reach} across a \
                 {apart} gap, so the drape is see-through between them"
            );
        }
    }
}

#[test]
fn the_folds_still_alternate_in_depth_so_the_drape_reads_as_gathered() {
    let mut scene = Scene::default();
    scene.scenery.push(curtain(4.0));
    let frame = build(&scene, &SceneValues::default(), &FrameStyle::default());
    let mut depths: Vec<(f32, f32)> = frame
        .meshes
        .iter()
        .filter(|(kind, _)| *kind == MeshKind::Cylinder)
        .flat_map(|(_, entries)| entries.iter())
        .map(|entry| {
            let (scale, _, translation) =
                Mat4::from_cols_array_2d(&entry.model).to_scale_rotation_translation();
            (translation.x, scale.z)
        })
        .collect();
    depths.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert!(
        depths.windows(2).all(|pair| pair[0].1 != pair[1].1),
        "every fold stands at a different depth from the one beside it: {depths:?}"
    );
}
