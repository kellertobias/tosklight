//! Native dependency/persistence tests. Adapter lifecycle ordering has separate regressions.
use super::publication_tests::output_with_programmers;
use super::*;
use light_application::{ActionError, ActionErrorKind};
use light_core::{AttributeKey, FixtureId, NativeColorBinding, NativeColorIdentity, Xyz};
use light_dynamics::{
    ActivationPolicy, DynamicDefinition, DynamicFamilyRepresentation, DynamicLaneBody,
    DynamicLaneMode, DynamicRandomGroup, DynamicRandomRange, DynamicRuntime, DynamicRuntimeSample,
    DynamicSpeed, DynamicSpeedTransport, DynamicValue, DynamicValueAddress, DynamicValueSource,
    MaxMinConfiguration, NativeColorModelCapability, PeriodicFunction, ProgrammingLaneBody,
    ProgrammingLaneConfiguration, PwmShape, Rational, ScalarSource, ScalarSourceResolver,
};
use light_engine::{NativeColorSourceCatalog, NativeColorSourceRevisionKey};
use light_fixture::{
    CanonicalTransform, ChannelBehavior, ChannelFunction, ChannelResolution, ColorPhysicalModel,
    FixtureChannel, FixtureProfile, HeadOpticalPath, NativeColorBinding as FixtureNativeBinding,
    OpticalEmitter, OpticalEmitterBand, OpticalProvenance, OpticalSource,
};
use std::cell::Cell;

fn playback() -> PlaybackRenderCapability {
    PlaybackRenderCapability::new(
        PlaybackService::new(EventBus::default()),
        Arc::new(
            crate::runtime::playback_telemetry::PlaybackTelemetrySampler::new(Arc::new(
                AtomicU16::new(40),
            )),
        ),
    )
}

fn definition() -> DynamicDefinition {
    serde_json::from_value(serde_json::json!({
        "id": Uuid::new_v4(), "pool_number": 1, "revision": 1, "name": "Finalizer test",
        "target_binding": {"type": "frozen_targets", "targets": [FixtureId::new()]},
        "lanes": [{
            "id": Uuid::new_v4(), "attribute": "intensity", "mode": "keyframes",
            "keyframes": {"points": [
                {"position": 0.0, "source": {"type": "value", "value": 0.0}, "interpolation": "linear"},
                {"position": 0.5, "source": {"type": "value", "value": 1.0}, "interpolation": "linear"}
            ]},
            "max_min": {"minimum": {"type": "value", "value": 0.0},
                "maximum": {"type": "value", "value": 1.0}, "function": "sinus"},
            "middle_amplitude": {"middle": {"type": "current"}, "amplitude": 0.5, "function": "sinus"},
            "speed_multiplier": {"numerator": 1, "denominator": 1}, "width": 1.0
        }],
        "phase": {"ordering": {"type": "selection"}, "offset_degrees": 0.0,
            "span_degrees": 0.0, "block_size": 1, "repeats": 1,
            "wings": false, "anchors_degrees": []},
        "speed": {"type": "fixed", "duration_millis": 1000},
        "default_activation": "start_now"
    }))
    .unwrap()
}

/// A validated fixture profile and the real catalogue compiler give this channel 8-bit bounds.
fn profile() -> FixtureProfile {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "Finalizer original UV source".into();
    profile.revision = 1;
    let mode = &mut profile.modes[0];
    let attribute = AttributeKey("color.uv".into());
    let channel = FixtureChannel {
        id: Uuid::new_v4(),
        head_id: mode.heads[0].id,
        split: 1,
        fixture_attribute: attribute.clone(),
        attribute: attribute.clone(),
        canonical_transform: CanonicalTransform::Identity,
        resolution: ChannelResolution::U8,
        secondary_slots: vec![],
        default_raw: 0,
        highlight_raw: 255,
        physical_min: None,
        physical_max: None,
        unit: None,
        invert: false,
        snap: false,
        reacts_to_virtual_intensity: false,
        virtual_intensity_inverted: false,
        behavior: ChannelBehavior::Controlled,
        functions: vec![ChannelFunction::continuous("UV", attribute, 255)],
    };
    mode.color_physical = Some(ColorPhysicalModel {
        version: 1,
        revision: 1,
        paths: vec![HeadOpticalPath {
            id: Uuid::new_v4(),
            head_id: mode.heads[0].id,
            controls: vec![channel.id],
            source: OpticalSource::Additive {
                emitters: vec![OpticalEmitter {
                    id: Uuid::new_v4(),
                    name: "UV".into(),
                    binding: FixtureNativeBinding {
                        channel_id: channel.id,
                        function_id: channel.functions[0].id,
                    },
                    xyz: Some(Xyz {
                        x: 0.1,
                        y: 0.2,
                        z: 0.3,
                    }),
                    spectrum: vec![],
                    band: OpticalEmitterBand::Ultraviolet,
                    native_reversed: false,
                    maximum_level: 1.0,
                    response_exponent: 1.0,
                    provenance: OpticalProvenance::default(),
                }],
            },
            filters: vec![],
            measurements: vec![],
        }],
    });
    mode.channels.push(channel);
    profile.validate().unwrap();
    profile
}

fn identity(profile: &FixtureProfile) -> NativeColorIdentity {
    profile
        .native_color_identity(profile.modes[0].id, profile.modes[0].heads[0].id)
        .unwrap()
}

fn catalogue(profiles: &[&FixtureProfile]) -> Arc<NativeColorSourceCatalog> {
    Arc::new(
        NativeColorSourceCatalog::from_revisions(profiles.iter().map(|profile| {
            NativeColorSourceCatalog::compile_revision(
                NativeColorSourceRevisionKey {
                    profile_id: profile.id,
                    revision: u64::from(profile.revision),
                    raw_store_digest: format!("retained-profile-{}", profile.id.0),
                },
                None,
                || Ok((**profile).clone()),
            )
        }))
        .unwrap(),
    )
}

fn native_definition(profile: &FixtureProfile, high: u32) -> DynamicDefinition {
    let channel = &profile.modes[0].channels[0];
    let mut definition = definition();
    definition.lanes[0].body = DynamicLaneBody::Programming(ProgrammingLaneBody {
        address: DynamicValueAddress {
            representation: DynamicFamilyRepresentation::DirectColor {
                source: identity(profile),
            },
            component: Some(light_core::programming::ProgrammingComponent::NativeColor(
                NativeColorBinding {
                    channel_id: channel.id,
                    function_id: channel.functions[0].id,
                },
            )),
        },
        configuration: ProgrammingLaneConfiguration::MaxMin(MaxMinConfiguration {
            minimum: DynamicValueSource::Value {
                value: DynamicValue::Native(0),
            },
            maximum: DynamicValueSource::Value {
                value: DynamicValue::Native(high),
            },
            function: PeriodicFunction::LinearUp,
            size: 1.0,
            pwm: PwmShape::default(),
        }),
    });
    definition
}

fn output() -> OutputResource {
    output_with_programmers(light_programmer::ProgrammerRegistry::with_clock(Arc::new(
        light_core::ManualClock::new(chrono::DateTime::from_timestamp_millis(0).unwrap()),
    )))
}

fn start(output: &OutputResource, definition: &DynamicDefinition) -> (Uuid, Uuid) {
    // Give the running clock an actual active Playback owner. A hand-built orphan would
    // correctly be released by destination reconciliation and cannot test retained history.
    let mut snapshot = output.snapshot().as_ref().clone();
    snapshot.playbacks = vec![serde_json::from_value(serde_json::json!({
        "number": 1, "name": "Finalizer owner", "target": {"type": "dynamic", "assignment": {
            "dynamic": {"dynamic_id": definition.id, "last_known_pool_number": definition.pool_number,
                "embedded_fallback": {"definition": definition}}
        }}
    })).unwrap()].into();
    output.replace_snapshot(snapshot).unwrap();
    output
        .engine
        .execute_playback(light_engine::EnginePlaybackCommand::Pool {
            number: 1,
            action: light_engine::PoolPlaybackAction::On,
        })
        .unwrap();
    output.reconcile_dynamic_runtime();
    let controllers = output.dynamics.lock().controllers();
    assert_eq!(controllers.len(), 1);
    (controllers[0].0, controllers[0].1.id)
}

fn install_missing(output: &OutputResource, definition: &DynamicDefinition) {
    output
        .replace_snapshot(EngineSnapshot {
            revision: 1,
            dynamics: vec![definition.clone()].into(),
            native_color_sources: catalogue(&[]),
            ..Default::default()
        })
        .unwrap();
    // Explicitly establish the current provider; the old installer only pairs definition tables.
    output
        .dynamics
        .lock()
        .refresh_native_color_models(output.snapshot().native_color_sources.clone())
        .unwrap();
}

#[test]
fn newly_verifiable_invalid_native_values_reject_before_persistence_or_publication() {
    let output = output();
    let profile = profile();
    let definition = native_definition(&profile, 256);
    install_missing(&output, &definition);
    start(&output, &definition);
    let old_snapshot = output.snapshot();
    let (cold_cursor, control_cursor) = output
        .dynamic_snapshot
        .begin_retained_history(
            &mut output.dynamics.lock(),
            &old_snapshot,
            std::num::NonZeroUsize::new(16).unwrap(),
        )
        .unwrap();
    let old_runtime = output.dynamics.lock().snapshot();
    let old_models = output.dynamics.lock().captured_native_color_models();
    let mut next = old_snapshot.as_ref().clone();
    next.revision += 1;
    next.native_color_sources = catalogue(&[&profile]);
    assert!(
        next.native_color_sources
            .resolve(&identity(&profile))
            .is_ok()
    );
    let prepared = output.prepare_snapshot(next).unwrap();
    let persisted = Cell::new(false);
    let result = output.finalize_snapshot(&playback(), prepared, || {
        persisted.set(true);
        Ok(())
    });
    assert!(
        result.is_err(),
        "the real 8-bit original must reject raw 256"
    );
    assert!(!persisted.get());
    assert!(Arc::ptr_eq(&output.snapshot(), &old_snapshot));
    let live = output.dynamics.lock();
    assert_eq!(live.snapshot(), old_runtime);
    assert!(Arc::ptr_eq(
        &live.captured_native_color_models(),
        &old_models
    ));
    assert!(output.dynamic_snapshot.matches(&old_snapshot));
    assert_eq!(live.control_cursor(), Some(control_cursor));
    assert!(
        output
            .dynamic_snapshot
            .cold_generations_since(cold_cursor)
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        old_models.resolve_capability(&identity(&profile)).unwrap(),
        NativeColorModelCapability::Unavailable(_)
    ));
}

#[test]
fn missing_native_model_remains_passive_and_installs_a_coherent_pair_after_persist() {
    let output = output();
    let profile = profile();
    let definition = native_definition(&profile, 256);
    install_missing(&output, &definition);
    start(&output, &definition);
    let before = output.snapshot();
    let runtime_before = output.dynamics.lock().snapshot();
    let mut next = before.as_ref().clone();
    next.revision += 1;
    next.native_color_sources = catalogue(&[]);
    let prepared = output.prepare_snapshot(next).unwrap();
    let expected = prepared.engine.snapshot_arc();
    let result = output
        .finalize_snapshot(&playback(), prepared, || {
            assert!(
                Arc::ptr_eq(&output.snapshot(), &before),
                "persist precedes Engine publication"
            );
            assert!(output.dynamic_snapshot.matches(&before));
            Ok(17)
        })
        .unwrap();
    assert_eq!(result, 17);
    assert!(Arc::ptr_eq(&output.snapshot(), &expected));
    let live = output.dynamics.lock();
    assert_eq!(live.snapshot(), runtime_before);
    assert_eq!(live.unavailable_native_sources().len(), 1);
    assert!(output.dynamic_snapshot.matches(&expected));
    assert!(matches!(
        live.captured_native_color_models()
            .resolve_capability(&identity(&profile))
            .unwrap(),
        NativeColorModelCapability::Unavailable(_)
    ));
}

#[test]
fn removing_unused_invalid_definition_does_not_hide_an_invalid_retained_instance() {
    for retained in [false, true] {
        let output = output();
        let profile = profile();
        let definition = native_definition(&profile, 256);
        install_missing(&output, &definition);
        if retained {
            start(&output, &definition);
        }
        let before = output.snapshot();
        let runtime_before = output.dynamics.lock().snapshot();
        let old_models = output.dynamics.lock().captured_native_color_models();
        let mut next = before.as_ref().clone();
        next.revision += 1;
        next.dynamics = Arc::default();
        next.native_color_sources = catalogue(&[&profile]);
        let prepared = output.prepare_snapshot(next).unwrap();
        let expected = prepared.engine.snapshot_arc();
        let persisted = Cell::new(false);
        let result = output.finalize_snapshot(&playback(), prepared, || {
            persisted.set(true);
            Ok(())
        });
        assert_eq!(result.is_err(), retained);
        assert_eq!(persisted.get(), !retained);
        if retained {
            assert!(Arc::ptr_eq(&output.snapshot(), &before));
            let live = output.dynamics.lock();
            assert_eq!(live.snapshot(), runtime_before);
            assert!(Arc::ptr_eq(
                &old_models,
                &live.captured_native_color_models()
            ));
            assert!(output.dynamic_snapshot.matches(&before));
        } else {
            assert!(Arc::ptr_eq(&output.snapshot(), &expected));
            assert!(output.dynamics.lock().snapshot().instances.is_empty());
            assert!(output.dynamic_snapshot.matches(&expected));
        }
    }
}

#[test]
fn failed_persistence_retains_live_registry_and_current_provider_without_new_pins() {
    let output = output();
    let original = profile();
    let replacement = profile();
    let old_definition = native_definition(&original, 255);
    let initial = output
        .prepare_snapshot(EngineSnapshot {
            revision: 1,
            dynamics: vec![old_definition.clone()].into(),
            native_color_sources: catalogue(&[&original]),
            ..Default::default()
        })
        .unwrap();
    output
        .finalize_snapshot(&playback(), initial, || Ok(()))
        .unwrap();
    start(&output, &old_definition);
    let before = output.snapshot();
    let runtime_before = output.dynamics.lock().snapshot();
    let old_models = output.dynamics.lock().captured_native_color_models();
    let original_model = old_models.resolve(&identity(&original)).unwrap();
    let replacement_definition = native_definition(&replacement, 255);
    let mut next = before.as_ref().clone();
    next.revision += 1;
    next.dynamics = vec![replacement_definition].into();
    next.native_color_sources = catalogue(&[&original, &replacement]);
    let prepared = output.prepare_snapshot(next).unwrap();
    let persisted = Cell::new(false);
    let result: Result<(), ActionError> = output.finalize_snapshot(&playback(), prepared, || {
        persisted.set(true);
        Err(ActionError::new(
            ActionErrorKind::Invalid,
            "test persistence failure",
        ))
    });
    assert!(
        persisted.get(),
        "native validation succeeded before the commit was attempted"
    );
    assert!(
        result
            .unwrap_err()
            .message
            .contains("test persistence failure")
    );
    assert!(Arc::ptr_eq(&output.snapshot(), &before));
    let live = output.dynamics.lock();
    assert_eq!(live.snapshot(), runtime_before);
    let current = live.captured_native_color_models();
    assert!(Arc::ptr_eq(&current, &old_models));
    assert!(Arc::ptr_eq(
        &current.resolve(&identity(&original)).unwrap(),
        &original_model
    ));
    assert!(matches!(
        current.resolve_capability(&identity(&replacement)).unwrap(),
        NativeColorModelCapability::Unavailable(_)
    ));
    assert!(output.dynamic_snapshot.matches(&before));
}

struct Sources;
impl ScalarSourceResolver for Sources {
    fn current(&self, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        Some(0.0)
    }
    fn preset(&self, _: &str, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        None
    }
}

fn sample(runtime: &mut DynamicRuntime, now: u64) -> Vec<DynamicRuntimeSample> {
    runtime.sample_all(
        now,
        10,
        &[DynamicSpeedTransport {
            effective_bpm: 60.0,
            phase_origin_millis: 0,
            phase_reference_millis: now,
            beat_phase: (now as f64 / 1000.0).rem_euclid(1.0),
            phase_advancing: true,
        }; 5],
        &Sources,
    )
}

#[test]
fn finalization_keeps_random_advancement_controller_edits_and_pause_after_preparation() {
    let output = output();
    let mut definition = definition();
    definition.lanes[0].legacy_mut().unwrap().mode = DynamicLaneMode::Random;
    let group = Uuid::new_v4();
    definition.lanes[0].random_group_id = Some(group);
    definition.default_activation = ActivationPolicy::JoinSyncNow;
    definition.speed = DynamicSpeed::SpeedGroup {
        group: light_dynamics::SpeedGroup::A,
        beats_per_cycle: Rational::ONE,
    };
    definition.random_groups.push(DynamicRandomGroup {
        id: group,
        seed: 17,
        range: DynamicRandomRange::LegacyScalar {
            low: ScalarSource::Value { value: 0.0 },
            high: ScalarSource::Value { value: 1.0 },
        },
        decision_interval_millis: 100,
        start_probability: 1.0,
        mean_duration_millis: 250,
        duration_spread_millis: 30,
        attack_ratio: 0.1,
        decay_ratio: 0.5,
    });
    install_missing(&output, &definition);
    let (_, controller) = start(&output, &definition);
    sample(&mut output.dynamics.lock(), 150);
    definition.revision += 1;
    definition.name = "Prepared before the latest transport changes".into();
    let mut next = output.snapshot().as_ref().clone();
    next.revision += 1;
    next.dynamics = vec![definition.clone()].into();
    let prepared = output.prepare_snapshot(next).unwrap();
    let expected_snapshot = prepared.engine.snapshot_arc();

    // These are edits accepted after snapshot preparation. Finalization must capture both
    // their authoritative Playback controls and the latest Dynamic sample history.
    let mut active = output.engine.active_dynamic_playbacks();
    active[0].size = 0.6;
    active[0].local_speed_multiplier = Rational {
        numerator: 3,
        denominator: 2,
    };
    output
        .engine
        .execute_playback(light_engine::EnginePlaybackCommand::RestoreActiveDynamics(
            active,
        ))
        .unwrap();
    output
        .engine
        .execute_playback(light_engine::EnginePlaybackCommand::SetDynamicsPaused(true))
        .unwrap();
    let (mut expected, held) = {
        let mut live = output.dynamics.lock();
        sample(&mut live, 675);
        live.update_controller(controller, Some(0.6), Some(1.5), Some(45.0))
            .unwrap();
        sample(&mut live, 700);
        live.set_global_paused(true, 700);
        let held = sample(&mut live, 750);
        assert!(!held.is_empty());
        let expected = live.snapshot();
        assert!(!expected.instances[0].random_streams.is_empty());
        assert!(!expected.instances[0].synchronized_hold_values.is_empty());
        (expected, held)
    };
    expected.instances[0].definition = definition;
    output
        .finalize_snapshot(&playback(), prepared, || Ok(()))
        .unwrap();
    assert!(Arc::ptr_eq(&output.snapshot(), &expected_snapshot));
    let mut live = output.dynamics.lock();
    assert_eq!(
        live.snapshot(),
        expected,
        "the finalization lease captures current state, never preparation-time history"
    );
    assert_eq!(sample(&mut live, 900), held);
    assert!(output.dynamic_snapshot.matches(&expected_snapshot));
}

fn dynamic_on(
    definition: &DynamicDefinition,
    link: Uuid,
    size: f32,
) -> light_dynamics::DynamicSemanticValue {
    light_dynamics::DynamicSemanticValue::DynamicOn {
        instance_link: link,
        dynamic: light_dynamics::DynamicReference {
            dynamic_id: Some(definition.id),
            last_known_pool_number: definition.pool_number,
            embedded_fallback: light_dynamics::DynamicDefinitionSnapshot {
                definition: Arc::new(definition.clone()),
            },
        },
        lane_id: definition.lanes[0].id,
        overrides: light_dynamics::DynamicInstanceOverrides {
            size,
            speed_multiplier: Rational::ONE,
            phase_offset_degrees: 0.0,
        },
        timing: Default::default(),
    }
}

#[test]
fn invalid_programmer_controller_rejects_before_commit_and_retry_uses_current_rows() {
    let registry = light_programmer::ProgrammerRegistry::default();
    let session = light_core::SessionId::new();
    registry.start(session);
    let output = output_with_programmers(registry.clone());
    let mut definition = definition();
    definition.target_binding = light_dynamics::DynamicTargetBinding::Targetless;
    install_missing(&output, &definition);
    let fixture_id = FixtureId::new();
    let link = Uuid::new_v4();
    let set = |size| {
        registry.apply_dynamic_values(
            session,
            &[light_programmer::DynamicProgrammerValueMutation::Set {
                fixture_id,
                attribute: AttributeKey::intensity(),
                value: dynamic_on(&definition, link, size),
            }],
            None,
        );
    };
    set(-1.0);
    let before = output.snapshot();
    let history = output.dynamics.lock().snapshot();
    let mut next = before.as_ref().clone();
    next.revision += 1;
    let committed = Cell::new(false);
    let result = output.finalize_snapshot(
        &playback(),
        output.prepare_snapshot(next.clone()).unwrap(),
        || {
            committed.set(true);
            Ok(())
        },
    );
    assert!(
        result
            .unwrap_err()
            .message
            .contains("Dynamic controller reconciliation")
    );
    assert!(!committed.get());
    assert!(Arc::ptr_eq(&before, &output.snapshot()));
    assert_eq!(history, output.dynamics.lock().snapshot());
    assert!(output.dynamic_snapshot.matches(&before));

    let prepared = output.prepare_snapshot(next).unwrap();
    // The accepted retry must see this later correction, not the row from preparation time.
    set(0.4);
    output
        .finalize_snapshot(&playback(), prepared, || {
            committed.set(true);
            assert!(Arc::ptr_eq(&before, &output.snapshot()));
            Ok(())
        })
        .unwrap();
    assert!(committed.get());
    let live = output.dynamics.lock().snapshot();
    assert_eq!(live.instances.len(), 1);
    assert_eq!(live.instances[0].controllers[0].size, 0.4);
    assert_eq!(live.instances[0].targets, vec![fixture_id]);
    assert!(output.dynamic_snapshot.matches(&output.snapshot()));
}

fn cue_list(definition: &DynamicDefinition, fixture_id: FixtureId) -> light_playback::CueList {
    let mut cue = light_playback::Cue::new(1_u16.into());
    cue.dynamic_changes.push(light_playback::CueDynamicChange {
        fixture_id,
        attribute: AttributeKey::intensity(),
        value: dynamic_on(definition, Uuid::new_v4(), 0.7),
        automatic_restore: false,
    });
    light_playback::CueList {
        id: light_core::CueListId::new(),
        name: "Destination Cue owner".into(),
        priority: 0,
        mode: light_playback::CueListMode::Sequence,
        looped: false,
        chaser_step_millis: 1000,
        speed_group: None,
        intensity_priority_mode: light_playback::IntensityPriorityMode::Htp,
        wrap_mode: Some(light_playback::WrapMode::Off),
        restart_mode: light_playback::RestartMode::FirstCue,
        force_cue_timing: false,
        disable_cue_timing: false,
        auto_off_at_zero: false,
        auto_off_flash_release: false,
        chaser_xfade_millis: 0,
        chaser_xfade_percent: Some(0),
        speed_multiplier: 1.0,
        cues: vec![cue],
    }
}

#[test]
fn finalized_cue_scope_is_reconciled_before_persistence_and_failed_commit_keeps_live() {
    let output = output();
    let [first, second] = [FixtureId::new(), FixtureId::new()];
    let mut dynamic = definition();
    dynamic.target_binding = light_dynamics::DynamicTargetBinding::FrozenTargets {
        targets: vec![first],
    };
    let list = cue_list(&dynamic, first);
    let list_id = list.id;
    output
        .replace_snapshot(EngineSnapshot {
            revision: 1,
            dynamics: vec![dynamic.clone()].into(),
            cue_lists: vec![list].into(),
            ..Default::default()
        })
        .unwrap();
    output
        .engine
        .execute_playback(light_engine::EnginePlaybackCommand::CueList {
            id: list_id,
            action: light_engine::CueListPlaybackAction::Go,
        })
        .unwrap();
    assert_eq!(output.engine.active_cue_dynamic_values().len(), 1);
    output.reconcile_dynamic_runtime();
    let before = output.snapshot();
    let (mut cold_cursor, mut control_cursor) = output
        .dynamic_snapshot
        .begin_retained_history(
            &mut output.dynamics.lock(),
            &before,
            std::num::NonZeroUsize::new(32).unwrap(),
        )
        .unwrap();
    let mut pending = output.dynamics.lock().fork_for_pending_preview();
    let mut pending_snapshot = before.clone();
    let history = output.dynamics.lock().snapshot();
    assert_eq!(history.instances.len(), 1);
    assert_eq!(history.instances[0].targets, vec![first]);
    let rows = output.engine.active_cue_dynamic_values();
    dynamic.target_binding = light_dynamics::DynamicTargetBinding::FrozenTargets {
        targets: vec![second],
    };
    dynamic.revision += 1;
    let mut next = before.as_ref().clone();
    next.revision += 1;
    next.dynamics = vec![dynamic].into();
    let result: Result<(), ActionError> = output.finalize_snapshot(
        &playback(),
        output.prepare_snapshot(next.clone()).unwrap(),
        || {
            assert!(Arc::ptr_eq(&before, &output.snapshot()));
            Err(ActionError::new(
                ActionErrorKind::Invalid,
                "test commit rejected",
            ))
        },
    );
    assert_eq!(result.unwrap_err().message, "test commit rejected");
    assert!(
        output
            .dynamic_snapshot
            .cold_generations_since(cold_cursor)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        output.dynamics.lock().control_cursor(),
        Some(control_cursor)
    );
    assert!(Arc::ptr_eq(&before, &output.snapshot()));
    assert_eq!(history, output.dynamics.lock().snapshot());
    assert_eq!(rows, output.engine.active_cue_dynamic_values());
    assert!(output.dynamic_snapshot.matches(&before));
    output
        .finalize_snapshot(&playback(), output.prepare_snapshot(next).unwrap(), || {
            Ok(())
        })
        .unwrap();
    let live = output.dynamics.lock().snapshot();
    assert_eq!(live.instances.len(), 1);
    assert_eq!(live.instances[0].id, history.instances[0].id);
    assert_eq!(live.instances[0].targets, vec![second]);
    assert_eq!(
        live.instances[0].started_at_millis,
        history.instances[0].started_at_millis
    );
    assert_eq!(
        live.instances[0].controllers,
        history.instances[0].controllers
    );
    assert_eq!(rows, output.engine.active_cue_dynamic_values());
    assert!(output.dynamic_snapshot.matches(&output.snapshot()));
    let events = output
        .dynamic_snapshot
        .cold_generations_since(cold_cursor)
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].control_boundary().unwrap(), control_cursor);
    events[0]
        .apply(
            &mut pending,
            &mut pending_snapshot,
            &mut control_cursor,
            &mut cold_cursor,
            &mut Default::default(),
        )
        .unwrap();
    assert!(Arc::ptr_eq(&pending_snapshot, &output.snapshot()));
    assert_eq!(pending.snapshot(), live);
    assert_eq!(
        Some(control_cursor),
        output.dynamics.lock().control_cursor()
    );
}

#[test]
fn successful_empty_cold_commits_have_distinct_events_without_control_changes() {
    let output = output();
    let mut snapshot = output.snapshot();
    let (mut generation, mut controls) = output
        .dynamic_snapshot
        .begin_retained_history(
            &mut output.dynamics.lock(),
            &snapshot,
            std::num::NonZeroUsize::new(8).unwrap(),
        )
        .unwrap();
    let initial_controls = controls;
    let mut pending = output.dynamics.lock().fork_for_pending_preview();
    for revision in [1, 2] {
        let mut destination = output.snapshot().as_ref().clone();
        destination.revision = revision;
        output
            .finalize_snapshot(
                &playback(),
                output.prepare_snapshot(destination).unwrap(),
                || {
                    assert!(
                        output
                            .dynamic_snapshot
                            .cold_generations_since(generation)
                            .unwrap()
                            .len()
                            < revision as usize
                    );
                    Ok(())
                },
            )
            .unwrap();
    }
    let events = output
        .dynamic_snapshot
        .cold_generations_since(generation)
        .unwrap();
    assert_eq!(events.len(), 2);
    assert_ne!(events[0].to, events[1].to);
    for event in events {
        event
            .apply(
                &mut pending,
                &mut snapshot,
                &mut controls,
                &mut generation,
                &mut Default::default(),
            )
            .unwrap();
        assert_eq!(controls, initial_controls);
    }
    assert!(Arc::ptr_eq(&snapshot, &output.snapshot()));
}

#[test]
fn preset_tables_follow_destination_definition_and_publish_only_after_commit() {
    let output = output();
    let address = DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Focus,
        component: Some(light_core::programming::ProgrammingComponent::Focus),
    };
    let mut definition = definition();
    let with_level = |definition: &mut DynamicDefinition, level| {
        definition.lanes[0].body = DynamicLaneBody::Programming(ProgrammingLaneBody {
            address: address.clone(),
            configuration: ProgrammingLaneConfiguration::MaxMin(MaxMinConfiguration {
                minimum: DynamicValueSource::Value {
                    value: DynamicValue::Scalar(0.0),
                },
                maximum: DynamicValueSource::Preset {
                    preset_id: "6.1".into(),
                    address: address.clone(),
                    last_valid_by_target: vec![],
                    retained: Some(Arc::new(light_dynamics::DynamicPresetTemplate {
                        universal: Some(light_core::AttributeValue::Normalized(level)),
                        ..Default::default()
                    })),
                },
                function: PeriodicFunction::LinearUp,
                size: 1.0,
                pwm: Default::default(),
            }),
        });
    };
    with_level(&mut definition, 0.3);
    install_missing(&output, &definition);
    let (instance, _) = start(&output, &definition);
    let initial = output.snapshot().as_ref().clone();
    output
        .finalize_snapshot(
            &playback(),
            output.prepare_snapshot(initial).unwrap(),
            || Ok(()),
        )
        .unwrap();
    let before = output.snapshot();
    let runtime_before = output.dynamics.lock().snapshot();
    let sources_before = output.dynamics.lock().preset_source_instances();
    let (mut generation, mut controls) = output
        .dynamic_snapshot
        .begin_retained_history(
            &mut output.dynamics.lock(),
            &before,
            std::num::NonZeroUsize::new(32).unwrap(),
        )
        .unwrap();
    let mut pending = output.dynamics.lock().fork_for_pending_preview();
    let mut pending_snapshot = before.clone();
    assert_eq!(sources_before.len(), 1);
    assert_eq!(
        sources_before[0].last_valid[0].values[0].value,
        DynamicValue::Scalar(0.3)
    );

    definition.revision += 1;
    with_level(&mut definition, 0.8);
    let mut next = before.as_ref().clone();
    next.revision += 1;
    next.dynamics = vec![definition].into();
    let result: Result<(), ActionError> = output.finalize_snapshot(
        &playback(),
        output.prepare_snapshot(next.clone()).unwrap(),
        || {
            Err(ActionError::new(
                ActionErrorKind::Invalid,
                "test preset commit rejected",
            ))
        },
    );
    assert_eq!(result.unwrap_err().message, "test preset commit rejected");
    assert!(
        output
            .dynamic_snapshot
            .cold_generations_since(generation)
            .unwrap()
            .is_empty()
    );
    assert!(Arc::ptr_eq(&output.snapshot(), &before));
    assert_eq!(output.dynamics.lock().snapshot(), runtime_before);
    let after_failure = output.dynamics.lock().preset_source_instances();
    assert_eq!(after_failure.len(), sources_before.len());
    assert_eq!(
        after_failure[0].dependency_generation,
        sources_before[0].dependency_generation
    );
    assert_eq!(after_failure[0].last_valid, sources_before[0].last_valid);
    assert!(output.dynamic_snapshot.matches(&before));
    output
        .finalize_snapshot(&playback(), output.prepare_snapshot(next).unwrap(), || {
            Ok(())
        })
        .unwrap();
    let sources = output.dynamics.lock().preset_source_instances();
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0].instance_id, instance);
    assert_eq!(
        sources[0].last_valid[0].values[0].value,
        DynamicValue::Scalar(0.8)
    );
    let events = output
        .dynamic_snapshot
        .cold_generations_since(generation)
        .unwrap();
    assert_eq!(events.len(), 1);
    events[0]
        .apply(
            &mut pending,
            &mut pending_snapshot,
            &mut controls,
            &mut generation,
            &mut Default::default(),
        )
        .unwrap();
    assert!(Arc::ptr_eq(&pending_snapshot, &output.snapshot()));
    assert_eq!(
        pending.preset_source_instances()[0].last_valid,
        sources[0].last_valid
    );
    let live = output.dynamics.lock().snapshot();
    assert_eq!(
        live.instances[0].controllers,
        runtime_before.instances[0].controllers
    );
    assert_eq!(
        live.instances[0].started_at_millis,
        runtime_before.instances[0].started_at_millis
    );
    assert!(output.dynamic_snapshot.matches(&output.snapshot()));
}
