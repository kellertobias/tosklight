#![forbid(unsafe_code)]

//! PosiStageNet on the wire, and nothing else.
//!
//! PSN is how a tracking system — OpenFollow, a Photon server, a BlackTrax bridge — says where the
//! things it is watching are. It is UDP multicast, so a desk listens rather than connects, and it
//! never asks the sender for anything. This crate turns those datagrams into positions and names.
//!
//! It knows nothing about sockets, shows, fixtures or time. A receiver owns the socket and decides
//! what a position *means*; what belongs here is only the reading of bytes, so that the meaning is
//! decided once against a decoder that is tested on its own.
//!
//! The protocol is v2.02, published by VYV Corporation and MA Lighting and free of royalty. Two
//! packets carry everything: `PSN_DATA` at the tracking rate (60 Hz by default, up to whatever the
//! hardware manages), and `PSN_INFO` about once a second, which is where the trackers' names live.
//! A tracker is identified by a number, and that number is all a data packet carries — so a desk
//! that wants to show an operator "Presenter" rather than "tracker 3" has to keep what the last
//! info packet said.
//!
//! Everything is little-endian, and every value is SI: metres, metres per second, metres per second
//! squared, radians.

mod chunk;
mod encode;
mod frame;
mod tracking;

pub use encode::{PSN_MAX_PACKET_BYTES, encode_data_frame, encode_info_packet};
pub use frame::{PsnFrame, PsnFrameAssembler};
pub use tracking::{PsnObservation, PsnSourceHealth, PsnTracked, PsnTracking};

use chunk::Chunks;
use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

/// Where a PSN sender transmits unless it has been moved.
pub const PSN_MULTICAST_ADDRESS: Ipv4Addr = Ipv4Addr::new(236, 10, 10, 10);
/// The port that goes with [`PSN_MULTICAST_ADDRESS`].
pub const PSN_PORT: u16 = 56565;

/// The default group a desk joins to hear a sender that has not been reconfigured.
#[must_use]
pub fn psn_multicast_destination() -> SocketAddr {
    SocketAddr::new(IpAddr::V4(PSN_MULTICAST_ADDRESS), PSN_PORT)
}

/// The protocol version this decoder was written against.
pub const PSN_VERSION_HIGH: u8 = 2;
/// The low half of [`PSN_VERSION_HIGH`].
pub const PSN_VERSION_LOW: u8 = 0;

const PSN_DATA_PACKET: u16 = 0x6755;
const PSN_INFO_PACKET: u16 = 0x6756;

const PACKET_HEADER: u16 = 0x0000;
const INFO_SYSTEM_NAME: u16 = 0x0001;
const INFO_TRACKER_LIST: u16 = 0x0002;
const INFO_TRACKER_NAME: u16 = 0x0000;
const DATA_TRACKER_LIST: u16 = 0x0001;
const DATA_TRACKER_POS: u16 = 0x0000;
const DATA_TRACKER_SPEED: u16 = 0x0001;
const DATA_TRACKER_ORI: u16 = 0x0002;
const DATA_TRACKER_STATUS: u16 = 0x0003;
const DATA_TRACKER_ACCEL: u16 = 0x0004;
const DATA_TRACKER_TARGET_POS: u16 = 0x0005;

/// What a datagram turned out to be.
///
/// A sender puts both packet kinds on the same group and port, so a receiver reads whatever
/// arrives and finds out afterwards.
#[derive(Clone, Debug, PartialEq)]
pub enum PsnPacket {
    Data(PsnDataPacket),
    Info(PsnInfoPacket),
}

/// Why a datagram could not be read.
///
/// Every one of these means the packet is dropped, never that the receiver should stop: a tracking
/// system on a busy network is exactly where a truncated or foreign datagram turns up.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PsnError {
    /// The datagram is not a PSN packet, or is a chunk kind this decoder does not know as a root.
    NotPsn,
    /// A chunk claims more data than the packet holds.
    Truncated,
    /// A packet with no `PSN_*_PACKET_HEADER`, which every packet is required to carry.
    MissingHeader,
}

impl std::fmt::Display for PsnError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::NotPsn => "not a PosiStageNet packet",
            Self::Truncated => "a PosiStageNet chunk claims more data than the packet holds",
            Self::MissingHeader => "a PosiStageNet packet arrived without its header chunk",
        })
    }
}

impl std::error::Error for PsnError {}

/// A point or a vector in the sender's own space, in SI units.
///
/// PSN states its axes plainly: positive x is right, positive y is up, positive z is depth. That is
/// the sender's stage, not the desk's — turning one into the other is a calibration decision and
/// deliberately not made here.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PsnVector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// The header every PSN packet carries, whichever kind it is.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PsnPacketHeader {
    /// Microseconds since the sender started. It is the sender's clock, not the desk's, and it
    /// says nothing about wall time — only about the order and spacing of packets from one sender.
    pub timestamp_micros: u64,
    pub version_high: u8,
    pub version_low: u8,
    /// Which frame this packet belongs to. One frame may be split across several packets.
    pub frame_id: u8,
    /// How many packets the sender split this frame into.
    pub frame_packet_count: u8,
}

/// One tracker's data, as one packet reported it.
///
/// Every field but the id is optional because the sender chooses what to include, and a field that
/// was not sent is not the same as a field that was sent as zero. A tracker standing still sends a
/// speed of zero; a sender that does not measure speed sends no speed chunk at all.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PsnTrackerData {
    pub id: u16,
    /// Metres.
    pub position: Option<PsnVector3>,
    /// Metres per second.
    pub speed: Option<PsnVector3>,
    /// An axis whose length is the rotation about it, in radians. Absolute, not accumulated.
    pub orientation: Option<PsnVector3>,
    /// The sender's own confidence in this tracker. The protocol does not fix a scale.
    pub validity: Option<f32>,
    /// Metres per second squared.
    pub acceleration: Option<PsnVector3>,
    /// Where the tracker is heading, in metres.
    pub target_position: Option<PsnVector3>,
}

/// One `PSN_DATA` packet: where the trackers are, right now, as far as this packet goes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PsnDataPacket {
    pub header: PsnPacketHeader,
    pub trackers: Vec<PsnTrackerData>,
}

/// One tracker's identity, as `PSN_INFO` reported it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PsnTrackerInfo {
    pub id: u16,
    /// Absent when the sender listed the tracker without naming it.
    pub name: Option<String>,
}

/// One `PSN_INFO` packet: who the sender is, and what its trackers are called.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PsnInfoPacket {
    pub header: PsnPacketHeader,
    pub system_name: Option<String>,
    pub trackers: Vec<PsnTrackerInfo>,
}

/// Read one datagram.
///
/// The root chunk is read before anything else is trusted, so a datagram from some other protocol
/// that happens to share the group is reported as foreign rather than as a broken PSN packet. That
/// distinction is what an operator needs: "not PSN" is a network to look at, "truncated" is a
/// sender to look at.
///
/// Below the root, unknown chunks are skipped rather than refused — the protocol requires it, and
/// it is what lets a desk keep working when a sender is newer than the desk.
pub fn decode(datagram: &[u8]) -> Result<PsnPacket, PsnError> {
    let mut roots = Chunks::new(datagram);
    let Some(root) = roots.peek_id() else {
        return Err(PsnError::NotPsn);
    };
    if root != PSN_DATA_PACKET && root != PSN_INFO_PACKET {
        return Err(PsnError::NotPsn);
    }
    let root = roots.next_chunk()?.ok_or(PsnError::NotPsn)?;
    if root.id == PSN_DATA_PACKET {
        decode_data(root.data).map(PsnPacket::Data)
    } else {
        decode_info(root.data).map(PsnPacket::Info)
    }
}

fn decode_data(body: &[u8]) -> Result<PsnDataPacket, PsnError> {
    let mut packet = PsnDataPacket::default();
    let mut header = None;
    let mut chunks = Chunks::new(body);
    while let Some(chunk) = chunks.next_chunk()? {
        match chunk.id {
            PACKET_HEADER => header = Some(decode_header(chunk.data)),
            DATA_TRACKER_LIST => {
                let mut trackers = Chunks::new(chunk.data);
                while let Some(tracker) = trackers.next_chunk()? {
                    packet
                        .trackers
                        .push(decode_tracker_data(tracker.id, tracker.data)?);
                }
            }
            _ => {}
        }
    }
    packet.header = header.ok_or(PsnError::MissingHeader)?;
    Ok(packet)
}

fn decode_tracker_data(id: u16, body: &[u8]) -> Result<PsnTrackerData, PsnError> {
    let mut tracker = PsnTrackerData {
        id,
        ..PsnTrackerData::default()
    };
    let mut chunks = Chunks::new(body);
    while let Some(chunk) = chunks.next_chunk()? {
        match chunk.id {
            DATA_TRACKER_POS => tracker.position = vector(chunk.data),
            DATA_TRACKER_SPEED => tracker.speed = vector(chunk.data),
            DATA_TRACKER_ORI => tracker.orientation = vector(chunk.data),
            DATA_TRACKER_STATUS => tracker.validity = float(chunk.data, 0),
            DATA_TRACKER_ACCEL => tracker.acceleration = vector(chunk.data),
            DATA_TRACKER_TARGET_POS => tracker.target_position = vector(chunk.data),
            _ => {}
        }
    }
    Ok(tracker)
}

fn decode_info(body: &[u8]) -> Result<PsnInfoPacket, PsnError> {
    let mut packet = PsnInfoPacket::default();
    let mut header = None;
    let mut chunks = Chunks::new(body);
    while let Some(chunk) = chunks.next_chunk()? {
        match chunk.id {
            PACKET_HEADER => header = Some(decode_header(chunk.data)),
            INFO_SYSTEM_NAME => packet.system_name = Some(text(chunk.data)),
            INFO_TRACKER_LIST => {
                let mut trackers = Chunks::new(chunk.data);
                while let Some(tracker) = trackers.next_chunk()? {
                    packet
                        .trackers
                        .push(decode_tracker_info(tracker.id, tracker.data)?);
                }
            }
            _ => {}
        }
    }
    packet.header = header.ok_or(PsnError::MissingHeader)?;
    Ok(packet)
}

fn decode_tracker_info(id: u16, body: &[u8]) -> Result<PsnTrackerInfo, PsnError> {
    let mut info = PsnTrackerInfo { id, name: None };
    let mut chunks = Chunks::new(body);
    while let Some(chunk) = chunks.next_chunk()? {
        if chunk.id == INFO_TRACKER_NAME {
            info.name = Some(text(chunk.data));
        }
    }
    Ok(info)
}

/// A header read as far as the packet actually goes.
///
/// The protocol says a structure may grow at the end and that a short read is the intended way to
/// stay compatible with an older sender, so a header that stops early leaves the rest at zero
/// rather than failing the packet.
fn decode_header(data: &[u8]) -> PsnPacketHeader {
    PsnPacketHeader {
        timestamp_micros: data.get(0..8).map_or(0, |bytes| {
            u64::from_le_bytes(bytes.try_into().unwrap_or([0; 8]))
        }),
        version_high: data.get(8).copied().unwrap_or_default(),
        version_low: data.get(9).copied().unwrap_or_default(),
        frame_id: data.get(10).copied().unwrap_or_default(),
        frame_packet_count: data.get(11).copied().unwrap_or_default(),
    }
}

fn vector(data: &[u8]) -> Option<PsnVector3> {
    Some(PsnVector3 {
        x: float(data, 0)?,
        y: float(data, 1)?,
        z: float(data, 2)?,
    })
}

fn float(data: &[u8], index: usize) -> Option<f32> {
    let start = index * 4;
    let bytes: [u8; 4] = data.get(start..start + 4)?.try_into().ok()?;
    Some(f32::from_le_bytes(bytes))
}

/// A name as the sender wrote it.
///
/// Senders differ over whether a name is terminated, so a trailing NUL and anything after it is
/// dropped. A name that is not valid UTF-8 is read lossily rather than thrown away: an operator is
/// better served by a name with one wrong character in it than by no name at all.
fn text(data: &[u8]) -> String {
    let end = data
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(data.len());
    String::from_utf8_lossy(&data[..end]).into_owned()
}

/// The trackers of one frame, keyed by id, in id order.
///
/// Handy where a caller wants to look one tracker up rather than walk the list.
#[must_use]
pub fn trackers_by_id(trackers: &[PsnTrackerData]) -> BTreeMap<u16, PsnTrackerData> {
    trackers
        .iter()
        .map(|tracker| (tracker.id, *tracker))
        .collect()
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
