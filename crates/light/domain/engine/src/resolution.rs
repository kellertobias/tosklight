use std::sync::atomic::Ordering;

use chrono::{DateTime, Utc};
use light_playback::{
    ActivePlayback, AutomaticPlaybackTransition, MoveInBlackCandidate, PlaybackEngine,
    PlaybackTickResult,
};

use super::{
    ContributionBatch, Engine, EngineContribution, EngineContributionResolver, ResolvedAttributes,
    ResolvedContributionIndex, RuntimeGeneration, sampled_values,
};

pub(crate) struct PlaybackResolution {
    pub(crate) preview_source: Option<PlaybackEngine>,
    pub(crate) contributions: Vec<EngineContribution>,
    pub(crate) move_in_black_candidates: Vec<MoveInBlackCandidate>,
    pub(crate) active_playbacks: Vec<ActivePlayback>,
    pub(crate) automatic_transitions: Vec<AutomaticPlaybackTransition>,
    pub(crate) cue_dynamic_values: Vec<light_playback::ActiveCueDynamicValue>,
    pub(crate) dynamic_playbacks: Vec<light_playback::ActiveDynamicPlayback>,
    pub(crate) dynamics_paused: bool,
}

pub(crate) struct ProgrammerLaneInputs<'a> {
    pub(crate) playback: &'a PlaybackResolution,
    pub(crate) states: &'a [light_programmer::ProgrammerOutputState],
    pub(crate) releases: &'a ContributionBatch,
    pub(crate) addresses:
        &'a parking_lot::Mutex<crate::programmer_resolution::ProgrammerAddressMemo>,
}

impl Engine {
    /// Evaluate only retained inputs. The caller owns the lane history, so Current/Release
    /// branches can fork it without changing the authoritative Live state.
    pub(crate) fn resolve_prepared_attributes(
        &self,
        frame: &crate::PreparedOutputFrame,
        sampled: &[ContributionBatch],
        continuity: &mut crate::OutputContinuityState,
        trace_sources: bool,
    ) -> ResolvedAttributes {
        self.resolve_prepared_lane_attributes(
            frame,
            sampled,
            continuity,
            trace_sources,
            ProgrammerLaneInputs {
                playback: &frame.playback,
                states: &frame.programmer.output_states,
                releases: &frame.releases,
                addresses: &self.programmer_addresses,
            },
        )
    }

    pub(crate) fn resolve_prepared_lane_attributes(
        &self,
        frame: &crate::PreparedOutputFrame,
        sampled: &[ContributionBatch],
        continuity: &mut crate::OutputContinuityState,
        trace_sources: bool,
        lane: ProgrammerLaneInputs<'_>,
    ) -> ResolvedAttributes {
        let generation = &frame.generation;
        let now = frame.sampled_at;
        let pair;
        let with_releases;
        let sampled: &[ContributionBatch] = if lane.releases.is_empty() {
            sampled
        } else if sampled.is_empty() {
            std::slice::from_ref(lane.releases)
        } else if let [sample] = sampled {
            pair = [sample.clone(), lane.releases.clone()];
            &pair
        } else {
            with_releases = sampled
                .iter()
                .cloned()
                .chain([lane.releases.clone()])
                .collect::<Vec<_>>();
            &with_releases
        };
        let has_samples = sampled.iter().any(|batch| !batch.is_empty());
        let has_replacements = sampled.iter().any(ContributionBatch::has_replacements);
        let playback = || {
            lane.playback
                .contributions
                .iter()
                .filter(|value| !has_replacements || !value.replaced_by(sampled))
        };
        let programmer = crate::timed(crate::RenderPhase::ProgrammerContributions, || {
            let underlay = crate::programmer_resolution::programmers_need_underlay(lane.states)
                .then(|| {
                    let mut underlay = ResolvedContributionIndex::from_contributions(playback());
                    if has_samples {
                        underlay.extend_sampled(sampled_values(sampled));
                    }
                    underlay
                });
            self.programmer_contributions_with_state(
                lane.states.to_vec(),
                generation,
                now,
                underlay.as_ref(),
                sampled,
                trace_sources,
                continuity,
                frame.programmer_fade_millis,
                &frame.colors,
                lane.addresses,
            )
        });
        // Read only by the Group colour overlay: without a Group colour it is never consulted.
        let programmer_colors = if frame.colors.is_empty() {
            Default::default()
        } else {
            programmer
                .iter()
                .filter(|value| value.attribute().0.as_ref() == "color")
                .map(EngineContribution::fixture_id)
                .collect()
        };
        let mut resolver =
            EngineContributionResolver::for_generation(generation.slots(), generation.frames());
        if trace_sources {
            resolver = resolver.tracing_sources();
        }
        // TL-639 round 7: with the Programmer's kept winners, every offer goes to the output
        // pool by slot range; a changed Programmer's owned winners are offered in turn.
        let pool = self.output_pool();
        crate::timed(crate::RenderPhase::ContributionMerge, || match programmer {
            crate::programmer_resolution::ProgrammerContributions::Shared(programmer) => {
                let bound = lane.playback.contributions.len()
                    + programmer.len()
                    + sampled
                        .iter()
                        .map(|batch| batch.samples().len())
                        .sum::<usize>();
                resolver.offer_borrowed_on(
                    (pool.as_deref(), &self.offer_scratch),
                    (
                        playback(),
                        &programmer,
                        sampled_values(sampled).filter(|_| has_samples),
                    ),
                    bound,
                );
            }
            programmer @ crate::programmer_resolution::ProgrammerContributions::Owned(_) => {
                resolver.extend_borrowed_contributions(playback());
                programmer.offer_to(&mut resolver);
                if has_samples {
                    resolver.extend_borrowed_samples(sampled_values(sampled));
                }
            }
        });
        crate::timed(crate::RenderPhase::GroupContributions, || {
            add_group_contributions(&mut resolver, generation.group_plan(), now);
        });
        let base = if lane.playback.move_in_black_candidates.is_empty() {
            Default::default()
        } else {
            resolver.values()
        };
        let move_in_black = crate::timed(crate::RenderPhase::MoveInBlack, || {
            Self::move_in_black_contributions_with_state(
                generation,
                lane.playback.move_in_black_candidates.clone(),
                &lane.playback.active_playbacks,
                &base,
                now,
                continuity,
            )
        });
        for (value, ordinal) in move_in_black {
            resolver.add_playback_unscaled(value, ordinal);
        }
        let mut resolved = crate::timed(crate::RenderPhase::ResolverFinish, || resolver.finish());
        Self::apply_group_color_contributions_from(
            &frame.colors,
            generation,
            &mut resolved,
            &programmer_colors,
        );
        Self::apply_captured_tracking(&frame.tracked, generation, &mut resolved);
        resolved.automatic_playback_transitions = lane.playback.automatic_transitions.clone();
        resolved
    }

    /// Advance scheduler-owned runtime exactly once on the authoritative output path.
    #[cfg(test)]
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
        crate::timed(crate::RenderPhase::ResolveTotal, || {
            self.resolve_attributes_inner(generation, now, sampled, advance_playback, false)
        })
    }

    fn resolve_attributes_inner(
        &self,
        generation: &RuntimeGeneration,
        now: DateTime<Utc>,
        sampled: &[ContributionBatch],
        advance_playback: bool,
        trace_sources: bool,
    ) -> ResolvedAttributes {
        let (continuity_revision, mut continuity) = self.capture_output_continuity();
        let fade_millis = self.programmer_fade_millis.load(Ordering::Relaxed);
        let colors = self.group_colors.read().clone();
        let programmers = self.programmers.active_output_states();
        let releases = self
            .programmer_releases
            .lock()
            .compile(&programmers, generation);
        // Most frames have no active Release. Only the release path needs an extra batch; its
        // source/fixture index is rebuilt on edits or Group membership changes, never per tick.
        let pair;
        let with_releases;
        let sampled: &[ContributionBatch] = if releases.is_empty() {
            sampled
        } else if sampled.is_empty() {
            std::slice::from_ref(&releases)
        } else if let [sample] = sampled {
            // The scheduler publishes one composed Dynamic batch. Keep that ordinary combined
            // path inline too, rather than allocating a vector on every output frame.
            pair = [sample.clone(), releases];
            &pair
        } else {
            with_releases = sampled
                .iter()
                .cloned()
                .chain([releases])
                .collect::<Vec<_>>();
            &with_releases
        };
        let has_samples = sampled.iter().any(|batch| !batch.is_empty());
        let mut playback = crate::timed(crate::RenderPhase::PlaybackResolution, || {
            self.resolve_playback(generation, now, advance_playback, sampled, false)
        });
        let programmer = crate::timed(crate::RenderPhase::ProgrammerContributions, || {
            // Inside the phase on purpose: reading the Programmer used to copy everything the
            // operator had programmed, and that cost belonged to no phase at all.
            let underlay = crate::programmer_resolution::programmers_need_underlay(&programmers)
                .then(|| {
                    let mut underlay = ResolvedContributionIndex::new(&playback.contributions);
                    if has_samples {
                        underlay.extend_sampled(sampled_values(sampled));
                    }
                    underlay
                });
            self.programmer_contributions_with_state(
                programmers,
                generation,
                now,
                underlay.as_ref(),
                sampled,
                trace_sources,
                &mut continuity,
                fade_millis,
                &colors,
                &self.programmer_addresses,
            )
        });
        // Read only by the Group colour overlay: without a Group colour it is never consulted.
        let programmer_colors = if colors.is_empty() {
            std::collections::HashSet::new()
        } else {
            programmer
                .iter()
                .filter(|contribution| &*contribution.attribute().0 == "color")
                .map(EngineContribution::fixture_id)
                .collect::<std::collections::HashSet<_>>()
        };
        let mut resolver =
            EngineContributionResolver::for_generation(generation.slots(), generation.frames());
        if trace_sources {
            resolver = resolver.tracing_sources();
        }
        crate::timed(crate::RenderPhase::ContributionMerge, || {
            // Each source is offered to the frame where it is. Appending one list to the other
            // first moved every Programmer value twice for no reason: the frame arbitrates them in
            // the order they arrive either way.
            let mut contributions = std::mem::take(&mut playback.contributions);
            resolver.extend(contributions.drain(..));
            self.recycle_contributions(contributions);
            programmer.offer_to(&mut resolver);
            if has_samples {
                resolver.extend_borrowed_samples(sampled_values(sampled));
            }
        });
        crate::timed(crate::RenderPhase::GroupContributions, || {
            add_group_contributions(&mut resolver, generation.group_plan(), now)
        });
        let base = if playback.move_in_black_candidates.is_empty() {
            crate::ResolvedValues::default()
        } else {
            resolver.values()
        };
        let move_in_black = crate::timed(crate::RenderPhase::MoveInBlack, || {
            Self::move_in_black_contributions_with_state(
                generation,
                playback.move_in_black_candidates,
                &playback.active_playbacks,
                &base,
                now,
                &mut continuity,
            )
        });
        for (contribution, transition_ordinal) in move_in_black {
            resolver.add_playback_unscaled(contribution, transition_ordinal);
        }
        let mut resolved = crate::timed(crate::RenderPhase::ResolverFinish, || resolver.finish());
        Self::apply_group_color_contributions_from(
            &colors,
            generation,
            &mut resolved,
            &programmer_colors,
        );
        // Last, and on the read path as well as the render path: a 3D Point that a tracking
        // system holds has to read as moved everywhere the desk looks at it — the visualizer, the
        // Stage view, and `Fixture 1 AT Fixture 5` all ask for resolved values, and a beam aimed
        // at where the point used to be is the whole failure this feature exists to avoid.
        self.apply_tracked_overrides(generation, &mut resolved);
        resolved.automatic_playback_transitions = playback.automatic_transitions;
        if advance_playback {
            assert!(
                self.commit_output_continuity_if_unchanged(continuity_revision, continuity),
                "test render continuity changed"
            );
        }
        resolved
    }

    fn apply_tracked_overrides(
        &self,
        generation: &RuntimeGeneration,
        resolved: &mut ResolvedAttributes,
    ) {
        Self::apply_captured_tracking(&self.tracking_frame(), generation, resolved);
    }

    fn apply_captured_tracking(
        tracked: &crate::TrackedInputFrame,
        generation: &RuntimeGeneration,
        resolved: &mut ResolvedAttributes,
    ) {
        for held in tracked.legacy_overrides.iter() {
            let owner = generation
                .point_projection()
                .owner_for(held.fixture_id, &held.attribute)
                .unwrap_or(held.fixture_id);
            resolved.override_value(owner, &held.attribute, held.value.clone(), None);
        }
        let index = generation.point_projection();
        for point in tracked.points.iter() {
            let Some(origin) = index.origin_for(point.fixture_id) else {
                continue;
            };
            let Some(normalized) = point.normalized_position(origin) else {
                continue;
            };
            index.override_position(point.fixture_id, normalized, resolved);
        }
    }

    pub(crate) fn resolve_playback(
        &self,
        generation: &RuntimeGeneration,
        now: DateTime<Utc>,
        advance: bool,
        sampled: &[ContributionBatch],
        capture_preview: bool,
    ) -> PlaybackResolution {
        if advance {
            let timecode = self.timecode_frame.load(Ordering::Relaxed);
            let mut playback = generation.playback().write();
            let PlaybackTickResult { transitions } =
                playback.tick(now, (timecode != u64::MAX).then_some(timecode));
            // The tick is the only mutation. Reading the contributions needs no exclusivity, so
            // the lock steps down atomically: no command slips in between the tick and the values
            // built from it, and status readers stop waiting out the whole resolve. This relies on
            // parking_lot's downgrade; the standard library's RwLock has none.
            let playback = parking_lot::RwLockWriteGuard::downgrade(playback);
            return Self::playback_resolution(
                &playback,
                now,
                transitions,
                sampled,
                capture_preview,
                &self.scratch,
            );
        }
        let playback = generation.playback().read();
        Self::playback_resolution(
            &playback,
            now,
            Vec::new(),
            sampled,
            capture_preview,
            &self.scratch,
        )
    }

    /// This frame's Cue contributions, filled into the buffers the last frame handed back.
    pub(crate) fn playback_resolution(
        playback: &PlaybackEngine,
        now: DateTime<Utc>,
        transitions: Vec<AutomaticPlaybackTransition>,
        sampled: &[ContributionBatch],
        capture_preview: bool,
        scratch: &parking_lot::Mutex<crate::engine::FrameScratch>,
    ) -> PlaybackResolution {
        let mut scratch = scratch.lock();
        let crate::engine::FrameScratch {
            playback: raw,
            contributions,
            playback_evidence,
        } = &mut *scratch;
        // No predicate: the compiled cue settled which attributes snap when the cue list compiled,
        // rather than being asked once per contribution per frame.
        playback.extend_contributions(now, None, raw);
        if sampled.iter().any(ContributionBatch::has_replacements) {
            raw.retain(|contribution| {
                let source = crate::ContributionSourceId::playback(contribution.source);
                !crate::replaces_source(sampled, &source, &contribution.value)
            });
        }
        contributions.clear();
        contributions.extend(raw.drain(..).map(|contribution| {
            EngineContribution::from_playback(contribution, playback_evidence)
        }));
        playback_evidence.finish_frame();
        PlaybackResolution {
            // Queued action preview must start from this exact Playback read boundary and clock.
            // A desk without queued actions pays no whole-runtime clone here.
            preview_source: capture_preview.then(|| playback.fork_for_preview(now)),
            contributions: std::mem::take(contributions),
            move_in_black_candidates: playback.move_in_black_candidates(),
            active_playbacks: playback.runtime(),
            automatic_transitions: transitions,
            cue_dynamic_values: playback.active_cue_dynamic_values(),
            dynamic_playbacks: playback.active_dynamic_playbacks(),
            dynamics_paused: playback.dynamics_paused(),
        }
    }

    /// Hand a frame's contribution buffer back so the next frame fills it rather than growing one.
    fn recycle_contributions(&self, mut contributions: Vec<EngineContribution>) {
        contributions.clear();
        let mut scratch = self.scratch.lock();
        if scratch.contributions.capacity() < contributions.capacity() {
            scratch.contributions = contributions;
        }
    }
}

/// Group programming was reduced to slots when the generation was built; the tick offers them.
fn add_group_contributions(
    resolver: &mut EngineContributionResolver,
    plan: &crate::group_plan::GroupContributionPlan,
    now: DateTime<Utc>,
) {
    for entry in plan.entries() {
        resolver.add_slot_unscaled(entry.slot, &entry.value, 0, now, entry.merge_mode);
    }
}
