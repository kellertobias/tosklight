//! Bounded reconstruction of one sender's PSN frames.
//!
//! PSN v2.02 has an eight-bit frame ID and packet count, but no packet ordinal or sender
//! session ID. Packet timestamps describe send time; split packets need not share one.
//! See <https://posistage.net/wp-content/uploads/2019/01/PosiStageNetprotocol_v2.02_2016_09_15.pdf>,
//! pages 5–7. Repeated decoded packets are conservatively ignored. Overlapping tracker
//! records or conflicting counts cannot prove completeness; retain the last accepted frame.

use crate::{PSN_MAX_PACKET_BYTES, PsnDataPacket, PsnTrackerData, PsnVector3};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PsnFrame {
    pub frame_id: u8,
    /// Earliest packet send timestamp in this frame, on the sender's clock.
    pub timestamp_micros: u64,
    pub trackers: Vec<PsnTrackerData>,
    /// False for discarded partial or ambiguous frames. These must not move live targets.
    pub complete: bool,
    pub packets_received: u8,
    pub packets_expected: u8,
}

/// Passive diagnostics; none of these releases a held positional sample.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PsnFrameRejection {
    Duplicate,
    /// Includes possible sender restart: the protocol cannot distinguish delayed old data.
    OutOfOrderOrRestart,
    /// Ambiguous serial distance, or split membership without a provable unique packet.
    Ambiguous,
    InconsistentPacketCount,
    Oversized,
}

/// One packet can close an incomplete frame and finish the next frame simultaneously.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PsnFrameUpdate {
    pub discarded: Option<PsnFrame>,
    pub completed: Option<PsnFrame>,
    pub rejection: Option<PsnFrameRejection>,
    pub started_new_frame: bool,
}

#[derive(Clone, Copy, Debug)]
struct Boundary {
    frame_id: u8,
    timestamp_micros: u64,
    complete: bool,
}

#[derive(Debug)]
struct OpenFrame {
    frame_id: u8,
    first_timestamp_micros: u64,
    last_timestamp_micros: u64,
    expected: u8,
    packets: Vec<PsnDataPacket>,
    trackers: BTreeMap<u16, PsnTrackerData>,
    ambiguous: bool,
}

impl OpenFrame {
    fn finish(self, complete: bool) -> PsnFrame {
        PsnFrame {
            frame_id: self.frame_id,
            timestamp_micros: self.first_timestamp_micros,
            trackers: self.trackers.into_values().collect(),
            complete,
            packets_received: self.packets.len() as u8,
            packets_expected: self.expected,
        }
    }
    fn boundary(&self) -> Boundary {
        Boundary {
            frame_id: self.frame_id,
            timestamp_micros: self.last_timestamp_micros,
            complete: false,
        }
    }
}

/// One assembler per sender and explicitly established source epoch. Storage is bounded by
/// 255 packets of protocol-bounded tracker content and one closed boundary; no history queue.
#[derive(Debug, Default)]
pub struct PsnFrameAssembler {
    open: Option<OpenFrame>,
    closed: Option<Boundary>,
}

impl PsnFrameAssembler {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            open: None,
            closed: None,
        }
    }

    /// Compatibility convenience: prefer a newly complete frame over a discarded predecessor.
    /// Use push_detailed when the caller also needs incomplete/rejection diagnostics.
    pub fn push(&mut self, packet: PsnDataPacket) -> Option<PsnFrame> {
        let update = self.push_detailed(&packet);
        update.completed.or(update.discarded)
    }

    pub fn push_detailed(&mut self, packet: &PsnDataPacket) -> PsnFrameUpdate {
        let mut update = PsnFrameUpdate::default();
        // Each tracker needs at least its chunk header. Bound direct decoded-packet callers too.
        if packet.trackers.len() > PSN_MAX_PACKET_BYTES / 4 {
            update.rejection = Some(PsnFrameRejection::Oversized);
            return update;
        }
        let same_open = self
            .open
            .as_ref()
            .is_some_and(|open| open.frame_id == packet.header.frame_id);
        if same_open {
            // Split packets can arrive in either order. Only the closed frame is a lower bound.
            if self
                .closed
                .is_some_and(|closed| packet.header.timestamp_micros < closed.timestamp_micros)
            {
                update.rejection = Some(PsnFrameRejection::OutOfOrderOrRestart);
                return update;
            }
        } else {
            let boundary = self.open.as_ref().map(OpenFrame::boundary).or(self.closed);
            if let Some(boundary) = boundary {
                if let Err(rejection) = newer(packet, boundary) {
                    update.rejection = Some(rejection);
                    return update;
                }
            }
            if let Some(previous) = self.open.take() {
                self.closed = Some(previous.boundary());
                update.discarded = Some(previous.finish(false));
            }
            self.open = Some(OpenFrame {
                frame_id: packet.header.frame_id,
                first_timestamp_micros: packet.header.timestamp_micros,
                last_timestamp_micros: packet.header.timestamp_micros,
                expected: packet.header.frame_packet_count.max(1),
                packets: Vec::new(),
                trackers: BTreeMap::new(),
                ambiguous: false,
            });
            update.started_new_frame = true;
        }
        let open = self.open.as_mut().expect("packet has an open frame");
        if open.packets.iter().any(|seen| same_packet(seen, packet)) {
            update.rejection = Some(PsnFrameRejection::Duplicate);
            return update;
        }
        if open.expected != packet.header.frame_packet_count.max(1) {
            open.ambiguous = true;
            update.rejection = Some(PsnFrameRejection::InconsistentPacketCount);
            return update;
        }
        if open.ambiguous {
            update.rejection = Some(PsnFrameRejection::Ambiguous);
            return update;
        }
        // No ordinal distinguishes retransmission/correction from another part. Disjoint
        // tracker chunks are the split membership which this conservative receiver accepts.
        let overlapping = packet
            .trackers
            .iter()
            .any(|tracker| open.trackers.contains_key(&tracker.id));
        let repeated_in_packet = packet.trackers.iter().enumerate().any(|(index, tracker)| {
            packet.trackers[..index]
                .iter()
                .any(|previous| previous.id == tracker.id)
        });
        if overlapping || repeated_in_packet || (open.expected > 1 && packet.trackers.is_empty()) {
            open.ambiguous = true;
            update.rejection = Some(PsnFrameRejection::Ambiguous);
            return update;
        }
        open.first_timestamp_micros = open
            .first_timestamp_micros
            .min(packet.header.timestamp_micros);
        open.last_timestamp_micros = open
            .last_timestamp_micros
            .max(packet.header.timestamp_micros);
        for tracker in &packet.trackers {
            open.trackers.insert(tracker.id, *tracker);
        }
        open.packets.push(packet.clone());
        if open.packets.len() == usize::from(open.expected) {
            let complete = self.open.take().expect("complete frame remains open");
            self.closed = Some(Boundary {
                complete: true,
                ..complete.boundary()
            });
            update.completed = Some(complete.finish(true));
        }
        update
    }

    /// Discard the current partial frame; keep its boundary so late data cannot revive it.
    pub fn flush(&mut self) -> Option<PsnFrame> {
        let open = self.open.take()?;
        self.closed = Some(open.boundary());
        Some(open.finish(false))
    }

    /// Explicit source/session boundary. Timestamp regression alone is not proof of restart.
    pub fn reset_source_epoch(&mut self) {
        self.open = None;
        self.closed = None;
    }
}

fn newer(packet: &PsnDataPacket, boundary: Boundary) -> Result<(), PsnFrameRejection> {
    use std::cmp::Ordering;
    if !boundary.complete && packet.header.frame_id == boundary.frame_id {
        return Err(PsnFrameRejection::Ambiguous);
    }
    match packet
        .header
        .timestamp_micros
        .cmp(&boundary.timestamp_micros)
    {
        Ordering::Greater => Ok(()),
        Ordering::Less => Err(PsnFrameRejection::OutOfOrderOrRestart),
        Ordering::Equal => match packet.header.frame_id.wrapping_sub(boundary.frame_id) {
            0 => Err(PsnFrameRejection::Duplicate),
            1..=127 => Ok(()),
            128 => Err(PsnFrameRejection::Ambiguous),
            _ => Err(PsnFrameRejection::OutOfOrderOrRestart),
        },
    }
}

// Bitwise comparisons recognize retransmitted NaNs without accepting invalid positions.
// This compares decoded content, not an invented protocol packet ID.
fn same_packet(left: &PsnDataPacket, right: &PsnDataPacket) -> bool {
    left.header == right.header
        && left.trackers.len() == right.trackers.len()
        && left
            .trackers
            .iter()
            .zip(&right.trackers)
            .all(|(left, right)| {
                left.id == right.id
                    && bits(left.position) == bits(right.position)
                    && bits(left.speed) == bits(right.speed)
                    && bits(left.orientation) == bits(right.orientation)
                    && bits(left.acceleration) == bits(right.acceleration)
                    && bits(left.target_position) == bits(right.target_position)
                    && left.validity.map(f32::to_bits) == right.validity.map(f32::to_bits)
                    && left.timestamp_micros == right.timestamp_micros
            })
}
fn bits(value: Option<PsnVector3>) -> Option<[u32; 3]> {
    value.map(|value| [value.x.to_bits(), value.y.to_bits(), value.z.to_bits()])
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod tests;
