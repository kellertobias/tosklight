//! Receiving Speed Groups from a Light desk.
//!
//! The Media Server is a receiver and never a publisher: the desk is authoritative for every
//! group's tempo. This module owns what Media does with what arrives, independent of the wire
//! protocol that carried it — which sender it follows, which updates it accepts, and what an
//! operator is told about the connection.
//!
//! Exactly one sender is followed at a time. The first sender heard owns the stream until it has
//! been silent for [`SPEED_GROUP_FRESHNESS`]; a second desk sending meanwhile is refused and
//! reported, so two desks can never take turns retiming a show. Within one sender a sequence
//! number orders the updates: a datagram that arrives late is dropped rather than stepping the
//! tempo backwards. A sender that restarts presents a new source identity, or reappears after the
//! freshness window, and is followed from its first update — that is what reconnecting means.

use std::collections::BTreeMap;
use std::net::SocketAddr;

use media_domain::tempo::SPEED_GROUP_FRESHNESS;
use media_domain::{SpeedGroupId, SpeedGroupSnapshot, Timestamp};

/// The highest Speed Group number a sender may name. Light has five groups; the limit leaves room
/// without letting a malformed datagram grow the table without bound.
pub const MAX_SPEED_GROUP: u32 = 64;

/// The highest tempo accepted, matching the desk's own Speed Group range.
pub const MAX_SPEED_GROUP_BPM: f64 = 999.0;

/// How many refused datagrams the operator can see, newest first.
const REJECTION_HISTORY: usize = 8;

/// One decoded Speed Group update, as a sender published it.
#[derive(Debug, Clone, PartialEq)]
pub struct SpeedGroupUpdate {
    /// The sender's instance identity. A restarted desk sends a new one.
    pub source: String,
    /// Increases with every update this source sends.
    pub sequence: u32,
    /// One-based Speed Group number.
    pub group: u32,
    /// The group's effective tempo.
    pub bpm: f64,
    /// Normalized beat position in `[0, 1)` when the update was sent.
    pub beat_phase: f64,
    /// False while the desk has the group paused.
    pub running: bool,
}

/// Why an update was not applied.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SpeedGroupRejection {
    /// The datagram was not a Speed Group update the contract describes.
    #[error("invalid message: {0}")]
    Invalid(String),
    /// A second sender spoke while another one was live.
    #[error("ignored {sender}: already following {current}")]
    CompetingSender { sender: String, current: String },
    /// An update older than one already applied.
    #[error("out-of-order update {sequence} from {sender} (last {last})")]
    OutOfOrder {
        sender: String,
        sequence: u32,
        last: u32,
    },
}

impl SpeedGroupRejection {
    fn validate(update: &SpeedGroupUpdate) -> Result<(), Self> {
        let invalid = |reason: String| Err(Self::Invalid(reason));
        if update.source.trim().is_empty() {
            return invalid("the source identity is empty".to_owned());
        }
        if !(1..=MAX_SPEED_GROUP).contains(&update.group) {
            return invalid(format!(
                "Speed Group {} is outside 1–{MAX_SPEED_GROUP}",
                update.group
            ));
        }
        if !update.bpm.is_finite() || !(0.0..=MAX_SPEED_GROUP_BPM).contains(&update.bpm) {
            return invalid(format!(
                "BPM {} is outside 0–{MAX_SPEED_GROUP_BPM}",
                update.bpm
            ));
        }
        if !update.beat_phase.is_finite() || !(0.0..1.0).contains(&update.beat_phase) {
            return invalid(format!(
                "beat phase {} is outside 0 up to 1",
                update.beat_phase
            ));
        }
        Ok(())
    }
}

/// Whether a sender is being followed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeedGroupConnection {
    /// Reception is off: no listen address is configured.
    Disabled,
    /// The listener could not start.
    Unavailable,
    /// Listening, but no valid update has arrived yet.
    Waiting,
    /// A sender is live.
    Connected,
    /// The followed sender went silent. Every group holds its last tempo.
    Lost,
}

impl SpeedGroupConnection {
    /// The stable wire name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Unavailable => "unavailable",
            Self::Waiting => "waiting",
            Self::Connected => "connected",
            Self::Lost => "lost",
        }
    }
}

/// One group, as the operator sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct SpeedGroupReading {
    pub group: u32,
    pub bpm: f64,
    pub beat_phase: f64,
    pub running: bool,
    pub fresh: bool,
    pub age_millis: u64,
}

/// A refused datagram, as the operator sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeedGroupRejectionRecord {
    pub from: Option<SocketAddr>,
    pub reason: String,
    pub at: Timestamp,
}

/// Everything an operator is told about Speed Group reception.
#[derive(Debug, Clone, PartialEq)]
pub struct SpeedGroupReceptionStatus {
    pub connection: SpeedGroupConnection,
    pub listening: Option<SocketAddr>,
    /// Why the listener is not running, when it is not.
    pub detail: Option<String>,
    /// The followed source identity and the address it sends from.
    pub sender: Option<String>,
    pub sender_address: Option<SocketAddr>,
    pub last_update_age_millis: Option<u64>,
    pub accepted: u64,
    pub rejected: u64,
    pub rejections: Vec<SpeedGroupRejectionRecord>,
    pub groups: Vec<SpeedGroupReading>,
}

#[derive(Debug, Clone, PartialEq)]
struct GroupEntry {
    bpm: f64,
    beat_phase: f64,
    running: bool,
    observed_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq)]
struct Sender {
    source: String,
    address: Option<SocketAddr>,
    sequence: u32,
    heard_at: Timestamp,
}

/// Speed Group reception state.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SpeedGroupReception {
    listening: Option<SocketAddr>,
    /// Whether reception was asked for at all.
    configured: bool,
    /// Why the listener could not start, when it could not.
    unavailable: Option<String>,
    sender: Option<Sender>,
    groups: BTreeMap<u32, GroupEntry>,
    accepted: u64,
    rejected: u64,
    rejections: Vec<SpeedGroupRejectionRecord>,
}

impl SpeedGroupReception {
    /// Reception that is listening on `address`.
    pub fn listening(address: SocketAddr) -> Self {
        Self {
            listening: Some(address),
            configured: true,
            ..Self::default()
        }
    }

    /// Reception that was asked for but could not start.
    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            listening: None,
            configured: true,
            unavailable: Some(reason.into()),
            ..Self::default()
        }
    }

    /// Applies one update, or records why it was refused.
    pub fn accept(
        &mut self,
        update: SpeedGroupUpdate,
        from: Option<SocketAddr>,
        now: Timestamp,
    ) -> Result<(), SpeedGroupRejection> {
        let result = self.admit(&update, from, now);
        match &result {
            Ok(()) => {
                self.accepted += 1;
                self.groups.insert(
                    update.group,
                    GroupEntry {
                        bpm: update.bpm,
                        beat_phase: update.beat_phase,
                        running: update.running,
                        observed_at: now,
                    },
                );
            }
            Err(rejection) => self.record_rejection(rejection.to_string(), from, now),
        }
        result
    }

    /// Records a datagram that could not even be decoded.
    pub fn reject(&mut self, reason: impl Into<String>, from: Option<SocketAddr>, now: Timestamp) {
        self.record_rejection(
            SpeedGroupRejection::Invalid(reason.into()).to_string(),
            from,
            now,
        );
    }

    fn admit(
        &mut self,
        update: &SpeedGroupUpdate,
        from: Option<SocketAddr>,
        now: Timestamp,
    ) -> Result<(), SpeedGroupRejection> {
        SpeedGroupRejection::validate(update)?;
        match &mut self.sender {
            Some(sender) if sender.source == update.source => {
                let silent = now.since(sender.heard_at) >= SPEED_GROUP_FRESHNESS;
                // A live sender only moves forward. One that fell silent is followed from
                // whatever it sends next, so a desk that lost and regained the network — or
                // restarted its counter — is never locked out.
                if update.sequence <= sender.sequence && !silent {
                    return Err(SpeedGroupRejection::OutOfOrder {
                        sender: update.source.clone(),
                        sequence: update.sequence,
                        last: sender.sequence,
                    });
                }
            }
            Some(sender) if now.since(sender.heard_at) < SPEED_GROUP_FRESHNESS => {
                return Err(SpeedGroupRejection::CompetingSender {
                    sender: update.source.clone(),
                    current: sender.source.clone(),
                });
            }
            _ => {
                // A different desk takes over only after the previous one went silent, and its
                // groups replace the previous desk's rather than mixing with them.
                self.groups.clear();
            }
        }
        self.sender = Some(Sender {
            source: update.source.clone(),
            address: from,
            sequence: update.sequence,
            heard_at: now,
        });
        Ok(())
    }

    fn record_rejection(&mut self, reason: String, from: Option<SocketAddr>, now: Timestamp) {
        self.rejected += 1;
        self.rejections.insert(
            0,
            SpeedGroupRejectionRecord {
                from,
                reason,
                at: now,
            },
        );
        self.rejections.truncate(REJECTION_HISTORY);
    }

    /// The clock a group last published, for [`media_domain::resolve_tempo`].
    ///
    /// A paused group is a tempo of zero: synchronized playback holds its frame until the desk
    /// resumes the group.
    pub fn snapshot(&self, group: SpeedGroupId) -> Option<SpeedGroupSnapshot> {
        let entry = self.groups.get(&group.value())?;
        Some(SpeedGroupSnapshot {
            group_id: group,
            bpm: if entry.running { entry.bpm } else { 0.0 },
            phase_beats: entry.beat_phase,
            observed_at: entry.observed_at,
        })
    }

    pub fn connection(&self, now: Timestamp) -> SpeedGroupConnection {
        if !self.configured {
            return SpeedGroupConnection::Disabled;
        }
        if self.unavailable.is_some() {
            return SpeedGroupConnection::Unavailable;
        }
        match &self.sender {
            None => SpeedGroupConnection::Waiting,
            Some(sender) if now.since(sender.heard_at) < SPEED_GROUP_FRESHNESS => {
                SpeedGroupConnection::Connected
            }
            Some(_) => SpeedGroupConnection::Lost,
        }
    }

    pub fn status(&self, now: Timestamp) -> SpeedGroupReceptionStatus {
        let age = |at: Timestamp| now.since(at).as_millis() as u64;
        SpeedGroupReceptionStatus {
            connection: self.connection(now),
            listening: self.listening,
            detail: self.unavailable.clone(),
            sender: self.sender.as_ref().map(|sender| sender.source.clone()),
            sender_address: self.sender.as_ref().and_then(|sender| sender.address),
            last_update_age_millis: self.sender.as_ref().map(|sender| age(sender.heard_at)),
            accepted: self.accepted,
            rejected: self.rejected,
            rejections: self.rejections.clone(),
            groups: self
                .groups
                .iter()
                .map(|(group, entry)| SpeedGroupReading {
                    group: *group,
                    bpm: entry.bpm,
                    beat_phase: entry.beat_phase,
                    running: entry.running,
                    fresh: now.since(entry.observed_at) < SPEED_GROUP_FRESHNESS,
                    age_millis: age(entry.observed_at),
                })
                .collect(),
        }
    }
}

#[cfg(test)]
#[path = "speed_group_reception_tests.rs"]
mod tests;
