//! One controller's preparation and the ways it is run: the frame's own sorted loop, and the
//! output, cache and working lists a parallel worker gives it instead (TL-639 round 5).
use super::*;

/// Where a controller's preparation writes: the frame's family groups, legacy fragments and
/// requirements, or a parallel worker's ordered record of the same (TL-639 round 5).
pub(super) trait PreparationOutput {
    /// One materialized leaf, without a temporary per-leaf vector.
    fn append_one(
        &mut self,
        target: FixtureId,
        owner: ProgrammingOwner,
        sample: FamilyCompositionSample,
    );
    fn append(
        &mut self,
        target: FixtureId,
        owner: ProgrammingOwner,
        samples: Vec<FamilyCompositionSample>,
    );
    fn legacy(&mut self, sample: DynamicRuntimeSample);
    fn require(&mut self, requirement: DynamicFamilyPreparationRequirement);
}

/// The frame's own output. Groups, their index and their sorted order persist between frames
/// (TL-842 C4): each frame writes a group's samples from its first slot on, overwriting last
/// frame's in place, and the preparation then trims what was not rewritten.
pub(super) struct FamilyOutput<'a> {
    pub(super) families: &'a mut Vec<DynamicFamilySampleGroup>,
    pub(super) family_indices: &'a mut HashMap<(FixtureId, ProgrammingOwner), usize>,
    /// Samples written to each group this frame.
    pub(super) cursors: &'a mut Vec<usize>,
    /// Whether this frame created a group (its order must be restored).
    pub(super) created: &'a mut bool,
    /// The group written last, for adjacent writes without a map lookup.
    pub(super) last: Option<usize>,
    pub(super) legacy: &'a mut Vec<DynamicRuntimeSample>,
    pub(super) requirements: &'a mut Vec<DynamicFamilyPreparationRequirement>,
}

impl FamilyOutput<'_> {
    fn group(&mut self, target: FixtureId, owner: ProgrammingOwner) -> usize {
        let adjacent = self.last.filter(|&index| {
            let group = &self.families[index];
            group.target == target && group.owner == owner
        });
        #[cfg(test)]
        let adjacent = adjacent.filter(|_| !super::tests::adjacent_output::forced_map());
        if let Some(index) = adjacent {
            #[cfg(test)]
            super::tests::adjacent_output::hit();
            return index;
        }
        let index = match self.family_indices.entry((target, owner)) {
            std::collections::hash_map::Entry::Occupied(index) => *index.get(),
            std::collections::hash_map::Entry::Vacant(index) => {
                let next = self.families.len();
                index.insert(next);
                self.families.push(DynamicFamilySampleGroup {
                    target,
                    owner,
                    samples: Vec::new(),
                });
                self.cursors.push(0);
                *self.created = true;
                next
            }
        };
        self.last = Some(index);
        index
    }

    fn write(&mut self, index: usize, sample: FamilyCompositionSample) {
        let written = &mut self.cursors[index];
        let samples = &mut self.families[index].samples;
        match samples.get_mut(*written) {
            Some(slot) => *slot = sample,
            None => samples.push(sample),
        }
        *written += 1;
    }
}

impl PreparationOutput for FamilyOutput<'_> {
    fn append_one(
        &mut self,
        target: FixtureId,
        owner: ProgrammingOwner,
        sample: FamilyCompositionSample,
    ) {
        let index = self.group(target, owner);
        self.write(index, sample);
    }

    fn append(
        &mut self,
        target: FixtureId,
        owner: ProgrammingOwner,
        samples: Vec<FamilyCompositionSample>,
    ) {
        if samples.is_empty() {
            return;
        }
        let index = self.group(target, owner);
        for sample in samples {
            self.write(index, sample);
        }
    }

    fn legacy(&mut self, sample: DynamicRuntimeSample) {
        self.legacy.push(sample);
    }

    fn require(&mut self, requirement: DynamicFamilyPreparationRequirement) {
        self.requirements.push(requirement);
    }
}

/// Last frame's compiled samples: owned by the frame (taken as used), or shared read-only by a
/// parallel section's workers (copied as used).
pub(super) enum Previous<'a> {
    Owned(&'a mut HashMap<CacheKey, CompiledSample>),
    Shared(&'a HashMap<CacheKey, CompiledSample>),
}

impl Previous<'_> {
    pub(super) fn take_matching(
        &mut self,
        key: &CacheKey,
        expression: &Arc<DynamicSampleExpression>,
    ) -> Option<CompiledSample> {
        match self {
            Self::Owned(previous) => previous
                .remove(key)
                .filter(|cached| cached.expression == *expression),
            // A changed tree must not allocate/copy its previous samples just to discard them.
            Self::Shared(previous) => previous
                .get(key)
                .filter(|cached| cached.expression == *expression)
                .cloned(),
        }
    }
}

/// This frame's compiled samples: into the frame's cache, or a worker's group log.
pub(super) enum Keep<'a> {
    Cache(&'a mut HashMap<CacheKey, CompiledSample>),
    Log(&'a mut Vec<(CacheKey, CompiledSample)>),
}

/// One controller's preparation: its working lists, last frame's compiled samples, this
/// frame's, and where it writes.
pub(super) struct Preparer<'a, O> {
    pub(super) controller: &'a mut Vec<DynamicRuntimeSample>,
    pub(super) position: &'a mut Vec<DynamicRuntimeSample>,
    pub(super) sampling_requirements: &'a [DynamicFamilyPreparationRequirement],
    pub(super) previous: Previous<'a>,
    pub(super) keep: Keep<'a>,
    pub(super) provenance: super::super::composition::LeafProvenance<'a>,
    pub(super) out: &'a mut O,
}

/// The controller key samples are grouped by: one preparation per instance, controller, target.
pub(super) fn controller_key(sample: &DynamicRuntimeSample) -> (Uuid, Uuid, Uuid) {
    (sample.instance_id, sample.controller_id, sample.target.0)
}

/// Sort the samples by controller key and lane into `scratch.order` and cut it into one range
/// per controller (`scratch.controllers`).
pub(super) fn sort_controllers(
    samples: &[DynamicRuntimeSample],
    scratch: &mut DynamicFamilyPreparationScratch,
    sort: &dyn Fn(&mut [ControllerSortKey]),
    reuse_order: bool,
) {
    let reusable = reuse_order
        && scratch.sort_keys.len() == samples.len()
        && scratch.order.len() == samples.len()
        && if samples.is_empty() {
            scratch.controllers.is_empty()
        } else {
            scratch
                .controllers
                .last()
                .is_some_and(|range| range.end == samples.len())
        }
        && scratch.sort_keys.iter().all(|(key, lane, index)| {
            samples
                .get(*index)
                .is_some_and(|sample| controller_key(sample) == *key && sample.lane_id == *lane)
        });
    #[cfg(test)]
    super::tests::controller_order::record(reusable);
    if reusable {
        return;
    }
    // Invalidate the completed layout before a caller-provided sorter can panic.
    scratch.order.clear();
    scratch.controllers.clear();
    // TL-639 round 4: sorted by keys gathered once rather than read through the samples on every
    // comparison. The index breaks ties; equal keys only occur for a duplicate lane, which the
    // controller's preparation rejects whatever their order.
    scratch.sort_keys.clear();
    scratch.sort_keys.extend(
        samples
            .iter()
            .enumerate()
            .map(|(index, sample)| (controller_key(sample), sample.lane_id, index)),
    );
    sort(&mut scratch.sort_keys);
    scratch
        .order
        .extend(scratch.sort_keys.iter().map(|(_, _, index)| *index));
    let mut start = 0;
    while start < scratch.order.len() {
        let first = controller_key(&samples[scratch.order[start]]);
        let mut end = start + 1;
        while end < scratch.order.len() && controller_key(&samples[scratch.order[end]]) == first {
            end += 1;
        }
        scratch.controllers.push(start..end);
        start = end;
    }
    // A custom/opt-out sorter may establish a noncanonical order. Never lend it to
    // a later canonical caller, even when that caller sees identical identities.
    if !reuse_order {
        scratch.sort_keys.clear();
    }
}

pub(super) fn prepare(
    samples: &[DynamicRuntimeSample],
    sources: &dyn DynamicValueSourceResolver,
    native_models: Option<&dyn DynamicNativeModelResolver>,
    previous: &mut HashMap<CacheKey, CompiledSample>,
    scratch: &mut DynamicFamilyPreparationScratch,
) -> Result<(), TransitionError> {
    sort_controllers(samples, scratch, &|keys| keys.sort_unstable(), true);
    let DynamicFamilyPreparationScratch {
        order,
        controllers,
        controller,
        position,
        families,
        family_indices,
        family_cursors,
        families_created,
        legacy,
        requirements,
        sampling_requirements,
        cache,
        provenance,
        ..
    } = scratch;
    let mut out = FamilyOutput {
        families,
        family_indices,
        cursors: family_cursors,
        created: families_created,
        last: None,
        legacy,
        requirements,
    };
    let mut preparer = Preparer {
        controller,
        position,
        sampling_requirements,
        previous: Previous::Owned(previous),
        keep: Keep::Cache(cache),
        provenance: provenance.owned(),
        out: &mut out,
    };
    for range in controllers.iter() {
        preparer.prepare_controller(samples, &order[range.clone()], sources, native_models)?;
    }
    preparer.controller.clear();
    Ok(())
}

/// A successful preparation's source activity; only the exact plain Color branch proves none.
#[derive(Clone, Copy)]
pub(super) enum ControllerPreparation {
    SourceFree,
    MayQuery,
}

impl<O: PreparationOutput> Preparer<'_, O> {
    /// Prepare one controller's samples (`indices`, one instance, controller and target).
    pub(super) fn prepare_controller(
        &mut self,
        samples: &[DynamicRuntimeSample],
        indices: &[usize],
        sources: &dyn DynamicValueSourceResolver,
        native_models: Option<&dyn DynamicNativeModelResolver>,
    ) -> Result<(), TransitionError> {
        self.prepare_controller_classified(samples, indices, sources, native_models)
            .map(|_| ())
    }

    pub(super) fn prepare_controller_classified(
        &mut self,
        samples: &[DynamicRuntimeSample],
        indices: &[usize],
        sources: &dyn DynamicValueSourceResolver,
        native_models: Option<&dyn DynamicNativeModelResolver>,
    ) -> Result<ControllerPreparation, TransitionError> {
        self.controller.clear();
        // Shape inspection is pure: generic pruning/validation retains its original order.
        let borrowed_plain = indices.iter().all(|index| {
            samples
                .get(*index)
                .is_some_and(|sample| plain_scalar::leaf(&sample.expression).is_some())
        });
        #[cfg(test)]
        let borrowed_plain =
            borrowed_plain && !super::tests::borrowed_plain_controller::forced_old();
        if borrowed_plain {
            let mut last_lane = None;
            // Validate the entire controller before any materialization or output, as before.
            for &index in indices {
                let sample = &samples[index];
                address::ensure(
                    last_lane != Some(sample.lane_id),
                    "duplicate Dynamic source lane",
                )?;
                address::ensure(
                    sample.activation_mix.is_finite()
                        && (0.0..=1.0).contains(&sample.activation_mix),
                    "Dynamic activation influence must be between zero and one",
                )?;
                sample.expression.validate()?;
                last_lane = Some(sample.lane_id);
            }
            for &index in indices {
                let sample = &samples[index];
                self.prepare_plain_scalar(
                    sample,
                    plain_scalar::leaf(&sample.expression).expect("selected Color leaf"),
                )?;
            }
            return Ok(ControllerPreparation::SourceFree);
        }
        let mut last_lane = None;
        let mut has_angles = false;
        for &index in indices {
            let sample = &samples[index];
            address::ensure(
                last_lane != Some(sample.lane_id),
                "duplicate Dynamic source lane",
            )?;
            address::ensure(
                sample.activation_mix.is_finite() && (0.0..=1.0).contains(&sample.activation_mix),
                "Dynamic activation influence must be between zero and one",
            )?;
            sample.expression.validate()?;
            last_lane = Some(sample.lane_id);
            if is_plain_leaf(&sample.expression) {
                // TL-639: a plain leaf has no exact branch to prune.
                has_angles |= sample.expression.contains_angles();
                self.controller.push(sample.clone());
            } else if let Some(expression) =
                prune_exact_branches(Arc::new(sample.expression.clone()))?
            {
                let mut sample = sample.clone();
                sample.expression = expression.as_ref().clone();
                has_angles |= sample.expression.contains_angles();
                self.controller.push(sample);
            }
        }
        let prepared = if has_angles {
            // The forest preserves original axis/lane ownership and Current dependencies even
            // for an ordinary Pan-only effect or an exact Target-to-Angle endpoint.
            self.prepare_position_controller(sources, native_models)
        } else {
            // Pure Target component lanes retain narrow masks and are never promoted to pairs.
            // TL-639 round 7: the samples move out of the controller buffer (cleared by the next
            // controller), and a legacy scalar leaf, its own only projection (`split_owners`),
            // passes straight through as its legacy fragment.
            let mut controller = std::mem::take(&mut *self.controller);
            let prepared = controller.drain(..).try_for_each(|sample| {
                if matches!(
                    sample.expression,
                    DynamicSampleExpression::LegacyScalar { .. }
                ) {
                    self.out.legacy(DynamicRuntimeSample {
                        address: None,
                        ..sample
                    });
                    Ok(())
                } else {
                    self.prepare_sample(sample, native_models)
                }
            });
            *self.controller = controller;
            prepared
        };
        prepared.map(|()| ControllerPreparation::MayQuery)
    }
}
