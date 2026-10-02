use super::*;
use crate::PsnError;

/// The same test-only encoder the wire tests use, kept to the packets these tests need.
fn chunk(id: u16, has_subchunks: bool, data: &[u8]) -> Vec<u8> {
    let len = u32::try_from(data.len()).expect("a test chunk fits");
    let header = u32::from(id) | (len << 16) | (u32::from(has_subchunks) << 31);
    let mut bytes = header.to_le_bytes().to_vec();
    bytes.extend_from_slice(data);
    bytes
}

fn header(frame_id: u8, frame_packet_count: u8) -> Vec<u8> {
    let mut data = 0_u64.to_le_bytes().to_vec();
    data.extend_from_slice(&[2, 0, frame_id, frame_packet_count]);
    chunk(0x0000, false, &data)
}

fn data(trackers: &[(u16, f32)], frame_id: u8, frame_packet_count: u8) -> Vec<u8> {
    let mut list = Vec::new();
    for (id, x) in trackers {
        let mut position = x.to_le_bytes().to_vec();
        position.extend_from_slice(&0.0_f32.to_le_bytes());
        position.extend_from_slice(&0.0_f32.to_le_bytes());
        list.extend(chunk(*id, true, &chunk(0x0000, false, &position)));
    }
    let mut body = header(frame_id, frame_packet_count);
    body.extend(chunk(0x0001, true, &list));
    chunk(0x6755, true, &body)
}

fn info(system: &str, trackers: &[(u16, &str)]) -> Vec<u8> {
    let mut list = Vec::new();
    for (id, name) in trackers {
        list.extend(chunk(*id, true, &chunk(0x0000, false, name.as_bytes())));
    }
    let mut body = header(0, 1);
    body.extend(chunk(0x0001, false, system.as_bytes()));
    body.extend(chunk(0x0002, true, &list));
    chunk(0x6756, true, &body)
}

#[test]
fn a_tracker_is_only_named_once_an_info_packet_has_said_so() {
    // Data packets carry numbers. This is the whole reason a listener keeps state at all.
    let mut tracking = PsnTracking::new();

    tracking.observe(&data(&[(4, 1.0)], 1, 1), 1_000);
    assert_eq!(tracking.tracker(4).expect("tracked").name, None);

    tracking.observe(&info("OpenFollow", &[(4, "Presenter")]), 1_100);

    assert_eq!(
        tracking.tracker(4).expect("tracked").name.as_deref(),
        Some("Presenter")
    );
    assert_eq!(tracking.system_name(), Some("OpenFollow"));
}

#[test]
fn a_name_learned_late_is_applied_to_a_tracker_already_being_followed() {
    let mut tracking = PsnTracking::new();
    tracking.observe(&data(&[(1, 0.0)], 1, 1), 0);
    tracking.observe(&info("Photon", &[(1, "Guitar")]), 10);
    tracking.observe(&data(&[(1, 2.0)], 2, 1), 20);

    let tracked = tracking.tracker(1).expect("tracked");
    assert_eq!(tracked.name.as_deref(), Some("Guitar"));
    assert_eq!(tracked.position().map(|p| p.x), Some(2.0));
}

#[test]
fn a_tracker_the_sender_stops_listing_keeps_its_last_position_and_grows_old() {
    // The alternative — forgetting it, or reading it as the origin — would swing a light to the
    // middle of the stage the moment a marker was occluded.
    let mut tracking = PsnTracking::new();
    tracking.observe(&data(&[(1, 5.0), (2, 6.0)], 1, 1), 1_000);
    tracking.observe(&data(&[(1, 5.5)], 2, 1), 1_020);

    let dropped = tracking.tracker(2).expect("still known");
    assert_eq!(dropped.position().map(|p| p.x), Some(6.0));
    assert_eq!(dropped.age_millis(1_020), 20);
    assert_eq!(tracking.tracker(1).expect("tracked").age_millis(1_020), 0);
}

#[test]
fn a_stale_position_is_withheld_from_a_control_path_but_still_visible() {
    let mut tracking = PsnTracking::new();
    tracking.observe(&data(&[(1, 3.0)], 1, 1), 0);

    // Fresh enough to aim at.
    assert_eq!(tracking.fresh_position(1, 100, 200).map(|p| p.x), Some(3.0));
    // Too old to aim at: better not to move than to move to where somebody was.
    assert_eq!(tracking.fresh_position(1, 500, 200), None);
    // Still shown to the operator, with its age, so the screen explains why nothing is moving.
    assert_eq!(tracking.tracker(1).expect("tracked").age_millis(500), 500);
}

#[test]
fn an_unknown_tracker_has_no_fresh_position() {
    let tracking = PsnTracking::new();
    assert_eq!(tracking.fresh_position(7, 0, 1_000), None);
}

#[test]
fn health_tells_silence_from_receiving_from_stale() {
    let mut tracking = PsnTracking::new();

    // A sender that was never switched on, and a desk on the wrong network, look the same — and
    // neither is an error.
    assert_eq!(tracking.health(0, 500), PsnSourceHealth::Silent);

    tracking.observe(&data(&[(1, 0.0)], 1, 1), 1_000);
    assert_eq!(tracking.health(1_200, 500), PsnSourceHealth::Receiving);
    assert_eq!(
        tracking.health(2_000, 500),
        PsnSourceHealth::Stale {
            silent_for_millis: 1_000
        }
    );
}

#[test]
fn a_foreign_datagram_is_counted_and_dropped_without_disturbing_what_is_known() {
    let mut tracking = PsnTracking::new();
    tracking.observe(&data(&[(1, 1.0)], 1, 1), 100);

    let ignored = tracking.observe(b"Art-Net\0not for us", 110);

    assert_eq!(ignored, PsnObservation::Ignored(PsnError::NotPsn));
    assert_eq!(tracking.ignored(), 1);
    assert_eq!(tracking.frames(), 1);
    assert_eq!(
        tracking
            .tracker(1)
            .expect("tracked")
            .position()
            .map(|p| p.x),
        Some(1.0)
    );
    // A dropped datagram is not evidence the sender is alive, so it must not refresh health.
    assert_eq!(
        tracking.health(700, 500),
        PsnSourceHealth::Stale {
            silent_for_millis: 600
        }
    );
}

#[test]
fn a_split_frame_records_nothing_until_it_is_whole() {
    // Half a frame is half a stage. Recording it would move some fixtures a frame early.
    let mut tracking = PsnTracking::new();

    let first = tracking.observe(&data(&[(1, 1.0)], 1, 2), 0);
    assert_eq!(first, PsnObservation::PartialFrame);
    assert!(tracking.tracker(1).is_none());

    let second = tracking.observe(&data(&[(2, 2.0)], 1, 2), 5);
    assert!(matches!(second, PsnObservation::Frame(_)));
    assert_eq!(tracking.trackers().len(), 2);
    assert_eq!(tracking.frames(), 1);
}

#[test]
fn trackers_are_listed_in_id_order_however_they_arrived() {
    let mut tracking = PsnTracking::new();
    tracking.observe(&data(&[(9, 0.0), (2, 0.0), (5, 0.0)], 1, 1), 0);

    assert_eq!(
        tracking.trackers().iter().map(|t| t.id).collect::<Vec<_>>(),
        vec![2, 5, 9]
    );
}

#[test]
fn an_info_packet_that_stops_offering_a_name_takes_the_name_away() {
    // The list in an info packet is the sender's current answer, not an addition to an old one.
    let mut tracking = PsnTracking::new();
    tracking.observe(&data(&[(1, 0.0)], 1, 1), 0);
    tracking.observe(&info("Photon", &[(1, "Presenter")]), 10);
    assert_eq!(
        tracking.tracker(1).expect("tracked").name.as_deref(),
        Some("Presenter")
    );

    tracking.observe(&info("Photon", &[]), 1_010);

    assert_eq!(tracking.tracker(1).expect("tracked").name, None);
    assert_eq!(tracking.last_info_at_millis(), Some(1_010));
}

fn sample_packet(frame_id: u8, timestamp: u64, position: Option<PsnVector3>) -> Vec<u8> {
    crate::encode_data_frame(
        timestamp,
        frame_id,
        &[PsnTrackerData {
            id: 1,
            position,
            ..Default::default()
        }],
    )
    .pop()
    .unwrap()
}

fn position(x: f32) -> Option<PsnVector3> {
    Some(PsnVector3 { x, y: 2.0, z: 3.0 })
}

#[test]
fn duplicate_or_old_frames_never_refresh_accepted_identity_position_or_age() {
    let mut tracking = PsnTracking::new();
    let original = sample_packet(3, 100, position(1.0));
    assert!(matches!(
        tracking.observe(&original, 50),
        PsnObservation::Frame(_)
    ));
    let identity = tracking.accepted_sample();
    assert_eq!(
        tracking.observe(&original, 1000),
        PsnObservation::Rejected(PsnFrameRejection::Duplicate)
    );
    assert_eq!(
        tracking.observe(&sample_packet(2, 90, position(9.0)), 1100),
        PsnObservation::Rejected(PsnFrameRejection::OutOfOrderOrRestart)
    );
    assert_eq!(tracking.accepted_sample(), identity);
    assert_eq!(tracking.frames(), 1);
    assert_eq!(tracking.tracker(1).unwrap().position(), position(1.0));
    assert_eq!(tracking.tracker(1).unwrap().age_millis(1100), 1050);
    assert_eq!(
        tracking.health(1100, 100),
        PsnSourceHealth::Stale {
            silent_for_millis: 1050
        }
    );
    assert_eq!(tracking.diagnostics().duplicate_datagrams, 1);
}

#[test]
fn incomplete_frame_holds_previous_position_and_cannot_drop_next_complete_frame() {
    let mut tracking = PsnTracking::new();
    tracking.observe(&data(&[(1, 1.0)], 1, 1), 10);
    let original = tracking.accepted_sample();
    tracking.observe(&data(&[(1, 9.0)], 2, 2), 20);
    assert_eq!(tracking.accepted_sample(), original);
    assert_eq!(tracking.tracker(1).unwrap().position().unwrap().x, 1.0);
    let result = tracking.observe(&data(&[(2, 2.0)], 3, 1), 30);
    assert!(matches!(result, PsnObservation::Frame(frame) if frame.frame_id == 3));
    assert_eq!(tracking.frames(), 2);
    assert_eq!(tracking.tracker(1).unwrap().position().unwrap().x, 1.0);
    assert_eq!(tracking.tracker(1).unwrap().age_millis(30), 20);
    assert_eq!(tracking.tracker(2).unwrap().position().unwrap().x, 2.0);
    assert_eq!(tracking.diagnostics().incomplete_frames, 1);
}

#[test]
fn a_completed_split_frame_has_one_identity_but_preserves_each_packet_arrival_age() {
    let mut tracking = PsnTracking::new();
    let first = data(&[(1, 1.0)], 1, 2);
    tracking.observe(&first, 100);
    assert_eq!(
        tracking.observe(&first, 200),
        PsnObservation::Rejected(PsnFrameRejection::Duplicate)
    );
    tracking.observe(&data(&[(2, 2.0)], 1, 2), 500);
    let first = tracking.tracker(1).unwrap();
    let second = tracking.tracker(2).unwrap();
    assert_eq!(first.position_sample, second.position_sample);
    assert_eq!(first.position_sample, tracking.accepted_sample());
    assert_eq!(first.position_sample.unwrap().accepted_at_millis, 500);
    assert_eq!(first.age_millis(500), 400);
    assert_eq!(second.age_millis(500), 0);
}

#[test]
fn omitted_and_nonfinite_positions_keep_original_position_age_and_sample_identity() {
    let mut tracking = PsnTracking::new();
    tracking.observe(&sample_packet(1, 100, position(1.0)), 10);
    let original = tracking.tracker(1).unwrap().position_sample;
    for (frame, value) in [
        (2, None),
        (3, position(f32::NAN)),
        (4, position(f32::INFINITY)),
        (5, position(f32::NEG_INFINITY)),
    ] {
        tracking.observe(
            &sample_packet(frame, u64::from(frame) * 100, value),
            u64::from(frame) * 10,
        );
        let held = tracking.tracker(1).unwrap();
        assert_eq!(held.position(), position(1.0));
        assert_eq!(held.updated_at_millis, 10);
        assert_eq!(held.position_sample, original);
    }
    assert_eq!(tracking.diagnostics().invalid_positions, 3);
    assert_eq!(tracking.tracker(1).unwrap().age_millis(100), 90);
    tracking.observe(&sample_packet(6, 600, position(2.0)), 70);
    assert_eq!(tracking.tracker(1).unwrap().position(), position(2.0));
    assert_eq!(tracking.tracker(1).unwrap().age_millis(100), 30);
    assert_ne!(tracking.tracker(1).unwrap().position_sample, original);
}

#[test]
fn a_tracker_without_a_valid_position_never_acquires_a_position_sample() {
    let mut tracking = PsnTracking::new();
    tracking.observe(&sample_packet(1, 100, position(f32::NAN)), 10);
    assert_eq!(tracking.tracker(1).unwrap().position(), None);
    assert_eq!(tracking.tracker(1).unwrap().position_sample, None);
    assert_eq!(tracking.fresh_position(1, 20, 100), None);
}

#[test]
fn timestamp_regression_requires_explicit_epoch_change_and_keeps_held_pose_until_then() {
    let mut tracking = PsnTracking::new();
    tracking.observe(&sample_packet(90, 900_000, position(1.0)), 10);
    let original = tracking.tracker(1).unwrap().position_sample.unwrap();
    let restarted = sample_packet(1, 100, position(2.0));
    assert_eq!(
        tracking.observe(&restarted, 60_000),
        PsnObservation::Rejected(PsnFrameRejection::OutOfOrderOrRestart)
    );
    assert_eq!(tracking.tracker(1).unwrap().position(), position(1.0));
    tracking.restart_source_epoch();
    assert_eq!(tracking.tracker(1).unwrap().position_sample, Some(original));
    assert_eq!(tracking.tracker(1).unwrap().updated_at_millis, 10);
    assert!(tracking.accepted_sample().is_none());
    assert!(matches!(
        tracking.observe(&restarted, 60_010),
        PsnObservation::Frame(_)
    ));
    assert_ne!(
        tracking.accepted_sample().unwrap().id.source_epoch,
        original.id.source_epoch
    );
    assert_eq!(tracking.tracker(1).unwrap().position(), position(2.0));
}

#[test]
fn timing_out_an_incomplete_frame_never_publishes_or_refreshes_it() {
    let mut tracking = PsnTracking::new();
    tracking.observe(&data(&[(1, 1.0)], 1, 1), 10);
    tracking.observe(&data(&[(1, 9.0)], 2, 2), 20);
    assert!(tracking.discard_incomplete());
    assert!(!tracking.discard_incomplete());
    assert_eq!(tracking.tracker(1).unwrap().position().unwrap().x, 1.0);
    assert_eq!(tracking.tracker(1).unwrap().updated_at_millis, 10);
    assert_eq!(
        tracking.observe(&data(&[(2, 2.0)], 2, 2), 30),
        PsnObservation::Rejected(PsnFrameRejection::Ambiguous)
    );
    assert!(tracking.tracker(2).is_none());
    assert_eq!(tracking.diagnostics().incomplete_frames, 1);
}

#[test]
fn oversized_input_is_passively_rejected_without_allocating_tracker_history() {
    let mut tracking = PsnTracking::new();
    assert_eq!(
        tracking.observe(&vec![0; PSN_MAX_PACKET_BYTES + 1], 10),
        PsnObservation::Rejected(PsnFrameRejection::Oversized)
    );
    assert!(tracking.tracked.is_empty());
    assert!(tracking.pending_arrivals.is_empty());
    assert_eq!(tracking.health(1000, 100), PsnSourceHealth::Silent);
}
