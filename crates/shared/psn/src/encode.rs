//! Writing PSN, for the times a desk has to be tested without a tracking system in the building.
//!
//! ToskLight never sends PSN in production — it is a receiver. This exists so that a rehearsal
//! room, a CI job or a developer's laptop can produce a stream that is *the real format*, rather
//! than a mock of it that agrees with whatever the decoder happens to do. A sender built here and
//! read back by [`crate::decode`] proves both halves against the specification at once.
//!
//! It is deliberately small: enough to describe trackers and split them across packets the way a
//! real sender must, and nothing else.

use crate::{PSN_VERSION_HIGH, PSN_VERSION_LOW, PsnInfoPacket, PsnTrackerData, PsnVector3};

/// The cap a real sender works to. One frame beyond this must be split.
pub const PSN_MAX_PACKET_BYTES: usize = 1500;

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

fn chunk(id: u16, has_subchunks: bool, data: &[u8]) -> Vec<u8> {
    // A chunk length is fifteen bits. Nothing this writes comes close, and a caller that managed
    // it would get a packet no receiver could read, so it is clamped rather than silently wrapped.
    let len = u32::try_from(data.len().min(0x7FFF)).unwrap_or(0x7FFF);
    let header = u32::from(id) | (len << 16) | (u32::from(has_subchunks) << 31);
    let mut bytes = header.to_le_bytes().to_vec();
    bytes.extend_from_slice(&data[..len as usize]);
    bytes
}

fn header_chunk(timestamp_micros: u64, frame_id: u8, frame_packet_count: u8) -> Vec<u8> {
    let mut data = timestamp_micros.to_le_bytes().to_vec();
    data.extend_from_slice(&[
        PSN_VERSION_HIGH,
        PSN_VERSION_LOW,
        frame_id,
        frame_packet_count,
    ]);
    chunk(PACKET_HEADER, false, &data)
}

fn vector_chunk(id: u16, vector: PsnVector3) -> Vec<u8> {
    let mut data = vector.x.to_le_bytes().to_vec();
    data.extend_from_slice(&vector.y.to_le_bytes());
    data.extend_from_slice(&vector.z.to_le_bytes());
    chunk(id, false, &data)
}

fn tracker_chunk(tracker: &PsnTrackerData) -> Vec<u8> {
    let mut body = Vec::new();
    if let Some(position) = tracker.position {
        body.extend(vector_chunk(DATA_TRACKER_POS, position));
    }
    if let Some(speed) = tracker.speed {
        body.extend(vector_chunk(DATA_TRACKER_SPEED, speed));
    }
    if let Some(orientation) = tracker.orientation {
        body.extend(vector_chunk(DATA_TRACKER_ORI, orientation));
    }
    if let Some(validity) = tracker.validity {
        body.extend(chunk(DATA_TRACKER_STATUS, false, &validity.to_le_bytes()));
    }
    if let Some(acceleration) = tracker.acceleration {
        body.extend(vector_chunk(DATA_TRACKER_ACCEL, acceleration));
    }
    if let Some(target) = tracker.target_position {
        body.extend(vector_chunk(DATA_TRACKER_TARGET_POS, target));
    }
    chunk(tracker.id, true, &body)
}

/// One frame as the datagrams a sender would put on the wire.
///
/// Trackers are packed until the next one would not fit, exactly as the 1500-byte cap forces a real
/// sender to do, and every packet carries the same `frame_id` with the final count. A frame small
/// enough for one datagram produces one.
///
/// The count can only be written once it is known, so the packets are built first and stamped
/// afterwards — which is also why this returns whole datagrams rather than an iterator.
#[must_use]
pub fn encode_data_frame(
    timestamp_micros: u64,
    frame_id: u8,
    trackers: &[PsnTrackerData],
) -> Vec<Vec<u8>> {
    let header = header_chunk(timestamp_micros, frame_id, 0);
    // Room for the packet root, the header chunk and the tracker list's own chunk header.
    let overhead = 4 + header.len() + 4;
    let mut packets = Vec::new();
    let mut current: Vec<u8> = Vec::new();
    for tracker in trackers {
        let encoded = tracker_chunk(tracker);
        if !current.is_empty() && overhead + current.len() + encoded.len() > PSN_MAX_PACKET_BYTES {
            packets.push(std::mem::take(&mut current));
        }
        current.extend(encoded);
    }
    if !current.is_empty() || packets.is_empty() {
        packets.push(current);
    }
    let count = u8::try_from(packets.len()).unwrap_or(u8::MAX);
    packets
        .into_iter()
        .map(|list| {
            let mut body = header_chunk(timestamp_micros, frame_id, count);
            body.extend(chunk(DATA_TRACKER_LIST, true, &list));
            chunk(PSN_DATA_PACKET, true, &body)
        })
        .collect()
}

/// One info packet: the sender's name, and the names of its trackers.
#[must_use]
pub fn encode_info_packet(packet: &PsnInfoPacket) -> Vec<u8> {
    let mut list = Vec::new();
    for tracker in &packet.trackers {
        let name = tracker.name.as_deref().unwrap_or_default();
        list.extend(chunk(
            tracker.id,
            true,
            &chunk(INFO_TRACKER_NAME, false, name.as_bytes()),
        ));
    }
    let mut body = header_chunk(
        packet.header.timestamp_micros,
        packet.header.frame_id,
        packet.header.frame_packet_count.max(1),
    );
    if let Some(system_name) = &packet.system_name {
        body.extend(chunk(INFO_SYSTEM_NAME, false, system_name.as_bytes()));
    }
    body.extend(chunk(INFO_TRACKER_LIST, true, &list));
    chunk(PSN_INFO_PACKET, true, &body)
}

#[cfg(test)]
#[path = "encode_tests.rs"]
mod tests;
