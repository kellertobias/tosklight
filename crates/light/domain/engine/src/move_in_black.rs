use crate::{
    Engine, MoveInBlackDiagnostic, MoveInBlackRuntime, OutputContinuityState, PreparedCandidate,
    RuntimeGeneration,
};
use chrono::{DateTime, Utc};
use light_core::TimedValue;
use light_playback::{ActivePlayback, MoveInBlackCandidate};
use std::collections::HashSet;

impl Engine {
    pub fn move_in_black_runtime(&self) -> Vec<MoveInBlackDiagnostic> {
        let mut diagnostics = self
            .output_continuity
            .lock()
            .move_in_black
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

    /// Move-in-Black observes the same captured generation, resolved underlay and clock as
    /// this lane's other contributions. Speculative callers retain their own state branch.
    pub(crate) fn move_in_black_contributions_with_state(
        generation: &RuntimeGeneration,
        candidates: Vec<MoveInBlackCandidate>,
        active: &[ActivePlayback],
        base_resolved: &crate::ResolvedValues,
        now: DateTime<Utc>,
        continuity: &mut OutputContinuityState,
    ) -> Vec<(TimedValue, u64)> {
        let runtimes = &mut continuity.move_in_black;
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
