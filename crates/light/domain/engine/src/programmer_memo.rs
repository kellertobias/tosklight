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
    ContributionSourceId,
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
#[derive(Clone, Default)]
pub(crate) struct ProgrammerTransitions {
    map: FxHashMap<ProgrammerTransitionKey, ProgrammerTransition>,
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
        self.map.insert(key, value);
        self.changed();
    }

    pub(crate) fn remove(&mut self, key: &ProgrammerTransitionKey) -> Option<ProgrammerTransition> {
        let removed = self.map.remove(key);
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
        self.map.entry(key)
    }

    pub(crate) fn retain(
        &mut self,
        keep: impl FnMut(&ProgrammerTransitionKey, &mut ProgrammerTransition) -> bool,
    ) {
        let before = self.map.len();
        self.map.retain(keep);
        if self.map.len() != before {
            self.changed();
        }
    }

    pub(crate) fn clear(&mut self) {
        if !self.map.is_empty() {
            self.map.clear();
            self.changed();
        }
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// One Programmer's resolved values in order, each with the source a sampled batch may replace.
pub(crate) type ResolvedProgrammerValues = Vec<(Addressed, Option<ContributionSourceId>)>;

struct Entry {
    /// Holds every captured vector alive, so pointer identity stays exact.
    states: Vec<ProgrammerOutputState>,
    generation: u64,
    flags: (bool, bool),
    transitions: u64,
    resolved: Arc<Vec<ResolvedProgrammerValues>>,
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
    ) -> Option<Arc<Vec<ResolvedProgrammerValues>>> {
        self.entries
            .iter()
            .find(|entry| {
                entry.generation == generation
                    && entry.flags == flags
                    && entry.transitions == transitions
                    && same_states(&entry.states, states)
            })
            .map(|entry| Arc::clone(&entry.resolved))
    }

    pub(crate) fn keep(
        &mut self,
        states: Vec<ProgrammerOutputState>,
        generation: u64,
        flags: (bool, bool),
        transitions: u64,
        resolved: Arc<Vec<ResolvedProgrammerValues>>,
    ) {
        self.entries.retain(|entry| {
            !(entry.generation == generation
                && entry.flags == flags
                && same_states(&entry.states, &states))
        });
        self.entries.insert(
            0,
            Entry {
                states,
                generation,
                flags,
                transitions,
                resolved,
            },
        );
        self.entries.truncate(RETAINED);
    }
}
