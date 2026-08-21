use crate::{FixtureError, MultiPatchInstance, PatchedFixture, PatchedHead, SplitPatch};
use light_core::FixtureId;
use std::collections::HashMap;

/// Rebuild the persisted logical-head mapping from the active fixture definition.
/// Existing IDs are retained by definition head index so programming remains stable.
pub fn reconcile_logical_heads(fixture: &mut PatchedFixture) -> bool {
    let before = fixture.logical_heads.clone();
    let mut existing = fixture
        .logical_heads
        .drain(..)
        .map(|head| (head.head_index, (head.fixture_id, head.profile_head_id)))
        .collect::<HashMap<_, _>>();
    fixture.logical_heads = fixture
        .definition
        .heads
        .iter()
        .filter(|head| !head.shared)
        .map(|head| {
            let (fixture_id, profile_head_id) = existing
                .remove(&head.index)
                .unwrap_or_else(|| (FixtureId::new(), None));
            PatchedHead {
                profile_head_id,
                head_index: head.index,
                fixture_id,
            }
        })
        .collect();
    before != fixture.logical_heads
}

impl PatchedFixture {
    pub fn effective_split_patches(&self) -> Vec<SplitPatch> {
        if self.split_patches.is_empty() {
            vec![SplitPatch {
                split: 1,
                universe: self.universe,
                address: self.address,
            }]
        } else {
            self.split_patches.clone()
        }
    }
}

impl MultiPatchInstance {
    pub fn effective_split_patches(&self) -> Vec<SplitPatch> {
        if self.split_patches.is_empty() {
            vec![SplitPatch {
                split: 1,
                universe: self.universe,
                address: self.address,
            }]
        } else {
            self.split_patches.clone()
        }
    }
}

/// Normalize a persisted patched fixture into the current portable snapshot and explicit split
/// assignment shape. This is intentionally an explicit reader rather than relying on serde
/// defaults: once written, a show no longer needs either the desk fixture library or a
/// universe/address fallback to understand its patch.
pub fn migrate_patched_fixture_to_v2(fixture: &mut PatchedFixture) -> Result<bool, FixtureError> {
    let original = serde_json::to_value(&*fixture)?;
    if fixture.definition.schema_version == 2 {
        // Schema-2 definitions already carry the authoritative profile snapshot. Its custom
        // deserializer migrates the nested profile to the current schema before this boundary is
        // reached, so promote the outer marker as well. Keeping the definition projections
        // untouched makes this lossless until the show compiler materializes the snapshot as an
        // immutable profile revision and replaces the inline definition with a lean reference.
        fixture.definition.schema_version = crate::FIXTURE_PROFILE_SCHEMA_VERSION;
    }

    let splits = fixture.definition.split_footprints();
    if fixture.split_patches.is_empty() && splits.len() == 1 {
        fixture.split_patches = splits
            .keys()
            .enumerate()
            .map(|(index, split)| SplitPatch {
                split: *split,
                universe: (index == 0).then_some(fixture.universe).flatten(),
                address: (index == 0).then_some(fixture.address).flatten(),
            })
            .collect();
    }
    for instance in &mut fixture.multipatch {
        if instance.split_patches.is_empty() && splits.len() == 1 {
            instance.split_patches = splits
                .keys()
                .enumerate()
                .map(|(index, split)| SplitPatch {
                    split: *split,
                    universe: (index == 0).then_some(instance.universe).flatten(),
                    address: (index == 0).then_some(instance.address).flatten(),
                })
                .collect();
        }
    }
    reconcile_logical_heads(fixture);
    fixture.definition.validate()?;
    let normalized = serde_json::to_value(&*fixture)?;
    Ok(normalized != original)
}
