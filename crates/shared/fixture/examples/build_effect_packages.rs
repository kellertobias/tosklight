use base64::{Engine as _, engine::general_purpose::STANDARD};
use light_fixture::{FixtureProfile, ProfileEffect, read_fixture_package, write_fixture_package};
use serde_json::Value;
use std::{fs, path::PathBuf};

const COLD_SPARK: &str = r#"export function effect(input) {
  const on = input.dmx[0] > 0;
  return { version: 1, emitters: [{ family: 'spark', origin: [0, 0.18, 0], direction: [0,1,0],
    width: 0.24, height: 0.5 + input.dmx[1] / 255 * 4.5, intensity: input.intensity,
    density: input.dmx[0] / 255, lifetime: 0.15 + input.dmx[2] / 255 * 3.85,
    color: [1,0.62,0.12], state: on ? 'hold' : 'off' }] };
}
"#;

const FLAME_FIVE: &str = r#"export function effect(input) {
  const level = input.dmx[0] / 255;
  const height = 0.4 + input.dmx[1] / 255 * 3.6;
  const angles = [-35,-18,0,18,35];
  return { version: 1, emitters: angles.map((degrees, nozzle) => {
    const radians = degrees * Math.PI / 180;
    return { family: 'flame', origin: [(nozzle - 2) * 0.16,0.16,0], direction: [Math.sin(radians),Math.cos(radians),0],
      width: 0.18, height, intensity: level, density: level, lifetime: 0.28,
      color: [1,0.12 + input.dmx[2] / 255 * 0.28,0.015], state: level > 0 ? 'hold' : 'off' };
  }) };
}
"#;

fn source_profile() -> FixtureProfile {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    read_fixture_package(
        &fs::read(root.join("assets/fixture-library/tosklight--visualizer-laser.toskfixture"))
            .unwrap(),
    )
    .unwrap()
}

fn build(
    filename: &str,
    id: &str,
    name: &str,
    short_name: &str,
    channels: usize,
    mode_name: &str,
    script: &str,
    notes: &str,
) {
    let mut value = serde_json::to_value(source_profile()).unwrap();
    let object = value.as_object_mut().unwrap();
    object.insert("schema_version".into(), Value::from(3));
    object.insert("id".into(), Value::from(id));
    object.insert("manufacturer".into(), Value::from("Generic"));
    object.insert("name".into(), Value::from(name));
    object.insert("short_name".into(), Value::from(short_name));
    object.insert("fixture_type".into(), Value::from("effect"));
    object.insert("notes".into(), Value::from(notes));
    object.insert("laser".into(), Value::Null);
    let modes = object.get_mut("modes").unwrap().as_array_mut().unwrap();
    modes.truncate(1);
    let mode = modes[0].as_object_mut().unwrap();
    mode.insert("name".into(), Value::from(mode_name));
    let mode_channels = mode.get_mut("channels").unwrap().as_array_mut().unwrap();
    mode_channels.truncate(channels);
    let labels = if id == "f6f72b7f-736c-4dd4-8c88-9c937ad3b91b" {
        ["Intensity", "Fountain height", "Spark lifetime"]
    } else {
        ["Intensity", "Flame height", "Fluid colour"]
    };
    for (channel, label) in mode_channels.iter_mut().zip(labels) {
        if let Some(function) = channel
            .get_mut("functions")
            .and_then(Value::as_array_mut)
            .and_then(|functions| functions.first_mut())
        {
            function
                .as_object_mut()
                .unwrap()
                .insert("name".into(), Value::from(label));
        }
    }
    mode.get_mut("splits").unwrap().as_array_mut().unwrap()[0]
        .as_object_mut()
        .unwrap()
        .insert("footprint".into(), Value::from(channels));
    let mut profile: FixtureProfile = serde_json::from_value(value).unwrap();
    profile.effect = Some(ProfileEffect {
        effect_script_asset: Some(format!(
            "data:text/javascript;base64,{}",
            STANDARD.encode(script)
        )),
        result_version: 1,
    });
    profile.revision = 1;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    fs::write(
        root.join("assets/fixture-library").join(filename),
        write_fixture_package(&profile).unwrap(),
    )
    .unwrap();
}

fn main() {
    build(
        "generic--cold-spark.toskfixture",
        "f6f72b7f-736c-4dd4-8c88-9c937ad3b91b",
        "Cold Spark Fountain",
        "Cold Spark",
        3,
        "Intensity, Height, Lifetime",
        COLD_SPARK,
        "Generic transferable Effect fixture demonstrating independent fountain height and visible spark lifetime. The package-owned effect.js maps the three DMX slots; the renderer contains no device program.",
    );
    build(
        "generic--five-nozzle-flame.toskfixture",
        "68b75b37-b37b-42b0-8c1a-b8635a6ea4cc",
        "Five-nozzle Flame Unit",
        "5 Flame",
        3,
        "Intensity, Height, Colour",
        FLAME_FIVE,
        "Generic five-nozzle Effect fixture. Trigger/off, nozzle angles, reach and supported fluid colour are entirely mapped by the transferable effect.js.",
    );
}
