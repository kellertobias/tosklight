//! Which selected fixtures are members of a whole selected Venue group, and the ink each is marked
//! in, so a group reads apart from one element picked on its own.

use super::FrameStyle;
use glam::Vec3;
use std::collections::HashSet;
use viz_scene::Scene;
use viz_scene::uuid::Uuid;

/// The fixtures selected as members of a whole Venue group: a group every member of which is
/// selected. A group of one reads as the element alone, as it does in the Architect.
pub(crate) fn grouped_selection(scene: &Scene, selected: &HashSet<Uuid>) -> HashSet<Uuid> {
    if selected.is_empty() {
        return HashSet::new();
    }
    scene
        .venue_groups
        .iter()
        .filter(|members| members.len() > 1 && members.iter().all(|id| selected.contains(id)))
        .flatten()
        .copied()
        .collect()
}

/// The ink a selected fixture is marked in: the group ink for a member of a selected group.
pub(crate) fn selection_ink(style: &FrameStyle, grouped: &HashSet<Uuid>, fixture_id: Uuid) -> Vec3 {
    if grouped.contains(&fixture_id) {
        style.group_selected_ink
    } else {
        style.selected_ink
    }
}
