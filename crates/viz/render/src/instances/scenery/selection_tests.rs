//! A Venue object the Architect selected is outlined in the selection ink, and nothing else is.
use super::*;
use crate::instances::build;
use viz_scene::FixtureInstance;

const CHOSEN: u128 = 7;
const OTHER: u128 = 8;

/// A generated Venue object and the fixture that carries it, `x` metres across the stage.
fn venue_object(scene: &mut Scene, id: u128, x: f32) {
    let instance_id = Uuid::from_u128(id + 100);
    scene.fixtures.push(FixtureInstance {
        fixture_id: Uuid::from_u128(id),
        instance_id,
        drawn_as_scenery: true,
        position: Vec3::new(x, 1.0, 0.0),
        ..FixtureInstance::default()
    });
    scene.scenery.push(SceneryObject {
        id: instance_id,
        position: Vec3::new(x, 1.0, 0.0),
        size: Vec3::new(2.0, 1.0, 1.0),
        colour: [0.3, 0.3, 0.3],
        roughness: 0.5,
        kind: SceneryKind::Box,
        ..SceneryObject::default()
    });
}

fn scene() -> Scene {
    let mut scene = Scene::default();
    venue_object(&mut scene, CHOSEN, 0.0);
    venue_object(&mut scene, OTHER, 6.0);
    scene
}

fn style() -> FrameStyle {
    FrameStyle {
        draw_beams: false,
        floor_grid: false,
        ..FrameStyle::default()
    }
}

fn selecting(ids: &[u128]) -> SceneValues {
    let mut values = SceneValues::default();
    values
        .selected_fixtures
        .extend(ids.iter().map(|id| Uuid::from_u128(*id)));
    values
}

fn selected_lines(frame: &FrameInstances, style: &FrameStyle) -> Vec<Vec3> {
    frame
        .lines
        .iter()
        .filter(|vertex| {
            (Vec3::from_slice(&vertex.colour[..3]) - style.selected_ink).length() < 1e-5
        })
        .map(|vertex| Vec3::from_array(vertex.position))
        .collect()
}

#[test]
fn a_selected_venue_object_gets_a_cage_without_changing_its_material() {
    let scene = scene();
    let style = style();
    let plain = build(&scene, &SceneValues::default(), &style);
    assert!(
        selected_lines(&plain, &style).is_empty(),
        "nothing selected"
    );

    let selected = build(&scene, &selecting(&[CHOSEN]), &style);
    let cage = selected_lines(&selected, &style);
    assert_eq!(cage.len(), 24, "one twelve-edge cage");
    assert!(
        cage.iter().all(|point| point.x.abs() < 1.5),
        "only the chosen object at x=0 is outlined; the other stands at x=6"
    );
    assert_eq!(selected.meshes.len(), plain.meshes.len());
    for ((kind, chosen), (plain_kind, unchosen)) in selected.meshes.iter().zip(&plain.meshes) {
        assert_eq!(kind, plain_kind);
        assert_eq!(
            bytemuck::cast_slice::<MeshInstance, u8>(chosen),
            bytemuck::cast_slice::<MeshInstance, u8>(unchosen),
            "the outline is presentation only"
        );
    }
}

#[test]
fn several_venue_objects_follow_the_selection_as_it_changes() {
    let scene = scene();
    let style = style();
    let both = build(&scene, &selecting(&[CHOSEN, OTHER]), &style);
    assert_eq!(selected_lines(&both, &style).len(), 48, "two cages");

    let moved = build(&scene, &selecting(&[OTHER]), &style);
    let cage = selected_lines(&moved, &style);
    assert_eq!(cage.len(), 24);
    assert!(
        cage.iter().all(|point| point.x > 4.0),
        "the cage moved to x=6"
    );

    let unrelated = build(&scene, &selecting(&[99]), &style);
    assert!(
        selected_lines(&unrelated, &style).is_empty(),
        "an id that names neither object outlines neither"
    );
}

/// An outline view already draws every object as its box; a selected one takes the ink instead.
#[test]
fn an_outline_view_draws_the_selected_object_in_the_selection_ink() {
    let scene = scene();
    let style = FrameStyle {
        scenery_surfaces: false,
        ..style()
    };
    let plain = build(&scene, &SceneValues::default(), &style);
    let selected = build(&scene, &selecting(&[CHOSEN]), &style);
    assert_eq!(selected.lines.len(), plain.lines.len(), "no second box");
    let cage = selected_lines(&selected, &style);
    assert_eq!(cage.len(), 24);
    assert!(cage.iter().all(|point| point.x.abs() < 1.5));
}
