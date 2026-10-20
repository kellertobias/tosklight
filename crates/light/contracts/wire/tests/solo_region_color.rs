use light_wire::v2::playback_solo::PlaybackSoloRegion;
use serde_json::json;

#[test]
fn optional_solo_color_keeps_legacy_wire_valid_and_survives_round_trip() {
    let legacy = json!({"id":"11111111-1111-4111-8111-111111111111","name":"Stage", "members":[{"page":1,"position":1},{"page":1,"position":2}]});
    let region: PlaybackSoloRegion = serde_json::from_value(legacy.clone()).unwrap();
    assert_eq!(region.color, None);
    assert_eq!(serde_json::to_value(region).unwrap(), legacy);
    let mut colored = legacy.clone();
    colored["color"] = json!("#aB12fF");
    let region: PlaybackSoloRegion = serde_json::from_value(colored.clone()).unwrap();
    assert_eq!(serde_json::to_value(region).unwrap(), colored);
    let mut reset = legacy;
    reset["color"] = serde_json::Value::Null;
    assert!(
        serde_json::from_value::<PlaybackSoloRegion>(reset)
            .unwrap()
            .color
            .is_none()
    );
}
