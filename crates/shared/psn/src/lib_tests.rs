//! Tests built on real packets.
//!
//! The fixtures are assembled with a small encoder rather than written out as byte literals, so a
//! test says what a sender sent rather than what a hex dump looks like. The encoder is only in the
//! tests on purpose: this crate receives PSN, it does not transmit it, and an encoder in the
//! library would be an untested export nothing calls.

use super::*;

/// One chunk, as a sender writes it.
fn chunk(id: u16, has_subchunks: bool, data: &[u8]) -> Vec<u8> {
    let len = u32::try_from(data.len()).expect("a test chunk fits");
    let header = u32::from(id) | (len << 16) | (u32::from(has_subchunks) << 31);
    let mut bytes = header.to_le_bytes().to_vec();
    bytes.extend_from_slice(data);
    bytes
}

fn header_chunk(timestamp: u64, frame_id: u8, frame_packet_count: u8) -> Vec<u8> {
    let mut data = timestamp.to_le_bytes().to_vec();
    data.extend_from_slice(&[
        PSN_VERSION_HIGH,
        PSN_VERSION_LOW,
        frame_id,
        frame_packet_count,
    ]);
    chunk(PACKET_HEADER, false, &data)
}

fn vector_chunk(id: u16, x: f32, y: f32, z: f32) -> Vec<u8> {
    let mut data = x.to_le_bytes().to_vec();
    data.extend_from_slice(&y.to_le_bytes());
    data.extend_from_slice(&z.to_le_bytes());
    chunk(id, false, &data)
}

/// A data packet carrying one fully described tracker.
fn data_packet(tracker_id: u16, frame_id: u8, frame_packet_count: u8) -> Vec<u8> {
    let mut tracker = vector_chunk(DATA_TRACKER_POS, 1.5, 2.0, -3.25);
    tracker.extend(vector_chunk(DATA_TRACKER_SPEED, 0.5, 0.0, 0.0));
    tracker.extend(vector_chunk(
        DATA_TRACKER_ORI,
        0.0,
        std::f32::consts::PI,
        0.0,
    ));
    tracker.extend(chunk(DATA_TRACKER_STATUS, false, &1.0_f32.to_le_bytes()));
    tracker.extend(vector_chunk(DATA_TRACKER_ACCEL, 0.0, -9.81, 0.0));
    tracker.extend(vector_chunk(DATA_TRACKER_TARGET_POS, 4.0, 0.0, 4.0));

    let list = chunk(DATA_TRACKER_LIST, true, &chunk(tracker_id, true, &tracker));
    let mut body = header_chunk(1_000_000, frame_id, frame_packet_count);
    body.extend(list);
    chunk(PSN_DATA_PACKET, true, &body)
}

fn position_only_packet(trackers: &[(u16, f32)], frame_id: u8, frame_packet_count: u8) -> Vec<u8> {
    let mut list = Vec::new();
    for (id, x) in trackers {
        list.extend(chunk(
            *id,
            true,
            &vector_chunk(DATA_TRACKER_POS, *x, 0.0, 0.0),
        ));
    }
    let mut body = header_chunk(7, frame_id, frame_packet_count);
    body.extend(chunk(DATA_TRACKER_LIST, true, &list));
    chunk(PSN_DATA_PACKET, true, &body)
}

fn info_packet(system: &str, trackers: &[(u16, &str)]) -> Vec<u8> {
    let mut list = Vec::new();
    for (id, name) in trackers {
        list.extend(chunk(
            *id,
            true,
            &chunk(INFO_TRACKER_NAME, false, name.as_bytes()),
        ));
    }
    let mut body = header_chunk(500, 3, 1);
    body.extend(chunk(INFO_SYSTEM_NAME, false, system.as_bytes()));
    body.extend(chunk(INFO_TRACKER_LIST, true, &list));
    chunk(PSN_INFO_PACKET, true, &body)
}

fn decoded_data(datagram: &[u8]) -> PsnDataPacket {
    match decode(datagram).expect("a data packet decodes") {
        PsnPacket::Data(packet) => packet,
        PsnPacket::Info(_) => panic!("expected a data packet"),
    }
}

#[test]
fn a_data_packet_reports_every_field_a_tracker_sent_in_si_units() {
    let packet = decoded_data(&data_packet(3, 9, 1));

    assert_eq!(packet.header.frame_id, 9);
    assert_eq!(packet.header.frame_packet_count, 1);
    assert_eq!(packet.header.timestamp_micros, 1_000_000);
    assert_eq!(packet.header.version_high, PSN_VERSION_HIGH);
    let tracker = packet.trackers.first().expect("one tracker");
    assert_eq!(tracker.id, 3);
    // Metres, and the sender's own axes: positive x right, positive y up, positive z depth.
    assert_eq!(
        tracker.position,
        Some(PsnVector3 {
            x: 1.5,
            y: 2.0,
            z: -3.25
        })
    );
    assert_eq!(
        tracker.speed,
        Some(PsnVector3 {
            x: 0.5,
            y: 0.0,
            z: 0.0
        })
    );
    assert_eq!(
        tracker.orientation,
        Some(PsnVector3 {
            x: 0.0,
            y: std::f32::consts::PI,
            z: 0.0
        })
    );
    assert_eq!(tracker.validity, Some(1.0));
    assert_eq!(
        tracker.acceleration,
        Some(PsnVector3 {
            x: 0.0,
            y: -9.81,
            z: 0.0
        })
    );
    assert_eq!(
        tracker.target_position,
        Some(PsnVector3 {
            x: 4.0,
            y: 0.0,
            z: 4.0
        })
    );
}

#[test]
fn a_field_the_sender_omitted_is_absent_rather_than_zero() {
    // A tracker standing still sends a speed of zero. A sender that does not measure speed sends
    // no speed chunk. Those must not read the same.
    let packet = decoded_data(&position_only_packet(&[(1, 2.0)], 1, 1));

    let tracker = packet.trackers.first().expect("one tracker");
    assert_eq!(
        tracker.position,
        Some(PsnVector3 {
            x: 2.0,
            y: 0.0,
            z: 0.0
        })
    );
    assert_eq!(tracker.speed, None);
    assert_eq!(tracker.validity, None);
}

#[test]
fn an_unknown_chunk_is_skipped_rather_than_refusing_the_packet() {
    // The protocol requires this: it is what lets a desk keep working against a newer sender.
    let mut tracker = chunk(0x7FFF, false, b"a chunk from a future protocol version");
    tracker.extend(vector_chunk(DATA_TRACKER_POS, 1.0, 1.0, 1.0));
    let mut body = header_chunk(1, 1, 1);
    body.extend(chunk(
        0x7FFE,
        false,
        b"and another, beside the tracker list",
    ));
    body.extend(chunk(DATA_TRACKER_LIST, true, &chunk(5, true, &tracker)));
    let datagram = chunk(PSN_DATA_PACKET, true, &body);

    let packet = decoded_data(&datagram);

    assert_eq!(packet.trackers.len(), 1);
    assert_eq!(packet.trackers[0].id, 5);
    assert_eq!(
        packet.trackers[0].position,
        Some(PsnVector3 {
            x: 1.0,
            y: 1.0,
            z: 1.0
        })
    );
}

#[test]
fn a_header_from_an_older_sender_is_read_as_far_as_it_goes() {
    // The protocol's own compatibility rule: a structure grows at the end, and a short read is how
    // a newer client stays compatible with an older server.
    let short = chunk(PACKET_HEADER, false, &42_u64.to_le_bytes());
    let mut body = short;
    body.extend(chunk(DATA_TRACKER_LIST, true, &[]));
    let packet = decoded_data(&chunk(PSN_DATA_PACKET, true, &body));

    assert_eq!(packet.header.timestamp_micros, 42);
    assert_eq!(packet.header.frame_packet_count, 0);
}

#[test]
fn a_packet_without_a_header_is_refused() {
    let body = chunk(DATA_TRACKER_LIST, true, &chunk(1, true, &[]));
    assert_eq!(
        decode(&chunk(PSN_DATA_PACKET, true, &body)),
        Err(PsnError::MissingHeader)
    );
}

#[test]
fn a_chunk_claiming_more_than_arrived_is_refused_rather_than_read_past() {
    let mut datagram = data_packet(1, 1, 1);
    datagram.truncate(datagram.len() - 6);
    assert_eq!(decode(&datagram), Err(PsnError::Truncated));
}

#[test]
fn a_foreign_datagram_is_not_psn() {
    assert_eq!(decode(b"Art-Net\0"), Err(PsnError::NotPsn));
    assert_eq!(decode(&[]), Err(PsnError::NotPsn));
}

#[test]
fn an_info_packet_carries_the_names_a_data_packet_never_does() {
    // This is why a desk keeps info packets at all: a data packet knows tracker 2, and only an info
    // packet knows it is called "Presenter".
    let datagram = info_packet("OpenFollow", &[(2, "Presenter"), (7, "Guitar")]);

    let PsnPacket::Info(packet) = decode(&datagram).expect("an info packet decodes") else {
        panic!("expected an info packet");
    };

    assert_eq!(packet.system_name.as_deref(), Some("OpenFollow"));
    assert_eq!(packet.trackers.len(), 2);
    assert_eq!(packet.trackers[0].id, 2);
    assert_eq!(packet.trackers[0].name.as_deref(), Some("Presenter"));
    assert_eq!(packet.trackers[1].name.as_deref(), Some("Guitar"));
}

#[test]
fn a_terminated_name_keeps_only_what_precedes_the_terminator() {
    // Senders differ over whether they terminate a name; an operator must not see the padding.
    let datagram = chunk(
        PSN_INFO_PACKET,
        true,
        &[
            header_chunk(1, 1, 1),
            chunk(INFO_SYSTEM_NAME, false, b"Photon\0\0\0\0"),
        ]
        .concat(),
    );

    let PsnPacket::Info(packet) = decode(&datagram).expect("decodes") else {
        panic!("expected an info packet");
    };
    assert_eq!(packet.system_name.as_deref(), Some("Photon"));
}

#[test]
fn a_split_frame_is_one_frame_again() {
    let mut assembler = PsnFrameAssembler::new();

    let first = assembler.push(decoded_data(&position_only_packet(&[(1, 1.0)], 4, 2)));
    let second = assembler.push(decoded_data(&position_only_packet(&[(2, 2.0)], 4, 2)));

    // Nothing is reported until the sender's promised packet count has arrived.
    assert!(first.is_none());
    let frame = second.expect("the second packet finishes frame 4");
    assert!(frame.complete);
    assert_eq!(frame.frame_id, 4);
    assert_eq!(frame.packets_received, 2);
    assert_eq!(
        frame.trackers.iter().map(|t| t.id).collect::<Vec<_>>(),
        vec![1, 2]
    );
}

#[test]
fn a_lost_packet_still_yields_the_trackers_that_arrived() {
    // The alternative is holding a frame forever because one datagram of it never came.
    let mut assembler = PsnFrameAssembler::new();
    assembler.push(decoded_data(&position_only_packet(&[(1, 1.0)], 4, 3)));

    let frame = assembler
        .push(decoded_data(&position_only_packet(&[(9, 9.0)], 5, 1)))
        .expect("the next frame's arrival closes the incomplete one");

    assert_eq!(frame.frame_id, 4);
    assert!(!frame.complete);
    assert_eq!(frame.packets_received, 1);
    assert_eq!(frame.packets_expected, 3);
    assert_eq!(frame.trackers.len(), 1);
}

#[test]
fn a_single_packet_frame_is_finished_by_that_packet() {
    let mut assembler = PsnFrameAssembler::new();
    let frame = assembler
        .push(decoded_data(&position_only_packet(&[(1, 1.0)], 8, 1)))
        .expect("one packet is the whole frame");
    assert!(frame.complete);
    assert_eq!(frame.packets_expected, 1);
}

#[test]
fn a_sender_that_repeats_a_tracker_in_one_frame_is_believed_the_second_time() {
    let mut assembler = PsnFrameAssembler::new();
    assembler.push(decoded_data(&position_only_packet(&[(1, 1.0)], 2, 2)));
    let frame = assembler
        .push(decoded_data(&position_only_packet(&[(1, 5.0)], 2, 2)))
        .expect("frame 2 completes");

    assert_eq!(frame.trackers.len(), 1);
    assert_eq!(frame.trackers[0].position.map(|p| p.x), Some(5.0));
}

#[test]
fn a_sender_that_falls_silent_leaves_nothing_stranded() {
    let mut assembler = PsnFrameAssembler::new();
    assembler.push(decoded_data(&position_only_packet(&[(1, 1.0)], 4, 4)));

    let flushed = assembler.flush().expect("the open frame is handed back");

    assert_eq!(flushed.frame_id, 4);
    assert!(!flushed.complete);
    assert!(assembler.flush().is_none());
}

#[test]
fn the_default_destination_is_the_group_a_desk_joins() {
    let destination = psn_multicast_destination();
    assert_eq!(destination.ip().to_string(), "236.10.10.10");
    assert_eq!(destination.port(), 56565);
}

#[test]
fn trackers_can_be_looked_up_by_id() {
    let packet = decoded_data(&position_only_packet(&[(9, 9.0), (1, 1.0)], 1, 1));
    let indexed = trackers_by_id(&packet.trackers);
    assert_eq!(indexed.keys().copied().collect::<Vec<_>>(), vec![1, 9]);
    assert_eq!(indexed[&9].position.map(|p| p.x), Some(9.0));
}
