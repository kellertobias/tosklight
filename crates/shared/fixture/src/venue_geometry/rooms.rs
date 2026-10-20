//! Connected wall centrelines used by CAD plans and native camera cutaway alike.
use crate::{VenueArchitecture, VenuePlanPoint, VenueSlabPurpose};
use uuid::Uuid;
#[derive(Clone, Debug)]
pub struct VenueWallRun {
    pub id: Uuid,
    pub start_millimetres: [f64; 2],
    pub end_millimetres: [f64; 2],
    pub floor_millimetres: f64,
    pub height_millimetres: f64,
    pub ceiling_height_millimetres: Option<u32>,
}
#[derive(Clone, Debug)]
pub struct VenueRoom {
    pub wall_ids: Vec<Uuid>,
    pub outline_millimetres: Vec<[f64; 2]>,
    pub floor_millimetres: f64,
    pub height_millimetres: f64,
    pub ceiling_requested: bool,
}
impl VenueRoom {
    /// World-local outline and elevation, suitable for a slab placed at the world origin.
    pub fn ceiling(&self) -> Option<VenueArchitecture> {
        self.ceiling_requested.then(|| VenueArchitecture::Slab {
            purpose: VenueSlabPurpose::Roof,
            outline_millimetres: self
                .outline_millimetres
                .iter()
                .map(|p| VenuePlanPoint {
                    x: p[0].round() as i32,
                    y: p[1].round() as i32,
                })
                .collect(),
            elevation_millimetres: (self.floor_millimetres + self.height_millimetres).round()
                as i32,
            thickness_millimetres: 150,
        })
    }
}
/// Only unambiguous, closed chains on a shared floor qualify as enclosing rooms. Endpoints
/// within 51 mm are joined, covering millimetre rounding after Architect's 50 mm snap.
pub fn venue_rooms(walls: &[VenueWallRun]) -> Vec<VenueRoom> {
    let close = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).hypot(a[1] - b[1]) < 51.0;
    let mut used = std::collections::HashSet::new();
    let mut result = Vec::new();
    for first in walls {
        if used.contains(&first.id) {
            continue;
        }
        let mut traversed = vec![first.id];
        let mut outline = vec![first.start_millimetres];
        let mut current = first.end_millimetres;
        let mut closed = false;
        for _ in 0..walls.len() {
            if close(current, first.start_millimetres) {
                closed = true;
                break;
            }
            outline.push(current);
            let candidates: Vec<_> = walls
                .iter()
                .filter(|w| {
                    !traversed.contains(&w.id)
                        && (w.floor_millimetres - first.floor_millimetres).abs() < 1.0
                        && (close(current, w.start_millimetres)
                            || close(current, w.end_millimetres))
                })
                .collect();
            if candidates.len() != 1 {
                break;
            }
            let next = candidates[0];
            let forward = close(current, next.start_millimetres);
            traversed.push(next.id);
            current = if forward {
                next.end_millimetres
            } else {
                next.start_millimetres
            };
        }
        if !closed || outline.len() < 3 {
            continue;
        }
        let ceiling = walls
            .iter()
            .filter(|w| traversed.contains(&w.id))
            .filter_map(|w| w.ceiling_height_millimetres)
            .min();
        let room = VenueRoom {
            wall_ids: traversed,
            outline_millimetres: outline,
            floor_millimetres: first.floor_millimetres,
            height_millimetres: ceiling.map(f64::from).unwrap_or(first.height_millimetres),
            ceiling_requested: ceiling.is_some(),
        };
        // Reject folded or self-intersecting chains before rendering or exporting a roof.
        let test = VenueArchitecture::Slab {
            purpose: VenueSlabPurpose::Roof,
            outline_millimetres: room
                .outline_millimetres
                .iter()
                .map(|p| VenuePlanPoint {
                    x: p[0].round() as i32,
                    y: p[1].round() as i32,
                })
                .collect(),
            elevation_millimetres: 0,
            thickness_millimetres: 150,
        };
        if test.validate().is_ok() {
            used.extend(room.wall_ids.iter().copied());
            result.push(room);
        }
    }
    result
}
