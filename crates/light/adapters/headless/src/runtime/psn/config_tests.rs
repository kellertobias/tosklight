use super::*;

fn zone(min: [f32; 3], max: [f32; 3]) -> PsnZone {
    PsnZone {
        id: Uuid::from_u128(1),
        name: "Downstage".into(),
        min_metres: min,
        max_metres: max,
        tracker_ids: Vec::new(),
        enter_macro_id: None,
        leave_macro_id: None,
        dwell_millis: 250,
    }
}

#[test]
fn a_show_written_before_psn_existed_reads_as_off() {
    let configuration: PsnConfiguration = serde_json::from_value(serde_json::json!({})).unwrap();
    assert!(!configuration.enabled);
    assert!(configuration.bindings.is_empty());
    assert_eq!(configuration.group, light_psn_wire::PSN_MULTICAST_ADDRESS);
    assert_eq!(configuration.port, light_psn_wire::PSN_PORT);
    configuration.validate().unwrap();
}

#[test]
fn a_binding_survives_a_round_trip_through_the_show() {
    let configuration = PsnConfiguration {
        enabled: true,
        bindings: vec![PsnBinding {
            id: Uuid::from_u128(7),
            tracker_id: 3,
            point_fixture_id: Uuid::from_u128(9),
            enabled: true,
        }],
        ..PsnConfiguration::default()
    };
    let json = serde_json::to_value(&configuration).unwrap();
    assert_eq!(json["bindings"][0]["trackerId"], 3);
    assert_eq!(
        serde_json::from_value::<PsnConfiguration>(json).unwrap(),
        configuration
    );
}

#[test]
fn an_address_that_is_not_a_multicast_group_is_refused_in_operator_words() {
    let configuration = PsnConfiguration {
        group: "10.0.0.4".parse().unwrap(),
        ..PsnConfiguration::default()
    };
    let message = configuration.validate().unwrap_err();
    assert!(message.contains("10.0.0.4"), "{message}");
    assert!(message.contains("multicast"), "{message}");
}

#[test]
fn a_zone_with_its_corners_the_wrong_way_round_is_refused() {
    let configuration = PsnConfiguration {
        zones: vec![zone([2.0, 0.0, 0.0], [-2.0, 3.0, 4.0])],
        ..PsnConfiguration::default()
    };
    assert!(
        configuration
            .validate()
            .unwrap_err()
            .contains("low corner above its high corner")
    );
}

#[test]
fn disabling_psn_deactivates_every_binding_without_forgetting_them() {
    let binding = PsnBinding {
        id: Uuid::from_u128(7),
        tracker_id: 3,
        point_fixture_id: Uuid::from_u128(9),
        enabled: true,
    };
    let configuration = PsnConfiguration {
        enabled: false,
        bindings: vec![binding],
        ..PsnConfiguration::default()
    };
    assert_eq!(configuration.active_bindings().count(), 0);
    assert_eq!(configuration.bindings.len(), 1);
}

#[test]
fn an_uncalibrated_rig_is_taken_at_its_word() {
    // PSN and the desk already agree on metres and on which way is up, so the default is the
    // identity: a tracking system whose origin is the show's origin needs no calibration at all.
    let calibration = PsnCalibration::default();
    assert_eq!(calibration.place_in_show([1.5, 2.0, -3.0]), [1.5, 2.0, -3.0]);
}

#[test]
fn a_tracking_system_set_up_facing_the_other_way_is_turned_about_the_up_axis() {
    let calibration = PsnCalibration {
        rotation_degrees: 90.0,
        offset_metres: [0.0, 0.0, 1.0],
        scale: 1.0,
    };
    let placed = calibration.place_in_show([2.0, 1.0, 0.0]);
    assert!((placed[0] - 0.0).abs() < 1e-4, "{placed:?}");
    assert!((placed[1] - 1.0).abs() < 1e-4, "{placed:?}");
    // Turned a quarter turn about up, then moved by the offset.
    assert!((placed[2] - (-1.0)).abs() < 1e-4, "{placed:?}");
}

#[test]
fn a_zone_watches_every_tracker_until_it_is_told_which() {
    let mut watched = zone([0.0; 3], [1.0; 3]);
    assert!(watched.watches(4));
    watched.tracker_ids = vec![2];
    assert!(watched.watches(2));
    assert!(!watched.watches(4));
}

#[test]
fn a_position_on_the_face_of_a_zone_is_inside_it() {
    // The performer standing exactly on the line is in the zone. Anything else needs the operator
    // to reason about floating point to place a box.
    let box_zone = zone([0.0, 0.0, 0.0], [2.0, 3.0, 4.0]);
    assert!(box_zone.contains([0.0, 0.0, 0.0]));
    assert!(box_zone.contains([2.0, 3.0, 4.0]));
    assert!(!box_zone.contains([2.001, 1.0, 1.0]));
}
