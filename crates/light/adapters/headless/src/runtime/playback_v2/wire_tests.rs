use super::*;

#[test]
fn direct_cue_list_selector_round_trips_without_changing_aggregate_identity() {
    let id = uuid::Uuid::from_u128(7);
    let exact = wire::PlaybackRuntimeIdentity::DirectCueList { cue_list_id: id };
    let aggregate = wire::PlaybackRuntimeIdentity::CueList { cue_list_id: id };
    let mapped = application_identities(vec![exact.clone(), aggregate.clone()]).unwrap();
    assert_ne!(mapped[0], mapped[1]);
    assert_eq!(wire_identity(mapped[0].clone()), exact);
    assert_eq!(wire_identity(mapped[1].clone()), aggregate);
    assert!(
        application_identities(vec![wire::PlaybackRuntimeIdentity::DirectCueList {
            cue_list_id: uuid::Uuid::nil()
        }])
        .is_err()
    );
}

fn request(surface_id: &str) -> wire::PlaybackActionRequest {
    wire::PlaybackActionRequest {
        address: wire::PlaybackAddress::Playback {
            page: 1,
            position: 1,
        },
        action: wire::PlaybackAction::Go { pressed: true },
        control_source: wire::PlaybackControlSource {
            surface_id: surface_id.into(),
            control_id: 7,
        },
    }
}

#[test]
fn stable_control_source_is_validated_and_preserved_at_the_application_boundary() {
    let command = application_command(request("pane:stable")).unwrap();
    assert_eq!(command.control_source.surface_id, "pane:stable");
    assert_eq!(command.control_source.control_id, 7);
    assert!(application_command(request(&"x".repeat(256))).is_ok());
    for invalid in [
        "",
        "   ",
        " leading",
        "trailing ",
        "nul\0byte",
        &"x".repeat(257),
    ] {
        assert!(
            application_command(request(invalid)).is_err(),
            "{invalid:?}"
        );
    }
}

#[test]
fn exact_addresses_current_positions_and_levels_are_validated_before_the_application_boundary() {
    for (page, position) in [(1, 1), (99, 1000)] {
        let mut valid = request("main");
        valid.address = wire::PlaybackAddress::Playback { page, position };
        assert_eq!(
            application_command(valid).unwrap().address,
            application::PlaybackAddress::Playback(
                light_playback::PlaybackAddress::new(page, position).unwrap()
            )
        );
    }
    for (page, position) in [(0, 1), (100, 1), (1, 0), (1, 1001)] {
        let mut invalid = request("main");
        invalid.address = wire::PlaybackAddress::Playback { page, position };
        assert!(application_command(invalid).is_err());
    }
    for position in [0, 1001] {
        let mut invalid = request("main");
        invalid.address = wire::PlaybackAddress::CurrentPage { position };
        assert!(application_command(invalid).is_err());
    }
    let mut current = request("main");
    current.address = wire::PlaybackAddress::CurrentPage { position: 1000 };
    assert_eq!(
        application_command(current).unwrap().address,
        application::PlaybackAddress::CurrentPage { position: 1000 }
    );
    let mut invalid_level = request("main");
    invalid_level.action = wire::PlaybackAction::Master { value: f32::NAN };
    assert!(application_command(invalid_level).is_err());
}

#[test]
fn expanded_configured_controls_are_validated_and_preserve_identity() {
    let mut expanded_button = request("expanded-button");
    expanded_button.action = wire::PlaybackAction::ConfiguredButton {
        number: 6,
        pressed: true,
    };
    let command = application_command(expanded_button).unwrap();
    assert_eq!(
        command.action,
        application::PlaybackAction::ConfiguredButton {
            number: 6,
            pressed: true,
        }
    );

    let mut second_fader = request("second-fader");
    second_fader.action = wire::PlaybackAction::ConfiguredFader {
        number: 2,
        level: 0.625,
    };
    let command = application_command(second_fader).unwrap();
    assert_eq!(
        command.action,
        application::PlaybackAction::ConfiguredFader {
            number: 2,
            level: application::PlaybackLevel::new(0.625),
        }
    );

    for (label, action) in [
        (
            "button-zero",
            wire::PlaybackAction::ConfiguredButton {
                number: 0,
                pressed: true,
            },
        ),
        (
            "button-seven",
            wire::PlaybackAction::ConfiguredButton {
                number: 7,
                pressed: true,
            },
        ),
        (
            "fader-zero",
            wire::PlaybackAction::ConfiguredFader {
                number: 0,
                level: 0.5,
            },
        ),
        (
            "fader-three",
            wire::PlaybackAction::ConfiguredFader {
                number: 3,
                level: 0.5,
            },
        ),
        (
            "fader-nan",
            wire::PlaybackAction::ConfiguredFader {
                number: 2,
                level: f32::NAN,
            },
        ),
    ] {
        let mut invalid = request(label);
        invalid.action = action;
        assert!(application_command(invalid).is_err(), "{label}");
    }
}

#[test]
fn software_absolute_actions_are_distinct_and_validate_levels_and_configured_controls() {
    for (body, expected) in [
        (
            serde_json::json!({"type":"master_absolute","value":0.999}),
            application::PlaybackAction::MasterAbsolute(application::PlaybackLevel::new(0.999)),
        ),
        (
            serde_json::json!({"type":"configured_fader_absolute","number":2,"level":0.25}),
            application::PlaybackAction::ConfiguredFaderAbsolute {
                number: 2,
                level: application::PlaybackLevel::new(0.25),
            },
        ),
    ] {
        let mut request = request("software:encoder");
        request.action = serde_json::from_value(body).unwrap();
        assert_eq!(application_command(request).unwrap().action, expected);
    }
    for body in [
        serde_json::json!({"type":"master_absolute","value":-0.1}),
        serde_json::json!({"type":"master_absolute","value":1.1}),
        serde_json::json!({"type":"configured_fader_absolute","number":3,"level":0.5}),
        serde_json::json!({"type":"configured_fader_absolute","number":1,"level":1.1}),
    ] {
        let mut request = request("software:encoder");
        request.action = serde_json::from_value(body).unwrap();
        assert!(application_command(request).is_err());
    }
}
