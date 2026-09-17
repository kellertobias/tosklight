//! Configuration migrations.
//!
//! Version 0 is the C++ application's `media/.info` document, which carries no version field at
//! all. Every later version is migrated forward one step at a time, so an installation that has
//! skipped releases still arrives at the current document.

use serde_json::{Map, Value, json};

use media_domain::{LayerPersonality, OutputId, OutputName};

/// The version this build writes.
pub const CURRENT_VERSION: u32 = 6;

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
    #[error(
        "the configuration contains {found} visualizers, but folders 250 through 255 hold at most {maximum}"
    )]
    TooManyVisualizers { found: usize, maximum: usize },
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
            0 => Ok(from_legacy_info(
                current.as_object().expect("checked above"),
            )),
            1 => Ok(without_personality_version(current)),
            2 => visualizers_into_final_banks(current),
            3 => Ok(with_effect_library(current)),
            4 => Ok(onto_the_mapping_layout(current)),
            5 => Ok(with_builtin_models(current)),
            other => unreachable!("no migration is registered for version {other}"),
        }?;
        version += 1;
    }
    Ok(current)
}

/// Version 3 → 4: effect definitions move out of volatile layer state into an addressed,
/// persisted library. Existing installations receive the same shipped starter presets as a new
/// configuration; no layer is activated because selector zero remains Off.
fn with_effect_library(mut document: Value) -> Value {
    if let Some(configuration) = document
        .get_mut("configuration")
        .and_then(Value::as_object_mut)
    {
        configuration.entry("effects").or_insert_with(|| {
            serde_json::to_value(media_domain::EffectLibrary::default())
                .expect("the default effect library is serializable")
        });
    }
    document["version"] = json!(4);
    document
}

/// Version 4 → 5: every output decodes the 3D-object-mapping layout.
///
/// The legacy, mask-positioning, full-master, and effect-bank channel layouts were retired before
/// launch, so the stored `personalityLayout` choice is dropped rather than translated. The 2-layer
/// and 8-layer personalities themselves are kept. The mapping layout is larger than any retired
/// one, so a start address that no longer leaves room for the whole block moves to the highest
/// address that does; the desk must then be repatched to match, which it already had to be.
fn onto_the_mapping_layout(mut document: Value) -> Value {
    if let Some(outputs) = document
        .get_mut("configuration")
        .and_then(|configuration| configuration.get_mut("outputs"))
        .and_then(Value::as_array_mut)
    {
        for output in outputs.iter_mut().filter_map(Value::as_object_mut) {
            output.remove("personalityLayout");
            let personality = output
                .get("personality")
                .cloned()
                .and_then(|value| serde_json::from_value::<LayerPersonality>(value).ok())
                .unwrap_or_default();
            let footprint = personality.footprint();
            let highest = media_domain::personality::UNIVERSE_SLOTS - footprint.total() + 1;
            if let Some(start) = output.get("startAddress").and_then(Value::as_u64)
                && start > u64::from(highest)
            {
                output.insert("startAddress".to_owned(), json!(highest));
            }
        }
    }
    document["version"] = json!(5);
    document
}

/// Version 5 → 6: the model library receives the built-in models.
///
/// A document without a model library receives the default one, Plane in slot 1. A stored
/// library keeps every imported model at its number and gains each built-in model whose default
/// slot is still empty, so no cue is redirected. A library that cannot be read is left alone for
/// the load to refuse with its own message.
fn with_builtin_models(mut document: Value) -> Value {
    if let Some(configuration) = document
        .get_mut("configuration")
        .and_then(Value::as_object_mut)
    {
        let library = match configuration.get("models") {
            None => Some(media_domain::ModelLibrary::default()),
            Some(stored) => serde_json::from_value::<media_domain::ModelLibrary>(stored.clone())
                .ok()
                .map(|mut library| {
                    library.add_builtins_to_free_slots();
                    library
                }),
        };
        if let Some(library) = library {
            configuration.insert(
                "models".to_owned(),
                serde_json::to_value(library).expect("a model library is serializable"),
            );
        }
    }
    document["version"] = json!(6);
    document
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

/// Version 1 → 2: the `personalityVersion` field is dropped.
///
/// It recorded the channel layout a show had been programmed against, so a migrated installation
/// could in principle be read the old way. Nothing ever read it, and it turned out there was
/// nothing to read it for: this product was never published, so no desk was ever patched against
/// the C++ application's 32-slot layer. There is one personality, and it is the one this build
/// speaks.
fn without_personality_version(document: Value) -> Value {
    let mut document = document;
    if let Some(outputs) = document
        .get_mut("configuration")
        .and_then(|configuration| configuration.get_mut("outputs"))
        .and_then(Value::as_array_mut)
    {
        for output in outputs {
            if let Some(output) = output.as_object_mut() {
                output.remove("personalityVersion");
            }
        }
    }
    document["version"] = json!(2);
    document
}

/// Version 2 → 3: text grows through folder 249 and generated visualizers are compacted into
/// folders 250–255. The address change is the explicitly declared pre-v1 compatibility break;
/// configurations and names survive in their previous address order.
fn visualizers_into_final_banks(mut document: Value) -> Result<Value, MigrationError> {
    const FIRST_FOLDER: usize = 250;
    const FOLDERS: usize = 6;
    const FILES_PER_FOLDER: usize = 254;
    const CAPACITY: usize = FOLDERS * FILES_PER_FOLDER;

    if let Some(entries) = document
        .get_mut("configuration")
        .and_then(|configuration| configuration.get_mut("visualizers"))
        .and_then(|visualizers| visualizers.get_mut("entries"))
        .and_then(Value::as_array_mut)
    {
        if entries.len() > CAPACITY {
            return Err(MigrationError::TooManyVisualizers {
                found: entries.len(),
                maximum: CAPACITY,
            });
        }
        for (index, entry) in entries.iter_mut().enumerate() {
            let Some(address) = entry.get_mut("address").and_then(Value::as_object_mut) else {
                continue;
            };
            address.insert(
                "folder".to_owned(),
                json!(FIRST_FOLDER + index / FILES_PER_FOLDER),
            );
            address.insert("file".to_owned(), json!(1 + index % FILES_PER_FOLDER));
        }
    }
    document["version"] = json!(3);
    Ok(document)
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
    fn a_document_that_still_carries_a_personality_version_is_brought_forward() {
        // What version 1 wrote, including the field that no longer exists. Refusing to read it
        // would strand a development installation on a document it wrote itself.
        let stored = r#"{
            "version": 1,
            "configuration": {
                "instanceId": "media",
                "outputs": [{
                    "id": "3f4b8e4e-1111-4111-8111-111111111111",
                    "name": "Main",
                    "enabled": true,
                    "target": { "kind": "offScreen" },
                    "resolution": { "width": 1920, "height": 1080 },
                    "presentation": "display-synchronized",
                    "personality": "eight-layers",
                    "personalityVersion": "v1-legacy",
                    "protocol": "art-net",
                    "universe": 0,
                    "startAddress": 1,
                    "citp": { "layerBase": 0, "sourceName": null },
                    "tempoSource": { "kind": "playback-bpm-channel" },
                    "statusOverlay": true
                }]
            }
        }"#;

        let configuration = load(stored).expect("a version 1 document still loads");
        assert_eq!(configuration.outputs.len(), 1);
        assert_eq!(
            configuration.outputs[0].personality,
            LayerPersonality::EightLayers
        );
        assert_eq!(configuration.outputs[0].start_address, 1);
    }

    #[test]
    fn what_this_build_writes_carries_no_personality_version() {
        let written = super::super::save(&crate::MediaConfiguration::default());
        assert!(
            !written.contains("personalityVersion"),
            "there is one personality, so nothing records which one"
        );
        assert!(written.contains("\"version\": 6"));
        assert!(
            !written.contains("personalityLayout"),
            "there is one channel layout, so nothing records which one"
        );
    }

    #[test]
    fn version_two_visualizers_are_compacted_into_folders_250_through_255() {
        let document = json!({
            "version": 2,
            "configuration": {
                "visualizers": {
                    "version": 1,
                    "entries": [
                        { "address": { "folder": 220, "file": 9 }, "configuration": { "name": "First" } },
                        { "address": { "folder": 249, "file": 4 }, "configuration": { "name": "Second" } }
                    ]
                }
            }
        });

        let migrated = migrate_to_current(document).expect("version two migrates");
        let entries = migrated["configuration"]["visualizers"]["entries"]
            .as_array()
            .expect("entries");
        assert_eq!(migrated["version"], json!(CURRENT_VERSION));
        assert_eq!(entries[0]["address"], json!({ "folder": 250, "file": 1 }));
        assert_eq!(entries[1]["address"], json!({ "folder": 250, "file": 2 }));
        assert_eq!(entries[0]["configuration"]["name"], json!("First"));
    }

    #[test]
    fn version_three_receives_the_addressed_effect_library_without_enabling_a_bank() {
        let migrated = migrate_to_current(json!({
            "version": 3,
            "configuration": {}
        }))
        .expect("version three migrates");
        assert_eq!(migrated["version"], json!(CURRENT_VERSION));
        let effects: media_domain::EffectLibrary =
            serde_json::from_value(migrated["configuration"]["effects"].clone()).unwrap();
        assert_eq!(effects.resolve(1).unwrap().name, "TV/CRT/VHS Simulation");
        assert_eq!(media_domain::EffectBankState::default().select, 0);
    }

    fn version_four_output(personality: &str, layout: &str, start_address: u16) -> Value {
        json!({
            "id": OutputId::new(),
            "name": personality,
            "personality": personality,
            "personalityLayout": layout,
            "protocol": "sacn",
            // One universe per personality, so two outputs in one document never overlap.
            "universe": if personality == "two-layers" { 3 } else { 4 },
            "startAddress": start_address,
        })
    }

    /// Every retired channel layout, stored by a version 4 document, loads as the mapping layout
    /// with the personality the operator chose.
    #[test]
    fn version_four_outputs_on_retired_layouts_load_as_the_mapping_personalities() {
        for layout in ["legacy", "current", "extended", "effect-banks", "mapping"] {
            let document = json!({
                "version": 4,
                "configuration": {
                    "outputs": [
                        version_four_output("two-layers", layout, 200),
                        version_four_output("eight-layers", layout, 1),
                    ]
                }
            });
            let migrated = migrate_to_current(document.clone()).expect("version four migrates");
            assert_eq!(migrated["version"], json!(CURRENT_VERSION));
            for output in migrated["configuration"]["outputs"].as_array().unwrap() {
                assert!(output.get("personalityLayout").is_none(), "{layout}");
            }

            let configuration = super::super::load(&document.to_string())
                .unwrap_or_else(|error| panic!("{layout}: {error}"));
            let outputs = &configuration.outputs;
            assert_eq!(outputs[0].personality, LayerPersonality::TwoLayers);
            assert_eq!(outputs[0].start_address, 200);
            assert_eq!(outputs[0].personality.footprint().total(), 158);
            assert_eq!(outputs[1].personality, LayerPersonality::EightLayers);
            assert_eq!(outputs[1].personality.footprint().total(), 512);
        }
    }

    /// An eight-layer effect-bank output fit 353 slots from address 100. The 512-slot mapping block
    /// only fits from address 1, so the migrated output moves there instead of refusing to start.
    #[test]
    fn a_start_address_the_mapping_block_no_longer_fits_moves_to_the_highest_valid_one() {
        let document = json!({
            "version": 4,
            "configuration": {
                "outputs": [version_four_output("eight-layers", "effect-banks", 100)]
            }
        });
        let configuration = super::super::load(&document.to_string()).unwrap();
        assert_eq!(configuration.outputs[0].start_address, 1);

        let two_layers = json!({
            "version": 4,
            "configuration": {
                "outputs": [version_four_output("two-layers", "legacy", 400)]
            }
        });
        let configuration = super::super::load(&two_layers.to_string()).unwrap();
        assert_eq!(configuration.outputs[0].start_address, 355);
    }

    #[test]
    fn an_unknown_personality_is_still_refused_after_migration() {
        let document = json!({
            "version": 4,
            "configuration": {
                "outputs": [version_four_output("four-layers", "mapping", 1)]
            }
        });
        assert!(super::super::load(&document.to_string()).is_err());
    }

    #[test]
    fn a_version_five_document_naming_a_channel_layout_is_refused() {
        let document = json!({
            "version": 5,
            "configuration": {
                "outputs": [version_four_output("two-layers", "legacy", 1)]
            }
        });
        assert!(
            super::super::load(&document.to_string()).is_err(),
            "version five has no channel-layout field to choose a retired layout with"
        );
    }

    #[test]
    fn an_eight_layer_legacy_info_document_starts_where_the_block_fits() {
        let configuration = load(r#"{ "fullMode": true, "artNetStartAddress": 45 }"#).unwrap();
        assert_eq!(
            configuration.outputs[0].personality,
            LayerPersonality::EightLayers
        );
        assert_eq!(configuration.outputs[0].start_address, 1);
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
        assert_eq!(
            CURRENT_VERSION, 6,
            "documents with built-in models are version 6"
        );
        let document = json!({ "version": CURRENT_VERSION, "configuration": { "outputs": [] } });
        assert_eq!(migrate_to_current(document.clone()).unwrap(), document);
    }

    /// A stored document of `version` with this build's default outputs and the given library.
    fn document_with_models(version: u32, models: Option<Value>) -> Value {
        let mut document =
            serde_json::to_value(super::super::ConfigurationDocument::default()).unwrap();
        document["version"] = json!(version);
        let configuration = document["configuration"].as_object_mut().unwrap();
        match models {
            Some(models) => configuration.insert("models".to_owned(), models),
            None => configuration.remove("models"),
        };
        document
    }

    #[test]
    fn a_version_five_document_without_a_model_library_receives_the_built_in_models() {
        let document = document_with_models(5, None);
        let configuration = super::super::load(&document.to_string()).unwrap();
        assert_eq!(configuration.models, media_domain::ModelLibrary::default());
        let plane = configuration.models.resolve(1).unwrap();
        assert_eq!(plane.builtin, Some(media_domain::BuiltinModel::Plane));
    }

    #[test]
    fn a_version_five_model_library_keeps_its_imports_and_gains_built_ins_in_free_slots() {
        let document = document_with_models(
            5,
            Some(json!({ "entries": [
                { "slot": 2, "name": "Truss", "file": "model-002.glb", "vertices": 8, "triangles": 12 },
                { "slot": 40, "name": "Screen", "file": "model-040.glb" }
            ] })),
        );
        let migrated = migrate_to_current(document.clone()).unwrap();
        assert_eq!(migrated["version"], json!(6));
        let models = super::super::load(&document.to_string()).unwrap().models;
        let summary: Vec<(u8, &str, Option<media_domain::BuiltinModel>)> = models
            .entries
            .iter()
            .map(|entry| (entry.slot, entry.name.as_str(), entry.builtin))
            .collect();
        use media_domain::BuiltinModel as B;
        assert_eq!(
            summary,
            vec![
                (1, "Plane", Some(B::Plane)),
                (2, "Truss", None),
                (3, "Sphere", Some(B::Sphere)),
                (4, "Cylinder", Some(B::Cylinder)),
                (5, "Pyramid", Some(B::Pyramid)),
                (40, "Screen", None),
            ],
            "the imported slot 2 keeps its number; the Cube waits for the operator to place it"
        );
        assert_eq!(models.resolve(2).unwrap().file, "model-002.glb");
    }

    #[test]
    fn a_version_six_library_the_operator_emptied_stays_empty() {
        let document = document_with_models(6, Some(json!({ "entries": [] })));
        let configuration = super::super::load(&document.to_string()).unwrap();
        assert!(configuration.models.entries.is_empty());
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
