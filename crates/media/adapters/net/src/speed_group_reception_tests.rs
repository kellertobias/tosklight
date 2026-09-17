use super::*;

const LISTEN: &str = "0.0.0.0:4810";

fn at(millis: u64) -> Timestamp {
    Timestamp::from_millis(millis)
}

fn from() -> Option<SocketAddr> {
    Some("192.168.1.10:50000".parse().unwrap())
}

fn update(source: &str, sequence: u32, group: u32, bpm: f64) -> SpeedGroupUpdate {
    SpeedGroupUpdate {
        source: source.to_owned(),
        sequence,
        group,
        bpm,
        beat_phase: 0.0,
        running: true,
    }
}

fn reception() -> SpeedGroupReception {
    SpeedGroupReception::listening(LISTEN.parse().unwrap())
}

fn bpm(reception: &SpeedGroupReception, group: u32) -> Option<f64> {
    reception
        .snapshot(SpeedGroupId::new(group))
        .map(|snapshot| snapshot.bpm)
}

#[test]
fn reception_reports_whether_it_was_asked_for_and_whether_it_started() {
    assert_eq!(
        SpeedGroupReception::default().connection(at(0)),
        SpeedGroupConnection::Disabled
    );
    assert_eq!(reception().connection(at(0)), SpeedGroupConnection::Waiting);
    let unavailable = SpeedGroupReception::unavailable("port 4810 is in use");
    let status = unavailable.status(at(0));
    assert_eq!(status.connection, SpeedGroupConnection::Unavailable);
    assert!(status.detail.unwrap().contains("in use"));
}

#[test]
fn an_accepted_update_becomes_the_groups_clock() {
    let mut reception = reception();
    reception
        .accept(update("desk", 1, 2, 128.0), from(), at(100))
        .unwrap();
    let snapshot = reception.snapshot(SpeedGroupId::new(2)).unwrap();
    assert_eq!(snapshot.bpm, 128.0);
    assert_eq!(snapshot.observed_at, at(100));
    assert_eq!(bpm(&reception, 1), None, "an unheard group has no clock");

    let status = reception.status(at(600));
    assert_eq!(status.connection, SpeedGroupConnection::Connected);
    assert_eq!(status.sender.as_deref(), Some("desk"));
    assert_eq!(status.sender_address, from());
    assert_eq!(status.last_update_age_millis, Some(500));
    assert_eq!(status.accepted, 1);
    assert_eq!(status.groups[0].group, 2);
    assert!(status.groups[0].fresh);
}

#[test]
fn a_paused_group_is_a_tempo_of_zero_but_still_reports_its_rate() {
    let mut reception = reception();
    let paused = SpeedGroupUpdate {
        running: false,
        ..update("desk", 1, 1, 120.0)
    };
    reception.accept(paused, from(), at(0)).unwrap();
    assert_eq!(bpm(&reception, 1), Some(0.0));
    let status = reception.status(at(0));
    assert_eq!(status.groups[0].bpm, 120.0);
    assert!(!status.groups[0].running);
}

#[test]
fn a_late_datagram_never_steps_the_tempo_backwards() {
    let mut reception = reception();
    reception
        .accept(update("desk", 5, 1, 130.0), from(), at(0))
        .unwrap();
    let late = reception.accept(update("desk", 4, 1, 90.0), from(), at(10));
    assert!(matches!(
        late,
        Err(SpeedGroupRejection::OutOfOrder { last: 5, .. })
    ));
    let repeated = reception.accept(update("desk", 5, 1, 90.0), from(), at(10));
    assert!(repeated.is_err(), "a duplicate is not applied twice");
    assert_eq!(bpm(&reception, 1), Some(130.0));
    assert_eq!(reception.status(at(10)).rejected, 2);
}

#[test]
fn a_lost_sender_holds_every_group_and_reconnects_with_a_reset_counter() {
    let mut reception = reception();
    reception
        .accept(update("desk", 900, 1, 128.0), from(), at(0))
        .unwrap();

    let lost = reception.status(at(SPEED_GROUP_FRESHNESS.as_millis() as u64));
    assert_eq!(lost.connection, SpeedGroupConnection::Lost);
    assert!(!lost.groups[0].fresh);
    assert_eq!(bpm(&reception, 1), Some(128.0), "the last clock is held");

    // The desk came back after a network loss with its counter starting over.
    reception
        .accept(update("desk", 1, 1, 100.0), from(), at(5_000))
        .unwrap();
    assert_eq!(bpm(&reception, 1), Some(100.0));
    assert_eq!(
        reception.connection(at(5_000)),
        SpeedGroupConnection::Connected
    );
}

#[test]
fn a_restarted_desk_takes_over_once_the_old_instance_is_silent() {
    let mut reception = reception();
    reception
        .accept(update("desk-old", 50, 1, 128.0), from(), at(0))
        .unwrap();
    reception
        .accept(update("desk-old", 51, 2, 90.0), from(), at(0))
        .unwrap();
    // The old desk is silent past the window; a new identity takes over and its groups replace
    // the old ones rather than mixing with them.
    reception
        .accept(update("desk-new", 1, 1, 110.0), from(), at(2_000))
        .unwrap();
    assert_eq!(bpm(&reception, 1), Some(110.0));
    assert_eq!(bpm(&reception, 2), None);
}

#[test]
fn a_second_desk_cannot_take_over_a_live_stream() {
    let mut reception = reception();
    reception
        .accept(update("desk-a", 1, 1, 128.0), from(), at(0))
        .unwrap();
    let competing = reception.accept(update("desk-b", 1, 1, 60.0), from(), at(100));
    assert!(matches!(
        competing,
        Err(SpeedGroupRejection::CompetingSender { .. })
    ));
    assert_eq!(bpm(&reception, 1), Some(128.0));
    let status = reception.status(at(100));
    assert!(status.rejections[0].reason.contains("desk-b"));
    assert_eq!(status.sender.as_deref(), Some("desk-a"));
}

#[test]
fn values_outside_the_contract_are_refused_and_reported() {
    let mut reception = reception();
    for bad in [
        update("desk", 1, 0, 120.0),
        update("desk", 1, MAX_SPEED_GROUP + 1, 120.0),
        update("desk", 1, 1, -1.0),
        update("desk", 1, 1, 1_000.0),
        update("desk", 1, 1, f64::NAN),
        update("", 1, 1, 120.0),
        SpeedGroupUpdate {
            beat_phase: 1.0,
            ..update("desk", 1, 1, 120.0)
        },
    ] {
        assert!(matches!(
            reception.accept(bad.clone(), from(), at(0)),
            Err(SpeedGroupRejection::Invalid(_))
        ));
    }
    reception.reject("not an OSC packet", from(), at(1));
    let status = reception.status(at(1));
    assert_eq!(status.accepted, 0);
    assert_eq!(status.rejected, 8);
    assert_eq!(status.connection, SpeedGroupConnection::Waiting);
    assert_eq!(status.rejections.len(), 8);
    assert!(status.rejections[0].reason.contains("not an OSC packet"));
    assert!(status.groups.is_empty());
    reception.reject("again", None, at(2));
    assert_eq!(
        reception.status(at(2)).rejections.len(),
        8,
        "the history is bounded"
    );
}
