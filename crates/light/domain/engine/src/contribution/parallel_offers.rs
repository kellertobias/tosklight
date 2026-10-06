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

/// One offer of a resolution.
#[derive(Clone, Copy)]
enum Offered<'a> {
    Contribution(&'a EngineContribution),
    Sample(&'a crate::ContributionSample),
}

impl EngineContributionResolver<'_> {
    /// Offer `playback`, then `programmer`, then `samples` (borrowed throughout), on `pool`
    /// when there are enough offers; the same frame as the single-threaded calls.
    pub(crate) fn offer_borrowed_on(
        &mut self,
        pool: Option<&OutputPool>,
        playback: &[&EngineContribution],
        programmer: &[&EngineContribution],
        samples: &[&crate::ContributionSample],
    ) {
        let offers = playback.len() + programmer.len() + samples.len();
        let Some(pool) = pool.filter(|pool| pool.workers() > 1 && offers >= MIN_PARALLEL_OFFERS)
        else {
            self.extend_borrowed_contributions(playback.iter().copied());
            self.extend_borrowed_contributions(programmer.iter().copied());
            self.extend_borrowed_samples(samples.iter().copied());
            return;
        };
        let items = playback
            .iter()
            .chain(programmer)
            .map(|candidate| Offered::Contribution(candidate))
            .chain(samples.iter().map(|sample| Offered::Sample(sample)));
        let plan = self.frame.shard_plan(pool.workers() * 4);
        let capacity = self.frame.capacity();
        let mut ranges = (0..plan.count())
            .map(|_| Vec::with_capacity(offers / plan.count() + offers / plan.count() / 4 + 16))
            .collect::<Vec<Vec<(u32, crate::Slot, Offered<'_>)>>>();
        for (position, item) in items.enumerate() {
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
                    ranges[plan.shard_of(slot)].push((position as u32, slot, item));
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
            .shards(plan)
            .into_iter()
            .map(parking_lot::Mutex::new)
            .collect::<Vec<_>>();
        let mut scratch = vec![(); pool.workers()];
        crate::parallel::run_ordered(Some(pool), &mut scratch, shards.len(), |_, range| {
            let mut shard = shards[range].lock();
            for &(position, slot, item) in &ranges[range] {
                offer_in(&mut shard, trace, position, slot, item);
            }
        });
        let touched = shards
            .into_iter()
            .map(|shard| shard.into_inner().into_touched())
            .collect::<Vec<_>>();
        self.frame.merge_touched(offers, &touched);
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
                Some(&pool),
                &playback.iter().collect::<Vec<_>>(),
                &programmer.iter().collect::<Vec<_>>(),
                &samples.iter().collect::<Vec<_>>(),
            );
            assert_eq!(rows(&ranged), rows(&single), "tracing {trace}");
            assert!(
                !single.overflow.is_empty(),
                "unnumbered names reach the overflow"
            );
        }
    }
}
