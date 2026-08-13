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
