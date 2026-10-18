//! One authoritative phase value per unique key. Dense cells contain row indices only.
use super::*;

type Key = (Uuid, FixtureId);
const MISSING: u32 = u32::MAX;
const MAX_CELLS: usize = 65_536;

pub(super) struct PhaseTable {
    rows: Vec<(Key, f32)>,
    dense: Option<DenseIndex>,
}
struct DenseIndex {
    lanes: Vec<Uuid>,
    targets: Vec<FixtureId>,
    indices: Vec<u32>,
}

// Reject excessive or sparse axes before allocating axis vectors or the dense index.
fn dense_cells(lanes: usize, targets: usize, rows: usize) -> Option<usize> {
    let cells = lanes.checked_mul(targets)?;
    (cells != 0
        && cells <= MAX_CELLS
        && rows <= u32::MAX as usize
        && cells <= rows.saturating_mul(4))
    .then_some(cells)
}
impl PhaseTable {
    pub(super) fn new(
        phases: HashMap<Key, f32>,
        definition: &DynamicDefinition,
        targets: &[FixtureId],
    ) -> Self {
        // The caller's unchanged std-map projector/restorer already resolved duplicate keys.
        let mut rows = phases.into_iter().collect::<Vec<_>>();
        rows.sort_unstable_by_key(|((lane, target), _)| (*lane, target.0));
        let mut table = Self { rows, dense: None };
        if let Some(cells) = dense_cells(definition.lanes.len(), targets.len(), table.rows.len()) {
            let mut dense = DenseIndex {
                lanes: definition.lanes.iter().map(|lane| lane.id).collect(),
                targets: targets.to_vec(),
                indices: vec![MISSING; cells],
            };
            for (target_index, target) in targets.iter().copied().enumerate() {
                for (lane_index, lane) in dense.lanes.iter().copied().enumerate() {
                    if let Ok(row) = table.find(&(lane, target)) {
                        dense.indices[target_index * dense.lanes.len() + lane_index] = row as u32;
                    }
                }
            }
            table.dense = Some(dense);
        }
        table
    }

    fn find(&self, key: &Key) -> Result<usize, usize> {
        self.rows
            .binary_search_by_key(&(key.0, key.1.0), |((lane, target), _)| (*lane, target.0))
    }
    pub(super) fn get(&self, key: &Key) -> Option<&f32> {
        self.find(key).ok().map(|row| &self.rows[row].1)
    }
    pub(super) fn iter(&self) -> impl ExactSizeIterator<Item = (&Key, &f32)> {
        self.rows.iter().map(|(key, phase)| (key, phase))
    }
    pub(super) fn get_indexed(
        &self,
        lane_index: usize,
        lane: Uuid,
        target_index: usize,
        target: FixtureId,
    ) -> Option<&f32> {
        if let Some(dense) = &self.dense
            && dense.lanes.get(lane_index) == Some(&lane)
            && dense.targets.get(target_index) == Some(&target)
        {
            let row = dense.indices[target_index * dense.lanes.len() + lane_index];
            return (row != MISSING).then(|| &self.rows[row as usize].1);
        }
        // Old tables remain valid through definition/target reorder and historical restores.
        self.get(&(lane, target))
    }
    pub(super) fn matches_projected(&self, phases: &HashMap<Key, f32>) -> bool {
        self.rows.len() == phases.len()
            && self
                .rows
                .iter()
                .all(|(key, phase)| phases.get(key) == Some(phase))
    }
}

#[cfg(test)]
mod tests;
