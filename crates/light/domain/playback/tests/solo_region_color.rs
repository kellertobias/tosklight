use light_playback::PlaybackSoloRegion;
use serde_json::json;

fn stored() -> serde_json::Value {
    json!({"id":"11111111-1111-4111-8111-111111111111","name":"Stage", "members":[{"page":1,"position":1},{"page":1,"position":2}]})
}

#[test]
fn legacy_solo_regions_keep_default_presentation_without_a_schema_migration() {
    let legacy = stored();
    let region: PlaybackSoloRegion = serde_json::from_value(legacy.clone()).unwrap();
    assert!(region.color.is_none());
    region.validate().unwrap();
    assert_eq!(serde_json::to_value(region).unwrap(), legacy);
    let mut explicit_null = legacy;
    explicit_null["color"] = serde_json::Value::Null;
    let region: PlaybackSoloRegion = serde_json::from_value(explicit_null).unwrap();
    assert!(region.color.is_none());
}

#[test]
fn renaming_a_colored_region_preserves_its_color_and_members_after_reopening() {
    let mut stored = stored();
    stored["color"] = json!("#aB12fF");
    let mut region: PlaybackSoloRegion = serde_json::from_value(stored).unwrap();
    region.validate().unwrap();
    let members = region.members.clone();
    region.name = "Renamed".into();
    let reopened: PlaybackSoloRegion =
        serde_json::from_value(serde_json::to_value(&region).unwrap()).unwrap();
    assert_eq!(reopened, region);
    assert_eq!(reopened.color.as_deref(), Some("#aB12fF"));
    assert_eq!(reopened.members, members);
}

#[test]
fn solo_colors_use_the_existing_exact_six_digit_playback_color_contract() {
    for color in ["#ffbe3c", "#AABBCC", "#000000"] {
        let mut value = stored();
        value["color"] = json!(color);
        serde_json::from_value::<PlaybackSoloRegion>(value)
            .unwrap()
            .validate()
            .unwrap();
    }
    for color in [
        "red",
        "#abc",
        "#aabbccdd",
        "aabbcc",
        "#GG1122",
        " #112233",
        "#112233 ",
        "#é1234",
    ] {
        let mut value = stored();
        value["color"] = json!(color);
        let error = serde_json::from_value::<PlaybackSoloRegion>(value)
            .unwrap()
            .validate()
            .unwrap_err();
        assert!(error.contains("color"), "{color}: {error}");
    }
}
