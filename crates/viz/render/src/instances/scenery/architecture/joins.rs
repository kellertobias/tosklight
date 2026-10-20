//! At most one effective posed wall generation is retained; camera/light frames reuse its mesh.
use super::*;
use light_fixture::{VenueGeometry, VenueWallPlacement};
use std::{cell::RefCell, collections::BTreeMap, sync::Arc};
type Meshes = Arc<BTreeMap<Uuid, VenueGeometry>>;
#[derive(Default)]
pub(super) struct Cache {
    walls: Vec<VenueWallPlacement>,
    meshes: Meshes,
    pub builds: u64,
}
impl Cache {
    pub fn resolve(&mut self, mut walls: Vec<VenueWallPlacement>) -> Meshes {
        walls.sort_by_key(|wall| wall.id);
        if walls != self.walls {
            self.meshes = Arc::new(light_fixture::venue_joined_walls(&walls));
            self.walls = walls;
            self.builds += 1;
        }
        self.meshes.clone()
    }
}
thread_local! {static CACHE:RefCell<Cache>=RefCell::new(Cache::default());}
pub(super) fn key(index: usize, object: &SceneryObject) -> Uuid {
    if object.id.is_nil() {
        Uuid::from_u128(index as u128 + 1)
    } else {
        object.id
    }
}
pub(super) fn resolve(objects: &[SceneryObject]) -> Meshes {
    let walls = objects
        .iter()
        .enumerate()
        .filter_map(|(index, object)| {
            let Some(VenueArchitecture::Wall { openings, .. }) = &object.architecture else {
                return None;
            };
            Some(VenueWallPlacement {
                id: key(index, object),
                size_millimetres: object.size.as_dvec3().to_array().map(|v| v * 1000.0),
                local_to_world: millimetre_pose(object),
                openings: openings.clone(),
            })
        })
        .collect();
    CACHE.with_borrow_mut(|cache| cache.resolve(walls))
}
#[cfg(test)]
mod tests;
