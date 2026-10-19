use super::*;

/// A one-consumer handoff of this frame's prepared family output. Moving each group's samples
/// empties only that output vector; compiled caches, legacy samples and requirements stay owned
/// by the preparation scratch. Group records and empty vector storage survive until retirement.
pub struct DrainingPreparedDynamicFamilySamples<'a> {
    pub families: &'a mut [DynamicFamilySampleGroup],
    pub requirements: &'a [DynamicFamilyPreparationRequirement],
}

impl DynamicFamilyPreparationScratch {
    /// The last preparation's result.
    pub fn prepared(&self) -> PreparedDynamicFamilySamples<'_> {
        PreparedDynamicFamilySamples {
            families: &self.families,
            legacy: &self.legacy,
            requirements: &self.requirements,
        }
    }

    /// Lend the completed family output for a consuming handoff. The caller must not expect to
    /// read its samples through `prepared()` afterwards. The compiled cache is not lent.
    pub fn prepared_for_drain(&mut self) -> DrainingPreparedDynamicFamilySamples<'_> {
        DrainingPreparedDynamicFamilySamples {
            families: &mut self.families,
            requirements: &self.requirements,
        }
    }

    /// The prepared groups again, so a consumer that took their sample vectors from
    /// [`Self::prepared_for_drain`] by swapping can swap them back (TL-842 C4). Returned
    /// vectors still hold this frame's samples; the next preparation rewrites them in place.
    pub fn families_for_return(&mut self) -> &mut [DynamicFamilySampleGroup] {
        &mut self.families
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{Sources, sample, whole};
    use super::*;

    #[test]
    fn draining_output_preserves_cache_requirements_legacy_and_next_preparation() {
        let samples = [
            sample(
                1,
                whole(ProgrammingOwner::Focus, AttributeValue::Normalized(0.6)),
            ),
            sample(
                2,
                crate::DynamicSampleExpression::LegacyScalar {
                    attribute: light_core::AttributeKey("intensity".into()),
                    value: 0.5,
                    occurrence: None,
                    dependency_occurrence: None,
                },
            ),
        ];
        let requirement = DynamicFamilyPreparationRequirement {
            target: samples[0].target,
            owner: ProgrammingOwner::Position,
            rank: rank(&samples[0]),
            reason: DynamicFamilyPreparationRequirementReason::Transition(
                TransitionRequirement::LiveJointAngles,
            ),
        };
        let mut scratch = DynamicFamilyPreparationScratch::default();
        let sources = Sources::default();
        for _ in 0..3 {
            prepare_dynamic_family_samples_with_requirements(
                &samples,
                std::slice::from_ref(&requirement),
                &sources,
                None,
                &mut scratch,
            )
            .unwrap();
            assert_eq!(scratch.legacy.len(), 1);
            assert!(!scratch.cache.is_empty());
            let cache = scratch
                .cache
                .iter()
                .map(|(key, entry)| (*key, entry.expression.clone(), entry.samples.len()))
                .collect::<Vec<_>>();
            let groups = scratch.families.len();
            let mut moved = Vec::new();
            {
                let view = scratch.prepared_for_drain();
                assert_eq!(view.requirements, [requirement.clone()]);
                for group in view.families {
                    moved.append(&mut group.samples);
                }
            }
            assert!(!moved.is_empty());
            assert_eq!(scratch.families.len(), groups);
            assert!(
                scratch
                    .families
                    .iter()
                    .all(|group| group.samples.is_empty())
            );
            assert_eq!(scratch.legacy.len(), 1);
            assert_eq!(scratch.requirements, [requirement.clone()]);
            assert_eq!(scratch.cache.len(), cache.len());
            for (key, expression, samples) in cache {
                let retained = &scratch.cache[&key];
                assert!(Arc::ptr_eq(&expression, &retained.expression));
                assert_eq!(retained.samples.len(), samples);
            }
        }
    }
}
