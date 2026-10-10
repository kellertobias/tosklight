use super::*;
use light_core::{AttributeKey, FixtureId, NativeColorBinding, PhysicalDataQuality, Xyz};
use light_dynamics::{
    DynamicController, DynamicControllerSource, DynamicDefinition, DynamicFamilyRepresentation,
    DynamicKeyframe, DynamicLaneBody, DynamicStartRequest, DynamicTargetScope, DynamicValue,
    DynamicValueAddress, DynamicValueSource, KeyframeConfiguration, ProgrammingLaneBody,
    ProgrammingLaneConfiguration,
};
use light_fixture::{
    ChannelFunction, ColorPhysicalModel, FixtureProfile, HeadOpticalPath, OpticalEmitter,
    OpticalEmitterBand, OpticalProvenance, OpticalSource,
};
use light_show::FixtureProfileRevision;
use serde_json::json;

fn data_dir() -> PathBuf {
    // The repository runner resolves this using tools/artifact-paths.sh, including overrides.
    let root = std::env::var_os("LIGHT_TMP_DIR")
        .expect("repository test runner sets the canonical LIGHT_TMP_DIR");
    PathBuf::from(root).join(format!("startup-runtime-pair-{}", Uuid::new_v4()))
}

fn original_profile() -> FixtureProfile {
    let mut profile = FixtureProfile::blank();
    profile.revision = 1;
    profile.manufacturer = "Test".into();
    profile.name = "Startup original source".into();
    let mode = &mut profile.modes[0];
    let head_id = mode.heads[0].id;
    let channel_id = Uuid::new_v4();
    let function = ChannelFunction::continuous("Red", AttributeKey("color.red".into()), 255);
    let binding = NativeColorBinding {
        channel_id,
        function_id: function.id,
    };
    mode.channels = vec![
        serde_json::from_value(json!({
            "id": channel_id, "head_id": head_id, "split": 1,
            "fixture_attribute": "color.red", "attribute": "color.red", "resolution": "u8",
            "default_raw": 0, "highlight_raw": 255, "functions": [function]
        }))
        .unwrap(),
    ];
    mode.splits[0].footprint = 1;
    mode.color_physical = Some(ColorPhysicalModel {
        version: 1,
        revision: 1,
        paths: vec![HeadOpticalPath {
            id: Uuid::new_v4(),
            head_id,
            controls: vec![channel_id],
            filters: vec![],
            measurements: vec![],
            source: OpticalSource::Additive {
                emitters: vec![OpticalEmitter {
                    id: Uuid::new_v4(),
                    name: "Red".into(),
                    binding,
                    xyz: Some(Xyz {
                        x: 1.,
                        y: 0.,
                        z: 0.,
                    }),
                    spectrum: vec![],
                    band: OpticalEmitterBand::Visible,
                    native_reversed: false,
                    maximum_level: 1.,
                    response_exponent: 1.,
                    provenance: OpticalProvenance {
                        quality: PhysicalDataQuality::Estimated,
                        ..Default::default()
                    },
                }],
            },
        }],
    });
    profile
}

fn definition() -> DynamicDefinition {
    serde_json::from_value(json!({
        "id": Uuid::new_v4(), "pool_number": 1, "revision": 1,
        "name": "Startup wave", "target_binding": {"type": "targetless"},
        "lanes": [{
            "id": Uuid::new_v4(), "attribute": "intensity", "mode": "keyframes",
            "keyframes": {"points": [
                {"position": 0., "source": {"type": "value", "value": 0.25}, "interpolation": "linear"},
                {"position": 0.5, "source": {"type": "value", "value": 0.75}, "interpolation": "linear"}
            ], "size": 1.},
            "max_min": {"minimum": {"type": "value", "value": 0.},
                "maximum": {"type": "value", "value": 1.}, "function": "sinus", "size": 1.},
            "middle_amplitude": {"middle": {"type": "current"}, "amplitude": 0.25,
                "function": "sinus", "size": 1.},
            "speed_multiplier": {"numerator": 1, "denominator": 1}, "width": 1.
        }],
        "phase": {"ordering": {"type": "selection"}, "offset_degrees": 0.,
            "span_degrees": 360., "block_size": 1, "repeats": 1, "wings": false,
            "anchors_degrees": []},
        "speed": {"type": "fixed", "duration_millis": 1000},
        "default_activation": "start_now"
    }))
    .unwrap()
}

fn native_definition(profile: &FixtureProfile, raw: u32) -> DynamicDefinition {
    let mode = &profile.modes[0];
    let channel = &mode.channels[0];
    let mut dynamic = definition();
    dynamic.lanes[0].body = DynamicLaneBody::Programming(ProgrammingLaneBody {
        address: DynamicValueAddress {
            representation: DynamicFamilyRepresentation::DirectColor {
                source: profile
                    .native_color_identity(mode.id, mode.heads[0].id)
                    .unwrap(),
            },
            component: Some(light_core::programming::ProgrammingComponent::NativeColor(
                NativeColorBinding {
                    channel_id: channel.id,
                    function_id: channel.functions[0].id,
                },
            )),
        },
        configuration: ProgrammingLaneConfiguration::Keyframes(KeyframeConfiguration {
            points: [0., 0.5]
                .into_iter()
                .map(|position| DynamicKeyframe {
                    position,
                    source: DynamicValueSource::Value {
                        value: DynamicValue::Native(raw),
                    },
                    interpolation: light_dynamics::ScalarInterpolation::Linear,
                })
                .collect(),
            size: 1.,
        }),
    });
    dynamic
}

fn show_entry(
    data_dir: &FsPath,
    profile: Option<&FixtureProfile>,
    dynamic: &DynamicDefinition,
) -> ShowEntry {
    let path = data_dir.join("shows/startup.show");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let (store, id) = ShowStore::create(&path, "Startup pair").unwrap();
    if let Some(profile) = profile {
        store
            .insert_fixture_profile_revision(
                &FixtureProfileRevision::from_profile(serde_json::to_value(profile).unwrap())
                    .unwrap(),
            )
            .unwrap();
    }
    store
        .put_object(
            "dynamic",
            &dynamic.id.to_string(),
            &serde_json::to_value(dynamic).unwrap(),
            0,
        )
        .unwrap();
    // A real pending portable migration must remain uncommitted if the Dynamic model rejects.
    store
        .put_object(
            "control_mapping",
            "midi-go",
            &json!({
                "name": "Legacy MIDI Go", "enabled": true,
                "trigger": {"type": "midi", "status": 144, "data1": 7},
                "action": {"type": "cue_go", "cue_list_id": Uuid::nil()}
            }),
            0,
        )
        .unwrap();
    ShowEntry {
        is_base_show: false,
        id,
        name: "Startup pair".into(),
        path: path.display().to_string(),
        revision: 0,
        updated_at: String::new(),
        created_at: None,
        last_loaded_at: None,
        revision_copy: None,
    }
}

fn start(runtime: &mut light_dynamics::DynamicRuntime, definition_id: Uuid) -> Uuid {
    runtime
        .start(DynamicStartRequest {
            definition_id,
            controller: DynamicController {
                id: Uuid::new_v4(),
                source: DynamicControllerSource::physical_playback(1),
                priority: 10,
                activated_at_millis: 0,
                size: 1.,
                speed_multiplier: 1.,
                phase_offset_degrees: 0.,
                paused: false,
            },
            target_scope: DynamicTargetScope {
                ordered_targets: vec![FixtureId::new()],
            },
            stage_positions: Default::default(),
            inherited_spatial_mapping: None,
            now_millis: 0,
            activation_delay_millis: 0,
            activation_duration_millis: 0,
            activation_policy_override: None,
            reuse_matching_targetless: false,
        })
        .unwrap()
}

#[test]
fn startup_native_definition_rejection_precedes_migration_and_engine_publication() {
    let data_dir = data_dir();
    let profile = original_profile();
    let dynamic = native_definition(&profile, 256);
    let entry = show_entry(&data_dir, Some(&profile), &dynamic);
    let engine = Engine::with_programming_contract_support(ProgrammerRegistry::default(), 1);
    let original = engine.snapshot();
    let store = ShowStore::open(&entry.path).unwrap();
    let revision = store.portable_revision().unwrap();
    let document = store.portable_document().unwrap();
    // This is specifically a model-aware Dynamic compile failure, after Engine validation.
    prepare_show_load(&entry, None)
        .unwrap()
        .prepare_runtime(|snapshot| engine.prepare_snapshot(snapshot))
        .unwrap();
    let error = match load_active_show_runtime_for_startup(&engine, &entry, &data_dir, 5) {
        Ok(_) => panic!("u8 source must reject native raw 256"),
        Err(error) => error,
    };
    assert!(
        error.contains("could not be loaded and might be corrupted or incompatible"),
        "{error}"
    );
    assert!(
        error.contains("native value is outside its pinned function"),
        "{error}"
    );
    assert!(Arc::ptr_eq(&original, &engine.snapshot()));
    assert_eq!(store.portable_revision().unwrap(), revision);
    assert_eq!(store.portable_document().unwrap(), document);
    assert!(
        !data_dir.join("backups").exists(),
        "failed preparation cannot create migration backups"
    );
    drop(store);
    std::fs::remove_dir_all(data_dir).unwrap();
}

#[test]
fn startup_retains_exact_unpatched_native_models_and_prepared_definitions() {
    let data_dir = data_dir();
    let profile = original_profile();
    let dynamic = native_definition(&profile, 255);
    let entry = show_entry(&data_dir, Some(&profile), &dynamic);
    let engine = Engine::with_programming_contract_support(ProgrammerRegistry::default(), 1);
    let mut runtime = load_active_show_runtime_for_startup(&engine, &entry, &data_dir, 5).unwrap();
    let snapshot = engine.snapshot();
    assert!(snapshot.fixtures.is_empty());
    let mode = &profile.modes[0];
    let source = profile
        .native_color_identity(mode.id, mode.heads[0].id)
        .unwrap();
    assert!(Arc::ptr_eq(
        &snapshot.native_color_sources.resolve(&source).unwrap(),
        &runtime
            .captured_native_color_models()
            .resolve(&source)
            .unwrap(),
    ));
    let instance = start(&mut runtime, dynamic.id);
    assert_eq!(
        runtime.instance_definition(instance).unwrap().as_ref(),
        &dynamic
    );
    assert!(runtime.unavailable_native_sources().is_empty());
    assert!(
        data_dir.join("backups").exists(),
        "successful pair commits its pending migration"
    );
    std::fs::remove_dir_all(data_dir).unwrap();
}

#[test]
fn startup_missing_original_model_remains_passive() {
    let data_dir = data_dir();
    let profile = original_profile();
    let dynamic = native_definition(&profile, 256);
    let entry = show_entry(&data_dir, None, &dynamic);
    let engine = Engine::with_programming_contract_support(ProgrammerRegistry::default(), 1);
    let mut runtime = load_active_show_runtime_for_startup(&engine, &entry, &data_dir, 5).unwrap();
    let instance = start(&mut runtime, dynamic.id);
    let unavailable = runtime.unavailable_native_sources();
    assert_eq!(unavailable.len(), 1);
    assert_eq!(unavailable[0].instance_id, instance);
    assert_eq!(engine.snapshot().dynamics.as_ref(), &vec![dynamic]);
    std::fs::remove_dir_all(data_dir).unwrap();
}

/// TL-552: production startup runs at the semantic contract and carries the prepared runtime.
#[test]
fn startup_state_carries_prepared_runtime_at_the_production_contract() {
    let data_dir = data_dir();
    let profile = original_profile();
    let dynamic = definition();
    let entry = show_entry(&data_dir, Some(&profile), &dynamic);
    let startup = startup_state::StartupState::load(startup_options::StartupOptions {
        data_dir: data_dir.clone(),
        show_file: Some(entry.path.into()),
        fixture_package_dir: None,
        extensions_dir: Some(data_dir.join("extensions")),
        bind: "127.0.0.1:0".parse().unwrap(),
        test_bench: true,
        visualizer_preview: false,
        osc_bind_override: None,
        output_bind_override: None,
    })
    .unwrap();
    assert_eq!(startup.active_show_error, None);
    assert_eq!(
        startup.engine.supported_programming_contract(),
        light_core::programming::SUPPORTED_PROGRAMMING_CONTRACT
    );
    let mode = &profile.modes[0];
    let source = profile
        .native_color_identity(mode.id, mode.heads[0].id)
        .unwrap();
    let mut runtime = startup.dynamics.lock();
    let instance = start(&mut runtime, dynamic.id);
    assert_eq!(
        runtime.instance_definition(instance).unwrap().as_ref(),
        &dynamic
    );
    assert!(Arc::ptr_eq(
        &startup
            .engine
            .snapshot()
            .native_color_sources
            .resolve(&source)
            .unwrap(),
        &runtime
            .captured_native_color_models()
            .resolve(&source)
            .unwrap(),
    ));
    drop(runtime);
    drop(startup);
    std::fs::remove_dir_all(data_dir).unwrap();
}

/// A runtime that cannot load the show (here an older contract-0 runtime meeting a typed native
/// Color Dynamic) enters recovery without retaining the rejected show's models. TL-552: the
/// production contract is 1, so the test-only startup harness supplies the older runtime.
#[test]
fn startup_recovery_does_not_retain_rejected_show_models() {
    let data_dir = data_dir();
    let profile = original_profile();
    let dynamic = native_definition(&profile, 255);
    let entry = show_entry(&data_dir, Some(&profile), &dynamic);
    let _contract_zero = crate::runtime::e2e_semantic_contract::startup_contract_override::at(0);
    let startup = startup_state::StartupState::load(startup_options::StartupOptions {
        data_dir: data_dir.clone(),
        show_file: Some(entry.path.into()),
        fixture_package_dir: None,
        extensions_dir: Some(data_dir.join("extensions")),
        bind: "127.0.0.1:0".parse().unwrap(),
        test_bench: true,
        visualizer_preview: false,
        osc_bind_override: None,
        output_bind_override: None,
    })
    .unwrap();
    assert!(startup.active_show_error.as_deref().is_some_and(|error| {
        error.contains("could not be loaded and might be corrupted or incompatible")
    }));
    assert!(startup.engine.snapshot().dynamics.is_empty());
    let mode = &profile.modes[0];
    let source = profile
        .native_color_identity(mode.id, mode.heads[0].id)
        .unwrap();
    assert!(
        startup
            .dynamics
            .lock()
            .captured_native_color_models()
            .resolve(&source)
            .is_err()
    );
    drop(startup);
    std::fs::remove_dir_all(data_dir).unwrap();
}
