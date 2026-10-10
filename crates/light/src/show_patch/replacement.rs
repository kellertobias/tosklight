//! Explicit replacement topology. Normal mode changes continue matching authored head IDs;
//! physical replacement may preserve a logical target only by an operator-supplied mapping.
use super::profiles::{ResolvedLogicalHead, ResolvedProfiles};
use super::record_index::StoredFixtureRecords;
use super::{
    PatchFixtureUpdateAction, PatchFixtureUpdateIntent, PatchFixturesCommand, PatchHeadReplacement,
};
use crate::{ActionError, ActionErrorKind};
use light_core::FixtureId;
use light_fixture::{PatchedHead, SplitPatch};
use std::collections::{HashMap, HashSet};

pub(super) fn reconcile_replacements(
    stored: &StoredFixtureRecords,
    profiles: &ResolvedProfiles,
    expanded: &mut PatchFixturesCommand,
    updates: &[PatchFixtureUpdateIntent],
) -> Result<(), ActionError> {
    for update in updates {
        let PatchFixtureUpdateAction::ReplaceProfile {
            profile,
            head_mapping,
            ..
        } = &update.action
        else {
            continue;
        };
        let input = expanded
            .fixtures
            .iter_mut()
            .find(|f| f.patch.fixture_id == update.fixture_id)
            .ok_or_else(|| invalid("replacement fixture is missing"))?;
        let existing = stored
            .get(update.fixture_id)
            .ok_or_else(|| invalid("replacement fixture does not exist"))?
            .record
            .patch()
            .map_err(|e| invalid(e.to_string()))?;
        let mode = profiles.mode(*profile)?;
        // Validate consent before any candidate is published. Stored stable heads, not a UI
        // definition projection, are the authority for existing programming targets.
        replace_heads(mode.logical_heads(), existing.logical_heads, head_mapping)?;
        let splits = &mode.projection().splits;
        reconcile_owner(
            &mut input.patch.split_patches,
            &mut input.patch.universe,
            &mut input.patch.address,
            splits,
            mode.dmx_patchable,
        );
        for copy in &mut input.patch.multipatch {
            reconcile_owner(
                &mut copy.split_patches,
                &mut copy.universe,
                &mut copy.address,
                splits,
                mode.dmx_patchable,
            );
        }
        input.patch.highlight_overrides.retain(|id, raw| {
            mode.channel_limits
                .get(id)
                .is_some_and(|limit| *raw <= *limit)
        });
    }
    Ok(())
}

fn reconcile_owner(
    patches: &mut Vec<SplitPatch>,
    universe: &mut Option<u16>,
    address: &mut Option<u16>,
    splits: &[light_fixture::FixtureSplit],
    dmx: bool,
) {
    let old: HashMap<_, _> = patches.iter().map(|p| (p.split, *p)).collect();
    *patches = splits
        .iter()
        .map(|split| {
            if dmx {
                old.get(&split.number).copied().unwrap_or(SplitPatch {
                    split: split.number,
                    universe: if split.number == 1 { *universe } else { None },
                    address: if split.number == 1 { *address } else { None },
                })
            } else {
                SplitPatch {
                    split: split.number,
                    universe: None,
                    address: None,
                }
            }
        })
        .collect();
    let primary = patches
        .iter()
        .find(|p| p.split == 1)
        .or_else(|| patches.first());
    *universe = primary.and_then(|p| p.universe);
    *address = primary.and_then(|p| p.address);
}

pub(super) fn replace_heads(
    target: &[ResolvedLogicalHead],
    existing: Vec<PatchedHead>,
    mapping: &[PatchHeadReplacement],
) -> Result<Vec<PatchedHead>, ActionError> {
    let old: HashSet<_> = existing.iter().map(|h| h.fixture_id).collect();
    let target_ids: HashSet<_> = target.iter().map(|h| h.profile_head_id).collect();
    let mut seen_old = HashSet::new();
    let mut assigned = HashMap::new();
    for entry in mapping {
        if !old.contains(&entry.fixture_id) || !seen_old.insert(entry.fixture_id) {
            return Err(invalid(
                "head_mapping must name each existing logical head exactly once",
            ));
        }
        if let Some(id) = entry.target_profile_head_id {
            if !target_ids.contains(&id) || assigned.insert(id, entry.fixture_id).is_some() {
                return Err(invalid(
                    "head_mapping target must be a unique logical head in the replacement mode",
                ));
            }
        }
    }
    if seen_old != old {
        return Err(invalid(
            "choose a replacement or explicitly leave unmatched for every existing logical head",
        ));
    }
    Ok(target
        .iter()
        .map(|head| PatchedHead {
            profile_head_id: Some(head.profile_head_id),
            head_index: head.head_index,
            fixture_id: assigned
                .remove(&head.profile_head_id)
                .unwrap_or_else(FixtureId::new),
        })
        .collect())
}
fn invalid(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;
    #[test]
    fn replacement_head_mapping_is_explicit_and_order_independent() {
        let old = FixtureId(Uuid::from_u128(1));
        let second = FixtureId(Uuid::from_u128(2));
        let a = Uuid::from_u128(3);
        let b = Uuid::from_u128(4);
        let existing = vec![
            PatchedHead {
                fixture_id: old,
                profile_head_id: Some(Uuid::from_u128(5)),
                head_index: 0,
            },
            PatchedHead {
                fixture_id: second,
                profile_head_id: Some(Uuid::from_u128(6)),
                head_index: 1,
            },
        ];
        let target = vec![
            ResolvedLogicalHead {
                profile_head_id: b,
                head_index: 0,
            },
            ResolvedLogicalHead {
                profile_head_id: a,
                head_index: 1,
            },
        ];
        let mapping = vec![
            PatchHeadReplacement {
                fixture_id: old,
                target_profile_head_id: Some(a),
            },
            PatchHeadReplacement {
                fixture_id: second,
                target_profile_head_id: Some(b),
            },
        ];
        let result = replace_heads(&target, existing.clone(), &mapping).unwrap();
        assert_eq!(result[0].fixture_id, second);
        assert_eq!(result[1].fixture_id, old);
        assert!(replace_heads(&target, existing.clone(), &mapping[..1]).is_err());
        let mut duplicate = mapping.clone();
        duplicate[1].target_profile_head_id = Some(a);
        assert!(replace_heads(&target, existing.clone(), &duplicate).is_err());
        let mut unmatched = mapping;
        unmatched[1].target_profile_head_id = None;
        let result = replace_heads(&target, existing, &unmatched).unwrap();
        assert_eq!(result[1].fixture_id, old);
        assert_ne!(result[0].fixture_id, second);
    }
}
