use super::*;

fn request(request_id: &str) -> wire::PlaybackActionRequest {
    wire::PlaybackActionRequest {
        request_id: request_id.into(),
        address: wire::PlaybackAddress::Playback { playback_number: 1 },
        action: wire::PlaybackAction::Go { pressed: true },
        surface: wire::PlaybackSurface::Virtual,
    }
}

#[test]
fn request_id_uses_the_shared_printable_byte_contract() {
    assert!(application_command(request("safe-request-1")).is_ok());
    for invalid in ["", "   ", "line\nbreak", &"x".repeat(129)] {
        assert_eq!(
            application_command(request(invalid)).unwrap_err(),
            "request_id must contain 1-128 printable bytes"
        );
    }
}

#[test]
fn playback_numbers_and_levels_are_validated_before_the_application_boundary() {
    let mut invalid_number = request("invalid-number");
    invalid_number.address = wire::PlaybackAddress::Playback { playback_number: 0 };
    assert!(application_command(invalid_number).is_err());

    let mut invalid_level = request("invalid-level");
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
    let (_, command) = application_command(expanded_button).unwrap();
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
    let (_, command) = application_command(second_fader).unwrap();
    assert_eq!(
        command.action,
        application::PlaybackAction::ConfiguredFader {
            number: 2,
            level: application::PlaybackLevel::new(0.625),
        }
    );

    for (request_id, action) in [
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
        let mut invalid = request(request_id);
        invalid.action = action;
        assert!(application_command(invalid).is_err(), "{request_id}");
    }
}
