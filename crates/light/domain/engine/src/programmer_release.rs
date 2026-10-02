//! Cold source-scoped Release projection. No fabricated zero participates in arbitration.
use crate::{
    ContributionBatch, ContributionReleaseCutoff, ContributionSourceId, RuntimeGeneration,
};
use chrono::{DateTime, Utc};
use light_core::ProgrammerId;
use light_dynamics::{DynamicAddressValue, DynamicSemanticValue, RankedSelection};
use light_programmer::{GroupReleaseProgrammerValue, ProgrammerOutputState};
use std::{collections::HashMap, sync::Arc};

type ReleaseSources = (
    ProgrammerId,
    Arc<Vec<DynamicAddressValue>>,
    Arc<Vec<GroupReleaseProgrammerValue>>,
);

#[derive(Default)]
pub(crate) struct ProgrammerReleaseMemo {
    sources: Vec<ReleaseSources>,
    rankings: Option<Arc<HashMap<String, RankedSelection>>>,
    batch: ContributionBatch,
}

impl ProgrammerReleaseMemo {
    pub(crate) fn compile(
        &mut self,
        programmers: &[ProgrammerOutputState],
        generation: &RuntimeGeneration,
    ) -> ContributionBatch {
        let rankings = generation.group_rankings_arc();
        if self
            .rankings
            .as_ref()
            .is_some_and(|previous| Arc::ptr_eq(previous, &rankings))
            && self.sources.len() == programmers.len()
            && self
                .sources
                .iter()
                .zip(programmers)
                .all(|((id, fixtures, groups), state)| {
                    *id == state.id
                        && Arc::ptr_eq(fixtures, &state.preload_dynamic_active)
                        && Arc::ptr_eq(groups, &state.preload_group_release_active)
                })
        {
            return self.batch.clone();
        }
        let mut releases = Vec::new();
        for state in programmers {
            for value in state.preload_dynamic_active.iter() {
                if !matches!(value.value, DynamicSemanticValue::Release) {
                    continue;
                }
                let Some(cutoff) = release_cutoff(value.changed_at_millis, value.programmer_order)
                else {
                    continue;
                };
                for source in [
                    ContributionSourceId::programmer(state.id),
                    ContributionSourceId::preload(state.id),
                ] {
                    releases.push((source, value.fixture_id, value.attribute.clone(), cutoff));
                }
            }
            for value in state.preload_group_release_active.iter() {
                let Some(ranking) = rankings.get(&value.group_id) else {
                    continue;
                };
                let Some(cutoff) = release_cutoff(value.changed_at_millis, value.programmer_order)
                else {
                    continue;
                };
                for source in [
                    ContributionSourceId::programmer_group(state.id, value.group_id.as_str()),
                    ContributionSourceId::preload_group(state.id, value.group_id.as_str()),
                ] {
                    for fixture in &ranking.ordered_fixture_ids {
                        releases.push((source.clone(), *fixture, value.attribute.clone(), cutoff));
                    }
                }
            }
        }
        self.sources = programmers
            .iter()
            .map(|state| {
                (
                    state.id,
                    Arc::clone(&state.preload_dynamic_active),
                    Arc::clone(&state.preload_group_release_active),
                )
            })
            .collect();
        self.rankings = Some(rankings);
        self.batch = ContributionBatch::releasing(releases);
        self.batch.clone()
    }
}

pub(crate) fn release_cutoff(
    millis: u64,
    programmer_order: u64,
) -> Option<ContributionReleaseCutoff> {
    Some(ContributionReleaseCutoff {
        changed_at: DateTime::<Utc>::from_timestamp_millis(i64::try_from(millis).ok()?)?,
        programmer_order,
    })
}
