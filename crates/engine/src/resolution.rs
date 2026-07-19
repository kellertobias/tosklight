use std::{collections::HashMap, sync::atomic::Ordering};

use chrono::{DateTime, Utc};
use light_core::MergeMode;
use light_playback::{
    ActivePlayback, AutomaticPlaybackTransition, MoveInBlackCandidate, PlaybackEngine,
    PlaybackTickResult,
};
use light_programmer::{GroupDefinition, resolve_group};

use super::{
    ContributionBatch, Engine, EngineContribution, EngineContributionResolver, EngineSnapshot,
    ResolvedAttributes, ResolvedContributionIndex, RuntimeGeneration, sampled_values,
};

struct PlaybackResolution {
    contributions: Vec<EngineContribution>,
    move_in_black_candidates: Vec<MoveInBlackCandidate>,
    active_playbacks: Vec<ActivePlayback>,
    automatic_transitions: Vec<AutomaticPlaybackTransition>,
}

impl Engine {
    /// Advance scheduler-owned runtime exactly once on the authoritative output path.
    pub(super) fn resolved_attributes_for_render(
        &self,
        generation: &RuntimeGeneration,
        now: DateTime<Utc>,
        sampled: &[ContributionBatch],
    ) -> ResolvedAttributes {
        self.resolve_attributes(generation, now, sampled, true)
    }

    /// Read the current projection without consuming an automatic transition before output can
    /// return it to the application boundary.
    pub(super) fn resolved_attributes_at(
        &self,
        generation: &RuntimeGeneration,
        now: DateTime<Utc>,
        sampled: &[ContributionBatch],
    ) -> ResolvedAttributes {
        self.resolve_attributes(generation, now, sampled, false)
    }

    fn resolve_attributes(
        &self,
        generation: &RuntimeGeneration,
        now: DateTime<Utc>,
        sampled: &[ContributionBatch],
        advance_playback: bool,
    ) -> ResolvedAttributes {
        let snapshot = generation.snapshot();
        let groups = generation.groups();
        let has_samples = sampled.iter().any(|batch| !batch.is_empty());
        let mut playback = self.resolve_playback(generation, now, advance_playback, sampled);
        let programmers = self.programmers.active();
        let programmer = {
            let underlay = crate::programmer_resolution::programmers_need_underlay(&programmers)
                .then(|| {
                    let mut underlay = ResolvedContributionIndex::new(&playback.contributions);
                    if has_samples {
                        underlay.extend_sampled(sampled_values(sampled));
                    }
                    underlay
                });
            self.programmer_contributions(
                programmers,
                generation,
                now,
                groups,
                underlay.as_ref(),
                sampled,
            )
        };
        playback.contributions.extend(programmer);
        let mut resolver = EngineContributionResolver::new(playback.contributions);
        if has_samples {
            resolver.extend_borrowed_samples(sampled_values(sampled));
        }
        add_group_contributions(&mut resolver, snapshot, groups, now);
        let base = if playback.move_in_black_candidates.is_empty() {
            HashMap::new()
        } else {
            resolver.values()
        };
        let move_in_black = self.move_in_black_contributions(
            generation,
            playback.move_in_black_candidates,
            &playback.active_playbacks,
            &base,
            now,
        );
        for contribution in move_in_black {
            resolver.add_unscaled(contribution);
        }
        let mut resolved = resolver.finish();
        resolved.automatic_playback_transitions = playback.automatic_transitions;
        resolved
    }

    fn resolve_playback(
        &self,
        generation: &RuntimeGeneration,
        now: DateTime<Utc>,
        advance: bool,
        sampled: &[ContributionBatch],
    ) -> PlaybackResolution {
        if advance {
            let timecode = self.timecode_frame.load(Ordering::Relaxed);
            let mut playback = generation.playback().write();
            let PlaybackTickResult { transitions } =
                playback.tick(now, (timecode != u64::MAX).then_some(timecode));
            return playback_resolution(generation, &playback, now, transitions, sampled);
        }
        let playback = generation.playback().read();
        playback_resolution(generation, &playback, now, Vec::new(), sampled)
    }
}

fn playback_resolution(
    generation: &RuntimeGeneration,
    playback: &PlaybackEngine,
    now: DateTime<Utc>,
    transitions: Vec<AutomaticPlaybackTransition>,
    sampled: &[ContributionBatch],
) -> PlaybackResolution {
    let mut contributions = playback.contributions_with_context_at(now, |fixture_id, attribute| {
        generation.attribute_is_snap(fixture_id, attribute)
    });
    if sampled.iter().any(ContributionBatch::has_replacements) {
        contributions.retain(|contribution| {
            let source = crate::ContributionSourceId::playback(contribution.source);
            !crate::replaces_source(sampled, &source, &contribution.value)
        });
    }
    PlaybackResolution {
        contributions: contributions
            .into_iter()
            .map(EngineContribution::from_playback)
            .collect(),
        move_in_black_candidates: playback.move_in_black_candidates(),
        active_playbacks: playback.runtime(),
        automatic_transitions: transitions,
    }
}

fn add_group_contributions(
    resolver: &mut EngineContributionResolver,
    snapshot: &EngineSnapshot,
    groups: &HashMap<String, GroupDefinition>,
    now: DateTime<Utc>,
) {
    for group in &snapshot.groups {
        let fixtures = resolve_group(&group.id, groups).unwrap_or_default();
        for fixture_id in fixtures {
            for (attribute, value) in &group.programming {
                resolver.add_borrowed_unscaled(
                    fixture_id,
                    attribute,
                    value,
                    0,
                    now,
                    if attribute.is_intensity() {
                        MergeMode::Htp
                    } else {
                        MergeMode::Ltp
                    },
                );
            }
        }
    }
}
