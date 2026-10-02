use super::*;
use serde_json::json;

#[test]
fn typed_lane_is_exclusive_and_round_trips_full_width_native_sources() {
    let lane = json!({
        "id": "00000000-0000-0000-0000-000000000001", "speed_multiplier": {"numerator":1,"denominator":1}, "width":1,
        "programming": {"address": {"representation":{"kind":"angles"}, "component":{"kind":"pan"}},
            "configuration": {"mode":"max_min","configuration":{
                "minimum":{"kind":"value","value":{"kind":"scalar","value":-720}},
                "maximum":{"kind":"value","value":{"kind":"scalar","value":720}},
                "function":"linear_up","size":1,"pwm":{"attack":0,"on":0.5,"decay":0,"off":0.5,"attack_interpolation":"linear","decay_interpolation":"linear"}
            }}}
    });
    let decoded: DynamicLaneProjection = serde_json::from_value(lane.clone()).unwrap();
    assert_eq!(
        serde_json::from_value::<DynamicLaneProjection>(serde_json::to_value(&decoded).unwrap())
            .unwrap(),
        decoded
    );
    let mut extended = lane.clone();
    extended["future_field"] = json!({"note":true});
    extended["programming"]["future_field"] = json!(true);
    assert_eq!(
        serde_json::from_value::<DynamicLaneProjection>(extended).unwrap(),
        decoded
    );
    for extra in [json!("pan"), json!(null)] {
        let mut mixed = lane.clone();
        mixed["attribute"] = extra;
        assert!(serde_json::from_value::<DynamicLaneProjection>(mixed).is_err());
    }
    let value = DynamicValueProjection::Native(u32::MAX - 1);
    let encoded = serde_json::to_value(&value).unwrap();
    assert_eq!(encoded["value"], u32::MAX - 1);
    assert_eq!(
        serde_json::from_value::<DynamicValueProjection>(encoded).unwrap(),
        value
    );
}

#[test]
fn random_range_does_not_accept_both_representations() {
    let mut group = json!({
        "id":"00000000-0000-0000-0000-000000000001","seed":1,
        "decision_interval_millis":250,"start_probability":0.5,"mean_duration_millis":500,"duration_spread_millis":100,"attack_ratio":0.1,"decay_ratio":0.1,
        "programming_range":{"low":{"kind":"current"},"high":{"kind":"value","value":{"kind":"scalar","value":720}}}
    });
    let decoded: DynamicRandomGroupProjection = serde_json::from_value(group.clone()).unwrap();
    assert_eq!(
        serde_json::from_value::<DynamicRandomGroupProjection>(
            serde_json::to_value(&decoded).unwrap()
        )
        .unwrap(),
        decoded
    );
    group["low"] = json!({"type":"current"});
    group["high"] = json!({"type":"value","value":1});
    assert!(serde_json::from_value::<DynamicRandomGroupProjection>(group).is_err());
}
