//! One controller's preparation and the ways it is run: the frame's own sorted loop, and the
//! output, cache and working lists a parallel worker gives it instead (TL-639 round 5).
use super::*;

/// Where a controller's preparation writes: the frame's family groups, legacy fragments and
/// requirements, or a parallel worker's ordered record of the same (TL-639 round 5).
pub(super) trait PreparationOutput {
    fn append(
        &mut self,
        target: FixtureId,
        owner: ProgrammingOwner,
        samples: Vec<FamilyCompositionSample>,
    );
    fn legacy(&mut self, sample: DynamicRuntimeSample);
    fn require(&mut self, requirement: DynamicFamilyPreparationRequirement);
}

/// The frame's own output.
pub(super) struct FamilyOutput<'a> {
    pub(super) families: &'a mut Vec<DynamicFamilySampleGroup>,
    pub(super) family_buffers: &'a mut Vec<Vec<FamilyCompositionSample>>,
    pub(super) family_indices: &'a mut HashMap<(FixtureId, ProgrammingOwner), usize>,
    pub(super) legacy: &'a mut Vec<DynamicRuntimeSample>,
    pub(super) requirements: &'a mut Vec<DynamicFamilyPreparationRequirement>,
}

impl PreparationOutput for FamilyOutput<'_> {
    fn append(
        &mut self,
        target: FixtureId,
        owner: ProgrammingOwner,
        samples: Vec<FamilyCompositionSample>,
    ) {
        if samples.is_empty() {
            return;
        }
        let index = *self
            .family_indices
            .entry((target, owner))
            .or_insert_with(|| {
                let index = self.families.len();
                self.families.push(DynamicFamilySampleGroup {
                    target,
                    owner,
                    samples: self.family_buffers.pop().unwrap_or_default(),
                });
                index
            });
        self.families[index].samples.extend(samples);
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
    pub(super) fn take(&mut self, key: &CacheKey) -> Option<CompiledSample> {
        match self {
            Self::Owned(previous) => previous.remove(key),
            Self::Shared(previous) => previous.get(key).cloned(),
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
) {
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
    scratch.sort_keys.sort_unstable();
    scratch.order.clear();
    scratch
        .order
        .extend(scratch.sort_keys.iter().map(|(_, _, index)| *index));
    scratch.controllers.clear();
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
}

pub(super) fn prepare(
    samples: &[DynamicRuntimeSample],
    sources: &dyn DynamicValueSourceResolver,
    native_models: Option<&dyn DynamicNativeModelResolver>,
    previous: &mut HashMap<CacheKey, CompiledSample>,
    scratch: &mut DynamicFamilyPreparationScratch,
) -> Result<(), TransitionError> {
    sort_controllers(samples, scratch);
    let DynamicFamilyPreparationScratch {
        order,
        controllers,
        controller,
        position,
        families,
        family_buffers,
        family_indices,
        legacy,
        requirements,
        sampling_requirements,
        cache,
        ..
    } = scratch;
    let mut out = FamilyOutput {
        families,
        family_buffers,
        family_indices,
        legacy,
        requirements,
    };
    let mut preparer = Preparer {
        controller,
        position,
        sampling_requirements,
        previous: Previous::Owned(previous),
        keep: Keep::Cache(cache),
        out: &mut out,
    };
    for range in controllers.iter() {
        preparer.prepare_controller(samples, &order[range.clone()], sources, native_models)?;
    }
    preparer.controller.clear();
    Ok(())
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
        self.controller.clear();
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
        if has_angles {
            // The forest preserves original axis/lane ownership and Current dependencies even
            // for an ordinary Pan-only effect or an exact Target-to-Angle endpoint.
            self.prepare_position_controller(sources, native_models)
        } else {
            // Pure Target component lanes retain narrow masks and are never promoted to pairs.
            for index in 0..self.controller.len() {
                self.prepare_sample(self.controller[index].clone(), native_models)?;
            }
            Ok(())
        }
    }
}
