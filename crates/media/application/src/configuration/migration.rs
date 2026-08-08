//! Configuration migrations.
//!
//! Version 0 is the C++ application's `media/.info` document, which carries no version field at
//! all. Every later version is migrated forward one step at a time, so an installation that has
//! skipped releases still arrives at the current document.

use serde_json::{Map, Value, json};

use media_domain::{LayerPersonality, OutputId, OutputName, PersonalityVersion};

/// The version this build writes.
pub const CURRENT_VERSION: u32 = 1;

/// Why a stored document cannot be brought forward.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MigrationError {
    #[error("the configuration document is a JSON {found}, not an object")]
    NotAnObject { found: &'static str },
    #[error(
        "the configuration document is version {found}, but this build writes version \
         {CURRENT_VERSION}; it was written by a newer Media Server"
    )]
    FromTheFuture { found: u32 },
    #[error("the configuration document has a non-numeric version field")]
    UnreadableVersion,
}

/// Brings any supported document to [`CURRENT_VERSION`].
pub fn migrate_to_current(document: Value) -> Result<Value, MigrationError> {
    let object = document.as_object().ok_or(MigrationError::NotAnObject {
        found: kind(&document),
    })?;

    let mut version = match object.get("version") {
        // No version field at all is the legacy `media/.info` document.
        None => 0,
        Some(Value::Number(number)) => {
            u32::try_from(number.as_u64().ok_or(MigrationError::UnreadableVersion)?)
                .map_err(|_| MigrationError::UnreadableVersion)?
        }
        Some(_) => return Err(MigrationError::UnreadableVersion),
    };

    if version > CURRENT_VERSION {
        return Err(MigrationError::FromTheFuture { found: version });
    }

    let mut current = document;
    while version < CURRENT_VERSION {
        current = match version {
            0 => from_legacy_info(current.as_object().expect("checked above")),
            other => unreachable!("no migration is registered for version {other}"),
        };
        version += 1;
    }
    Ok(current)
}

fn kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Version 0 → 1: the single-output `media/.info` document becomes a versioned document with an
/// `outputs` collection.
///
/// The legacy document held one global patch, one monitor, and one set of audio tuning. All of
/// it belongs to the one output the installation already had, so the migration mints a stable
/// identity for it and keeps every value the operator set.
fn from_legacy_info(legacy: &Map<String, Value>) -> Value {
    let full_mode = legacy
        .get("fullMode")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let protocol = match legacy.get("dmxProtocol").and_then(Value::as_str) {
        Some("sacn") => "sacn",
        _ => "art-net",
    };
    let universe = if protocol == "sacn" {
        number(legacy, "sacnUniverse", 1)
    } else {
        number(legacy, "artNetStartUniverse", 0)
    };

    // A migrated show keeps the channel layout it was programmed against. Moving to v2 is a
    // deliberate operator action, not something a migration does behind their back.
    let personality = if full_mode {
        LayerPersonality::EightLayers
    } else {
        LayerPersonality::TwoLayers
    };
    let target_codec = match legacy.get("targetCodec").and_then(Value::as_str) {
        Some("prores") => "pro-res",
        _ => "h264",
    };

    let output = json!({
        "id": OutputId::new(),
        "name": OutputName::new("Main"),
        "enabled": true,
        "target": legacy_target(legacy),
        "resolution": { "width": 1920, "height": 1080 },
        "presentation": "display-synchronized",
        "personality": personality,
        "personalityVersion": PersonalityVersion::V1Legacy,
        "protocol": protocol,
        "universe": universe,
        "startAddress": number(legacy, "artNetStartAddress", 1),
        "citp": { "layerBase": 0, "sourceName": Value::Null },
        "tempoSource": { "kind": "playback-bpm-channel" },
        "statusOverlay": legacy.get("showOverlay").and_then(Value::as_bool).unwrap_or(true),
    });

    json!({
        "version": 1,
        "configuration": {
            "instanceId": "media",
            "network": Value::Object(Map::new()),
            "library": { "root": "media", "targetCodec": target_codec },
            "audio": {
                "device": legacy_audio_device(legacy),
                "inputGain": float(legacy, "audioVolume"),
                "beatSensitivity": float(legacy, "audioSensitivity"),
                "eqBass": float(legacy, "audioEqBass"),
                "eqMid": float(legacy, "audioEqMid"),
                "eqTreble": float(legacy, "audioEqTreble"),
            },
            "outputs": [output],
        },
    })
}

fn legacy_target(legacy: &Map<String, Value>) -> Value {
    match legacy.get("monitor").and_then(Value::as_u64) {
        // The legacy application stored the monitor but never applied it, so index 0 carries no
        // operator intent worth preserving as a window binding.
        None | Some(0) => json!({ "kind": "offScreen" }),
        Some(index) => json!({
            "kind": "monitor",
            "monitor": { "by": "index", "value": index },
            "fullscreen": true,
        }),
    }
}

fn legacy_audio_device(legacy: &Map<String, Value>) -> Value {
    match legacy.get("audioDeviceId").and_then(Value::as_i64) {
        None | Some(-1) => json!({ "by": "systemDefault" }),
        Some(index) => json!({ "by": "index", "value": index }),
    }
}

fn number(legacy: &Map<String, Value>, key: &str, fallback: u64) -> u64 {
    legacy.get(key).and_then(Value::as_u64).unwrap_or(fallback)
}

fn float(legacy: &Map<String, Value>, key: &str) -> f64 {
    legacy.get(key).and_then(Value::as_f64).unwrap_or(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::configuration::{
        AudioDeviceSelector, DmxProtocol, MonitorSelector, OutputTarget, TargetCodec, load,
    };

    /// The document a v0.x installation actually has on disk, as `Config::toJson` writes it.
    fn legacy_info() -> &'static str {
        r#"{
            "monitor": 2,
            "artNetStartUniverse": 3,
            "artNetStartAddress": 45,
            "targetCodec": "prores",
            "showOverlay": false,
            "fullMode": false,
            "dmxProtocol": "artnet",
            "sacnUniverse": 1,
            "audioDeviceId": 4,
            "audioVolume": 0.5,
            "audioSensitivity": 1.5,
            "audioEqBass": 0.25,
            "audioEqMid": 0.75,
            "audioEqTreble": 2.0
        }"#
    }

    #[test]
    fn a_legacy_info_document_migrates_into_one_output() {
        let configuration = load(legacy_info()).unwrap();

        assert_eq!(configuration.outputs.len(), 1);
        let output = &configuration.outputs[0];
        assert_eq!(
            output.personality,
            LayerPersonality::TwoLayers,
            "fullMode false is two layers"
        );
        assert_eq!(output.personality_version, PersonalityVersion::V1Legacy);
        assert_eq!(output.protocol, DmxProtocol::ArtNet);
        assert_eq!(output.universe, 3);
        assert_eq!(output.start_address, 45);
        assert!(!output.status_overlay);
        assert_eq!(
            output.target,
            OutputTarget::Monitor {
                monitor: MonitorSelector::Index(2),
                fullscreen: true
            }
        );
    }

    #[test]
    fn legacy_library_and_audio_settings_survive() {
        let configuration = load(legacy_info()).unwrap();
        assert_eq!(configuration.library.target_codec, TargetCodec::ProRes);
        assert_eq!(configuration.audio.device, AudioDeviceSelector::Index(4));
        assert_eq!(configuration.audio.input_gain, 0.5);
        assert_eq!(configuration.audio.beat_sensitivity, 1.5);
        assert_eq!(configuration.audio.eq_bass, 0.25);
        assert_eq!(configuration.audio.eq_mid, 0.75);
        assert_eq!(configuration.audio.eq_treble, 2.0);
    }

    #[test]
    fn an_sacn_installation_keeps_its_own_universe() {
        let configuration =
            load(r#"{ "dmxProtocol": "sacn", "sacnUniverse": 7, "artNetStartUniverse": 3 }"#)
                .unwrap();
        assert_eq!(configuration.outputs[0].protocol, DmxProtocol::Sacn);
        assert_eq!(configuration.outputs[0].universe, 7);
    }

    #[test]
    fn an_empty_legacy_document_migrates_to_the_legacy_defaults() {
        let configuration = load("{}").unwrap();
        let output = &configuration.outputs[0];
        assert_eq!(
            output.personality,
            LayerPersonality::EightLayers,
            "fullMode defaulted true"
        );
        assert_eq!(output.start_address, 1);
        assert!(output.status_overlay);
        assert_eq!(output.target, OutputTarget::OffScreen);
        assert_eq!(
            configuration.audio.device,
            AudioDeviceSelector::SystemDefault
        );
    }

    #[test]
    fn a_current_document_is_left_alone() {
        let document = json!({ "version": 1, "configuration": { "outputs": [] } });
        assert_eq!(migrate_to_current(document.clone()).unwrap(), document);
    }

    #[test]
    fn a_newer_document_is_refused_rather_than_guessed_at() {
        let error = migrate_to_current(json!({ "version": 99, "configuration": {} })).unwrap_err();
        assert_eq!(error, MigrationError::FromTheFuture { found: 99 });
    }

    #[test]
    fn a_non_object_document_is_refused() {
        let error = migrate_to_current(json!([1, 2, 3])).unwrap_err();
        assert_eq!(error, MigrationError::NotAnObject { found: "array" });
    }

    #[test]
    fn a_non_numeric_version_is_refused() {
        let error = migrate_to_current(json!({ "version": "one" })).unwrap_err();
        assert_eq!(error, MigrationError::UnreadableVersion);
    }
}
