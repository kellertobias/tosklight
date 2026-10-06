//! The resolver's offers on the output pool (TL-639 round 7).
//!
//! A resolution offers the Playback contributions, the Programmer's kept winners and the
//! sampled values to their slots, in that order. Arbitration of a slot reads and writes only
//! that slot, so the frame first finds every offer's slot in order, offering the few unnumbered
//! pairs to the overflow map right away as before, and groups the numbered offers by slot range
//! ([`crate::frame_state::FrameShard`]); each range then takes its offers on a pool thread in
//! their original order. Every slot sees the single-threaded sequence of offers, and the
//! ranges' first writes are merged back in offer order, so the resolved frame is the
//! single-threaded one for any worker count.
use super::*;
use crate::frame_state::FrameShard;
use crate::parallel::OutputPool;

/// Below this many offers a resolution offers on the caller (two in tests).
const MIN_PARALLEL_OFFERS: usize = if cfg!(test) { 2 } else { 4096 };

/// The room a parallel resolution's offers need, kept between frames.
#[derive(Default)]
pub(crate) struct OfferScratch {
    /// Per slot range: each numbered offer's position and slot.
    ranges: Vec<Vec<(u32, crate::Slot)>>,
    /// Per slot range: its first writes.
    touched: Vec<Vec<(u32, u32)>>,
    by_position: Vec<u32>,
}

/// One offer of a resolution.
#[derive(Clone, Copy)]
enum Offered<'a> {
    Contribution(&'a EngineContribution),
    Sample(&'a crate::ContributionSample),
}

impl EngineContributionResolver<'_> {
    /// Offer `playback`, then `programmer`, then `samples` (borrowed throughout; at most
    /// `bound` offers), on `pool` when there are enough; the same frame as the single-threaded
    /// calls.
    pub(crate) fn offer_borrowed_on<'x, 'p: 'x, 'q: 'x, 's: 'x>(
        &mut self,
        (pool, scratch): (Option<&OutputPool>, &parking_lot::Mutex<OfferScratch>),
        (playback, programmer, samples): (
            impl Iterator<Item = &'p EngineContribution>,
            &'q [EngineContribution],
            impl Iterator<Item = &'s crate::ContributionSample>,
        ),
        bound: usize,
    ) {
        // Never from a pool thread: a resolution there is a lazily observed source answering a
        // parallel section (a Freeze's captured values for pinning), and a nested section could
        // take up another of that section's items, which would wait on this very resolution.
        let Some(pool) = pool.filter(|pool| {
            pool.workers() > 1
                && bound >= MIN_PARALLEL_OFFERS
                && rayon::current_thread_index().is_none()
        }) else {
            self.extend_borrowed_contributions(playback);
            self.extend_borrowed_contributions(programmer);
            self.extend_borrowed_samples(samples);
            return;
        };
        // Another thread's resolution holding the kept room never makes this one wait.
        let mut kept = scratch.try_lock();
        let mut fresh = OfferScratch::default();
        let scratch = kept.as_deref_mut().unwrap_or(&mut fresh);
        // The closures shorten each list's lifetime to the frame's; the variants alone cannot.
        #[allow(clippy::redundant_closure)]
        let items = playback
            .map(|candidate| Offered::<'x>::Contribution(candidate))
            .chain(
                programmer
                    .iter()
                    .map(|candidate| Offered::<'x>::Contribution(candidate)),
            )
            .chain(samples.map(|sample| Offered::<'x>::Sample(sample)))
            .collect::<Vec<_>>();
        let offers = items.len();
        let plan = self.frame.shard_plan(pool.workers() * 4);
        let capacity = self.frame.capacity();
        let ranges = &mut scratch.ranges;
        ranges.resize_with(plan.count(), Vec::new);
        ranges.iter_mut().for_each(Vec::clear);
        for (position, &item) in items.iter().enumerate() {
            let (fixture_id, attribute, address) = match item {
                Offered::Contribution(candidate) => (
                    candidate.value.fixture_id,
                    &candidate.value.attribute,
                    candidate.address,
                ),
                Offered::Sample(sample) => (
                    sample.value().fixture_id,
                    &sample.value().attribute,
                    sample.address(),
                ),
            };
            match self.slot_for(fixture_id, attribute, address) {
                // A slot beyond the frame is ignored, as a single offer ignores it.
                Some(slot) if slot.index() < capacity => {
                    ranges[plan.shard_of(slot)].push((position as u32, slot));
                }
                Some(_) => {}
                None => match item {
                    Offered::Contribution(candidate) => {
                        self.extend_borrowed_contributions(std::iter::once(candidate))
                    }
                    Offered::Sample(sample) => {
                        self.extend_borrowed_samples(std::iter::once(sample))
                    }
                },
            }
        }
        let trace = self.trace_sources;
        let shards = self
            .frame
            .shards(plan, &mut scratch.touched)
            .into_iter()
            .map(parking_lot::Mutex::new)
            .collect::<Vec<_>>();
        let (items, ranges) = (&items, &scratch.ranges);
        let mut workers = vec![(); pool.workers()];
        crate::parallel::run_ordered(Some(pool), &mut workers, shards.len(), |_, range| {
            let mut shard = shards[range].lock();
            for &(position, slot) in &ranges[range] {
                offer_in(&mut shard, trace, position, slot, items[position as usize]);
            }
        });
        for (kept, shard) in scratch.touched.iter_mut().zip(shards) {
            *kept = shard.into_inner().into_touched();
        }
        self.frame
            .merge_touched(offers, &scratch.touched, &mut scratch.by_position);
        scratch.touched.iter_mut().for_each(Vec::clear);
    }
}

/// One numbered offer to its range, exactly as [`EngineContributionResolver::add_borrowed`]
/// offers it.
fn offer_in(
    shard: &mut FrameShard<'_>,
    trace: bool,
    position: u32,
    slot: crate::Slot,
    item: Offered<'_>,
) {
    match item {
        Offered::Contribution(candidate) => {
            let value = &candidate.value;
            shard.offer_with_origin(
                position,
                slot,
                offer_of(value, candidate.transition_ordinal),
                contribution_trace(trace, candidate),
                |winner| {
                    winner.value = value.value.clone();
                    winner.pending_transition = candidate.pending_transition.clone();
                },
            );
        }
        Offered::Sample(sample) => {
            let value = sample.value();
            shard.offer_with_origin(
                position,
                slot,
                offer_of(value, sample.transition_ordinal()),
                sample_trace(trace, sample),
                |winner| {
                    winner.value = value.value.clone();
                    winner.pending_transition = None;
                },
            );
        }
    }
}

/// What a contribution offers its slot, apart from the value.
pub(super) fn offer_of(value: &TimedValue, transition_ordinal: Option<u64>) -> crate::Offer {
    crate::Offer {
        priority: value.priority,
        changed_at: value.changed_at,
        merge_mode: value.merge_mode,
        transition_ordinal,
        normalized: value.value.normalized().unwrap_or(0.0),
    }
}

/// A borrowed contribution's traced origin and evidence (none unless tracing).
pub(super) fn contribution_trace(
    trace: bool,
    candidate: &EngineContribution,
) -> (
    OfferedOrigin<'_>,
    Option<&std::sync::Arc<crate::ContributionFamilyEvidence>>,
) {
    let origin = match (&candidate.origin, candidate.playback_source) {
        _ if !trace => OfferedOrigin::None,
        (Some(origin), _) => OfferedOrigin::Shared(origin),
        (None, Some(source)) => OfferedOrigin::Built {
            source: std::borrow::Cow::Owned(crate::ContributionSourceId::playback(source)),
            value: &candidate.value,
            transition_ordinal: candidate.transition_ordinal,
        },
        (None, None) => OfferedOrigin::None,
    };
    let evidence = trace
        .then_some(candidate.family_evidence.as_ref())
        .flatten();
    (origin, evidence)
}

/// A sample's traced origin and evidence (none unless tracing).
pub(super) fn sample_trace(
    trace: bool,
    sample: &crate::ContributionSample,
) -> (
    OfferedOrigin<'_>,
    Option<&std::sync::Arc<crate::ContributionFamilyEvidence>>,
) {
    let origin = match sample.replacement_source() {
        Some(source) if trace => OfferedOrigin::Built {
            source: std::borrow::Cow::Borrowed(source),
            value: sample.value(),
            transition_ordinal: sample.transition_ordinal(),
        },
        _ => OfferedOrigin::None,
    };
    let evidence = trace.then(|| sample.family_evidence()).flatten();
    (origin, evidence)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn timed(
        fixture: FixtureId,
        attribute: &str,
        level: f32,
        at: i64,
        mode: MergeMode,
    ) -> TimedValue {
        TimedValue {
            fixture_id: fixture,
            attribute: AttributeKey(attribute.into()),
            value: AttributeValue::Normalized(level),
            priority: if attribute == "zoom" { 5 } else { 0 },
            changed_at: Utc.timestamp_millis_opt(at).unwrap(),
            programmer_order: 0,
            merge_mode: mode,
            fade: false,
            fade_millis: None,
            delay_millis: None,
        }
    }

    /// Every winner of a fill in first-write order, and the overflow, as comparable rows.
    fn rows(resolver: &EngineContributionResolver<'_>) -> Vec<String> {
        let mut rows = resolver
            .frame
            .occupied()
            .map(|(slot, winner)| {
                format!(
                    "{} {:?} {} {} {:?} {:?} {:?} {:?}",
                    slot.index(),
                    winner.value,
                    winner.priority,
                    winner.changed_at,
                    winner.merge_mode,
                    winner.transition_ordinal,
                    winner.origin,
                    winner.family_evidence.as_ref().map(std::sync::Arc::as_ptr),
                )
            })
            .collect::<Vec<_>>();
        let mut overflow = resolver
            .overflow
            .iter()
            .map(|((fixture, attribute), winner)| {
                format!(
                    "overflow {fixture:?} {attribute:?} {:?} {:?}",
                    winner.value, winner.origin
                )
            })
            .collect::<Vec<_>>();
        overflow.sort();
        rows.extend(overflow);
        rows
    }

    /// TL-639 round 7: offers on the pool by slot range resolve exactly the single-threaded
    /// fill: winners, first-write order, traced origins and evidence, and the overflow, for
    /// LTP and HTP slots, priorities, ties, addressed and named pairs and unnumbered names.
    #[test]
    fn offers_by_slot_range_resolve_the_single_threaded_fill() {
        let fixtures = (1..=6)
            .map(|id| FixtureId(uuid::Uuid::from_u128(id)))
            .collect::<Vec<_>>();
        let patched = fixtures
            .iter()
            .map(|fixture| {
                crate::frame_slots::legacy_test_fixture(
                    *fixture,
                    &["intensity", "pan", "tilt", "zoom"],
                )
            })
            .collect::<Vec<_>>();
        let slots = std::sync::Arc::new(crate::SlotTable::compile(7, &patched));
        let source = crate::ContributionSourceId::programmer_transient(
            light_core::ProgrammerId(uuid::Uuid::from_u128(9)),
            "sampled",
        );
        let evidence = std::sync::Arc::new(crate::ContributionFamilyEvidence::new(Vec::new()));
        let attributes = ["intensity", "pan", "tilt", "zoom", "gobo"];
        let mut playback = Vec::new();
        let mut programmer = Vec::new();
        let mut samples = Vec::new();
        for (index, fixture) in fixtures.iter().enumerate().rev() {
            for (offset, attribute) in attributes.iter().enumerate() {
                let mode = if *attribute == "intensity" {
                    MergeMode::Htp
                } else {
                    MergeMode::Ltp
                };
                let at = (index * 7 + offset * 3) as i64 % 11;
                let mut played = EngineContribution::unscaled(timed(
                    *fixture,
                    attribute,
                    0.1 * offset as f32,
                    at,
                    mode,
                ));
                played.transition_ordinal = Some(offset as u64);
                playback.push(played);
                let mut edit = EngineContribution::unscaled(timed(
                    *fixture,
                    attribute,
                    0.05 * index as f32,
                    5,
                    mode,
                ));
                if index % 2 == 0 {
                    edit.family_evidence = Some(std::sync::Arc::clone(&evidence));
                }
                programmer.push(edit);
                let address = slots
                    .slot(*fixture, &AttributeKey((*attribute).into()))
                    .map(|slot| light_core::FrameAddress {
                        generation: 7,
                        slot: slot.index() as u32,
                    });
                let value = timed(*fixture, attribute, 0.3, at + 1, mode);
                samples.push(if offset % 2 == 0 {
                    crate::ContributionSample::replacing(value, source.clone()).at(address)
                } else {
                    crate::ContributionSample::independent(value)
                });
            }
        }
        let pool = OutputPool::new(4).expect("a pool of four");
        for trace in [false, true] {
            let resolver = |trace| {
                let resolver = EngineContributionResolver::unpooled(&slots);
                if trace {
                    resolver.tracing_sources()
                } else {
                    resolver
                }
            };
            let mut single = resolver(trace);
            single.extend_borrowed_contributions(&playback);
            single.extend_borrowed_contributions(&programmer);
            single.extend_borrowed_samples(&samples);
            let mut ranged = resolver(trace);
            ranged.offer_borrowed_on(
                (Some(&pool), &parking_lot::Mutex::default()),
                (playback.iter(), &programmer, samples.iter()),
                playback.len() + programmer.len() + samples.len(),
            );
            assert_eq!(rows(&ranged), rows(&single), "tracing {trace}");
            assert!(
                !single.overflow.is_empty(),
                "unnumbered names reach the overflow"
            );
            // From a pool thread (a lazily observed source answering a parallel section) the
            // offers stay on that thread, with the same frame.
            let nested = parking_lot::Mutex::new(resolver(trace));
            light_dynamics::InstanceWorkers::run_indexed(&pool, 2, &|index| {
                assert!(rayon::current_thread_index().is_some());
                if index == 0 {
                    nested.lock().offer_borrowed_on(
                        (Some(&pool), &parking_lot::Mutex::default()),
                        (playback.iter(), &programmer, samples.iter()),
                        playback.len() + programmer.len() + samples.len(),
                    );
                }
            });
            assert_eq!(
                rows(&nested.into_inner()),
                rows(&single),
                "nested, tracing {trace}"
            );
        }
    }
}
