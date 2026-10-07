//! Adapt immutable Playback history once, without making Playback depend on engine types.
use crate::{
    ContributionFamilyEntry, ContributionFamilyEvidence, ContributionFamilyFootprint,
    ContributionFamilyRole, ContributionSourceId,
};
use light_core::ProgrammerEditStamp;
use light_playback::{PlaybackFamilyEvidence, PlaybackFamilyFootprint, PlaybackFamilyRole};
use rustc_hash::FxHashMap;
use std::sync::{Arc, Weak};

#[derive(Default)]
pub(crate) struct PlaybackEvidenceCache {
    entries: FxHashMap<usize, CachedEvidence>,
}

struct CachedEvidence {
    // Reserve allocation identity without retaining Playback frames or their history.
    source: Weak<PlaybackFamilyEvidence>,
    projected: Arc<ContributionFamilyEvidence>,
    used: bool,
}

impl PlaybackEvidenceCache {
    pub(crate) fn project(
        &mut self,
        evidence: &Arc<PlaybackFamilyEvidence>,
    ) -> Arc<ContributionFamilyEvidence> {
        let key = Arc::as_ptr(evidence) as usize;
        if let Some(cached) = self.entries.get_mut(&key) {
            debug_assert_eq!(cached.source.as_ptr(), Arc::as_ptr(evidence));
            cached.used = true;
            return Arc::clone(&cached.projected);
        }
        let entries = evidence
            .entries()
            .iter()
            .map(|entry| {
                ContributionFamilyEntry::new(
                    ContributionSourceId::playback(entry.occurrence.source),
                    ProgrammerEditStamp {
                        changed_at: entry.occurrence.action_changed_at,
                        programmer_order: 0,
                    },
                    match entry.footprint {
                        PlaybackFamilyFootprint::Whole => ContributionFamilyFootprint::Whole,
                        PlaybackFamilyFootprint::Component(component) => {
                            ContributionFamilyFootprint::Component(component)
                        }
                    },
                    match entry.role {
                        PlaybackFamilyRole::Authored => ContributionFamilyRole::Authored,
                        PlaybackFamilyRole::CalculationDependency => {
                            ContributionFamilyRole::CalculationDependency
                        }
                    },
                )
                .with_transition_ordinal(Some(entry.occurrence.action_ordinal))
                .with_authored_cue_id(entry.occurrence.authored_cue_id)
                .with_effective_fields(entry.effective_fields.clone())
            })
            .collect::<Vec<_>>();
        let projected = Arc::new(ContributionFamilyEvidence::new(entries));
        self.entries.insert(
            key,
            CachedEvidence {
                source: Arc::downgrade(evidence),
                projected: Arc::clone(&projected),
                used: true,
            },
        );
        projected
    }

    /// This cache is a projection optimization, not retained ownership. Bound it by the latest
    /// sampled frame even when other runtime objects keep much older source history alive.
    pub(crate) fn finish_frame(&mut self) {
        self.entries.retain(|_, cached| {
            let used = cached.used;
            cached.used = false;
            used
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use light_core::{
        CueListId,
        programming::{
            ColorComponent, ProgrammingComponent, ProgrammingFieldScope,
            ProgrammingTraceField as Field,
        },
    };
    use light_playback::{PlaybackFamilyEntry, PlaybackSourceOccurrence, SequenceMasterSource};
    use uuid::Uuid;

    fn evidence(cue: Uuid) -> Arc<PlaybackFamilyEvidence> {
        Arc::new(
            PlaybackFamilyEvidence::try_new(vec![PlaybackFamilyEntry {
                occurrence: PlaybackSourceOccurrence {
                    source: SequenceMasterSource {
                        playback_number: None,
                        playback_identity: None,
                        cue_list_id: CueListId::new(),
                        temporary: true,
                    },
                    action_changed_at: chrono::DateTime::from_timestamp(120, 345_678_901).unwrap(),
                    action_ordinal: 17,
                    authored_cue_id: Some(cue),
                },
                footprint: PlaybackFamilyFootprint::Component(ProgrammingComponent::Color(
                    ColorComponent::Amber,
                )),
                role: PlaybackFamilyRole::CalculationDependency,
                effective_fields: ProgrammingFieldScope::new([
                    Field::ColorXyz,
                    Field::ColorRecipeRed,
                    Field::ColorRecipeGreen,
                    Field::ColorRecipeBlue,
                ]),
            }])
            .unwrap(),
        )
    }

    #[test]
    fn projected_history_preserves_original_metadata_and_reuses_its_arc() {
        let original = evidence(Uuid::new_v4());
        let entry = &original.entries()[0];
        let mut cache = PlaybackEvidenceCache::default();
        let projected = cache.project(&original);
        let actual = &projected.entries()[0];
        assert_eq!(
            actual.source(),
            &ContributionSourceId::playback(entry.occurrence.source)
        );
        assert_eq!(
            actual.stamp().changed_at,
            entry.occurrence.action_changed_at
        );
        assert_eq!(actual.stamp().programmer_order, 0);
        assert_eq!(actual.transition_ordinal(), Some(17));
        assert_eq!(actual.authored_cue_id(), entry.occurrence.authored_cue_id);
        assert_eq!(
            actual.footprint(),
            ContributionFamilyFootprint::Component(ProgrammingComponent::Color(
                ColorComponent::Amber
            ))
        );
        assert_eq!(actual.role(), ContributionFamilyRole::CalculationDependency);
        assert_eq!(actual.effective_fields(), Some(&entry.effective_fields));
        cache.finish_frame();
        for _ in 0..3 {
            assert!(Arc::ptr_eq(&projected, &cache.project(&original)));
            cache.finish_frame();
        }
        assert_eq!(cache.entries.len(), 1);
        assert_eq!(
            Arc::strong_count(&original),
            1,
            "projection must not retain Playback state"
        );
        let weak = Arc::downgrade(&original);
        drop(original);
        assert!(weak.upgrade().is_none());
        cache.finish_frame();
        assert!(cache.entries.is_empty());
    }

    #[test]
    fn cache_retirement_tracks_used_evidence_even_if_old_runtime_history_is_still_alive() {
        let old = evidence(Uuid::new_v4());
        let next = evidence(Uuid::new_v4());
        let mut cache = PlaybackEvidenceCache::default();
        let old_projected = cache.project(&old);
        cache.finish_frame();
        let next_projected = cache.project(&next);
        cache.finish_frame();
        assert_eq!(cache.entries.len(), 1);
        assert!(Arc::ptr_eq(&next_projected, &cache.project(&next)));
        assert_eq!(
            old_projected.entries()[0].authored_cue_id(),
            old.entries()[0].occurrence.authored_cue_id
        );
        assert_eq!(Arc::strong_count(&old), 1);
    }
}
