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
        let mut playback = crate::timed(crate::RenderPhase::PlaybackResolution, || {
            self.resolve_playback(generation, now, advance_playback, sampled)
        });
        let programmer = crate::timed(crate::RenderPhase::ProgrammerContributions, || {
            // Inside the phase on purpose: reading the Programmer used to copy everything the
            // operator had programmed, and that cost belonged to no phase at all.
            let programmers = self.programmers.active_output_states();
            let underlay = crate::programmer_resolution::programmers_need_underlay(&programmers)
                .then(|| {
                    let mut underlay = ResolvedContributionIndex::new(&playback.contributions);
                    if has_samples {
                        underlay.extend_sampled(sampled_values(sampled));
                    }
                    underlay
                });
            self.programmer_contributions(programmers, generation, now, underlay.as_ref(), sampled)
        });
        let programmer_colors = programmer
            .iter()
            .filter(|contribution| contribution.attribute().0 == "color")
            .map(EngineContribution::fixture_id)
            .collect::<std::collections::HashSet<_>>();
        playback.contributions.extend(programmer);
        let mut resolver =
            EngineContributionResolver::for_generation(generation.slots(), generation.frames());
        resolver.extend(playback.contributions);
        if has_samples {
            resolver.extend_borrowed_samples(sampled_values(sampled));
        }
        crate::timed(crate::RenderPhase::GroupContributions, || {
            add_group_contributions(&mut resolver, snapshot, groups, now)
        });
        let base = if playback.move_in_black_candidates.is_empty() {
            crate::ResolvedValues::default()
        } else {
            resolver.values()
        };
        let move_in_black = crate::timed(crate::RenderPhase::MoveInBlack, || {
            self.move_in_black_contributions(
                generation,
                playback.move_in_black_candidates,
                &playback.active_playbacks,
                &base,
                now,
            )
        });
        for (contribution, transition_ordinal) in move_in_black {
            resolver.add_playback_unscaled(contribution, transition_ordinal);
        }
        let mut resolved = crate::timed(crate::RenderPhase::ResolverFinish, || resolver.finish());
        self.apply_group_color_contributions(generation, &mut resolved, &programmer_colors);
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
            return playback_resolution(&playback, now, transitions, sampled);
        }
        let playback = generation.playback().read();
        playback_resolution(&playback, now, Vec::new(), sampled)
    }
}

fn playback_resolution(
    playback: &PlaybackEngine,
    now: DateTime<Utc>,
    transitions: Vec<AutomaticPlaybackTransition>,
    sampled: &[ContributionBatch],
) -> PlaybackResolution {
    let mut contributions = playback.contributions_with_context_at(now, |_, attribute| {
        light_playback::attribute_uses_snap_transition(attribute)
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
    for group in snapshot.groups.iter() {
        let fixtures = resolve_group(&group.id, groups).unwrap_or_default();
        if fixtures.is_empty() || group.programming.is_empty() {
            continue;
        }
        // A Group fans one programmed attribute out to every member, so its name is reduced to a
        // number once here rather than hashed once per member fixture.
        let programming = group
            .programming
            .iter()
            .filter_map(|(attribute, value)| {
                Some((
                    resolver.attribute_id(attribute)?,
                    value,
                    if attribute.is_intensity() {
                        MergeMode::Htp
                    } else {
                        MergeMode::Ltp
                    },
                ))
            })
            .collect::<Vec<_>>();
        for fixture_id in fixtures {
            for (attribute, value, merge_mode) in &programming {
                // A member that does not have this attribute is left alone rather than given a
                // value nothing would ever project.
                resolver.add_numbered_unscaled(fixture_id, *attribute, value, 0, now, *merge_mode);
            }
        }
    }
}
