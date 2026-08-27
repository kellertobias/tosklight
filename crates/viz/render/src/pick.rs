//! What a pointer is over.
//!
//! Turning a cursor position into the element it is on top of, using the same camera the picture
//! was drawn with. Nothing here changes what is drawn: it reads the scene and answers with an
//! element and a world point. No input is bound to it today — the left button is deliberately free
//! — but a selection or inspection gesture is what it is for.

use crate::camera::Ray;
use glam::{Quat, Vec3};
use viz_scene::{Scene, euler_degrees};

/// The smallest half-extent a fixture body is given while picking, so a compact lantern on a bar
/// is still something the operator can hit with a mouse.
const MINIMUM_HALF_EXTENT: f32 = 0.12;

/// What a click landed on.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PickedElement {
    /// Index into [`Scene::fixtures`].
    Fixture(usize),
    /// Index into [`Scene::scenery`].
    Scenery(usize),
    /// Nothing was under the cursor: the point came from the stage floor, or from straight ahead.
    Nothing,
}

/// One resolved click.
#[derive(Clone, Copy, Debug)]
pub struct Pick {
    pub element: PickedElement,
    /// The world point the operator effectively pointed at.
    pub point: Vec3,
}

/// Resolve what `ray` points at. `reach` is how far ahead to fall back to when the ray leaves the
/// scene entirely — normally the camera's own orbit distance, so an empty click keeps the feel of
/// the view the operator already has.
pub fn pick(scene: &Scene, ray: &Ray, reach: f32, points: &[viz_scene::PointPose]) -> Pick {
    let mut nearest: Option<Pick> = None;
    let mut nearest_distance = f32::INFINITY;

    for (index, fixture) in scene.fixtures.iter().enumerate() {
        let half = (fixture.body.size * 0.5).max(Vec3::splat(MINIMUM_HALF_EXTENT));
        // Hit the fixture where it is drawn. A fixture slaved to a 3D Point that moved is no
        // longer where the rig put it, and an operator must be able to click the thing they see.
        let (position, orientation) = fixture.placed_by(points);
        let Some(distance) = box_hit(ray, position, half, orientation) else {
            continue;
        };
        if distance < nearest_distance {
            nearest_distance = distance;
            // The fixture's own centre, not the face the ray happened to graze: a caller asking
            // about a fixture wants the fixture, not a point on its casing.
            nearest = Some(Pick {
                element: PickedElement::Fixture(index),
                point: position,
            });
        }
    }

    for (index, object) in scene.scenery.iter().enumerate() {
        let half = (object.size * 0.5).max(Vec3::splat(1e-3));
        let orientation = euler_degrees(object.rotation_degrees);
        let Some(distance) = box_hit(ray, object.position, half, orientation) else {
            continue;
        };
        if distance < nearest_distance {
            nearest_distance = distance;
            // A floor or a cyc is far larger than the click, so the point that was actually
            // touched is the honest pivot here.
            nearest = Some(Pick {
                element: PickedElement::Scenery(index),
                point: ray.origin + ray.direction * distance,
            });
        }
    }

    if let Some(pick) = nearest {
        return pick;
    }

    let floor = if scene.bounds.is_empty() {
        0.0
    } else {
        scene.bounds.min.y
    };
    if let Some(distance) = floor_hit(ray, floor, reach) {
        return Pick {
            element: PickedElement::Nothing,
            point: ray.origin + ray.direction * distance,
        };
    }
    Pick {
        element: PickedElement::Nothing,
        point: ray.origin + ray.direction * reach.max(0.5),
    }
}

/// Distance along `ray` to an oriented box, or `None` when it misses.
fn box_hit(ray: &Ray, centre: Vec3, half: Vec3, orientation: Quat) -> Option<f32> {
    let inverse = orientation.inverse();
    let origin = inverse * (ray.origin - centre);
    let direction = inverse * ray.direction;
    let mut entry = f32::NEG_INFINITY;
    let mut exit = f32::INFINITY;
    for axis in 0..3 {
        let half_extent = half[axis].max(1e-4);
        if direction[axis].abs() < 1e-6 {
            if origin[axis].abs() > half_extent {
                return None;
            }
            continue;
        }
        let first = (-half_extent - origin[axis]) / direction[axis];
        let second = (half_extent - origin[axis]) / direction[axis];
        entry = entry.max(first.min(second));
        exit = exit.min(first.max(second));
        if entry > exit {
            return None;
        }
    }
    if exit < 0.0 {
        return None;
    }
    // A ray that starts inside the box hits it here and now.
    Some(entry.max(0.0))
}

/// Distance along `ray` to the stage floor, ignoring a hit so far away it is off the premises.
fn floor_hit(ray: &Ray, floor_height: f32, reach: f32) -> Option<f32> {
    if ray.direction.y > -1e-4 {
        return None;
    }
    let distance = (floor_height - ray.origin.y) / ray.direction.y;
    if distance <= 0.0 || distance > reach.max(1.0) * 8.0 {
        return None;
    }
    Some(distance)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::{CameraControl, ResolvedCamera};
    use viz_scene::{
        Aabb, BodyKind, Camera, FixtureBody, FixtureInstance, SceneryKind, SceneryObject, ViewMode,
    };

    fn fixture(position: Vec3) -> FixtureInstance {
        FixtureInstance {
            instance_id: viz_scene::uuid::Uuid::nil(),
            fixture_id: viz_scene::uuid::Uuid::nil(),
            name: "Spot".into(),
            number: Some(1),
            position,
            rotation_degrees: Vec3::ZERO,
            position_master: None,
            bracket_degrees: 0.0,
            shaper_degrees: None,
            installed_colour: [1.0; 3],
            installed_shaper_angles_degrees: [0.0; 4],
            body: FixtureBody {
                size: Vec3::new(0.3, 0.4, 0.3),
                kind: BodyKind::MovingHead,
            },
            patched: true,
            address: None,
            model: None,
            fallback: None,
        }
    }

    fn scene() -> Scene {
        let mut scene = Scene::default();
        scene.fixtures.push(fixture(Vec3::new(-3.0, 5.0, 0.0)));
        scene.fixtures.push(fixture(Vec3::new(3.0, 5.0, 0.0)));
        scene.scenery.push(SceneryObject {
            id: viz_scene::uuid::Uuid::nil(),
            name: "Stage".into(),
            position: Vec3::new(0.0, -0.05, 0.0),
            rotation_degrees: Vec3::ZERO,
            size: Vec3::new(20.0, 0.1, 12.0),
            colour: [0.2, 0.2, 0.2],
            roughness: 0.8,
            kind: SceneryKind::Floor,
            chords: 0,
        });
        scene.recompute_bounds();
        scene
    }

    /// The camera the picture was drawn with, so a pick answers for what the operator can see.
    fn camera(control: &CameraControl, bounds: Aabb) -> ResolvedCamera {
        let base = Camera::default();
        ResolvedCamera::resolve(
            &control.camera(&base, ViewMode::Full3d),
            ViewMode::Full3d,
            1600.0 / 900.0,
            bounds,
        )
    }

    /// Pointing at a fixture has to name that fixture, whichever one of them it is, and answer
    /// with its own position rather than something nearby.
    #[test]
    fn a_click_on_a_fixture_picks_that_fixture() {
        let scene = scene();
        let control = CameraControl {
            target: Vec3::new(0.0, 5.0, 0.0),
            distance: 14.0,
            pitch: 0.0,
            ..CameraControl::default()
        };
        let resolved = camera(&control, scene.bounds);
        for (index, expected) in [0_usize, 1].iter().enumerate() {
            let position = scene.fixtures[*expected].position;
            let (x, y) = resolved
                .project(position, 1600.0, 900.0)
                .expect("the fixture is in frame");
            let ray = resolved.ray_through(x, y, 1600.0, 900.0);
            let pick = pick(&scene, &ray, control.distance, &[]);
            assert_eq!(
                pick.element,
                PickedElement::Fixture(*expected),
                "click {index} at {x},{y} picked {:?}",
                pick.element
            );
            assert!((pick.point - position).length() < 1e-3);
        }
    }

    /// The near fixture wins when two line up, so a pick never names something hidden behind what
    /// the operator can see.
    #[test]
    fn the_nearest_element_along_the_ray_wins() {
        let mut scene = Scene::default();
        scene.fixtures.push(fixture(Vec3::new(0.0, 2.0, 4.0)));
        scene.fixtures.push(fixture(Vec3::new(0.0, 2.0, -4.0)));
        scene.recompute_bounds();
        let ray = Ray {
            origin: Vec3::new(0.0, 2.0, 20.0),
            direction: Vec3::NEG_Z,
        };
        assert_eq!(
            pick(&scene, &ray, 20.0, &[]).element,
            PickedElement::Fixture(0)
        );
    }

    /// Clicking past the rig still has to give a usable pivot, or the drag would do nothing.
    #[test]
    fn a_click_on_nothing_falls_back_to_the_stage_floor() {
        let scene = scene();
        let ray = Ray {
            origin: Vec3::new(0.0, 6.0, 14.0),
            direction: Vec3::new(0.0, -0.6, -0.8).normalize(),
        };
        let pick = pick(&scene, &ray, 14.0, &[]);
        assert!(matches!(
            pick.element,
            PickedElement::Scenery(_) | PickedElement::Nothing
        ));
        assert!(
            pick.point.y < 0.2,
            "an empty click lands on the floor, got {:?}",
            pick.point
        );
    }

    /// A ray into the sky has no floor to land on and still must not produce a wild pivot.
    #[test]
    fn a_click_at_the_sky_stays_within_reach() {
        let scene = Scene::default();
        let ray = Ray {
            origin: Vec3::new(0.0, 2.0, 10.0),
            direction: Vec3::new(0.0, 0.7, -0.7).normalize(),
        };
        let pick = pick(&scene, &ray, 12.0, &[]);
        assert_eq!(pick.element, PickedElement::Nothing);
        assert!((pick.point - ray.origin).length() <= 12.0 + 1e-3);
    }
}
