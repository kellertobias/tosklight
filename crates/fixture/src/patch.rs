use crate::{
    FixtureError, FixtureProfile, MultiPatchInstance, PatchedFixture, PatchedHead, SplitPatch,
};
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

/// Normalize a persisted patched fixture into the schema-v2 portable snapshot and explicit split
/// assignment shape. This is intentionally an explicit reader/migration rather than relying on
/// serde defaults: once written, a show no longer needs either the desk fixture library or the
/// legacy universe/address fallback to understand its patch.
pub fn migrate_patched_fixture_to_v2(fixture: &mut PatchedFixture) -> Result<bool, FixtureError> {
    let original = serde_json::to_value(&*fixture)?;
    if fixture.definition.schema_version == 1 {
        let legacy = fixture.definition.clone();
        let mut profile = FixtureProfile::from_legacy_modes(std::slice::from_ref(&legacy))
            .map_err(|error| FixtureError::Invalid(error.to_string()))?;
        // An embedded snapshot retains the selected legacy revision as its portable identity. The
        // desk library may independently migrate the same source into its own revision sequence.
        profile.revision = legacy.revision.max(1);
        let mode_id = profile
            .modes
            .first()
            .map(|mode| mode.id)
            .ok_or_else(|| FixtureError::Invalid("migrated fixture has no mode".into()))?;
        let mut definition = profile
            .resolved_definition(mode_id)
            .map_err(|error| FixtureError::Invalid(error.to_string()))?;
        // These compatibility projections are still consumed by existing programmer and stage
        // surfaces. Keeping them verbatim avoids a behavior change while schema-v2 runtime paths
        // use the complete embedded profile snapshot.
        definition.heads = legacy.heads;
        definition.color_calibration = legacy.color_calibration;
        definition.physical.pan_range_degrees = legacy.physical.pan_range_degrees;
        definition.physical.tilt_range_degrees = legacy.physical.tilt_range_degrees;
        definition.safe_values = legacy.safe_values;
        fixture.definition = definition;
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
