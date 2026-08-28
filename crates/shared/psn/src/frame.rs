//! Putting one frame back together from the packets it was split into.
//!
//! A PSN datagram is capped at 1500 bytes, so a sender watching many trackers splits one moment
//! into several packets and stamps them all with the same `frame_id` and the number of packets it
//! used. A receiver that reads packets one at a time sees half a stage move, then the other half.
//!
//! This puts them back together. It is deliberately state over packets and nothing else: no clock,
//! no socket, no timeouts. A receiver that wants to age a frame out owns a clock and can; what a
//! frame *is* is decided here, and can be tested by feeding it packets.

use crate::{PsnDataPacket, PsnTrackerData};

/// One moment as the sender described it, from however many packets that took.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PsnFrame {
    pub frame_id: u8,
    /// The sender's timestamp from the frame's first packet.
    pub timestamp_micros: u64,
    pub trackers: Vec<PsnTrackerData>,
    /// Whether every packet the sender said this frame had actually arrived.
    ///
    /// An incomplete frame is still worth having: the trackers that did arrive are current, and the
    /// rest simply were not updated this time round. A caller that must not act on a partial view
    /// can check this; one that only reads the tracker it cares about need not.
    pub complete: bool,
    pub packets_received: u8,
    pub packets_expected: u8,
}

/// Collects packets until a frame is whole.
///
/// One assembler belongs to one sender. Two senders on the same group will interleave `frame_id`s
/// that mean nothing to each other, so a receiver that hears two must keep an assembler per source
/// address rather than feeding both into this.
#[derive(Debug, Default)]
pub struct PsnFrameAssembler {
    open: Option<PsnFrame>,
}

impl PsnFrameAssembler {
    #[must_use]
    pub const fn new() -> Self {
        Self { open: None }
    }

    /// Take one packet, and hand back a frame when one is finished.
    ///
    /// A frame finishes either because every packet the sender promised has arrived, or because a
    /// packet of the *next* frame turned up first — which is how a lost packet is noticed at all.
    /// The frame handed back in that second case is marked incomplete rather than dropped, because
    /// the trackers in it are real and current.
    pub fn push(&mut self, packet: PsnDataPacket) -> Option<PsnFrame> {
        let mut finished = None;
        let starting_new_frame = self
            .open
            .as_ref()
            .is_some_and(|open| open.frame_id != packet.header.frame_id);
        if starting_new_frame {
            finished = self.open.take();
        }
        let open = self.open.get_or_insert_with(|| PsnFrame {
            frame_id: packet.header.frame_id,
            timestamp_micros: packet.header.timestamp_micros,
            trackers: Vec::new(),
            complete: false,
            packets_received: 0,
            packets_expected: packet.header.frame_packet_count,
        });
        // A sender may revise how many packets it needed; the latest word is the one to believe.
        open.packets_expected = packet.header.frame_packet_count;
        open.packets_received = open.packets_received.saturating_add(1);
        for tracker in packet.trackers {
            merge(&mut open.trackers, tracker);
        }
        // `frame_packet_count` is documented as how many packets to expect, and a sender that says
        // zero has still sent this one, so a single-packet frame is complete either way.
        if open.packets_received >= open.packets_expected.max(1) {
            let mut whole = self.open.take().unwrap_or_default();
            whole.complete = true;
            // The finished-early frame is handed back first only if both happen at once, which
            // cannot: a frame that completes here was opened by this very packet or an earlier one
            // of the same id, so at most one of the two is Some.
            return finished.or(Some(whole));
        }
        finished
    }

    /// Give up the frame in progress, if any, and hand it back as it stands.
    ///
    /// A sender that stops mid-frame leaves the last packets held here forever otherwise. A
    /// receiver with a clock calls this when the source has gone quiet.
    pub fn flush(&mut self) -> Option<PsnFrame> {
        self.open.take()
    }
}

/// One tracker per id, last word winning.
///
/// A tracker appearing twice in one frame is the sender correcting itself, so the later record
/// replaces the earlier one in place rather than being appended beside it.
fn merge(trackers: &mut Vec<PsnTrackerData>, tracker: PsnTrackerData) {
    if let Some(existing) = trackers
        .iter_mut()
        .find(|existing| existing.id == tracker.id)
    {
        *existing = tracker;
    } else {
        trackers.push(tracker);
    }
}
