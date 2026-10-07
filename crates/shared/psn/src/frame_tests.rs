use super::*;
use crate::PsnPacketHeader;

fn packet(frame: u8, count: u8, timestamp: u64, tracker: u16) -> PsnDataPacket {
    PsnDataPacket {
        header: PsnPacketHeader {
            frame_id: frame,
            frame_packet_count: count,
            timestamp_micros: timestamp,
            ..Default::default()
        },
        trackers: vec![PsnTrackerData {
            id: tracker,
            position: Some(PsnVector3 {
                x: f32::from(tracker),
                y: 0.0,
                z: 0.0,
            }),
            ..Default::default()
        }],
    }
}

#[test]
fn compatibility_push_keeps_new_complete_frame_after_an_incomplete_predecessor() {
    let mut assembler = PsnFrameAssembler::new();
    assert!(assembler.push(packet(1, 2, 10, 1)).is_none());
    let accepted = assembler.push(packet(2, 1, 20, 2)).unwrap();
    assert!(accepted.complete);
    assert_eq!(accepted.frame_id, 2);
    assert_eq!(accepted.trackers[0].id, 2);
}

#[test]
fn repeated_split_packet_never_counts_as_the_missing_part() {
    let mut assembler = PsnFrameAssembler::new();
    let first = packet(2, 2, 10, 1);
    assert!(assembler.push_detailed(&first).completed.is_none());
    for _ in 0..300 {
        let duplicate = assembler.push_detailed(&first);
        assert_eq!(duplicate.rejection, Some(PsnFrameRejection::Duplicate));
        assert!(duplicate.completed.is_none());
        assert_eq!(assembler.open.as_ref().unwrap().packets.len(), 1);
    }
    let completed = assembler
        .push_detailed(&packet(2, 2, 11, 2))
        .completed
        .unwrap();
    assert_eq!(completed.packets_received, 2);
    assert_eq!(completed.trackers.len(), 2);
}

#[test]
fn split_packet_timestamps_are_per_packet_and_arrival_order_can_reverse() {
    let mut assembler = PsnFrameAssembler::new();
    assembler.push(packet(9, 1, 90, 0)).unwrap();
    assert!(assembler.push(packet(10, 2, 105, 2)).is_none());
    let completed = assembler.push(packet(10, 2, 100, 1)).unwrap();
    assert!(completed.complete);
    assert_eq!(completed.timestamp_micros, 100);
    assert_eq!(
        completed
            .trackers
            .iter()
            .map(|tracker| tracker.id)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(
        assembler.push_detailed(&packet(9, 1, 90, 0)).rejection,
        Some(PsnFrameRejection::OutOfOrderOrRestart)
    );
}

#[test]
fn equal_timestamp_serial_order_handles_wrap_duplicates_and_half_range_ambiguity() {
    let mut assembler = PsnFrameAssembler::new();
    assert!(assembler.push(packet(255, 1, 0, 1)).unwrap().complete);
    assert!(assembler.push(packet(0, 1, 0, 1)).unwrap().complete);
    assert_eq!(
        assembler.push_detailed(&packet(0, 1, 0, 1)).rejection,
        Some(PsnFrameRejection::Duplicate)
    );
    assert_eq!(
        assembler.push_detailed(&packet(255, 1, 0, 1)).rejection,
        Some(PsnFrameRejection::OutOfOrderOrRestart)
    );
    assert_eq!(
        assembler.push_detailed(&packet(128, 1, 0, 1)).rejection,
        Some(PsnFrameRejection::Ambiguous)
    );
    assert!(assembler.push(packet(1, 1, 0, 1)).unwrap().complete);
}

#[test]
fn authoritative_newer_send_time_allows_more_than_half_a_counter_of_lost_frames() {
    let mut assembler = PsnFrameAssembler::new();
    assembler.push(packet(1, 1, 10, 1)).unwrap();
    assert!(assembler.push(packet(200, 1, 500, 1)).unwrap().complete);
    assert_eq!(
        assembler.push_detailed(&packet(199, 1, 490, 1)).rejection,
        Some(PsnFrameRejection::OutOfOrderOrRestart)
    );
}

#[test]
fn changing_packet_count_cannot_manufacture_a_complete_frame() {
    let mut assembler = PsnFrameAssembler::new();
    assembler.push(packet(1, 3, 10, 1));
    assert_eq!(
        assembler.push_detailed(&packet(1, 2, 11, 2)).rejection,
        Some(PsnFrameRejection::InconsistentPacketCount)
    );
    assert_eq!(
        assembler.push_detailed(&packet(1, 3, 12, 3)).rejection,
        Some(PsnFrameRejection::Ambiguous)
    );
    let next = assembler.push_detailed(&packet(2, 1, 20, 4));
    assert!(!next.discarded.unwrap().complete);
    assert!(next.completed.unwrap().complete);
}

#[test]
fn a_retransmitted_nan_packet_is_a_duplicate_and_does_not_grow_storage() {
    let mut assembler = PsnFrameAssembler::new();
    let mut invalid = packet(1, 2, 10, 1);
    invalid.trackers[0].position.as_mut().unwrap().x = f32::NAN;
    assembler.push_detailed(&invalid);
    assert_eq!(
        assembler.push_detailed(&invalid).rejection,
        Some(PsnFrameRejection::Duplicate)
    );
    assert_eq!(assembler.open.as_ref().unwrap().packets.len(), 1);
}

#[test]
fn direct_packet_api_cannot_allocate_an_unbounded_frame() {
    let mut assembler = PsnFrameAssembler::new();
    let mut oversized = packet(1, 1, 10, 1);
    oversized.trackers = vec![PsnTrackerData::default(); PSN_MAX_PACKET_BYTES / 4 + 1];
    assert_eq!(
        assembler.push_detailed(&oversized).rejection,
        Some(PsnFrameRejection::Oversized)
    );
    assert!(assembler.open.is_none());
}
