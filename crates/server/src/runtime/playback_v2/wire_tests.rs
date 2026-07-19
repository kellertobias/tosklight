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
