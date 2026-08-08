//! Which sender owns a universe.
//!
//! Two consoles can address the same universe, deliberately or by accident. E1.31 says the highest
//! priority wins and that a source which stops sending is released after a timeout; this
//! implements that for both protocols, so an Art-Net sender is arbitrated by the same rules even
//! though its packets carry no priority of their own.
//!
//! Arbitration reads no clock. Every observation is stamped by the caller, so the timeout is
//! testable without waiting.

use std::collections::HashMap;
use std::net::IpAddr;
use std::time::Duration;

use media_domain::Timestamp;

/// How long a source keeps a universe after its last packet.
///
/// E1.31 specifies 2.5 seconds, and applying the same figure to Art-Net keeps the two protocols
/// behaving alike rather than one lingering longer than the other.
pub const SOURCE_TIMEOUT: Duration = Duration::from_millis(2_500);

/// A sender, as far as arbitration is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceKey {
    /// Art-Net carries no sender identity, so the address it came from is the identity.
    ArtNet { address: IpAddr },
    /// sACN carries a CID, which survives a sender changing address.
    Sacn { cid: [u8; 16] },
}

/// What arbitration decided about a packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Winner {
    /// This packet's sender owns the universe; apply it.
    Accepted,
    /// Another sender owns the universe at a higher priority; ignore this one.
    Outranked { by_priority: u8 },
    /// The packet repeats or predates one already applied from this sender.
    OutOfOrder,
}

#[derive(Debug, Clone, Copy)]
struct Observed {
    priority: u8,
    sequence: u8,
    last_seen: Timestamp,
}

/// Tracks who is sending what, per universe.
#[derive(Debug, Default)]
pub struct SourceArbiter {
    sources: HashMap<(u16, SourceKey), Observed>,
}

impl SourceArbiter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a packet and decides whether it should be applied.
    pub fn observe(
        &mut self,
        universe: u16,
        source: SourceKey,
        priority: u8,
        sequence: u8,
        now: Timestamp,
    ) -> Winner {
        self.expire(now);

        // A sequence number that goes backwards is a reordered or duplicated datagram. Zero
        // disables sequencing, so a sender using it is never rejected on this ground.
        if let Some(previous) = self.sources.get(&(universe, source))
            && sequence != 0
            && previous.sequence != 0
            && !is_newer(sequence, previous.sequence)
        {
            return Winner::OutOfOrder;
        }

        let highest_other = self
            .sources
            .iter()
            .filter(|((other_universe, other_source), _)| {
                *other_universe == universe && *other_source != source
            })
            .map(|(_, observed)| observed.priority)
            .max();

        self.sources.insert(
            (universe, source),
            Observed {
                priority,
                sequence,
                last_seen: now,
            },
        );

        match highest_other {
            Some(other) if other > priority => Winner::Outranked { by_priority: other },
            _ => Winner::Accepted,
        }
    }

    /// Forgets a source that has said it is finished, so its universe is released immediately
    /// rather than after the timeout.
    pub fn terminate(&mut self, universe: u16, source: SourceKey) {
        self.sources.remove(&(universe, source));
    }

    /// How many sources are currently live on a universe.
    pub fn sources_on(&self, universe: u16) -> usize {
        self.sources
            .keys()
            .filter(|(candidate, _)| *candidate == universe)
            .count()
    }

    /// Drops sources that have stopped sending.
    pub fn expire(&mut self, now: Timestamp) {
        self.sources
            .retain(|_, observed| now.since(observed.last_seen) < SOURCE_TIMEOUT);
    }
}

/// Whether `candidate` is newer than `previous`, accounting for the wrap at 255.
///
/// E1.31 treats a difference outside -20..0 as newer, which tolerates reordering without letting
/// a stale packet through after a wrap.
fn is_newer(candidate: u8, previous: u8) -> bool {
    let difference = candidate.wrapping_sub(previous) as i8;
    difference > 0 || difference < -20
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;

    fn art_net(last: u8) -> SourceKey {
        SourceKey::ArtNet {
            address: IpAddr::V4(Ipv4Addr::new(10, 0, 0, last)),
        }
    }

    fn sacn(marker: u8) -> SourceKey {
        SourceKey::Sacn { cid: [marker; 16] }
    }

    fn at(millis: u64) -> Timestamp {
        Timestamp::from_millis(millis)
    }

    #[test]
    fn a_single_sender_is_accepted() {
        let mut arbiter = SourceArbiter::new();
        assert_eq!(
            arbiter.observe(1, art_net(5), 100, 1, at(0)),
            Winner::Accepted
        );
        assert_eq!(
            arbiter.observe(1, art_net(5), 100, 2, at(20)),
            Winner::Accepted
        );
        assert_eq!(arbiter.sources_on(1), 1);
    }

    #[test]
    fn the_higher_priority_sender_wins_the_universe() {
        let mut arbiter = SourceArbiter::new();
        arbiter.observe(1, sacn(1), 150, 1, at(0));
        assert_eq!(
            arbiter.observe(1, sacn(2), 100, 1, at(10)),
            Winner::Outranked { by_priority: 150 }
        );
        assert_eq!(
            arbiter.observe(1, sacn(1), 150, 2, at(20)),
            Winner::Accepted
        );
    }

    #[test]
    fn equal_priorities_both_apply_rather_than_one_being_silently_dropped() {
        // Deliberately permissive: two senders at the same priority is a patching mistake the
        // operator should see as flicker, not something to hide by picking one arbitrarily.
        let mut arbiter = SourceArbiter::new();
        assert_eq!(arbiter.observe(1, sacn(1), 100, 1, at(0)), Winner::Accepted);
        assert_eq!(
            arbiter.observe(1, sacn(2), 100, 1, at(10)),
            Winner::Accepted
        );
        assert_eq!(arbiter.sources_on(1), 2);
    }

    #[test]
    fn a_repeated_or_reordered_sequence_is_dropped() {
        let mut arbiter = SourceArbiter::new();
        arbiter.observe(1, sacn(1), 100, 10, at(0));
        assert_eq!(
            arbiter.observe(1, sacn(1), 100, 10, at(10)),
            Winner::OutOfOrder
        );
        assert_eq!(
            arbiter.observe(1, sacn(1), 100, 9, at(20)),
            Winner::OutOfOrder
        );
        assert_eq!(
            arbiter.observe(1, sacn(1), 100, 11, at(30)),
            Winner::Accepted
        );
    }

    #[test]
    fn the_sequence_wrap_at_255_is_not_mistaken_for_a_stale_packet() {
        let mut arbiter = SourceArbiter::new();
        arbiter.observe(1, sacn(1), 100, 254, at(0));
        assert_eq!(
            arbiter.observe(1, sacn(1), 100, 255, at(10)),
            Winner::Accepted
        );
        assert_eq!(
            arbiter.observe(1, sacn(1), 100, 1, at(20)),
            Winner::Accepted,
            "wrapped"
        );
    }

    #[test]
    fn a_sender_using_sequence_zero_is_never_rejected_for_it() {
        let mut arbiter = SourceArbiter::new();
        for millis in [0, 10, 20, 30] {
            assert_eq!(
                arbiter.observe(1, art_net(5), 100, 0, at(millis)),
                Winner::Accepted
            );
        }
    }

    #[test]
    fn a_source_that_stops_sending_releases_its_universe() {
        let mut arbiter = SourceArbiter::new();
        arbiter.observe(1, sacn(1), 200, 1, at(0));
        assert_eq!(arbiter.sources_on(1), 1);

        let expiry = SOURCE_TIMEOUT.as_millis() as u64;
        arbiter.expire(at(expiry - 1));
        assert_eq!(arbiter.sources_on(1), 1, "still within the timeout");

        arbiter.expire(at(expiry));
        assert_eq!(arbiter.sources_on(1), 0);
    }

    #[test]
    fn a_lower_priority_sender_takes_over_once_the_higher_one_stops() {
        let mut arbiter = SourceArbiter::new();
        arbiter.observe(1, sacn(1), 200, 1, at(0));
        assert!(matches!(
            arbiter.observe(1, sacn(2), 100, 1, at(10)),
            Winner::Outranked { .. }
        ));

        let after = SOURCE_TIMEOUT.as_millis() as u64 + 100;
        assert_eq!(
            arbiter.observe(1, sacn(2), 100, 2, at(after)),
            Winner::Accepted
        );
    }

    #[test]
    fn a_terminated_stream_releases_its_universe_at_once() {
        let mut arbiter = SourceArbiter::new();
        arbiter.observe(1, sacn(1), 200, 1, at(0));
        arbiter.terminate(1, sacn(1));
        assert_eq!(arbiter.sources_on(1), 0);
        assert_eq!(
            arbiter.observe(1, sacn(2), 100, 1, at(10)),
            Winner::Accepted,
            "without waiting out the timeout"
        );
    }

    #[test]
    fn universes_are_arbitrated_independently() {
        let mut arbiter = SourceArbiter::new();
        arbiter.observe(1, sacn(1), 200, 1, at(0));
        assert_eq!(
            arbiter.observe(2, sacn(2), 100, 1, at(10)),
            Winner::Accepted,
            "a high-priority sender on universe 1 says nothing about universe 2"
        );
    }

    #[test]
    fn the_two_protocols_identify_senders_differently_but_arbitrate_alike() {
        let mut arbiter = SourceArbiter::new();
        // An Art-Net sender is its address; an sACN sender is its CID, so the same machine
        // speaking both is two sources.
        assert_eq!(
            arbiter.observe(1, art_net(5), 100, 1, at(0)),
            Winner::Accepted
        );
        assert_eq!(arbiter.observe(1, sacn(1), 100, 1, at(0)), Winner::Accepted);
        assert_eq!(arbiter.sources_on(1), 2);
    }
}
