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
fn tracking_input_keeps_world_target_while_status_reports_point_reach_limit() {
    let resource = resource(bound());
    frame_at(&resource, 3, [140.0, 0.0, 0.0], 100);
    let first = resource.tick(100);
    assert_eq!(
        first.status.placements[0].position_metres,
        [100.0, 0.0, 0.0]
    );
    assert!(first.status.placements[0].out_of_reach);
    assert_eq!(
        first.tracking.bindings[0].position_metres,
        [140.0, 0.0, 0.0]
    );

    resource.install_point_locations(HashMap::from([(POINT, [50.0, 0.0, 0.0])]));
    let committed = resource.committed_tracking_frame(110);
    assert_eq!(committed.bindings, first.tracking.bindings);
    assert_eq!(
        committed.accepted_sequence,
        first.tracking.accepted_sequence
    );
    assert_ne!(committed.point_generation, first.tracking.point_generation);
    assert_eq!(committed.sampled_at_millis, 110);
    assert_eq!(committed.bindings[0].position_received_at_millis, 100);
    assert_eq!(
        resource.status(110).placements[0].position_metres,
        [140.0, 0.0, 0.0]
    );

    let mut disabled = bound();
    disabled.enabled = false;
    resource.install(disabled);
    assert!(resource.committed_tracking_frame(120).bindings.is_empty());
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
    assert_eq!(tick.status.placements[0].position_metres, [2.0, 4.0, 1.0]);

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
    assert_eq!(tick.status.placements[0].position_metres, [2.0, 4.0, 1.0]);
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

    assert_eq!(tick.status.placements[0].position_metres, [1.0, 0.0, 6.0]);
    assert_eq!(
        tick.status.trackers[0].position_metres,
        Some([1.0, 0.0, 6.0])
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
fn status_reads_only_the_last_committed_zone_state() {
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
    frame_at(&resource, 3, [0.0, 1.0, 0.0], 1_000);

    for now in [1_000, 1_010, 1_020] {
        let status = resource.status(now);
        assert_eq!(status.trackers.len(), 1);
        assert!(status.occupied_zones.is_empty());
        assert!(status.placements.is_empty());
    }
    let tick = resource.tick(1_030);
    assert_eq!(tick.zone_transitions, vec![(ZONE, ZoneTransition::Entered)]);
    assert_eq!(resource.status(1_040).occupied_zones, vec![ZONE]);
    assert_eq!(resource.status(1_050).occupied_zones, vec![ZONE]);
    assert!(resource.tick(1_060).zone_transitions.is_empty());
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

fn source_frame(
    resource: &PsnResource,
    source: SocketAddr,
    tracker_id: u16,
    x: f32,
    timestamp: u64,
    now: u64,
) {
    let tracker = PsnTrackerData {
        id: tracker_id,
        position: Some(PsnVector3 { x, y: 0.0, z: 0.0 }),
        ..Default::default()
    };
    for datagram in encode_data_frame(timestamp, 1, &[tracker]) {
        resource.observe(source, &datagram, now);
    }
}

#[test]
fn equal_configuration_in_another_show_retires_sources_holds_and_point_cache() {
    let resource = resource(bound());
    let first = light_core::ShowId(Uuid::from_u128(100));
    let second = light_core::ShowId(Uuid::from_u128(101));
    resource.install_for_show(Some(first), bound());
    resource.install_point_locations(HashMap::from([(POINT, [0.0; 3])]));
    frame_at(&resource, 3, [2.0, 1.0, -4.0], 1_000);
    let initial = resource.tick(1_000);
    assert_eq!(initial.tracking.show_id, Some(first));
    assert_eq!(initial.tracking.bindings.len(), 1);
    let generation = resource.generation();
    resource.install_for_show(Some(second), bound());
    assert_ne!(resource.generation(), generation);
    assert!(resource.committed_overrides().is_empty());
    let current = resource.tick(1_100);
    assert!(current.overrides.is_empty());
    assert!(current.status.trackers.is_empty());
    assert_eq!(current.tracking.show_id, Some(second));
    assert!(resource.inner.lock().point_locations.is_empty());
}

#[test]
fn a_forced_reopen_resets_equal_identity_but_ordinary_install_is_idempotent() {
    let resource = resource(bound());
    frame_at(&resource, 3, [2.0, 0.0, 0.0], 1_000);
    resource.tick(1_000);
    let generation = resource.generation();
    let compiled = Arc::clone(&resource.inner.lock().configuration);
    resource.install_for_show(None, bound());
    resource.status(1_100);
    resource.tick(1_100);
    assert!(Arc::ptr_eq(&compiled, &resource.inner.lock().configuration));
    assert_eq!(generation, resource.generation());
    assert!(!resource.committed_overrides().is_empty());
    resource.reset_for_show(None, bound());
    assert_ne!(generation, resource.generation());
    assert!(resource.committed_overrides().is_empty());
    assert!(resource.status(1_200).sources.is_empty());
}

#[test]
fn reusing_a_binding_id_for_an_unheard_tracker_cannot_reuse_its_previous_hold() {
    let resource = resource(bound());
    frame_at(&resource, 3, [2.0, 0.0, 0.0], 1_000);
    resource.tick(1_000);
    let generation = resource.generation();
    let mut next = bound();
    next.bindings[0].tracker_id = 4;
    resource.install(next);
    assert_eq!(
        resource.generation(),
        generation,
        "binding edits must not reopen the socket"
    );
    assert!(
        resource.committed_overrides().is_empty(),
        "retirement is immediate, before a tick"
    );
    assert!(resource.tick(1_100).overrides.is_empty());
    frame_at(&resource, 4, [4.0, 0.0, 0.0], 1_200);
    assert_eq!(
        resource.tick(1_200).status.placements[0].position_metres[0],
        4.0
    );
}

#[test]
fn equal_arrival_source_ties_use_address_order_independently_of_insertion_order() {
    let lower: SocketAddr = "10.0.0.1:56565".parse().unwrap();
    let higher: SocketAddr = "10.0.0.2:56565".parse().unwrap();
    for ordered in [[higher, lower], [lower, higher]] {
        let resource = resource(bound());
        for source in ordered {
            source_frame(
                &resource,
                source,
                3,
                if source == lower { 1.0 } else { 2.0 },
                10,
                1_000,
            );
        }
        let tick = resource.tick(1_000);
        assert_eq!(tick.status.placements[0].position_metres[0], 1.0);
        assert_eq!(tick.tracking.bindings[0].identity.source, lower);
        assert_eq!(tick.tracking.bindings[0].position_received_at_millis, 1_000);
        source_frame(&resource, higher, 3, 3.0, 20, 1_010);
        let next = resource.tick(1_010);
        assert_eq!(next.tracking.bindings[0].identity.source, higher);
        assert_eq!(next.status.placements[0].position_metres[0], 3.0);
    }
}

#[test]
fn calibrated_overflow_holds_last_finite_point_and_zone_without_refreshing_identity() {
    let mut configuration = bound();
    configuration.calibration.scale = 2.0;
    configuration.zones = vec![PsnZone {
        id: ZONE,
        name: "Held zone".into(),
        min_metres: [-5.0; 3],
        max_metres: [5.0; 3],
        tracker_ids: vec![3],
        enter_macro_id: None,
        leave_macro_id: None,
        dwell_millis: 0,
    }];
    let resource = resource(configuration);
    source_frame(&resource, sender(), 3, 1.0, 10, 100);
    let initial = resource.tick(100);
    assert_eq!(initial.status.occupied_zones, vec![ZONE]);
    let identity = initial.tracking.bindings[0].identity;
    source_frame(&resource, sender(), 3, f32::MAX, 20, 200);
    let overflow = resource.tick(200);
    assert!(overflow.zone_transitions.is_empty());
    assert_eq!(overflow.status.occupied_zones, vec![ZONE]);
    assert_eq!(overflow.status.invalid_calibrated_positions, 1);
    assert_eq!(overflow.status.trackers[0].position_metres, None);
    assert_eq!(overflow.status.placements[0].position_metres[0], 2.0);
    assert_eq!(overflow.tracking.bindings[0].identity, identity);
    assert_eq!(
        overflow.tracking.bindings[0].position_received_at_millis,
        100
    );
    assert!(
        overflow
            .overrides
            .iter()
            .all(|held| held.value.normalized().is_some_and(f32::is_finite))
    );
}

#[test]
fn malformed_senders_never_enter_the_bounded_source_map() {
    let resource = resource(bound());
    for port in 1..=500 {
        resource.observe(SocketAddr::from(([10, 0, 0, 1], port)), b"not PSN", 1_000);
    }
    assert!(resource.inner.lock().sources.is_empty());
    let status = resource.status(1_000);
    assert_eq!(status.ignored_datagrams, 500);
    assert_eq!(status.health, Some(PsnHealth::Silent));
    assert!(status.error.is_none());
}

#[test]
fn source_capacity_rejects_newcomers_without_evicting_a_held_source() {
    let resource = resource(bound());
    source_frame(&resource, sender(), 3, 1.0, 10, 1_000);
    resource.tick(1_000);
    for index in 1..MAX_PSN_SOURCES {
        source_frame(
            &resource,
            SocketAddr::from(([10, 1, 0, 1], index as u16)),
            100 + index as u16,
            0.0,
            10,
            1_000,
        );
    }
    let newcomer = SocketAddr::from(([10, 1, 1, 1], 9));
    source_frame(&resource, newcomer, 3, 9.0, 20, 1_010);
    let full = resource.tick(1_010);
    assert_eq!(resource.inner.lock().sources.len(), MAX_PSN_SOURCES);
    assert_eq!(full.status.rejected_source_datagrams, 1);
    assert_eq!(full.status.placements[0].position_metres[0], 1.0);
    assert_eq!(full.tracking.bindings[0].identity.source, sender());
    source_frame(&resource, sender(), 3, 2.0, 30, 1_020);
    assert_eq!(
        resource.tick(1_020).status.placements[0].position_metres[0],
        2.0
    );
    assert!(!resource.inner.lock().sources.contains_key(&newcomer));
}

#[test]
fn old_socket_packets_cannot_acquire_the_next_generation() {
    let resource = resource(bound());
    let old = resource.generation();
    resource.reset_for_show(None, bound());
    let packet = encode_data_frame(
        10,
        1,
        &[PsnTrackerData {
            id: 3,
            position: Some(PsnVector3 {
                x: 9.0,
                y: 0.0,
                z: 0.0,
            }),
            ..Default::default()
        }],
    )
    .pop()
    .unwrap();
    resource.observe_for_generation(old, sender(), &packet, 1_000);
    assert!(resource.status(1_000).sources.is_empty());
    resource.observe_for_generation(resource.generation(), sender(), &packet, 1_010);
    assert_eq!(resource.status(1_010).sources.len(), 1);
}

#[test]
fn timestamp_reset_holds_until_explicit_disable_enable_not_a_silence_heuristic() {
    let resource = resource(bound());
    source_frame(&resource, sender(), 3, 1.0, 1_000_000, 100);
    let first = resource.tick(100);
    source_frame(&resource, sender(), 3, 9.0, 10, 60_000);
    let rejected = resource.tick(60_000);
    assert_eq!(
        rejected.tracking.bindings[0].identity,
        first.tracking.bindings[0].identity
    );
    assert_eq!(rejected.status.placements[0].position_metres[0], 1.0);
    assert_eq!(rejected.status.sources[0].diagnostics.rejected_datagrams, 1);
    let mut disabled = bound();
    disabled.enabled = false;
    resource.install(disabled);
    assert!(resource.committed_overrides().is_empty());
    resource.install(bound());
    source_frame(&resource, sender(), 3, 9.0, 10, 60_010);
    let restarted = resource.tick(60_010);
    assert_eq!(restarted.status.placements[0].position_metres[0], 9.0);
    assert_ne!(
        restarted.tracking.bindings[0].identity.source_generation,
        first.tracking.bindings[0].identity.source_generation
    );
}

#[test]
fn diagnostics_and_sample_provenance_are_exposed_without_status_mutations() {
    let resource = resource(bound());
    let packet = encode_data_frame(
        10,
        1,
        &[PsnTrackerData {
            id: 3,
            position: Some(PsnVector3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            }),
            ..Default::default()
        }],
    )
    .pop()
    .unwrap();
    resource.observe(sender(), &packet, 100);
    let tick = resource.tick(100);
    let expected = tick.tracking.bindings[0].identity;
    resource.observe(sender(), &packet, 200);
    let status = resource.status(300);
    assert_eq!(status.sources[0].accepted_sample, Some(expected));
    assert_eq!(status.sources[0].diagnostics.duplicate_datagrams, 1);
    assert_eq!(status.trackers[0].accepted_sample, Some(expected));
    assert_eq!(status.trackers[0].age_millis, 200);
    assert_eq!(resource.status(300), status);
    let after = resource.tick(300);
    assert_eq!(
        after.tracking.accepted_sequence,
        tick.tracking.accepted_sequence
    );
    assert_eq!(after.tracking.bindings, tick.tracking.bindings);
}

#[test]
fn calibration_edits_reproject_retained_input_without_a_new_packet_or_freshness_reset() {
    let resource = resource(bound());
    source_frame(&resource, sender(), 3, 1.0, 10, 100);
    let initial = resource.tick(100);
    let mut configuration = bound();
    configuration.calibration.offset_metres = [2.0, 3.0, 4.0];
    resource.install(configuration);
    let committed = resource.committed_tracking_frame(250);
    assert_eq!(committed.bindings[0].position_metres, [3.0, 3.0, 4.0]);
    assert_eq!(
        committed.bindings[0].identity,
        initial.tracking.bindings[0].identity
    );
    assert_eq!(committed.bindings[0].position_received_at_millis, 100);
    assert_eq!(
        committed.accepted_sequence,
        initial.tracking.accepted_sequence
    );
    assert_ne!(
        committed.configuration_generation,
        initial.tracking.configuration_generation
    );
    let updated = resource.tick(300);
    assert_eq!(
        updated.status.placements[0].position_metres,
        [3.0, 3.0, 4.0]
    );
    assert_eq!(
        updated.tracking.bindings[0].identity,
        initial.tracking.bindings[0].identity
    );
    assert_eq!(
        updated.tracking.bindings[0].position_received_at_millis,
        100
    );
    assert_eq!(
        updated.tracking.accepted_sequence,
        initial.tracking.accepted_sequence
    );
    assert_ne!(
        updated.tracking.configuration_generation,
        initial.tracking.configuration_generation
    );
    assert_eq!(
        updated.tracking.source_generation,
        initial.tracking.source_generation
    );
    assert_eq!(updated.status.trackers[0].age_millis, 200);
}

#[test]
fn calibration_commit_reprojects_held_pose_without_consuming_the_zone_edge() {
    let mut configuration = bound();
    configuration.zones = vec![PsnZone {
        id: ZONE,
        name: "Calibration target".into(),
        min_metres: [-1.0; 3],
        max_metres: [1.0; 3],
        tracker_ids: vec![3],
        enter_macro_id: Some(ENTER_MACRO),
        leave_macro_id: None,
        dwell_millis: 0,
    }];
    let resource = resource(configuration.clone());
    frame_at(&resource, 3, [9.0, 0.0, 0.0], 100);
    let initial = resource.tick(100);
    assert!(initial.zone_transitions.is_empty());
    configuration.calibration.offset_metres = [-9.0, 0.0, 0.0];
    resource.install(configuration);

    for now in [150, 160, 170] {
        let committed = resource.committed_tracking_frame(now);
        assert_eq!(committed.bindings[0].position_metres, [0.0; 3]);
        assert_eq!(
            committed.bindings[0].identity,
            initial.tracking.bindings[0].identity
        );
        assert_eq!(committed.bindings[0].position_received_at_millis, 100);
        assert!(resource.status(now).occupied_zones.is_empty());
        assert_eq!(resource.status(now).placements[0].position_metres, [0.0; 3]);
    }
    let entered = resource.tick(200);
    assert_eq!(
        entered.zone_transitions,
        vec![(ZONE, super::super::zones::ZoneTransition::Entered)]
    );
    assert!(resource.tick(210).zone_transitions.is_empty());
}

#[test]
fn calibration_reprojects_exact_held_sample_and_overflow_holds_until_finite_reprojection() {
    let resource = resource(bound());
    source_frame(&resource, sender(), 3, 2.0, 10, 100);
    let initial = resource.tick(100);
    // A newer source position is accepted by ingress but has not become the committed hold.
    source_frame(&resource, sender(), 3, 0.0, 20, 200);
    let mut configuration = bound();
    configuration.calibration.scale = f32::MAX;
    resource.install(configuration);
    let overflow = resource.committed_tracking_frame(210);
    assert_eq!(overflow.bindings[0].position_metres, [2.0, 0.0, 0.0]);
    assert_eq!(
        overflow.bindings[0].identity,
        initial.tracking.bindings[0].identity
    );
    assert_eq!(overflow.bindings[0].position_received_at_millis, 100);
    assert_eq!(resource.status(210).invalid_calibrated_positions, 1);
    assert!(resource.status(210).error.is_none());

    let mut finite = bound();
    finite.calibration.scale = 2.0;
    resource.install(finite);
    let reprojected = resource.committed_tracking_frame(220);
    assert_eq!(reprojected.bindings[0].position_metres, [4.0, 0.0, 0.0]);
    assert_eq!(
        reprojected.bindings[0].identity,
        initial.tracking.bindings[0].identity
    );
    assert_eq!(reprojected.bindings[0].position_received_at_millis, 100);
    assert_eq!(resource.status(220).invalid_calibrated_positions, 0);
    let next = resource.tick(230);
    assert_eq!(next.tracking.bindings[0].position_metres, [0.0; 3]);
    assert_ne!(
        next.tracking.bindings[0].identity,
        initial.tracking.bindings[0].identity
    );
    assert_eq!(next.tracking.bindings[0].position_received_at_millis, 200);
}

#[test]
fn calibrated_overflow_cannot_replace_a_newer_hold_with_an_older_competing_source() {
    let older: SocketAddr = "10.0.0.1:56565".parse().unwrap();
    let mut configuration = bound();
    configuration.calibration.scale = 2.0;
    configuration.zones = vec![PsnZone {
        id: ZONE,
        name: "Keep the actual held source".into(),
        min_metres: [-5.0; 3],
        max_metres: [5.0; 3],
        tracker_ids: vec![3],
        enter_macro_id: None,
        leave_macro_id: None,
        dwell_millis: 0,
    }];
    let resource = resource(configuration);
    source_frame(&resource, older, 3, 10.0, 10, 100);
    assert!(resource.tick(100).status.occupied_zones.is_empty());
    source_frame(&resource, sender(), 3, 1.0, 10, 200);
    let valid = resource.tick(200);
    assert_eq!(valid.tracking.bindings[0].identity.source, sender());
    assert_eq!(valid.status.occupied_zones, vec![ZONE]);

    source_frame(&resource, sender(), 3, f32::MAX, 20, 300);
    let held = resource.tick(300);
    assert_eq!(
        held.tracking.bindings, valid.tracking.bindings,
        "older finite competing sample must not replace the last valid world hold"
    );
    assert_eq!(held.status.placements[0].position_metres, [2.0, 0.0, 0.0]);
    assert_eq!(held.status.invalid_calibrated_positions, 1);
    assert_eq!(held.status.occupied_zones, vec![ZONE]);
    assert!(held.zone_transitions.is_empty());
    assert!(held.status.error.is_none());

    source_frame(&resource, older, 3, 10.0, 20, 400);
    let takeover = resource.tick(400);
    assert_eq!(takeover.tracking.bindings[0].identity.source, older);
    assert_eq!(
        takeover.tracking.bindings[0].position_received_at_millis,
        400
    );
    assert_eq!(
        takeover.status.placements[0].position_metres,
        [20.0, 0.0, 0.0]
    );
    assert_eq!(
        takeover.zone_transitions,
        vec![(ZONE, ZoneTransition::Left)]
    );
}

#[test]
fn same_sender_new_samples_in_one_receiver_millisecond_refresh_the_held_position() {
    let resource = resource(bound());
    source_frame(&resource, sender(), 3, 1.0, 10, 100);
    let first = resource.tick(100);
    source_frame(&resource, sender(), 3, 2.0, 11, 100);
    let next = resource.tick(100);
    assert_eq!(next.status.placements[0].position_metres, [2.0, 0.0, 0.0]);
    assert_eq!(next.tracking.bindings[0].position_received_at_millis, 100);
    assert_ne!(
        next.tracking.bindings[0].identity,
        first.tracking.bindings[0].identity
    );
    assert_eq!(
        next.tracking.bindings[0].identity.sample.id.sequence,
        first.tracking.bindings[0].identity.sample.id.sequence + 1
    );

    source_frame(&resource, sender(), 3, 2.0, 11, 200);
    let duplicate = resource.tick(200);
    assert_eq!(duplicate.tracking.bindings, next.tracking.bindings);
    assert_eq!(
        duplicate.tracking.accepted_sequence,
        next.tracking.accepted_sequence
    );
}

#[test]
fn malformed_stored_duplicate_ids_withhold_all_conflicting_rows_and_keep_independent_bindings() {
    let independent_id = Uuid::from_u128(30);
    let independent_point = Uuid::from_u128(31);
    for reverse in [false, true] {
        for duplicate_enabled in [false, true] {
            let mut configuration = bound();
            configuration.bindings.push(PsnBinding {
                id: BINDING,
                tracker_id: 4,
                point_fixture_id: Uuid::from_u128(32),
                enabled: duplicate_enabled,
            });
            configuration.bindings.push(PsnBinding {
                id: independent_id,
                tracker_id: 5,
                point_fixture_id: independent_point,
                enabled: true,
            });
            if reverse {
                configuration.bindings.reverse();
            }
            let resource = resource(configuration.clone());
            resource.install_point_locations(HashMap::from([
                (POINT, [0.0; 3]),
                (Uuid::from_u128(32), [0.0; 3]),
                (independent_point, [0.0; 3]),
            ]));
            frame_at(&resource, 3, [1.0, 0.0, 0.0], 100);
            frame_at(&resource, 4, [2.0, 0.0, 0.0], 110);
            frame_at(&resource, 5, [3.0, 0.0, 0.0], 120);
            let tick = resource.tick(120);
            assert_eq!(
                resource.configuration(),
                configuration,
                "never rewrite a loaded show"
            );
            assert_eq!(tick.status.conflicting_binding_rows, 2);
            assert!(tick.status.error.is_none());
            assert_eq!(
                tick.status.trackers.len(),
                3,
                "withholding bindings does not reject sources"
            );
            assert_eq!(tick.tracking.bindings.len(), 1);
            assert_eq!(tick.tracking.bindings[0].binding_id, independent_id);
            assert_eq!(
                tick.tracking.bindings[0].point_fixture_id,
                independent_point
            );
            assert_eq!(tick.tracking.bindings[0].position_metres, [3.0, 0.0, 0.0]);
            assert_eq!(tick.status.placements.len(), 1);
            assert_eq!(tick.status.placements[0].binding_id, independent_id);
            assert_eq!(tick.overrides.len(), 3);
            assert!(
                tick.overrides
                    .iter()
                    .all(|value| value.fixture_id.0 == independent_point)
            );
            let observed = resource.status(130);
            assert_eq!(observed.conflicting_binding_rows, 2);
            assert_eq!(
                resource.committed_tracking_frame(130).bindings,
                tick.tracking.bindings
            );
        }
    }
}

#[test]
fn installing_a_duplicate_binding_retires_its_hold_and_repair_restores_only_the_unique_row() {
    let resource = resource(bound());
    frame_at(&resource, 3, [1.0, 0.0, 0.0], 100);
    let valid = resource.tick(100);
    assert_eq!(valid.tracking.bindings.len(), 1);

    let mut malformed = bound();
    malformed.bindings.push(PsnBinding {
        id: BINDING,
        tracker_id: 4,
        point_fixture_id: Uuid::from_u128(32),
        enabled: false,
    });
    resource.install(malformed.clone());
    assert!(
        resource.committed_tracking_frame(110).bindings.is_empty(),
        "retire before the next tick"
    );
    assert_eq!(resource.status(110).conflicting_binding_rows, 2);
    assert!(resource.tick(120).tracking.bindings.is_empty());
    assert_eq!(resource.configuration(), malformed);

    resource.install(bound());
    assert_eq!(resource.status(130).conflicting_binding_rows, 0);
    let repaired = resource.tick(130);
    assert_eq!(
        repaired.tracking.bindings, valid.tracking.bindings,
        "same source sample keeps its original age and identity"
    );
    assert!(repaired.status.error.is_none());
}
