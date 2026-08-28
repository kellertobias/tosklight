//! Round trips, mostly.
//!
//! An encoder tested against its own decoder would agree with itself about a mistake, so the
//! decoder's tests stay built from hand-assembled packets and these tests ask a different question:
//! that what this writes is what the decoder — written from the specification — reads back.

use super::*;
use crate::{
    PsnFrameAssembler, PsnInfoPacket, PsnPacket, PsnPacketHeader, PsnTrackerInfo, PsnTracking,
    decode,
};

fn tracker(id: u16, x: f32) -> PsnTrackerData {
    PsnTrackerData {
        id,
        position: Some(PsnVector3 { x, y: 1.0, z: 2.0 }),
        ..PsnTrackerData::default()
    }
}

fn data_of(datagram: &[u8]) -> crate::PsnDataPacket {
    match decode(datagram).expect("decodes") {
        PsnPacket::Data(data) => data,
        PsnPacket::Info(_) => panic!("expected a data packet"),
    }
}

#[test]
fn a_written_frame_reads_back_field_for_field() {
    let full = PsnTrackerData {
        id: 6,
        position: Some(PsnVector3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
        }),
        speed: Some(PsnVector3 {
            x: 0.25,
            y: 0.0,
            z: -0.5,
        }),
        orientation: Some(PsnVector3 {
            x: 0.0,
            y: 1.5,
            z: 0.0,
        }),
        validity: Some(0.75),
        acceleration: Some(PsnVector3 {
            x: 0.0,
            y: -9.81,
            z: 0.0,
        }),
        target_position: Some(PsnVector3 {
            x: 8.0,
            y: 0.0,
            z: 8.0,
        }),
        timestamp_micros: Some(12_000),
    };

    let packets = encode_data_frame(12_345, 7, &[full]);

    assert_eq!(packets.len(), 1);
    let decoded = data_of(&packets[0]);
    assert_eq!(decoded.header.timestamp_micros, 12_345);
    assert_eq!(decoded.header.frame_id, 7);
    assert_eq!(decoded.header.frame_packet_count, 1);
    assert_eq!(decoded.trackers, vec![full]);
}

#[test]
fn a_field_left_out_stays_left_out() {
    // The encoder must not invent a zero for something the caller did not describe, or the
    // decoder's absent-is-not-zero rule would be untestable end to end.
    let packets = encode_data_frame(0, 1, &[tracker(1, 4.0)]);

    let decoded = data_of(&packets[0]);
    assert_eq!(decoded.trackers[0].speed, None);
    assert_eq!(decoded.trackers[0].validity, None);
}

#[test]
fn a_per_tracker_timestamp_survives_the_round_trip() {
    // Not in the v2.02 document's packet scheme, but in the protocol's own reference
    // implementation and on the wire from real senders. A packet's header timestamp says when the
    // packet was sent; this says when the tracker was measured, and only the second gives a desk
    // honest per-tracker latency.
    let measured = PsnTrackerData {
        id: 3,
        timestamp_micros: Some(9_876_543_210),
        ..tracker(3, 0.0)
    };

    let packets = encode_data_frame(1, 1, &[measured]);

    let decoded = data_of(&packets[0]);
    assert_eq!(decoded.trackers[0].timestamp_micros, Some(9_876_543_210));
    // And a sender that does not stamp its trackers still reads as not having stamped them.
    let unstamped = data_of(&encode_data_frame(1, 1, &[tracker(4, 0.0)])[0]);
    assert_eq!(unstamped.trackers[0].timestamp_micros, None);
}

#[test]
fn a_frame_too_large_for_one_datagram_is_split_and_reassembles() {
    // A hundred and twenty trackers is past the 1500-byte cap, which is the whole reason frame_id
    // exists: at twenty bytes each they need two datagrams.
    let trackers = (0..120)
        .map(|index| tracker(index, f32::from(index)))
        .collect::<Vec<_>>();

    let packets = encode_data_frame(99, 3, &trackers);

    assert!(packets.len() > 1, "a large frame must be split");
    for packet in &packets {
        assert!(
            packet.len() <= PSN_MAX_PACKET_BYTES,
            "a sender must not exceed the protocol's packet cap"
        );
        let decoded = data_of(packet);
        assert_eq!(decoded.header.frame_id, 3);
        assert_eq!(
            usize::from(decoded.header.frame_packet_count),
            packets.len(),
            "every packet of a frame carries the final count"
        );
    }

    let mut assembler = PsnFrameAssembler::new();
    let mut frame = None;
    for packet in &packets {
        frame = assembler.push(data_of(packet)).or(frame);
    }
    let frame = frame.expect("the split frame reassembles");
    assert!(frame.complete);
    assert_eq!(frame.trackers.len(), 120);
}

#[test]
fn a_frame_with_no_trackers_is_still_one_packet() {
    // A sender with nothing to report still says so, and a receiver must not read that as silence.
    let packets = encode_data_frame(1, 1, &[]);

    assert_eq!(packets.len(), 1);
    let decoded = data_of(&packets[0]);
    assert!(decoded.trackers.is_empty());
    assert_eq!(decoded.header.frame_packet_count, 1);
}

#[test]
fn a_written_info_packet_names_the_system_and_its_trackers() {
    let packet = PsnInfoPacket {
        header: PsnPacketHeader {
            timestamp_micros: 5,
            ..PsnPacketHeader::default()
        },
        system_name: Some("Rehearsal sender".into()),
        trackers: vec![
            PsnTrackerInfo {
                id: 1,
                name: Some("Presenter".into()),
            },
            PsnTrackerInfo { id: 2, name: None },
        ],
    };

    let PsnPacket::Info(decoded) = decode(&encode_info_packet(&packet)).expect("decodes") else {
        panic!("expected an info packet");
    };

    assert_eq!(decoded.system_name.as_deref(), Some("Rehearsal sender"));
    assert_eq!(decoded.trackers[0].name.as_deref(), Some("Presenter"));
    // A tracker the sender lists without a name comes back as an empty name, not as absent: the
    // sender did send a name chunk, and it was empty.
    assert_eq!(decoded.trackers[1].name.as_deref(), Some(""));
}

#[test]
fn a_written_stream_drives_the_listener_end_to_end() {
    // The point of having an encoder at all: a desk can be tested without a tracking system.
    let mut tracking = PsnTracking::new();
    let info = PsnInfoPacket {
        header: PsnPacketHeader::default(),
        system_name: Some("Rehearsal sender".into()),
        trackers: vec![PsnTrackerInfo {
            id: 2,
            name: Some("Presenter".into()),
        }],
    };
    tracking.observe(&encode_info_packet(&info), 0);

    for (step, frame_id) in (1..=3_u8).enumerate() {
        for packet in encode_data_frame(
            u64::from(frame_id) * 1_000,
            frame_id,
            &[tracker(2, step as f32)],
        ) {
            tracking.observe(&packet, u64::from(frame_id) * 16);
        }
    }

    let tracked = tracking.tracker(2).expect("tracked");
    assert_eq!(tracked.name.as_deref(), Some("Presenter"));
    assert_eq!(tracked.position().map(|p| p.x), Some(2.0));
    assert_eq!(tracking.frames(), 3);
    assert_eq!(tracking.ignored(), 0);
    assert_eq!(tracking.system_name(), Some("Rehearsal sender"));
}
