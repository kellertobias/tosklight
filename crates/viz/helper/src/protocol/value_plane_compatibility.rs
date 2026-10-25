//! TL877: Option omission corrupts positional serialization, including following emitters/scans.
use super::*;
use viz_scene::*;
fn mixed() -> SceneValues {
    let emitter: EmitterInstance = serde_json::from_value(serde_json::json!({
        "fixture_index":0,"head_index":0,"label":"head","local_origin":[0,0,0],
        "tilt_pivot":[0,0,0],"local_orientation_degrees":[0,0,0],"pan":null,"tilt":null,
        "beam_angle_degrees":10,"field_angle_degrees":20,"optics":EmitterOptics::default(),"kind":"Beam",
        "cells":EmitterLayoutCells::single(),"laser":null,"effect":null,
        "live_shaper_angle_roles":[false,false,false,false],"shaper_roles":[false,false,false,false],
        "live_shaper_rotation_role":false
    })).unwrap();
    let mut second = emitter.clone();
    second.fixture_index = 1;
    let scene = Scene {
        fixtures: vec![
            FixtureInstance::default(),
            FixtureInstance {
                lamp_response: LampResponseProfile::TungstenNominal,
                ..Default::default()
            },
        ],
        emitters: vec![emitter, second],
        ..Default::default()
    };
    let mut values = SceneValues::default();
    values.resize(2);
    values.emitters[0].intensity = 0.4;
    values.emitters[0].colour = [0.2, 0.4, 0.6];
    values.emitters[1].cells = vec![CellValue {
        intensity: 0.3,
        ..Default::default()
    }];
    values.advance_lamp_response(&scene, 0.0);
    values.emitters[1].intensity = 0.75;
    values.advance_lamp_response(&scene, 0.025);
    values.laser_scans[0].points_per_second = 30_000.0;
    values.laser_scans[0].slots = vec![1, 2, 3];
    values.laser_scans[0].points = vec![ScanPoint {
        x: 0.25,
        y: -0.4,
        colour: [1.0, 0.5, 0.25],
        dwell: 1.0,
    }];
    values.frame = 17;
    assert!(values.emitters[0].lamp_response.is_none());
    assert!(values.emitters[1].lamp_response.is_some());
    values
}
#[test]
fn mixed_none_some_emitters_cells_and_lasers_survive_positional_channel() {
    let values = mixed();
    let carried = ToHelper::Values {
        payload: encode(&values).unwrap(),
    };
    let decoded: ToHelper = decode(&encode(&carried).unwrap()).unwrap();
    let ToHelper::Values { payload } = decoded else {
        panic!("wrong value-plane variant")
    };
    let arrived: SceneValues = decode(&payload).unwrap();
    assert_eq!(
        serde_json::to_value(&arrived).unwrap(),
        serde_json::to_value(&values).unwrap()
    );
    assert!(arrived.emitters[0].lamp_response.is_none());
    assert!(arrived.emitters[1].lamp_response.as_ref().unwrap().output() > 0.0);
}
#[test]
fn legacy_json_omitted_none_and_populated_thermal_state_remain_readable() {
    let values = mixed();
    let mut legacy = serde_json::to_value(&values).unwrap();
    legacy["emitters"][0]
        .as_object_mut()
        .unwrap()
        .remove("lamp_response");
    let arrived: SceneValues = serde_json::from_value(legacy).unwrap();
    assert!(arrived.emitters[0].lamp_response.is_none());
    assert_eq!(
        serde_json::to_value(&arrived).unwrap(),
        serde_json::to_value(&values).unwrap()
    );
    assert!(serde_json::to_value(&arrived).unwrap()["emitters"][0]["lamp_response"].is_null());
}
