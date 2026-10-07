//! Exercise the real checkpoint publication seam, including dependency preparation before IDs
//! are reserved. Detached helper tests alone cannot prove these publication properties.
use super::publication_tests::output;
use super::*;
use crate::runtime::dynamic_source_origins::*;
use light_core::programming::{
    PositionIntent, ProgrammingComponent, ProgrammingOwner, ScalarIntent,
};
use light_core::{AttributeKey, AttributeValue, FixtureId, NativeColorBinding};
use light_dynamics::{
    DynamicController, DynamicControllerSource, DynamicDefinition, DynamicFamilyRepresentation,
    DynamicLaneBody, DynamicPresetGroupTemplate, DynamicPresetTemplate, DynamicRuntime,
    DynamicRuntimeSnapshot, DynamicSpeedTransport, DynamicStartRequest, DynamicTargetScope,
    DynamicValue, DynamicValueAddress, DynamicValueSource, DynamicValueSourceResolver,
    MaxMinConfiguration, PeriodicFunction, Position3d, ProgrammingLaneBody,
    ProgrammingLaneConfiguration, RankDirection, ScalarSourceResolver, SpatialPosition,
    SpatialProjection, SpatialSelectionMapping, SpatialSelectionShape,
};
use light_engine::{NativeColorSourceCatalog, NativeColorSourceRevisionKey};
use light_fixture::{ChannelFunction, FixtureProfile};
use light_programmer::{GroupDefinition, GroupFixtureSource};
use std::num::NonZeroUsize;

fn definition(targets: &[FixtureId]) -> DynamicDefinition {
    serde_json::from_value(serde_json::json!({
        "id": Uuid::new_v4(), "pool_number": 1, "revision": 1, "name": "Restore publication",
        "target_binding": {"type": "frozen_targets", "targets": targets},
        "lanes": [{
            "id": Uuid::new_v4(), "attribute": "pan", "mode": "keyframes",
            "keyframes": {"points": [
                {"position": 0.0, "source": {"type": "value", "value": 0.0}, "interpolation": "linear"}
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
    })).unwrap()
}

fn pan_preset(targets: &[FixtureId]) -> DynamicDefinition {
    let mut definition = definition(targets);
    let address = DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Angles,
        component: Some(ProgrammingComponent::Pan),
    };
    let source = DynamicValueSource::Preset {
        preset_id: "3.1".into(),
        address: address.clone(),
        last_valid_by_target: vec![],
        retained: Some(Arc::new(DynamicPresetTemplate {
            groups: vec![DynamicPresetGroupTemplate {
                group_id: "front".into(),
                value: AttributeValue::Position(Arc::new(PositionIntent::Angles {
                    pan_degrees: ScalarIntent::Spread(vec![0., 100.]),
                    tilt_degrees: ScalarIntent::Value(0.),
                })),
            }],
            ..Default::default()
        })),
    };
    definition.lanes[0].body = DynamicLaneBody::Programming(ProgrammingLaneBody {
        address,
        configuration: ProgrammingLaneConfiguration::MaxMin(MaxMinConfiguration {
            minimum: source.clone(),
            maximum: source,
            function: PeriodicFunction::LinearUp,
            size: 1.,
            pwm: Default::default(),
        }),
    });
    definition
}

fn start(runtime: &mut DynamicRuntime, definition: &DynamicDefinition, targets: &[FixtureId]) {
    runtime.install_definitions([definition.clone()]).unwrap();
    runtime
        .start(DynamicStartRequest {
            definition_id: definition.id,
            controller: DynamicController {
                id: Uuid::new_v4(),
                source: DynamicControllerSource::physical_playback(1),
                priority: 100,
                activated_at_millis: 5,
                size: 1.,
                speed_multiplier: 1.,
                phase_offset_degrees: 0.,
                paused: false,
            },
            target_scope: DynamicTargetScope {
                ordered_targets: targets.to_vec(),
            },
            stage_positions: HashMap::new(),
            inherited_spatial_mapping: None,
            now_millis: 5,
            activation_delay_millis: 0,
            activation_duration_millis: 0,
            activation_policy_override: None,
            reuse_matching_targetless: false,
        })
        .unwrap();
}

fn show(targets: &[FixtureId], spatial: bool, reversed: bool) -> EngineSnapshot {
    let mut group = GroupDefinition {
        id: "front".into(),
        name: "Front".into(),
        source: Some(GroupFixtureSource::Explicit {
            fixture_ids: if reversed && !spatial {
                targets.iter().rev().copied().collect()
            } else {
                targets.to_vec()
            },
        }),
        ..Default::default()
    };
    if spatial {
        group.mapping = Some(SpatialSelectionMapping {
            projection: SpatialProjection::from_preset(
                light_dynamics::ProjectionPreset::Top,
                Position3d::default(),
            ),
            shape: SpatialSelectionShape::Grid {
                angle_degrees: 0.,
                direction: RankDirection::Ascending,
            },
        });
    }
    EngineSnapshot {
        groups: vec![group].into(),
        dynamic_stage_positions: Arc::new(
            targets
                .iter()
                .enumerate()
                .map(|(index, target)| {
                    let rank = if reversed {
                        targets.len() - 1 - index
                    } else {
                        index
                    };
                    (
                        *target,
                        SpatialPosition {
                            x: rank as f32 * 10.,
                            y: 0.,
                            z: 0.,
                        },
                    )
                })
                .collect(),
        ),
        ..Default::default()
    }
}

fn table(runtime: &DynamicRuntime) -> HashMap<FixtureId, f32> {
    runtime
        .preset_source_instances()
        .into_iter()
        .flat_map(|instance| instance.last_valid)
        .flat_map(|record| record.values)
        .map(|record| {
            let DynamicValue::Scalar(value) = record.value else {
                panic!("expected Pan scalar")
            };
            (record.target, value)
        })
        .collect()
}

fn without_tables(mut snapshot: DynamicRuntimeSnapshot) -> DynamicRuntimeSnapshot {
    for instance in &mut snapshot.instances {
        instance.preset_source_values.clear();
    }
    snapshot
}

struct Sources;
impl ScalarSourceResolver for Sources {
    fn current(&self, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        Some(0.)
    }
    fn preset(&self, _: &str, _: FixtureId, _: &AttributeKey) -> Option<f32> {
        None
    }
}
impl DynamicValueSourceResolver for Sources {
    fn current(&self, _: FixtureId, _: &DynamicValueAddress) -> Option<DynamicValue> {
        None
    }
    fn preset(
        &self,
        _: &light_dynamics::DynamicPresetSourceBinding,
        _: Uuid,
        _: FixtureId,
    ) -> Option<DynamicValue> {
        None
    }
}

fn restores_changed_group(spatial: bool) {
    let output = output();
    let targets = [FixtureId::new(), FixtureId::new()];
    let definition = pan_preset(&targets);
    let initial = show(&targets, spatial, false);
    output.replace_snapshot(initial.clone()).unwrap();
    {
        let mut runtime = output.dynamics.lock();
        start(&mut runtime, &definition, &targets);
        output_scheduler::materialize_cold_preset_dependencies(&initial, &initial, &mut runtime)
            .unwrap();
        assert_eq!(
            table(&runtime),
            [(targets[0], 0.), (targets[1], 100.)].into()
        );
    }
    let checkpoint = output.dynamic_source_checkpoint().unwrap();
    let stored = checkpoint.runtime.clone();
    let beginning = output
        .dynamics
        .lock()
        .begin_control_recording(NonZeroUsize::new(8).unwrap());
    output
        .replace_snapshot(show(&targets, spatial, true))
        .unwrap();
    let (cold_cursor, _) = output
        .dynamic_snapshot
        .begin_retained_history(
            &mut output.dynamics.lock(),
            &output.snapshot(),
            NonZeroUsize::new(8).unwrap(),
        )
        .unwrap();
    output
        .restore_dynamic_source_checkpoint(checkpoint)
        .unwrap();
    let mut runtime = output.dynamics.lock();
    assert!(
        output
            .dynamic_snapshot
            .cold_generations_since(cold_cursor)
            .is_err()
    );
    assert_eq!(
        table(&runtime),
        [(targets[0], 100.), (targets[1], 0.)].into()
    );
    assert!(!runtime.has_pending_preset_sources());
    assert_eq!(without_tables(runtime.snapshot()), without_tables(stored));
    assert_eq!(
        runtime.committed_sample_boundary(),
        None,
        "restore must prepare without sampling"
    );
    assert!(matches!(
        runtime.controls_since(beginning),
        Some(Err(light_dynamics::DynamicControlLogError::WrongEpoch))
    ));
    let transport = DynamicSpeedTransport {
        effective_bpm: 60.,
        phase_origin_millis: 0,
        phase_reference_millis: 250,
        beat_phase: 0.25,
        phase_advancing: true,
    };
    let first: HashMap<_, _> = runtime
        .sample_all_programming_addressed(250, 10, &[transport; 5], &Sources, &Sources, None)
        .unwrap()
        .into_iter()
        .filter_map(|sample| {
            sample
                .expression
                .programming_leaf()
                .map(|(_, value)| (sample.target, value.clone()))
        })
        .collect();
    assert_eq!(
        first,
        [
            (targets[0], DynamicValue::Scalar(100.)),
            (targets[1], DynamicValue::Scalar(0.))
        ]
        .into()
    );
}

#[test]
fn checkpoint_publication_rebuilds_changed_group_order_before_first_sample() {
    restores_changed_group(false);
}

#[test]
fn checkpoint_publication_rebuilds_changed_spatial_ranks_before_first_sample() {
    restores_changed_group(true);
}

fn uv_profile() -> FixtureProfile {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "Restore dependency validation".into();
    profile.revision = 1;
    let mode = &mut profile.modes[0];
    let channel: light_fixture::FixtureChannel = serde_json::from_value(serde_json::json!({
        "id": Uuid::new_v4(), "head_id": mode.heads[0].id, "split": 1,
        "fixture_attribute": "color.uv", "attribute": "color.uv", "resolution": "u8",
        "default_raw": 0, "highlight_raw": 255,
        "functions": [ChannelFunction::continuous("UV", AttributeKey("color.uv".into()), 255)]
    }))
    .unwrap();
    mode.color_physical = Some(light_fixture::ColorPhysicalModel {
        version: 1,
        revision: 1,
        paths: vec![light_fixture::HeadOpticalPath {
            id: Uuid::new_v4(),
            head_id: mode.heads[0].id,
            controls: vec![channel.id],
            source: light_fixture::OpticalSource::Additive {
                emitters: vec![light_fixture::OpticalEmitter {
                    id: Uuid::new_v4(),
                    name: "UV".into(),
                    binding: light_fixture::NativeColorBinding {
                        channel_id: channel.id,
                        function_id: channel.functions[0].id,
                    },
                    xyz: Some(light_core::Xyz {
                        x: 0.1,
                        y: 0.2,
                        z: 0.3,
                    }),
                    spectrum: vec![],
                    band: light_fixture::OpticalEmitterBand::Ultraviolet,
                    native_reversed: false,
                    maximum_level: 1.,
                    response_exponent: 1.,
                    provenance: Default::default(),
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

fn native_checkpoint(profile: &FixtureProfile) -> DynamicRuntimeSourceCheckpoint {
    let targets = [FixtureId::new()];
    let mut definition = definition(&targets);
    let mode = &profile.modes[0];
    let channel = &mode.channels[0];
    definition.lanes[0].body = DynamicLaneBody::Programming(ProgrammingLaneBody {
        address: DynamicValueAddress {
            representation: DynamicFamilyRepresentation::DirectColor {
                source: profile
                    .native_color_identity(mode.id, mode.heads[0].id)
                    .unwrap(),
            },
            component: Some(ProgrammingComponent::NativeColor(NativeColorBinding {
                channel_id: channel.id,
                function_id: channel.functions[0].id,
            })),
        },
        configuration: ProgrammingLaneConfiguration::MaxMin(MaxMinConfiguration {
            minimum: DynamicValueSource::Value {
                value: DynamicValue::Native(0),
            },
            maximum: DynamicValueSource::Value {
                value: DynamicValue::Native(1_000),
            },
            function: PeriodicFunction::LinearUp,
            size: 1.,
            pwm: Default::default(),
        }),
    });
    let mut runtime = DynamicRuntime::default();
    start(&mut runtime, &definition, &targets);
    let mut origins = DynamicSourceOrigins::default();
    let binding = DynamicSourceBinding::StaticBaseline {
        target: targets[0],
        owner: ProgrammingOwner::Focus,
    };
    origins
        .bind(
            binding,
            DynamicSourceOrigin::StaticBaseline {
                sources: vec![DynamicStaticSourceEntry {
                    source: DynamicStaticSource::Playback {
                        source: DynamicSequenceSource {
                            playback_number: Some(1),
                            playback_identity: Some(
                                light_playback::PlaybackIdentity::physical(1).unwrap(),
                            ),
                            cue_list_id: light_core::CueListId::new(),
                            temporary: false,
                        },
                    },
                    changed_at: chrono::DateTime::from_timestamp_millis(100).unwrap(),
                    programmer_order: 0,
                    transition_ordinal: Some(900),
                    authored_cue_id: None,
                    footprint: DynamicStaticFootprint::Whole,
                    role: DynamicStaticRole::Authored,
                    effective_fields: None,
                }],
            },
        )
        .unwrap();
    origins.unbind(&binding);
    DynamicRuntimeSourceCheckpoint {
        runtime: runtime.snapshot(),
        origins: Some(origins.snapshot()),
    }
}

#[test]
fn unavailable_original_restores_passively_but_verified_invalid_dependency_never_publishes() {
    let profile = uv_profile();
    let checkpoint = native_checkpoint(&profile);
    let passive = output();
    passive
        .restore_dynamic_source_checkpoint(checkpoint.clone())
        .unwrap();
    assert_eq!(passive.dynamic_runtime_snapshot().instances.len(), 1);
    assert!(
        !passive
            .dynamics
            .lock()
            .unavailable_native_sources()
            .is_empty()
    );
    assert_eq!(passive.engine.playback_source_occurrence_watermark(), 900);

    let output = output();
    let catalog =
        NativeColorSourceCatalog::from_revisions([NativeColorSourceCatalog::compile_revision(
            NativeColorSourceRevisionKey {
                profile_id: profile.id,
                revision: u64::from(profile.revision),
                raw_store_digest: format!("restore-{}-{}", profile.id.0, profile.revision),
            },
            None,
            || Ok(profile.clone()),
        )])
        .unwrap();
    output
        .replace_snapshot(EngineSnapshot {
            native_color_sources: Arc::new(catalog),
            ..Default::default()
        })
        .unwrap();
    let (cold_cursor, beginning) = output
        .dynamic_snapshot
        .begin_retained_history(
            &mut output.dynamics.lock(),
            &output.snapshot(),
            NonZeroUsize::new(8).unwrap(),
        )
        .unwrap();
    output.set_dynamic_runtime_paused(true);
    let before = output.dynamic_runtime_snapshot();
    let cursor = output.dynamics.lock().control_cursor();
    let origins = output.dynamic_source_origins.load_full();
    let engine = output.snapshot();
    let watermark = output.engine.playback_source_occurrence_watermark();
    let error = output
        .restore_dynamic_source_checkpoint(checkpoint)
        .unwrap_err();
    assert!(!error.to_string().is_empty());
    assert_eq!(output.dynamic_runtime_snapshot(), before);
    assert_eq!(output.dynamics.lock().control_cursor(), cursor);
    assert!(
        output
            .dynamic_snapshot
            .cold_generations_since(cold_cursor)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        output
            .dynamics
            .lock()
            .controls_since(beginning)
            .unwrap()
            .unwrap()
            .len(),
        1
    );
    assert!(Arc::ptr_eq(
        &output.dynamic_source_origins.load_full(),
        &origins
    ));
    assert!(Arc::ptr_eq(&output.snapshot(), &engine));
    assert_eq!(
        output.engine.playback_source_occurrence_watermark(),
        watermark,
        "dependency failure must precede occurrence reservation"
    );
}

fn destination_with_original(profile: &FixtureProfile) -> EngineSnapshot {
    let catalog =
        NativeColorSourceCatalog::from_revisions([NativeColorSourceCatalog::compile_revision(
            NativeColorSourceRevisionKey {
                profile_id: profile.id,
                revision: u64::from(profile.revision),
                raw_store_digest: format!("prepared-restore-{}-{}", profile.id.0, profile.revision),
            },
            None,
            || Ok(profile.clone()),
        )])
        .unwrap();
    EngineSnapshot {
        revision: 42,
        native_color_sources: Arc::new(catalog),
        ..Default::default()
    }
}

#[test]
fn obsolete_unavailable_native_runtime_cannot_veto_an_incoming_empty_checkpoint() {
    let profile = uv_profile();
    let output = output();
    output
        .restore_dynamic_source_checkpoint(native_checkpoint(&profile))
        .unwrap();
    assert_eq!(output.dynamic_runtime_snapshot().instances.len(), 1);
    assert!(
        !output
            .dynamics
            .lock()
            .unavailable_native_sources()
            .is_empty()
    );
    // The old controller requests 1000 from an 8-bit channel. It belongs only to the
    // outgoing runtime and must be gone before destination-native validation is attempted.
    output
        .replace_snapshot(destination_with_original(&profile))
        .unwrap();
    let cursor = output
        .dynamics
        .lock()
        .begin_control_recording(NonZeroUsize::new(8).unwrap());
    output
        .restore_dynamic_source_checkpoint(DynamicRuntimeSourceCheckpoint {
            runtime: Default::default(),
            origins: None,
        })
        .unwrap();
    assert!(output.dynamic_runtime_snapshot().instances.is_empty());
    assert!(
        output
            .dynamic_source_origins
            .load()
            .snapshot()
            .records
            .is_empty()
    );
    assert!(matches!(
        output.dynamics.lock().controls_since(cursor),
        Some(Err(light_dynamics::DynamicControlLogError::WrongEpoch))
    ));
    assert_eq!(
        output.engine.playback_source_occurrence_watermark(),
        900,
        "existing reservations remain monotonic"
    );
}

#[test]
fn rejected_prepared_show_restore_leaves_engine_runtime_origins_and_reservations_untouched() {
    let profile = uv_profile();
    let output = output();
    let beginning = output
        .dynamics
        .lock()
        .begin_control_recording(NonZeroUsize::new(8).unwrap());
    output.set_dynamic_runtime_paused(true);
    let before = output.dynamic_runtime_snapshot();
    let cursor = output.dynamics.lock().control_cursor();
    let origins = output.dynamic_source_origins.load_full();
    let engine = output.snapshot();
    let watermark = output.engine.playback_source_occurrence_watermark();
    let prepared = output
        .prepare_snapshot(destination_with_original(&profile))
        .unwrap();
    let error = output
        .prepare_snapshot_restore(prepared, native_checkpoint(&profile))
        .unwrap_err();
    assert!(!error.to_string().is_empty());
    assert!(Arc::ptr_eq(&output.snapshot(), &engine));
    assert!(output.dynamic_snapshot.matches(&engine));
    assert_eq!(output.dynamic_runtime_snapshot(), before);
    assert_eq!(output.dynamics.lock().control_cursor(), cursor);
    assert_eq!(
        output
            .dynamics
            .lock()
            .controls_since(beginning)
            .unwrap()
            .unwrap()
            .len(),
        1
    );
    assert!(Arc::ptr_eq(
        &output.dynamic_source_origins.load_full(),
        &origins
    ));
    assert_eq!(
        output.engine.playback_source_occurrence_watermark(),
        watermark
    );
}

#[test]
fn prepared_release_publishes_the_exact_validated_runtime_origins_and_engine_together() {
    let profile = uv_profile();
    let mut checkpoint = native_checkpoint(&profile);
    let DynamicLaneBody::Programming(body) =
        &mut checkpoint.runtime.instances[0].definition.lanes[0].body
    else {
        unreachable!()
    };
    let ProgrammingLaneConfiguration::MaxMin(configuration) = &mut body.configuration else {
        unreachable!()
    };
    configuration.maximum = DynamicValueSource::Value {
        value: DynamicValue::Native(200),
    };
    let output = output();
    let beginning = output
        .dynamics
        .lock()
        .begin_control_recording(NonZeroUsize::new(8).unwrap());
    output.set_dynamic_runtime_paused(true);
    let old_cursor = output.dynamics.lock().control_cursor();
    let old_runtime = output.dynamic_runtime_snapshot();
    let old_engine = output.snapshot();
    let old_origins = output.dynamic_source_origins.load_full();
    let mut destination = destination_with_original(&profile);
    destination.dynamics = vec![checkpoint.runtime.instances[0].definition.clone()].into();
    let prepared = output.prepare_snapshot(destination).unwrap();
    let prepared = output
        .prepare_snapshot_restore(prepared, checkpoint)
        .unwrap();
    let expected_engine = prepared.engine.snapshot_arc();
    let restored = prepared
        .restored
        .as_ref()
        .expect("prepared token retains the checkpoint");
    let expected_runtime = restored.runtime.snapshot();
    let expected_origins = restored.origins.snapshot();
    let expected_cursor = restored.runtime.control_cursor();
    assert!(!restored.runtime.has_pending_preset_sources());
    assert!(restored.runtime.unavailable_native_sources().is_empty());
    assert_eq!(restored.playback_source_occurrence_watermark, 900);
    // Preparation alone must not reserve an occurrence or switch any published state.
    assert_eq!(output.engine.playback_source_occurrence_watermark(), 0);
    assert!(Arc::ptr_eq(&output.snapshot(), &old_engine));
    assert_eq!(output.dynamic_runtime_snapshot(), old_runtime);
    assert_eq!(output.dynamics.lock().control_cursor(), old_cursor);
    assert!(Arc::ptr_eq(
        &output.dynamic_source_origins.load_full(),
        &old_origins
    ));

    output.install_prepared_snapshot_releasing_playback(prepared);
    assert!(Arc::ptr_eq(&output.snapshot(), &expected_engine));
    assert!(output.dynamic_snapshot.matches(&expected_engine));
    assert_eq!(output.dynamic_runtime_snapshot(), expected_runtime);
    assert_eq!(
        output.dynamic_source_origins.load().snapshot(),
        expected_origins
    );
    assert_eq!(output.dynamics.lock().control_cursor(), expected_cursor);
    assert!(matches!(
        output.dynamics.lock().controls_since(beginning),
        Some(Err(light_dynamics::DynamicControlLogError::WrongEpoch))
    ));
    assert_eq!(output.engine.playback_source_occurrence_watermark(), 900);
    assert!(
        !output.dynamic_runtime_snapshot().global_paused,
        "the incoming checkpoint replaces outgoing pause state"
    );
}
