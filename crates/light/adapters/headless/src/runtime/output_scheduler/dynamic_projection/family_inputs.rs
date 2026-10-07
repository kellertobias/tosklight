//! One family candidate set contains both running Dynamics and captured Fixed masks.
//! Requirements travel with the same target/owner, including when no candidate can evaluate.
//! This is input assembly, not a visibility policy: the compositor/publisher must decide which
//! unavailable contributions remain relevant after output gates and higher masks are applied.
#![allow(dead_code)]

use super::fixed_masks::{CompiledFixedMaskState, FixedMaskRequirement, PreparedFixedMask};
use light_core::{FixtureId, programming::ProgrammingOwner};
use light_dynamics::{
    DynamicFamilyPreparationRequirement, DynamicFamilySampleGroup, DynamicSourceOccurrenceId,
    FamilySampleRank, PreparedDynamicFamilySamples, ProgrammingFamilyFixAt,
};
use rustc_hash::FxHashMap;

#[derive(Clone)]
pub(super) enum CapturedFamilyRequirement {
    Dynamic(DynamicFamilyPreparationRequirement),
    Fixed {
        rank: FamilySampleRank,
        occurrence: Option<DynamicSourceOccurrenceId>,
        mask: ProgrammingFamilyFixAt,
        activation_mix: f32,
        reason: FixedMaskRequirement,
    },
}

pub(super) struct CapturedFamilyInput {
    pub group: DynamicFamilySampleGroup,
    /// A group with only requirements is intentional. Dropping it would silently lose the
    /// operator's Fixed mask or make an unavailable complete Angle pair look like no input.
    pub requirements: Vec<CapturedFamilyRequirement>,
}

#[derive(Default)]
pub(super) struct CapturedFamilyInputScratch {
    groups: Vec<CapturedFamilyInput>,
    spare: Vec<CapturedFamilyInput>,
    indices: FxHashMap<(FixtureId, ProgrammingOwner), usize>,
}

impl CapturedFamilyInputScratch {
    /// Empty every group. With `pool`, the samples (holding the last references to last frame's
    /// compiled expressions) are freed on a pool thread (TL-639 round 5).
    pub fn clear(&mut self, pool: Option<&light_engine::parallel::OutputPool>) {
        let mut garbage = Vec::new();
        for mut entry in self.groups.drain(..) {
            match pool {
                Some(_) => garbage.append(&mut entry.group.samples),
                None => entry.group.samples.clear(),
            }
            entry.requirements.clear();
            self.spare.push(entry);
        }
        self.indices.clear();
        if let Some(pool) = pool.filter(|_| !garbage.is_empty()) {
            pool.drop_later(garbage);
        }
    }

    /// A Fixture Freeze holds these owners' parameters: no Dynamic or Fixed mask drives them, so
    /// their group stays and renders the (frozen) static baseline.
    pub fn hold_frozen(&mut self, frozen: &rustc_hash::FxHashSet<(FixtureId, ProgrammingOwner)>) {
        if frozen.is_empty() {
            return;
        }
        for entry in &mut self.groups {
            if frozen.contains(&(entry.group.target, entry.group.owner)) {
                entry.group.samples.clear();
                entry.requirements.clear();
            }
        }
    }

    /// Add only genuinely static owners. A requirement-only Dynamic group remains a
    /// requirement; it must never be silently replaced by its underlying static value.
    pub fn with_static_targets(
        &mut self,
        targets: &[(FixtureId, ProgrammingOwner)],
    ) -> (&[CapturedFamilyInput], Vec<(FixtureId, ProgrammingOwner)>) {
        let mut added = Vec::new();
        for &(target, owner) in targets {
            if !self.indices.contains_key(&(target, owner)) {
                self.group(target, owner);
                added.push((target, owner));
            }
        }
        (&self.groups, added)
    }

    fn group(&mut self, target: FixtureId, owner: ProgrammingOwner) -> &mut CapturedFamilyInput {
        let index = *self.indices.entry((target, owner)).or_insert_with(|| {
            let mut group = self.spare.pop().unwrap_or_else(|| CapturedFamilyInput {
                group: DynamicFamilySampleGroup {
                    target,
                    owner,
                    samples: Vec::new(),
                },
                requirements: Vec::new(),
            });
            group.group.target = target;
            group.group.owner = owner;
            let index = self.groups.len();
            self.groups.push(group);
            index
        });
        &mut self.groups[index]
    }
}

/// Inputs must come from the matching completed sampling proof and Fixed compilation in one
/// frame transaction. This never resamples, reads Current, changes retained history, or applies
/// masters. Scalar fragments already went through stage one and are deliberately not emitted.
/// Disabled/delayed Fixed rows have already been validated by the compiler; they do not mask
/// anything or produce notices until they actually participate.
pub(super) fn assemble_captured_family_inputs<'a>(
    dynamic: &PreparedDynamicFamilySamples<'_>,
    fixed: &[PreparedFixedMask],
    scratch: &'a mut CapturedFamilyInputScratch,
    pool: Option<&light_engine::parallel::OutputPool>,
) -> &'a [CapturedFamilyInput] {
    scratch.clear(pool);
    for group in dynamic.families {
        scratch
            .group(group.target, group.owner)
            .group
            .samples
            .extend(group.samples.iter().cloned());
    }
    for requirement in dynamic.requirements {
        scratch
            .group(requirement.target, requirement.owner)
            .requirements
            .push(CapturedFamilyRequirement::Dynamic(requirement.clone()));
    }
    for row in fixed.iter().filter(|row| row.participates()) {
        let group = scratch.group(row.target, row.owner);
        match &row.state {
            CompiledFixedMaskState::Ready(sample) => {
                group.group.samples.push(sample.clone().into())
            }
            CompiledFixedMaskState::Requires(reason) => {
                group.requirements.push(CapturedFamilyRequirement::Fixed {
                    rank: row.rank,
                    occurrence: row.occurrence,
                    mask: row.mask.clone(),
                    activation_mix: row.authored_activation_mix,
                    reason: reason.clone(),
                });
            }
        }
    }
    // Stable group traversal does not define arbitration. The compositor retains each exact
    // source rank, including Cue submillisecond recency and complete Position cohorts.
    scratch
        .groups
        .sort_unstable_by_key(|entry| (entry.group.target.0, entry.group.owner.id()));
    // Retain only this frame's group capacities. A large removed rig does not retain source
    // expressions or an ever-growing inventory of spare groups.
    scratch.spare.clear();
    &scratch.groups
}

#[cfg(test)]
mod tests;
