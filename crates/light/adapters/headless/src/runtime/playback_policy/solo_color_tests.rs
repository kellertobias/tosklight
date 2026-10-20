use super::*;

fn region(color: Option<&str>) -> PlaybackSoloRegion {
    PlaybackSoloRegion {
        id: uuid::Uuid::new_v4(),
        name: "Stage".into(),
        color: color.map(str::to_owned),
        members: vec![
            light_wire::v2::playback::PlaybackAddressValue {
                page: 1,
                position: 1,
            },
            light_wire::v2::playback::PlaybackAddressValue {
                page: 1,
                position: 2,
            },
        ],
    }
}

#[test]
fn solo_color_validation_preserves_persisted_color_across_wire_and_runtime_models() {
    let saved = SoloStore {
        revision: 7,
        regions: vec![region(Some("#12aBcF")), region(None)],
    };
    let mut reopened: SoloStore =
        serde_json::from_value(serde_json::to_value(&saved).unwrap()).unwrap();
    reopened.regions[0].name = "Renamed".into();
    let validated = validate_playback_solo_regions(reopened.regions).unwrap();
    assert_eq!(validated[0].color.as_deref(), Some("#12aBcF"));
    assert_eq!(validated[1].color, None);
    let runtime: Vec<light_playback::PlaybackSoloRegion> =
        serde_json::from_value(serde_json::to_value(&validated).unwrap()).unwrap();
    assert_eq!(runtime[0].color, validated[0].color);
    let restored: Vec<PlaybackSoloRegion> =
        serde_json::from_value(serde_json::to_value(runtime).unwrap()).unwrap();
    assert_eq!(restored, validated);
}

#[test]
fn solo_color_validation_rejects_non_hex_and_non_six_digit_colors() {
    for color in ["red", "#abc", "#12345678", "#12345g", " #123456"] {
        assert!(
            validate_playback_solo_regions(vec![region(Some(color))]).is_err(),
            "{color}"
        );
    }
}
