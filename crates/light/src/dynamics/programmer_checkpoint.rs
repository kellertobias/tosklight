//! Restoring a persisted Dynamic runtime checkpoint against the retained Programmer source.

use light_dynamics::{DynamicRuntimeError, DynamicRuntimeSnapshot};
use light_programmer::ProgrammerRegistry;

/// Rewrites legacy Programmer controller ids in `snapshot` to the instance links the retained
/// Programmer Dynamic source still owns. Returns how many controllers were normalized.
pub fn normalize_programmer_dynamic_checkpoint(
    programmers: &ProgrammerRegistry,
    snapshot: &mut DynamicRuntimeSnapshot,
) -> Result<usize, DynamicRuntimeError> {
    let mut links = Vec::new();
    if let Some((programmer, _, normal, active)) = programmers.retained_dynamic_source() {
        for row in normal.iter().chain(active.iter()) {
            if let Some(link) = row.value.track_key().instance_link {
                links.push((light_core::ProgrammerId(programmer), link));
            }
        }
    }
    light_dynamics::normalize_legacy_programmer_controller_ids(snapshot, &links)
}
