//! The whole path a marker takes, driven by real packets.
//!
//! Every test here starts from bytes a sender would actually transmit — built with the same
//! encoder the repository's test sender uses — so what is being checked is the desk's answer to
//! PosiStageNet, not to a convenient in-memory shape of it.

use super::super::config::{PsnBinding, PsnConfiguration, PsnZone};
use super::*;
use light_psn_wire::{
    PsnInfoPacket, PsnTrackerData, PsnTrackerInfo, PsnVector3, encode_data_frame,
    encode_info_packet,
};

const SENDER: &str = "10.0.0.9:56565";
const BINDING: Uuid = Uuid::from_u128(1);
const POINT: Uuid = Uuid::from_u128(2);
const ZONE: Uuid = Uuid::from_u128(3);
const ENTER_MACRO: Uuid = Uuid::from_u128(4);

fn sender() -> SocketAddr {
    SENDER.parse().unwrap()
}

fn bound() -> PsnConfiguration {
    PsnConfiguration {
        enabled: true,
        bindings: vec![PsnBinding {
            id: BINDING,
            tracker_id: 3,
            point_fixture_id: POINT,
            enabled: true,
        }],
        ..PsnConfiguration::default()
    }
}

fn resource(configuration: PsnConfiguration) -> PsnResource {
    let resource = PsnResource::new();
    resource.install(configuration);
    resource.install_point_locations(HashMap::from([(POINT, [0.0, 0.0, 0.0])]));
    resource
}

fn frame_at(resource: &PsnResource, tracker_id: u16, position: [f32; 3], now_millis: u64) {
    let tracker = PsnTrackerData {
        id: tracker_id,
        position: Some(PsnVector3 {
            x: position[0],
            y: position[1],
            z: position[2],
        }),
        ..PsnTrackerData::default()
    };
    for datagram in encode_data_frame(now_millis * 1_000, 1, &[tracker]) {
        resource.observe(sender(), &datagram, now_millis);
    }
}

fn held_axis(tick: &PsnTick, attribute: &str) -> Option<f32> {
    tick.overrides
        .iter()
        .find(|held| &*held.attribute.0 == attribute)
        .and_then(|held| held.value.normalized())
}

#[test]
fn a_marker_that_moves_moves_the_point_it_is_bound_to() {
    let resource = resource(bound());
    frame_at(&resource, 3, [2.0, 1.0, -4.0], 1_000);

    let tick = resource.tick(1_000);

    assert_eq!(tick.status.health, Some(PsnHealth::Receiving));
    assert_eq!(tick.overrides.len(), 3);
    assert_eq!(
        held_axis(&tick, "point.position.x"),
        Some(super::super::bindings::normalized_axis(2.0))
    );
    assert_eq!(tick.status.placements[0].position_metres, [2.0, 1.0, -4.0]);

    frame_at(&resource, 3, [3.0, 1.0, -4.0], 1_050);
    let moved = resource.tick(1_050);
    assert_eq!(moved.status.placements[0].position_metres[0], 3.0);
}

#[test]
fn a_tracker_nothing_is_bound_to_moves_nothing() {
    // The acceptance criterion in plain terms: traffic on the group is not permission to move a
    // light. Only a binding an operator made is.
    let resource = resource(PsnConfiguration {
        enabled: true,
        ..PsnConfiguration::default()
    });
    frame_at(&resource, 3, [2.0, 1.0, -4.0], 1_000);

    let tick = resource.tick(1_000);

    assert!(tick.overrides.is_empty());
    assert_eq!(tick.status.trackers.len(), 1);
    assert_eq!(tick.status.trackers[0].tracker_id, 3);
}

#[test]
fn a_source_that_stops_holds_its_last_position_and_says_it_is_stale() {
    let resource = resource(bound());
    frame_at(&resource, 3, [2.0, 1.0, -4.0], 1_000);
    resource.tick(1_000);

    // Five seconds of silence, against a one-second stale timeout.
    let tick = resource.tick(6_000);

    assert!(matches!(tick.status.health, Some(PsnHealth::Stale { .. })));
    assert!(tick.status.trackers[0].stale);
    assert_eq!(tick.status.placements[0].position_metres, [2.0, 1.0, -4.0]);
    assert_eq!(
        held_axis(&tick, "point.position.x"),
        Some(super::super::bindings::normalized_axis(2.0))
    );
}

#[test]
fn switching_the_source_off_gives_the_point_back() {
    let resource = resource(bound());
    frame_at(&resource, 3, [2.0, 1.0, -4.0], 1_000);
    assert_eq!(resource.tick(1_000).overrides.len(), 3);

    resource.install(PsnConfiguration {
        enabled: false,
        ..bound()
    });

    let tick = resource.tick(1_100);
    assert!(tick.overrides.is_empty());
    assert!(!tick.status.enabled);
}

#[test]
fn unbinding_gives_the_point_back_while_the_stream_keeps_arriving() {
    let resource = resource(bound());
    frame_at(&resource, 3, [2.0, 1.0, -4.0], 1_000);
    assert_eq!(resource.tick(1_000).overrides.len(), 3);

    resource.install(PsnConfiguration {
        enabled: true,
        ..PsnConfiguration::default()
    });
    frame_at(&resource, 3, [2.5, 1.0, -4.0], 1_100);

    let tick = resource.tick(1_100);
    assert!(tick.overrides.is_empty());
    // Still listening, still reporting: only the binding went away.
    assert_eq!(tick.status.trackers.len(), 1);
}

#[test]
fn a_name_arrives_in_its_own_packet_and_is_kept() {
    let resource = resource(bound());
    frame_at(&resource, 3, [0.0, 0.0, 0.0], 1_000);
    assert_eq!(resource.tick(1_000).status.trackers[0].name, None);

    let info = encode_info_packet(&PsnInfoPacket {
        system_name: Some("OpenFollow".into()),
        trackers: vec![PsnTrackerInfo {
            id: 3,
            name: Some("Presenter".into()),
        }],
        ..PsnInfoPacket::default()
    });
    resource.observe(sender(), &info, 1_100);
    frame_at(&resource, 3, [0.0, 0.0, 0.0], 1_150);

    let tick = resource.tick(1_150);
    assert_eq!(tick.status.trackers[0].name.as_deref(), Some("Presenter"));
    assert_eq!(tick.status.system_names, vec!["OpenFollow".to_owned()]);
}

#[test]
fn a_datagram_from_something_else_on_the_group_is_counted_and_dropped() {
    let resource = resource(bound());
    // Art-Net, which shares a lighting network with everything else.
    resource.observe(sender(), b"Art-Net\0\x00\x50\x00\x0e", 1_000);

    let tick = resource.tick(1_000);

    assert_eq!(tick.status.ignored_datagrams, 1);
    assert!(tick.overrides.is_empty());
    assert!(tick.status.trackers.is_empty());
}

#[test]
fn calibration_puts_the_marker_where_the_show_says_it_is() {
    let mut configuration = bound();
    configuration.calibration.offset_metres = [0.0, 0.0, 4.0];
    let resource = resource(configuration);
    frame_at(&resource, 3, [1.0, 2.0, 0.0], 1_000);

    let tick = resource.tick(1_000);

    assert_eq!(tick.status.placements[0].position_metres, [1.0, 2.0, 4.0]);
    assert_eq!(
        tick.status.trackers[0].position_metres,
        Some([1.0, 2.0, 4.0])
    );
}

#[test]
fn walking_into_a_zone_asks_for_its_macro_once() {
    let mut configuration = bound();
    configuration.zones = vec![PsnZone {
        id: ZONE,
        name: "Downstage".into(),
        min_metres: [-1.0, 0.0, -1.0],
        max_metres: [1.0, 3.0, 1.0],
        tracker_ids: Vec::new(),
        enter_macro_id: Some(ENTER_MACRO),
        leave_macro_id: None,
        dwell_millis: 0,
    }];
    let resource = resource(configuration);

    frame_at(&resource, 3, [9.0, 1.0, 0.0], 1_000);
    assert!(resource.tick(1_000).zone_transitions.is_empty());

    frame_at(&resource, 3, [0.0, 1.0, 0.0], 1_100);
    let entered = resource.tick(1_100);
    assert_eq!(
        entered.zone_transitions,
        vec![(ZONE, super::super::zones::ZoneTransition::Entered)]
    );
    assert_eq!(entered.status.occupied_zones, vec![ZONE]);

    frame_at(&resource, 3, [0.1, 1.0, 0.0], 1_200);
    assert!(resource.tick(1_200).zone_transitions.is_empty());
}

#[test]
fn a_source_that_disappears_does_not_empty_its_zones() {
    // A tracking system falling off the network is not everybody walking off stage. Nothing runs.
    let mut configuration = bound();
    configuration.zones = vec![PsnZone {
        id: ZONE,
        name: "Downstage".into(),
        min_metres: [-1.0, 0.0, -1.0],
        max_metres: [1.0, 3.0, 1.0],
        tracker_ids: Vec::new(),
        enter_macro_id: Some(ENTER_MACRO),
        leave_macro_id: Some(Uuid::from_u128(5)),
        dwell_millis: 0,
    }];
    let resource = resource(configuration);
    frame_at(&resource, 3, [0.0, 1.0, 0.0], 1_000);
    resource.tick(1_000);

    let silent = resource.tick(30_000);

    assert!(silent.zone_transitions.is_empty());
    assert_eq!(silent.status.occupied_zones, vec![ZONE]);
}

#[test]
fn nothing_heard_at_all_is_reported_as_silence_rather_than_a_fault() {
    let resource = resource(bound());

    let tick = resource.tick(5_000);

    assert_eq!(tick.status.health, Some(PsnHealth::Silent));
    assert!(tick.status.error.is_none());
    assert!(tick.overrides.is_empty());
}

#[test]
fn moving_the_source_forgets_what_the_old_group_said() {
    let resource = resource(bound());
    frame_at(&resource, 3, [2.0, 1.0, -4.0], 1_000);
    assert_eq!(resource.tick(1_000).overrides.len(), 3);

    let mut moved = bound();
    moved.port = 56_566;
    resource.install(moved);

    let tick = resource.tick(1_100);
    assert!(tick.status.trackers.is_empty());
    assert!(tick.overrides.is_empty());
}
