use crate::{
    Engine, MoveInBlackDiagnostic, MoveInBlackRuntime, PreparedCandidate, RuntimeGeneration,
};
use chrono::{DateTime, Utc};
use light_core::{AttributeKey, AttributeValue, FixtureId, TimedValue};
use light_playback::{ActivePlayback, MoveInBlackCandidate};
use std::collections::{HashMap, HashSet};

impl Engine {
    pub fn move_in_black_runtime(&self) -> Vec<MoveInBlackDiagnostic> {
        let mut diagnostics = self
            .move_in_black
            .lock()
            .values()
            .map(MoveInBlackRuntime::diagnostic)
            .collect::<Vec<_>>();
        diagnostics.sort_by(|left, right| {
            left.playback_number
                .cmp(&right.playback_number)
                .then_with(|| left.fixture_id.0.cmp(&right.fixture_id.0))
        });
        diagnostics
    }

    pub(crate) fn move_in_black_contributions(
        &self,
        generation: &RuntimeGeneration,
        candidates: Vec<MoveInBlackCandidate>,
        active: &[ActivePlayback],
        base_resolved: &HashMap<(FixtureId, AttributeKey), AttributeValue>,
        now: DateTime<Utc>,
    ) -> Vec<TimedValue> {
        let mut runtimes = self.move_in_black.lock();
        let mut present = HashSet::new();
        for candidate in candidates {
            let candidate = PreparedCandidate::new(generation, candidate, base_resolved);
            present.insert(candidate.key);
            let runtime = runtimes
                .entry(candidate.key)
                .or_insert_with(|| MoveInBlackRuntime::new(&candidate, now));
            runtime.update(candidate, now);
        }
        for (key, runtime) in runtimes.iter_mut() {
            if !present.contains(key) {
                runtime.update_absent(*key, active, now);
            }
        }
        runtimes
            .iter()
            .filter(|(key, runtime)| runtime.contributes(key, &present, now))
            .flat_map(|(_, runtime)| runtime.timed_values())
            .collect()
    }
}
