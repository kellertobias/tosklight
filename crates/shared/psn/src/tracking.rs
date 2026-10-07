//! What a listener remembers between packets.
//!
//! PSN is a one-way stream with no handshake, so a desk cannot ask a sender anything: everything it
//! knows, it knows because a packet said so and because it kept it. Three things have to be kept.
//!
//! **Names**, because a data packet carries only a tracker number and the names arrive once a
//! second in a different packet. **The last position of each tracker**, because a tracker that
//! stops being listed has not moved to the origin — it has stopped being reported, and those are
//! not the same. **When each of those last happened**, because "where the presenter is" is only
//! usable alongside "and that was 40 milliseconds ago".
//!
//! Time is passed in rather than read. The caller owns a clock; this owns what is true at a moment
//! it is told about, which is what makes staleness testable without waiting for it.

use crate::{
    PSN_MAX_PACKET_BYTES, PsnFrame, PsnFrameAssembler, PsnFrameRejection, PsnPacket,
    PsnTrackerData, PsnVector3,
};
use std::collections::BTreeMap;

/// What one datagram turned out to be worth.
#[derive(Clone, Debug, PartialEq)]
pub enum PsnObservation {
    /// A frame finished. Every tracker in it has been recorded.
    Frame(PsnFrame),
    /// A data packet that did not finish its frame yet.
    PartialFrame,
    /// An info packet. Names and the system name have been recorded.
    Info,
    /// The datagram was not usable. It is counted and dropped.
    Ignored(crate::PsnError),
    /// Readable data which cannot establish a new coherent sample. Hold previous positions.
    Rejected(PsnFrameRejection),
}

/// Local identity within one sender resource. A caller combines this with its source/show
/// identity. Sender timestamps never stand in for the receiver's monotonic arrival clock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PsnSampleId {
    pub source_epoch: u64,
    pub sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PsnAcceptedSample {
    pub id: PsnSampleId,
    pub frame_id: u8,
    pub sender_timestamp_micros: u64,
    /// Receiver time when all accepted parts were available.
    pub accepted_at_millis: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PsnIngressDiagnostics {
    pub duplicate_datagrams: u64,
    pub rejected_datagrams: u64,
    pub incomplete_frames: u64,
    pub ambiguous_datagrams: u64,
    pub invalid_positions: u64,
}

/// One tracker, as this listener currently understands it.
#[derive(Clone, Debug, PartialEq)]
pub struct PsnTracked {
    pub id: u16,
    /// What the sender calls it, once an info packet has said so.
    pub name: Option<String>,
    /// Latest metadata, with position retained from the last finite positional sample.
    /// Its authoritative age and identity are separate from optional metadata updates.
    pub data: PsnTrackerData,
    /// Receiver arrival of the last finite position (metadata cannot refresh position age).
    pub updated_at_millis: u64,
    pub position_sample: Option<PsnAcceptedSample>,
}

impl PsnTracked {
    /// How long ago this tracker was last reported.
    ///
    /// Saturating rather than signed: a caller passing a moment before the update is asking a
    /// question with no sensible answer, and zero is the least surprising one.
    #[must_use]
    pub const fn age_millis(&self, now_millis: u64) -> u64 {
        now_millis.saturating_sub(self.updated_at_millis)
    }

    /// Where it is, if the sender reported a position at all.
    #[must_use]
    pub const fn position(&self) -> Option<PsnVector3> {
        self.data.position
    }
}

/// How the source is doing, in the terms an operator needs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PsnSourceHealth {
    /// Nothing has ever arrived. Not an error — a sender that has not been switched on looks
    /// exactly like this, and so does a desk on the wrong network.
    Silent,
    /// Packets are arriving.
    Receiving,
    /// Packets arrived and then stopped. The positions held are the last ones heard, and they are
    /// this old.
    Stale { silent_for_millis: u64 },
}

/// Everything a listener knows about one sender.
#[derive(Debug, Default)]
pub struct PsnTracking {
    assembler: PsnFrameAssembler,
    names: BTreeMap<u16, String>,
    tracked: BTreeMap<u16, PsnTracked>,
    system_name: Option<String>,
    last_packet_at_millis: Option<u64>,
    last_info_at_millis: Option<u64>,
    frames: u64,
    ignored: u64,
    pending_arrivals: BTreeMap<u16, u64>,
    source_epoch: u64,
    sample_sequence: u64,
    accepted_sample: Option<PsnAcceptedSample>,
    diagnostics: PsnIngressDiagnostics,
}

impl PsnTracking {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Take one datagram, as of a moment on the caller's clock.
    pub fn observe(&mut self, datagram: &[u8], now_millis: u64) -> PsnObservation {
        if datagram.len() > PSN_MAX_PACKET_BYTES {
            return self.reject(PsnFrameRejection::Oversized);
        }
        match crate::decode(datagram) {
            Err(error) => {
                self.ignored = self.ignored.saturating_add(1);
                PsnObservation::Ignored(error)
            }
            Ok(PsnPacket::Info(info)) => {
                self.last_packet_at_millis = Some(now_millis);
                self.last_info_at_millis = Some(now_millis);
                self.system_name = info.system_name;
                // A sender may drop a tracker from its list, so the list replaces rather than
                // merges: a name that is no longer offered is no longer the tracker's name.
                self.names = info
                    .trackers
                    .into_iter()
                    .filter_map(|tracker| tracker.name.map(|name| (tracker.id, name)))
                    .collect();
                for (id, tracked) in &mut self.tracked {
                    tracked.name = self.names.get(id).cloned();
                }
                PsnObservation::Info
            }
            Ok(PsnPacket::Data(data)) => {
                let update = self.assembler.push_detailed(&data);
                if update.discarded.is_some() {
                    self.diagnostics.incomplete_frames =
                        self.diagnostics.incomplete_frames.saturating_add(1);
                }
                if update.started_new_frame {
                    self.pending_arrivals.clear();
                }
                if let Some(rejection) = update.rejection {
                    return self.reject(rejection);
                }
                for tracker in &data.trackers {
                    self.pending_arrivals.insert(tracker.id, now_millis);
                }
                let Some(frame) = update.completed else {
                    return PsnObservation::PartialFrame;
                };
                self.last_packet_at_millis = Some(now_millis);
                self.record(&frame, now_millis);
                self.pending_arrivals.clear();
                PsnObservation::Frame(frame)
            }
        }
    }

    fn record(&mut self, frame: &PsnFrame, now_millis: u64) {
        self.frames = self.frames.saturating_add(1);
        self.sample_sequence = self.sample_sequence.wrapping_add(1);
        if self.sample_sequence == 0 {
            self.source_epoch = self.source_epoch.wrapping_add(1);
        }
        let sample = PsnAcceptedSample {
            id: PsnSampleId {
                source_epoch: self.source_epoch,
                sequence: self.sample_sequence,
            },
            frame_id: frame.frame_id,
            sender_timestamp_micros: frame.timestamp_micros,
            accepted_at_millis: now_millis,
        };
        self.accepted_sample = Some(sample);
        for tracker in &frame.trackers {
            let valid_position = tracker.position.filter(|position| {
                position.x.is_finite() && position.y.is_finite() && position.z.is_finite()
            });
            if tracker.position.is_some() && valid_position.is_none() {
                self.diagnostics.invalid_positions =
                    self.diagnostics.invalid_positions.saturating_add(1);
            }
            let previous = self.tracked.get(&tracker.id);
            let mut data = *tracker;
            data.position = valid_position.or_else(|| previous.and_then(PsnTracked::position));
            let (updated_at_millis, position_sample) = if valid_position.is_some() {
                (
                    self.pending_arrivals
                        .get(&tracker.id)
                        .copied()
                        .unwrap_or(now_millis),
                    Some(sample),
                )
            } else {
                previous.map_or((now_millis, None), |previous| {
                    (previous.updated_at_millis, previous.position_sample)
                })
            };
            self.tracked.insert(
                tracker.id,
                PsnTracked {
                    id: tracker.id,
                    name: self.names.get(&tracker.id).cloned(),
                    data,
                    updated_at_millis,
                    position_sample,
                },
            );
        }
    }

    fn reject(&mut self, rejection: PsnFrameRejection) -> PsnObservation {
        self.ignored = self.ignored.saturating_add(1);
        self.diagnostics.rejected_datagrams = self.diagnostics.rejected_datagrams.saturating_add(1);
        if rejection == PsnFrameRejection::Duplicate {
            self.diagnostics.duplicate_datagrams =
                self.diagnostics.duplicate_datagrams.saturating_add(1);
        }
        if matches!(
            rejection,
            PsnFrameRejection::Ambiguous | PsnFrameRejection::InconsistentPacketCount
        ) {
            self.diagnostics.ambiguous_datagrams =
                self.diagnostics.ambiguous_datagrams.saturating_add(1);
        }
        PsnObservation::Rejected(rejection)
    }

    #[must_use]
    pub const fn accepted_sample(&self) -> Option<PsnAcceptedSample> {
        self.accepted_sample
    }

    #[must_use]
    pub const fn diagnostics(&self) -> PsnIngressDiagnostics {
        self.diagnostics
    }

    /// Caller-owned timeout/configuration boundary: incomplete data never becomes a position.
    pub fn discard_incomplete(&mut self) -> bool {
        let discarded = self.assembler.flush().is_some();
        if discarded {
            self.diagnostics.incomplete_frames =
                self.diagnostics.incomplete_frames.saturating_add(1);
        }
        self.pending_arrivals.clear();
        discarded
    }

    /// An explicit receiver decision that a new sender session owns this source. PSN has no
    /// session identifier, so timestamp regression alone never invokes this automatically.
    /// Retained poses keep their previous sample identities and ages until new valid data arrives.
    pub fn restart_source_epoch(&mut self) {
        self.discard_incomplete();
        self.assembler.reset_source_epoch();
        self.source_epoch = self.source_epoch.wrapping_add(1);
        self.sample_sequence = 0;
        self.accepted_sample = None;
    }

    /// The sender's own name for itself, once an info packet has said.
    #[must_use]
    pub fn system_name(&self) -> Option<&str> {
        self.system_name.as_deref()
    }

    /// Every tracker heard from, in id order.
    #[must_use]
    pub fn trackers(&self) -> Vec<&PsnTracked> {
        self.tracked.values().collect()
    }

    #[must_use]
    pub fn tracker(&self, id: u16) -> Option<&PsnTracked> {
        self.tracked.get(&id)
    }

    /// A tracker's position, but only while it is fresh enough to act on.
    ///
    /// This is the accessor a control path should use. Aiming a light at where somebody was three
    /// seconds ago is worse than not moving it, so the freshness question is answered here rather
    /// than left to each caller to remember.
    #[must_use]
    pub fn fresh_position(
        &self,
        id: u16,
        now_millis: u64,
        stale_after_millis: u64,
    ) -> Option<PsnVector3> {
        let tracked = self.tracked.get(&id)?;
        (tracked.age_millis(now_millis) <= stale_after_millis).then_some(tracked.position())?
    }

    /// How the source is doing at this moment.
    #[must_use]
    pub const fn health(&self, now_millis: u64, stale_after_millis: u64) -> PsnSourceHealth {
        match self.last_packet_at_millis {
            None => PsnSourceHealth::Silent,
            Some(last) => {
                let silent_for_millis = now_millis.saturating_sub(last);
                if silent_for_millis > stale_after_millis {
                    PsnSourceHealth::Stale { silent_for_millis }
                } else {
                    PsnSourceHealth::Receiving
                }
            }
        }
    }

    /// How many complete, accepted frames have been recorded.
    #[must_use]
    pub const fn frames(&self) -> u64 {
        self.frames
    }

    /// How many datagrams were dropped as unreadable or foreign.
    ///
    /// Worth showing an operator beside the healthy counts: a steady climb here with frames also
    /// arriving means something else is talking on the group.
    #[must_use]
    pub const fn ignored(&self) -> u64 {
        self.ignored
    }

    /// When an info packet was last heard, on the caller's clock.
    ///
    /// Names come only from info packets, so a listener with frames but no info is a listener that
    /// can only offer numbers.
    #[must_use]
    pub const fn last_info_at_millis(&self) -> Option<u64> {
        self.last_info_at_millis
    }
}

#[cfg(test)]
#[path = "tracking_tests.rs"]
mod tests;
