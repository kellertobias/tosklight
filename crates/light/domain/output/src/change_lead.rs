//! TL-659: change lead time, the longest time from when a Cue or Dynamic should start outputting
//! to when the output frame that first carries it was sent.
//!
//! The domains mark start instants in their `light_core::ChangeLeadLedger`; the output frame that
//! first carries them claims the earliest one and hands it here once the frame is on the wire.
//! Everything below is a handful of atomic operations on fixed storage: recording a frame takes
//! no lock and never allocates, so it adds only constant-time timestamp work to the output path.
//! Times are microseconds of application time, the clock the starts are scheduled on.

use serde::Serialize;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};

/// Seconds of recent history behind the recent maximum, the same window as every other
/// `recent_*` output reading.
pub const CHANGE_LEAD_RECENT_WINDOW_SECONDS: u32 = crate::OUTPUT_RECENT_WINDOW.as_secs() as u32;

/// A lead longer than this is not a start reaching output late: it is a running source being
/// re-established (an edited Dynamic restarting on its original activation, say), whose start
/// instant lies long in the past. It is counted as excluded instead of as the maximum.
pub const CHANGE_LEAD_PLAUSIBLE_LIMIT_MICROS: u64 = 5_000_000;

const RECENT_BUCKETS: usize = CHANGE_LEAD_RECENT_WINDOW_SECONDS as usize;
const NO_START: i64 = i64::MAX;

/// Change lead readings for operators.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ChangeLeadSnapshot {
    /// Longest lead since the desk started or the operator last reset it.
    pub maximum_micros: Option<u64>,
    /// Longest lead sent within the last [`CHANGE_LEAD_RECENT_WINDOW_SECONDS`].
    pub recent_maximum_micros: Option<u64>,
    /// Lead of the most recently measured start.
    pub last_micros: Option<u64>,
    /// Frames that carried at least one start since the last reset.
    pub samples: u64,
    /// Starts whose lead exceeded [`CHANGE_LEAD_PLAUSIBLE_LIMIT_MICROS`].
    pub excluded: u64,
}

/// Shared between the output lane, which records, and readers, which snapshot or reset.
#[derive(Debug)]
pub struct ChangeLeadTime {
    maximum: AtomicU64,
    last: AtomicU64,
    samples: AtomicU64,
    excluded: AtomicU64,
    /// Per application-time second: `(second << 32) | longest lead in that second`.
    recent: [AtomicU64; RECENT_BUCKETS],
    /// Earliest start claimed by a frame that was never sent, carried to the next sent frame.
    carried: AtomicI64,
}

impl Default for ChangeLeadTime {
    fn default() -> Self {
        Self {
            maximum: AtomicU64::new(0),
            last: AtomicU64::new(0),
            samples: AtomicU64::new(0),
            excluded: AtomicU64::new(0),
            recent: std::array::from_fn(|_| AtomicU64::new(0)),
            carried: AtomicI64::new(NO_START),
        }
    }
}

impl ChangeLeadTime {
    /// One output frame left the desk at `sent_micros`, carrying starts from `frame_start` on.
    ///
    /// A start claimed by an earlier frame that never reached the wire is carried by this one.
    pub fn frame_sent(&self, frame_start: Option<i64>, sent_micros: i64) {
        let carried = self.carried.swap(NO_START, Ordering::AcqRel);
        let carried = (carried != NO_START).then_some(carried);
        let Some(start) = light_core::earliest_start(frame_start, carried) else {
            return;
        };
        let lead = u64::try_from(sent_micros.saturating_sub(start)).unwrap_or(0);
        if lead > CHANGE_LEAD_PLAUSIBLE_LIMIT_MICROS {
            self.excluded.fetch_add(1, Ordering::Relaxed);
            return;
        }
        self.maximum.fetch_max(lead, Ordering::Relaxed);
        self.last.store(lead, Ordering::Relaxed);
        self.samples.fetch_add(1, Ordering::Relaxed);
        let second = second_of(sent_micros);
        let bucket = &self.recent[second as usize % RECENT_BUCKETS];
        // Leads stay below the plausibility limit, so they fit the low 32 bits.
        let _ = bucket.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |packed| {
            let previous = if packed >> 32 == second {
                packed & u64::from(u32::MAX)
            } else {
                0
            };
            Some((second << 32) | previous.max(lead))
        });
    }

    /// The frame that claimed `frame_start` was not sent; the next sent frame carries it.
    pub fn frame_not_sent(&self, frame_start: Option<i64>) {
        if let Some(start) = frame_start {
            self.carried.fetch_min(start, Ordering::AcqRel);
        }
    }

    /// Readings at application time `now_micros`.
    pub fn snapshot(&self, now_micros: i64) -> ChangeLeadSnapshot {
        let samples = self.samples.load(Ordering::Relaxed);
        let measured = |value: u64| (samples > 0).then_some(value);
        let now = second_of(now_micros);
        let recent = self
            .recent
            .iter()
            .map(|bucket| bucket.load(Ordering::Relaxed))
            .filter(|packed| {
                let second = packed >> 32;
                *packed != 0 && second <= now && now - second < RECENT_BUCKETS as u64
            })
            .map(|packed| packed & u64::from(u32::MAX))
            .max();
        ChangeLeadSnapshot {
            maximum_micros: measured(self.maximum.load(Ordering::Relaxed)),
            recent_maximum_micros: recent,
            last_micros: measured(self.last.load(Ordering::Relaxed)),
            samples,
            excluded: self.excluded.load(Ordering::Relaxed),
        }
    }

    /// The operator's reset: every reading starts over. A start already claimed but not yet
    /// sent is still measured.
    pub fn reset(&self) {
        self.samples.store(0, Ordering::Relaxed);
        self.maximum.store(0, Ordering::Relaxed);
        self.last.store(0, Ordering::Relaxed);
        self.excluded.store(0, Ordering::Relaxed);
        for bucket in &self.recent {
            bucket.store(0, Ordering::Relaxed);
        }
    }
}

/// Application-time second of `micros`, clamped into the 32 bits a bucket keeps.
fn second_of(micros: i64) -> u64 {
    u64::try_from(micros.div_euclid(1_000_000))
        .unwrap_or(0)
        .min(u64::from(u32::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A deterministic application clock: every instant is written out, none is read from a
    /// system clock.
    const ORIGIN: i64 = 1_800_000_000_000_000;

    fn at(millis: i64) -> i64 {
        ORIGIN + millis * 1_000
    }

    #[test]
    fn the_longest_lead_is_reported_as_the_maximum() {
        let lead = ChangeLeadTime::default();
        // A Cue GO 10 ms before the frame that carried it was sent.
        lead.frame_sent(Some(at(0)), at(10));
        // A Dynamic started 41 ms before its first frame left.
        lead.frame_sent(Some(at(100)), at(141));
        // Frames without a start record nothing.
        lead.frame_sent(None, at(160));
        // A scheduled start sent 3 ms after its beat.
        lead.frame_sent(Some(at(200)), at(203));

        let snapshot = lead.snapshot(at(203));
        assert_eq!(snapshot.maximum_micros, Some(41_000));
        assert_eq!(snapshot.recent_maximum_micros, Some(41_000));
        assert_eq!(snapshot.last_micros, Some(3_000));
        assert_eq!(snapshot.samples, 3);
        assert_eq!(snapshot.excluded, 0);
    }

    #[test]
    fn the_recent_maximum_forgets_leads_older_than_its_window() {
        let lead = ChangeLeadTime::default();
        lead.frame_sent(Some(at(0)), at(50));
        lead.frame_sent(Some(at(30_000)), at(30_020));

        let later = at(61_000);
        let snapshot = lead.snapshot(later);
        assert_eq!(snapshot.maximum_micros, Some(50_000), "the maximum stays");
        assert_eq!(snapshot.recent_maximum_micros, Some(20_000));
        assert_eq!(lead.snapshot(at(200_000)).recent_maximum_micros, None);
    }

    #[test]
    fn a_start_of_a_frame_that_was_not_sent_is_measured_by_the_next_sent_frame() {
        let lead = ChangeLeadTime::default();
        lead.frame_not_sent(Some(at(0)));
        lead.frame_sent(None, at(45));

        assert_eq!(lead.snapshot(at(45)).maximum_micros, Some(45_000));
    }

    #[test]
    fn a_re_established_source_is_excluded_rather_than_reported() {
        let lead = ChangeLeadTime::default();
        lead.frame_sent(Some(at(0)), at(60_000));

        let snapshot = lead.snapshot(at(60_000));
        assert_eq!(snapshot.maximum_micros, None);
        assert_eq!(snapshot.excluded, 1);
    }

    #[test]
    fn reset_starts_every_reading_over() {
        let lead = ChangeLeadTime::default();
        lead.frame_sent(Some(at(0)), at(30));
        lead.reset();

        assert_eq!(lead.snapshot(at(30)), ChangeLeadSnapshot::default());
        lead.frame_sent(Some(at(40)), at(52));
        assert_eq!(lead.snapshot(at(52)).maximum_micros, Some(12_000));
    }
}
