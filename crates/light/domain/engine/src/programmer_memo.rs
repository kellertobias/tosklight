//! TL-639: Programmer contributions computed once while nothing they depend on changes.
//!
//! Without a fade, a Programmer's contributions are a pure function of its captured value
//! vectors (shared `Arc`s the registry replaces on every edit), the runtime generation, the
//! source-tracing flags and the lane's retained transition history. Every hybrid frame resolves
//! its static baseline twice and every frame resolves it again, so an unchanged Programmer was
//! re-evaluated value by value two or three times per tick. The memo keeps the evaluation from
//! before the sampled-replacement filter: the filter and the winner arbitration still run on
//! every call, against that call's own samples.
//!
//! The transition history is identified by a process-unique version that every change replaces,
//! so an equal version proves equal content. An evaluation is kept only when it left the
//! history unchanged, which makes reusing it exactly the same as evaluating again.
use crate::{
    programmer_fade::{ProgrammerTransition, ProgrammerTransitionKey},
    programmer_resolution::Addressed,
};
use light_programmer::ProgrammerOutputState;
use rustc_hash::FxHashMap;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

/// Retained Programmer transitions of one output lane, with the identity of their content.
///
/// The map is shared between clones and copied on the first change (TL-639 round 4): the output
/// continuity is cloned for every capture and both static resolutions of a frame, and without a
/// fade nothing changes it.
#[derive(Clone, Default)]
pub(crate) struct ProgrammerTransitions {
    map: Arc<FxHashMap<ProgrammerTransitionKey, ProgrammerTransition>>,
    /// A fresh process-unique number after every change; clones keep it. The empty default is
    /// version 0 in every lane, which is exact because every empty history is equal.
    version: u64,
}

static NEXT_VERSION: AtomicU64 = AtomicU64::new(1);

impl ProgrammerTransitions {
    fn changed(&mut self) {
        self.version = NEXT_VERSION.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn version(&self) -> u64 {
        self.version
    }

    pub(crate) fn get(&self, key: &ProgrammerTransitionKey) -> Option<&ProgrammerTransition> {
        self.map.get(key)
    }

    pub(crate) fn insert(&mut self, key: ProgrammerTransitionKey, value: ProgrammerTransition) {
        Arc::make_mut(&mut self.map).insert(key, value);
        self.changed();
    }

    pub(crate) fn remove(&mut self, key: &ProgrammerTransitionKey) -> Option<ProgrammerTransition> {
        if !self.map.contains_key(key) {
            return None;
        }
        let removed = Arc::make_mut(&mut self.map).remove(key);
        if removed.is_some() {
            self.changed();
        }
        removed
    }

    /// Mutable access to one transition; counted as a change.
    pub(crate) fn entry(
        &mut self,
        key: ProgrammerTransitionKey,
    ) -> std::collections::hash_map::Entry<'_, ProgrammerTransitionKey, ProgrammerTransition> {
        self.changed();
        Arc::make_mut(&mut self.map).entry(key)
    }

    /// Keeps the transitions `keep` accepts; copies a shared map only when one is removed.
    pub(crate) fn retain(
        &mut self,
        mut keep: impl FnMut(&ProgrammerTransitionKey, &ProgrammerTransition) -> bool,
    ) {
        if self
            .map
            .iter()
            .all(|(key, transition)| keep(key, transition))
        {
            return;
        }
        let map = Arc::make_mut(&mut self.map);
        let before = map.len();
        map.retain(|key, transition| keep(key, transition));
        if map.len() != before {
            self.changed();
        }
    }

    pub(crate) fn clear(&mut self) {
        if !self.map.is_empty() {
            self.map = Arc::default();
            self.changed();
        }
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// One Programmer's resolved values in order, each with the source a sampled batch may replace.
pub(crate) type ResolvedProgrammerValues = Vec<(
    Addressed,
    Option<crate::programmer_resolution::SourceReplacement>,
)>;

/// TL-639 round 2: the arbitrated contributions of one kept evaluation. The replacement filter
/// still runs against each call's own samples; the winners are a pure function of the evaluation
/// and of which values that filter removed, so they are kept per removed set (the static lane
/// removes none, the scalar lane the values its samples replace). Readers borrow them.
#[derive(Default)]
pub(crate) struct KeptWinners(parking_lot::Mutex<Vec<(Vec<u32>, ArbitratedWinners)>>);

/// One arbitration's contributions, shared by every reader of the same removed set.
type ArbitratedWinners = Arc<Vec<crate::EngineContribution>>;

/// Distinct removed sets kept per evaluation.
const KEPT_WINNER_SETS: usize = 4;

impl KeptWinners {
    /// The winners after removing exactly `removed` (positions in evaluation order), computed by
    /// `arbitrate` the first time this set is seen.
    pub(crate) fn get_or_arbitrate(
        &self,
        removed: Vec<u32>,
        arbitrate: impl FnOnce() -> Vec<crate::EngineContribution>,
    ) -> ArbitratedWinners {
        if let Some((_, winners)) = self.0.lock().iter().find(|(set, _)| *set == removed) {
            return Arc::clone(winners);
        }
        let winners = Arc::new(arbitrate());
        let mut kept = self.0.lock();
        if kept.len() < KEPT_WINNER_SETS {
            kept.push((removed, Arc::clone(&winners)));
        }
        winners
    }
}

/// One kept evaluation and its winners.
pub(crate) struct KeptEvaluation {
    pub resolved: Arc<Vec<ResolvedProgrammerValues>>,
    pub winners: Arc<KeptWinners>,
}

struct Entry {
    /// Holds every captured vector alive, so pointer identity stays exact.
    states: Vec<ProgrammerOutputState>,
    generation: u64,
    flags: (bool, bool),
    transitions: u64,
    resolved: Arc<Vec<ResolvedProgrammerValues>>,
    winners: Arc<KeptWinners>,
}

/// The most recent evaluations (Live and Preload lanes, with and without source tracing).
#[derive(Default)]
pub(crate) struct ProgrammerContributionMemo {
    entries: Vec<Entry>,
}

const RETAINED: usize = 4;

fn same_states(left: &[ProgrammerOutputState], right: &[ProgrammerOutputState]) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.id == right.id
                && left.priority == right.priority
                && Arc::ptr_eq(&left.values, &right.values)
                && Arc::ptr_eq(&left.transient_values, &right.transient_values)
                && Arc::ptr_eq(&left.group_values, &right.group_values)
                && Arc::ptr_eq(&left.replacement_provenance, &right.replacement_provenance)
                && Arc::ptr_eq(&left.preload_active, &right.preload_active)
                && Arc::ptr_eq(&left.preload_group_active, &right.preload_group_active)
                && Arc::ptr_eq(&left.preload_dynamic_active, &right.preload_dynamic_active)
                && Arc::ptr_eq(
                    &left.preload_group_release_active,
                    &right.preload_group_release_active,
                )
        })
}

impl ProgrammerContributionMemo {
    /// Forget every kept evaluation (tests compare against fresh evaluations).
    #[cfg(test)]
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    /// The kept evaluation of exactly these inputs, if any. `flags` are (trace sources,
    /// sampled replacements present), which select the recorded origins and replacement ids.
    pub(crate) fn find(
        &self,
        states: &[ProgrammerOutputState],
        generation: u64,
        flags: (bool, bool),
        transitions: u64,
    ) -> Option<KeptEvaluation> {
        self.entries
            .iter()
            .find(|entry| {
                entry.generation == generation
                    && entry.flags == flags
                    && entry.transitions == transitions
                    && same_states(&entry.states, states)
            })
            .map(|entry| KeptEvaluation {
                resolved: Arc::clone(&entry.resolved),
                winners: Arc::clone(&entry.winners),
            })
    }

    pub(crate) fn keep(
        &mut self,
        states: Vec<ProgrammerOutputState>,
        generation: u64,
        flags: (bool, bool),
        transitions: u64,
        resolved: Arc<Vec<ResolvedProgrammerValues>>,
    ) -> Arc<KeptWinners> {
        self.entries.retain(|entry| {
            !(entry.generation == generation
                && entry.flags == flags
                && same_states(&entry.states, &states))
        });
        let winners = Arc::new(KeptWinners::default());
        self.entries.insert(
            0,
            Entry {
                states,
                generation,
                flags,
                transitions,
                resolved,
                winners: Arc::clone(&winners),
            },
        );
        self.entries.truncate(RETAINED);
        winners
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TL-639 round 4: clones share the history until one of them changes it; an unchanged
    /// retain or an absent removal neither copies it nor counts as a change.
    #[test]
    fn transition_clones_share_the_history_until_one_changes() {
        let mut history = ProgrammerTransitions::default();
        let copy = history.clone();
        assert!(Arc::ptr_eq(&history.map, &copy.map));
        history.retain(|_, _| true);
        let absent = ProgrammerTransitionKey {
            programmer_id: light_core::ProgrammerId::new(),
            source: crate::programmer_fade::ProgrammerTransitionSource::Programmer,
            fixture_id: light_core::FixtureId::new(),
            attribute: light_core::AttributeKey::intensity(),
        };
        assert!(history.remove(&absent).is_none());
        assert!(Arc::ptr_eq(&history.map, &copy.map));
        assert_eq!(history.version(), copy.version());
        history.clear();
        assert_eq!(
            history.version(),
            copy.version(),
            "an empty history stays unchanged"
        );
    }

    #[test]
    fn kept_winners_belong_to_one_evaluation_and_removed_set() {
        let mut memo = ProgrammerContributionMemo::default();
        let resolved = Arc::new(Vec::new());
        let winners = memo.keep(Vec::new(), 7, (true, false), 3, Arc::clone(&resolved));
        let kept = memo.find(&[], 7, (true, false), 3).expect("kept");
        assert!(Arc::ptr_eq(&kept.winners, &winners));
        // Every input that selects the evaluation selects its winners too.
        assert!(memo.find(&[], 8, (true, false), 3).is_none(), "generation");
        assert!(
            memo.find(&[], 7, (false, false), 3).is_none(),
            "tracing flag"
        );
        assert!(
            memo.find(&[], 7, (true, true), 3).is_none(),
            "replacements flag"
        );
        assert!(
            memo.find(&[], 7, (true, false), 4).is_none(),
            "transition history"
        );
        // Keeping the evaluation again starts with no winners.
        let renewed = memo.keep(Vec::new(), 7, (true, false), 3, resolved);
        assert!(!Arc::ptr_eq(&renewed, &winners));
        let arbitrations = std::cell::Cell::new(0);
        let arbitrate = |removed: Vec<u32>| {
            renewed.get_or_arbitrate(removed, || {
                arbitrations.set(arbitrations.get() + 1);
                Vec::new()
            })
        };
        let none = arbitrate(Vec::new());
        assert!(Arc::ptr_eq(&none, &arbitrate(Vec::new())));
        assert_eq!(arbitrations.get(), 1);
        let first = arbitrate(vec![0]);
        assert!(!Arc::ptr_eq(&none, &first));
        assert_eq!(arbitrations.get(), 2);
        // More distinct sets than are kept are still answered, by arbitrating each time.
        for removed in 1..8 {
            arbitrate(vec![removed]);
            arbitrate(vec![removed]);
        }
        assert!(arbitrations.get() > 2 + 7);
        assert!(Arc::ptr_eq(&none, &arbitrate(Vec::new())));
    }
}
