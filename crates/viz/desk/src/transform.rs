//! The one documented effective transform per physical instance.
//!
//! Two authorities exist in the show: the patch's own `location`/`rotation`, and the stage-layout
//! object's `positions3d` (with a legacy 2D `positions` map). The renderer must not guess between
//! them, so this module states the resolution order once, matching what the ToskLight Stage
//! already does:
//!
//! 1. the stage-layout `positions3d` entry for the physical instance, else
//! 2. the patch location and rotation when the patch actually places the fixture, else
//! 3. the migrated legacy 2D entry for the root fixture, else
//! 4. the deterministic patch-order grid slot.
//!
//! # Axes
//!
//! Desk storage uses `x` across the stage, `y` upstage (away from the audience), and `z` up, in
//! metres for stage layout and millimetres for the patch. The renderer uses `x` across, `y` up,
//! and `z` towards the audience, so the conversion is `(x, z, -y)`.

use crate::wire::{Location, Rotation, StagePosition2d, StagePosition3d};
use glam::Vec3;

/// One resolved placement in renderer world space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub position: Vec3,
    pub rotation_degrees: Vec3,
    pub source: PlacementSource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlacementSource {
    StageLayout3d,
    PatchLocation,
    MigratedLayout2d,
    DefaultGrid,
}

/// Desk storage axes to renderer world axes.
pub fn to_world(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, z, -y)
}

/// Desk storage rotation to renderer world rotation, matching the Stage's `(rx, rz, ry)` mapping.
pub fn rotation_to_world(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, z, y)
}

/// The live transform a 3D Point is contributing, in renderer world space.
///
/// A point rests at its patched origin, so an untouched point carries a zero offset and no
/// rotation and leaves everything slaved to it exactly where the rig put it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MasterTransform {
    /// Where the point itself sits when the show is loaded.
    pub origin: Vec3,
    /// How far the operator has since moved it.
    pub offset: Vec3,
    /// How far the operator has since turned it, in degrees about its own origin.
    pub rotation_degrees: Vec3,
}

/// Carry one placement with the 3D Point it is slaved to.
///
/// The slave keeps the placement the rig gave it. The point turns it about the point's own
/// origin and then moves it, so a fixture hung two metres stage-left of a truss point stays two
/// metres stage-left of it however the truss is flown or angled. Rotating first and translating
/// second is what makes the offset read as "where it sits on the point" rather than "where it
/// sits on the stage".
pub fn slave_to_master(placement: Placement, master: MasterTransform) -> Placement {
    let turn = glam::Quat::from_euler(
        glam::EulerRot::YXZ,
        master.rotation_degrees.y.to_radians(),
        master.rotation_degrees.x.to_radians(),
        master.rotation_degrees.z.to_radians(),
    );
    Placement {
        position: master.origin + turn * (placement.position - master.origin) + master.offset,
        // The fixture turns with the point it hangs on, so its own aim rides on top.
        rotation_degrees: placement.rotation_degrees + master.rotation_degrees,
        source: placement.source,
    }
}

/// Resolve one physical instance.
pub fn resolve(
    stored_3d: Option<&StagePosition3d>,
    patch_location: Location,
    patch_rotation: Rotation,
    legacy_2d: Option<&StagePosition2d>,
    grid_index: usize,
) -> Placement {
    if let Some(stored) = stored_3d {
        return Placement {
            position: to_world(stored.x, stored.y, stored.z),
            rotation_degrees: rotation_to_world(
                stored.rotation_x,
                stored.rotation_y,
                stored.rotation_z,
            ),
            source: PlacementSource::StageLayout3d,
        };
    }
    if patch_location.x != 0 || patch_location.y != 0 || patch_location.z != 0 {
        return Placement {
            position: to_world(
                patch_location.x as f32 / 1000.0,
                patch_location.y as f32 / 1000.0,
                patch_location.z as f32 / 1000.0,
            ),
            rotation_degrees: rotation_to_world(
                patch_rotation.x,
                patch_rotation.y,
                patch_rotation.z,
            ),
            source: PlacementSource::PatchLocation,
        };
    }
    if let Some(legacy) = legacy_2d {
        // The desk's documented 2D migration: percent across, percent upstage, fixed trim height.
        return Placement {
            position: to_world(
                (legacy.x / 100.0 - 0.5) * 12.0,
                (legacy.y / 100.0) * 8.0,
                5.0,
            ),
            rotation_degrees: rotation_to_world(0.0, 0.0, legacy.rotation),
            source: PlacementSource::MigratedLayout2d,
        };
    }
    Placement {
        position: to_world(
            -5.25 + (grid_index % 8) as f32 * 1.5,
            1.0 + (grid_index / 8) as f32 * 1.6,
            5.0,
        ),
        rotation_degrees: Vec3::ZERO,
        source: PlacementSource::DefaultGrid,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stored(x: f32, y: f32, z: f32) -> StagePosition3d {
        StagePosition3d {
            x,
            y,
            z,
            rotation_x: 0.0,
            rotation_y: 0.0,
            rotation_z: 0.0,
            crowd_width_metres: None,
            crowd_depth_metres: None,
        }
    }

    fn placed(position: Vec3) -> Placement {
        Placement {
            position,
            rotation_degrees: Vec3::ZERO,
            source: PlacementSource::PatchLocation,
        }
    }

    #[test]
    fn an_untouched_point_leaves_everything_slaved_to_it_where_the_rig_put_it() {
        let rigged = placed(Vec3::new(2.0, 6.0, -1.0));
        let master = MasterTransform {
            origin: Vec3::new(0.0, 6.0, 0.0),
            ..MasterTransform::default()
        };
        assert_eq!(slave_to_master(rigged, master), rigged);
    }

    #[test]
    fn moving_a_point_carries_its_slaves_the_same_distance() {
        let rigged = placed(Vec3::new(2.0, 6.0, -1.0));
        let master = MasterTransform {
            origin: Vec3::new(0.0, 6.0, 0.0),
            offset: Vec3::new(0.0, -1.5, 0.0),
            rotation_degrees: Vec3::ZERO,
        };
        let moved = slave_to_master(rigged, master);
        // Flying the truss down a metre and a half takes the lantern with it and changes nothing
        // about where it sits along the truss.
        assert!((moved.position - Vec3::new(2.0, 4.5, -1.0)).length() < 1e-4);
    }

    #[test]
    fn turning_a_point_swings_its_slaves_about_the_point_rather_than_the_stage() {
        // Two metres stage-right of a point that is not at the origin.
        let rigged = placed(Vec3::new(4.0, 6.0, 0.0));
        let master = MasterTransform {
            origin: Vec3::new(2.0, 6.0, 0.0),
            offset: Vec3::ZERO,
            rotation_degrees: Vec3::new(0.0, 90.0, 0.0),
        };
        let turned = slave_to_master(rigged, master);
        // A quarter turn about the point puts it two metres towards the audience of the point,
        // still two metres away from it. Had it turned about the stage origin it would have
        // landed four metres out instead.
        assert!((turned.position - Vec3::new(2.0, 6.0, -2.0)).length() < 1e-4);
        assert!((turned.position - master.origin).length() - 2.0 < 1e-4);
        // The lantern is turned by the truss, so its own aim rides on top.
        assert_eq!(turned.rotation_degrees, Vec3::new(0.0, 90.0, 0.0));
    }

    #[test]
    fn a_point_turns_its_slaves_before_it_moves_them() {
        let rigged = placed(Vec3::new(4.0, 6.0, 0.0));
        let master = MasterTransform {
            origin: Vec3::new(2.0, 6.0, 0.0),
            offset: Vec3::new(0.0, 0.0, -3.0),
            rotation_degrees: Vec3::new(0.0, 90.0, 0.0),
        };
        // Turning first keeps the offset reading as "where it sits on the point": the swing
        // happens about the point, and only then does the whole assembly move upstage.
        let moved = slave_to_master(rigged, master);
        assert!((moved.position - Vec3::new(2.0, 6.0, -5.0)).length() < 1e-4);
    }

    #[test]
    fn upstage_storage_maps_to_negative_renderer_z() {
        // Desk `y` grows upstage; the renderer's `+Z` points at the audience.
        assert_eq!(to_world(1.0, 7.0, 4.0), Vec3::new(1.0, 4.0, -7.0));
    }

    #[test]
    fn the_stage_layout_entry_wins_over_the_patch_location() {
        let placement = resolve(
            Some(&stored(1.0, 2.0, 3.0)),
            Location {
                x: 9_000,
                y: 9_000,
                z: 9_000,
            },
            Rotation::default(),
            None,
            0,
        );
        assert_eq!(placement.source, PlacementSource::StageLayout3d);
        assert_eq!(placement.position, Vec3::new(1.0, 3.0, -2.0));
    }

    #[test]
    fn a_placed_patch_location_is_used_when_no_layout_entry_exists() {
        let placement = resolve(
            None,
            Location {
                x: -2_250,
                y: 7_000,
                z: 4_650,
            },
            Rotation::default(),
            None,
            0,
        );
        assert_eq!(placement.source, PlacementSource::PatchLocation);
        assert!((placement.position - Vec3::new(-2.25, 4.65, -7.0)).length() < 1e-5);
    }

    #[test]
    fn a_legacy_two_dimensional_entry_migrates_before_the_default_grid() {
        let legacy = StagePosition2d {
            x: 50.0,
            y: 50.0,
            rotation: 30.0,
        };
        let placement = resolve(
            None,
            Location::default(),
            Rotation::default(),
            Some(&legacy),
            3,
        );
        assert_eq!(placement.source, PlacementSource::MigratedLayout2d);
        assert_eq!(placement.position.x, 0.0);
        assert_eq!(placement.rotation_degrees.y, 30.0);
    }

    #[test]
    fn unplaced_fixtures_fall_back_to_a_deterministic_grid() {
        let first = resolve(None, Location::default(), Rotation::default(), None, 0);
        let ninth = resolve(None, Location::default(), Rotation::default(), None, 8);
        assert_eq!(first.source, PlacementSource::DefaultGrid);
        assert_eq!(first.position.x, ninth.position.x);
        assert_ne!(first.position.z, ninth.position.z);
    }
}
