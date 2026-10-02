use super::*;
use light_core::programming::{
    ColorProgram, NativeColorRecipe, PositionIntent, ProgrammingComponent, ScalarIntent,
};
use light_core::{
    AttributeKey, AttributeValue, FixtureId, NativeColorBinding, NativeColorIdentity,
    NativeColorValue, Xyz,
};
use light_dynamics::{
    DynamicController, DynamicControllerSource, DynamicDefinition, DynamicFamilyRepresentation,
    DynamicLaneBody, DynamicPresetGroupTemplate, DynamicPresetTemplate, DynamicRuntimeSnapshot,
    DynamicSpeedTransport, DynamicStartRequest, DynamicTargetScope, DynamicValue,
    DynamicValueAddress, DynamicValueSource, DynamicValueSourceResolver, MaxMinConfiguration,
    PeriodicFunction, Position3d, ProgrammingLaneBody, ProgrammingLaneConfiguration, PwmShape,
    RankDirection, ScalarSourceResolver, SpatialPosition, SpatialProjection,
    SpatialSelectionMapping, SpatialSelectionShape,
};
use light_engine::{NativeColorSourceCatalog, NativeColorSourceRevisionKey};
use light_fixture::{
    CanonicalTransform, ChannelBehavior, ChannelFunction, ChannelResolution, ColorPhysicalModel,
    FixtureChannel, FixtureProfile, HeadOpticalPath, NativeColorBinding as FixtureNativeBinding,
    OpticalEmitter, OpticalEmitterBand, OpticalProvenance, OpticalSource,
};
use light_programmer::{GroupDefinition, GroupFixtureSource, GroupReference, SelectionRule};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

#[test]
fn semantic_checkpoint_restores_fresh_tables_without_resetting_state_and_first_sample_uses_them() {
    let [a, b] = [FixtureId::new(), FixtureId::new()];
    let definition = pan_definition(&[a, b], group_template("front", vec![0.0, 100.0]));
    let previous = snapshot(vec![spatial_group("front", &[a, b])], [(a, 0.0), (b, 10.0)]);
    let live = running(&previous, &definition);
    assert_eq!(table(&live), [(a, 0.0), (b, 100.0)].into());
    let stored = live.snapshot();
    let live_before = (live.snapshot(), live.preset_source_instances().len());

    // The destination moved both fixtures, so the Group ranks are swapped.
    let destination = snapshot(vec![spatial_group("front", &[a, b])], [(a, 10.0), (b, 0.0)]);
    let mut prepared = prepare_restored_dynamic_candidate(
        &destination,
        &DynamicRuntime::default(),
        checkpoint(&stored),
    )
    .unwrap();

    assert_eq!(table(&prepared.runtime), [(a, 100.0), (b, 0.0)].into());
    assert!(!prepared.runtime.has_pending_preset_sources());
    assert_eq!(
        without_tables(prepared.runtime.snapshot()),
        without_tables(stored)
    );
    assert_eq!(prepared.playback_source_occurrence_watermark, 0);
    assert!(prepared.origins.snapshot().bindings.is_empty());
    assert!(prepared.presets.missing_groups.is_empty());
    let sampled = first_typed_sample(&mut prepared.runtime);
    assert_eq!(
        sampled,
        [
            (a, DynamicValue::Scalar(100.0)),
            (b, DynamicValue::Scalar(0.0))
        ]
        .into()
    );
    assert_eq!(
        (live.snapshot(), live.preset_source_instances().len()),
        live_before
    );
}

#[test]
fn direct_preset_uses_the_exact_original_and_a_changed_provider_never_substitutes_it() {
    let original = profile(1);
    let fixture = FixtureId::new();
    let definition = direct_preset_definition(&original, fixture, 200);
    // Captured on a desk without the original model: the lane was suspended.
    let stored = running(&snapshot(Vec::new(), []), &definition).snapshot();

    let available = with_catalogue(&[&original]);
    let mut prepared = prepare_restored_dynamic_candidate(
        &available,
        &DynamicRuntime::default(),
        checkpoint(&stored),
    )
    .unwrap();
    assert_eq!(table(&prepared.runtime), [(fixture, 200.0)].into());
    assert!(prepared.presets.source_quality.is_empty());
    assert_eq!(
        first_typed_sample(&mut prepared.runtime),
        [(fixture, DynamicValue::Native(200))].into()
    );

    // Same profile, newer revision: a different original identity. It is not substituted.
    let mut revised = original.clone();
    revised.revision = 2;
    let changed = with_catalogue(&[&revised]);
    let prepared = prepare_restored_dynamic_candidate(
        &changed,
        &DynamicRuntime::default(),
        checkpoint(&stored),
    )
    .unwrap();
    assert!(table(&prepared.runtime).is_empty());
    assert!(!prepared.presets.source_quality.is_empty());
    assert!(
        prepared
            .presets
            .source_quality
            .iter()
            .all(|quality| matches!(
                quality.issue.reason,
                light_application::DynamicPresetSourceIssueReason::NativeUnavailable(_)
            ))
    );
}

#[test]
fn available_original_rejects_invalid_native_values_but_an_unavailable_one_stays_passive() {
    let original = profile(1);
    let definition = native_value_definition(&original, FixtureId::new(), 1_000);
    let stored = running(&snapshot(Vec::new(), []), &definition).snapshot();

    let unavailable = snapshot(Vec::new(), []);
    assert!(
        prepare_restored_dynamic_candidate(
            &unavailable,
            &DynamicRuntime::default(),
            checkpoint(&stored)
        )
        .is_ok()
    );

    let base = DynamicRuntime::default();
    let identity = identity(&original);
    let error = prepare_restored_dynamic_candidate(
        &with_catalogue(&[&original]),
        &base,
        checkpoint(&stored),
    )
    .err()
    .expect("an available original validates retained native values");
    assert!(
        matches!(error, RestoredDynamicCandidateError::Runtime(_)),
        "{error}"
    );
    // The failed candidate pinned the verified original only on its detached pins.
    assert!(matches!(
        base.captured_native_color_models()
            .resolve_capability(&identity),
        Ok(light_dynamics::NativeColorModelCapability::Unavailable(_))
    ));
    assert!(base.snapshot().instances.is_empty());
}

#[test]
fn absent_group_is_passive_but_an_invalid_existing_group_fails_the_whole_candidate() {
    let [a, b] = [FixtureId::new(), FixtureId::new()];
    let definition = pan_definition(&[a, b], group_template("front", vec![0.0, 100.0]));
    let previous = snapshot(vec![spatial_group("front", &[a, b])], [(a, 0.0), (b, 10.0)]);
    let live = running(&previous, &definition);
    let stored = live.snapshot();
    let live_before = live.snapshot();

    let absent = snapshot(Vec::new(), [(a, 0.0), (b, 10.0)]);
    let prepared = prepare_restored_dynamic_candidate(&absent, &live, checkpoint(&stored)).unwrap();
    assert_eq!(prepared.presets.missing_groups.len(), 1);
    assert_eq!(table(&prepared.runtime), [(a, 0.0), (b, 100.0)].into());

    let invalid = snapshot(
        vec![reference_group("front", "ghost")],
        [(a, 0.0), (b, 10.0)],
    );
    let error = prepare_restored_dynamic_candidate(&invalid, &live, checkpoint(&stored))
        .err()
        .expect("a nested Group that cannot resolve is not passive");
    assert!(matches!(
        error,
        RestoredDynamicCandidateError::Presets(ColdPresetMaterializationError::InvalidGroup { .. })
    ));
    assert_eq!(live.snapshot(), live_before);
    assert_eq!(table(&live), [(a, 0.0), (b, 100.0)].into());
}

#[test]
fn malformed_runtime_or_catalogue_fails_before_anything_is_published() {
    let [a, b] = [FixtureId::new(), FixtureId::new()];
    let definition = pan_definition(&[a, b], group_template("front", vec![0.0, 100.0]));
    let previous = snapshot(vec![spatial_group("front", &[a, b])], [(a, 0.0), (b, 10.0)]);
    let live = running(&previous, &definition);
    let live_before = live.snapshot();

    let mut malformed = live.snapshot();
    malformed.instances[0].controllers.clear();
    assert!(matches!(
        prepare_restored_dynamic_candidate(&previous, &live, checkpoint(&malformed)),
        Err(RestoredDynamicCandidateError::Runtime(_))
    ));

    let mut catalogue = checkpoint(&live.snapshot());
    let mut unsupported = DynamicSourceOrigins::default().snapshot();
    unsupported.version = unsupported.version.wrapping_add(1);
    catalogue.origins = Some(unsupported);
    assert!(matches!(
        prepare_restored_dynamic_candidate(&previous, &live, catalogue),
        Err(RestoredDynamicCandidateError::Checkpoint(_))
    ));
    assert_eq!(live.snapshot(), live_before);
}

#[test]
fn repeated_preparation_is_deterministic_and_fresh() {
    let [a, b] = [FixtureId::new(), FixtureId::new()];
    let definition = pan_definition(&[a, b], group_template("front", vec![0.0, 100.0]));
    let show = snapshot(vec![spatial_group("front", &[a, b])], [(a, 0.0), (b, 10.0)]);
    let stored = running(&show, &definition).snapshot();
    let first =
        prepare_restored_dynamic_candidate(&show, &DynamicRuntime::default(), checkpoint(&stored))
            .unwrap();
    let second =
        prepare_restored_dynamic_candidate(&show, &DynamicRuntime::default(), checkpoint(&stored))
            .unwrap();
    assert_eq!(first.runtime.snapshot(), second.runtime.snapshot());
    assert!(!first.runtime.has_pending_preset_sources());
    assert_eq!(
        first.presets.installed_instances,
        second.presets.installed_instances
    );
}

fn checkpoint(runtime: &DynamicRuntimeSnapshot) -> DynamicRuntimeSourceCheckpoint {
    DynamicRuntimeSourceCheckpoint {
        runtime: runtime.clone(),
        origins: None,
    }
}

fn without_tables(mut snapshot: DynamicRuntimeSnapshot) -> DynamicRuntimeSnapshot {
    for instance in &mut snapshot.instances {
        instance.preset_source_values.clear();
    }
    snapshot
}

fn running(show: &light_engine::EngineSnapshot, definition: &DynamicDefinition) -> DynamicRuntime {
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    let light_dynamics::DynamicTargetBinding::FrozenTargets { targets } =
        &definition.target_binding
    else {
        unreachable!("test Dynamics use frozen targets")
    };
    runtime
        .start(DynamicStartRequest {
            definition_id: definition.id,
            controller: DynamicController {
                id: Uuid::new_v4(),
                source: DynamicControllerSource::physical_playback(1),
                priority: 100,
                activated_at_millis: 5,
                size: 1.0,
                speed_multiplier: 1.0,
                phase_offset_degrees: 0.0,
                paused: false,
            },
            target_scope: DynamicTargetScope {
                ordered_targets: targets.clone(),
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
    materialize_pending_preset_dependencies(show, &mut runtime).unwrap();
    runtime
}

fn table(runtime: &DynamicRuntime) -> HashMap<FixtureId, f32> {
    runtime
        .preset_source_instances()
        .into_iter()
        .flat_map(|instance| instance.last_valid)
        .flat_map(|record| record.values)
        .map(|fallback| {
            let value = match fallback.value {
                DynamicValue::Scalar(value) => value,
                DynamicValue::Native(value) => value as f32,
                other => panic!("unexpected table value {other:?}"),
            };
            (fallback.target, value)
        })
        .collect()
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

/// Minimum and maximum both read the Preset, so every phase samples exactly the table value.
fn first_typed_sample(runtime: &mut DynamicRuntime) -> HashMap<FixtureId, DynamicValue> {
    let transport = DynamicSpeedTransport {
        effective_bpm: 60.0,
        phase_origin_millis: 0,
        phase_reference_millis: 250,
        beat_phase: 0.25,
        phase_advancing: true,
    };
    runtime
        .sample_all_programming_addressed(250, 10, &[transport; 5], &Sources, &Sources, None)
        .unwrap()
        .into_iter()
        .filter_map(|sample| {
            sample
                .expression
                .programming_leaf()
                .map(|(_, value)| (sample.target, value.clone()))
        })
        .collect()
}

fn snapshot<const N: usize>(
    groups: Vec<GroupDefinition>,
    positions: [(FixtureId, f32); N],
) -> light_engine::EngineSnapshot {
    light_engine::EngineSnapshot {
        groups: groups.into(),
        dynamic_stage_positions: Arc::new(
            positions
                .into_iter()
                .map(|(fixture, x)| (fixture, SpatialPosition { x, y: 0.0, z: 0.0 }))
                .collect(),
        ),
        ..Default::default()
    }
}

fn with_catalogue(profiles: &[&FixtureProfile]) -> light_engine::EngineSnapshot {
    light_engine::EngineSnapshot {
        native_color_sources: Arc::new(
            NativeColorSourceCatalog::from_revisions(profiles.iter().map(|profile| {
                NativeColorSourceCatalog::compile_revision(
                    NativeColorSourceRevisionKey {
                        profile_id: profile.id,
                        revision: u64::from(profile.revision),
                        raw_store_digest: format!("retained-{}-{}", profile.id.0, profile.revision),
                    },
                    None,
                    || Ok((**profile).clone()),
                )
            }))
            .unwrap(),
        ),
        ..Default::default()
    }
}

fn spatial_group(id: &str, fixtures: &[FixtureId]) -> GroupDefinition {
    GroupDefinition {
        id: id.into(),
        name: id.into(),
        source: Some(GroupFixtureSource::Explicit {
            fixture_ids: fixtures.to_vec(),
        }),
        mapping: Some(SpatialSelectionMapping {
            projection: SpatialProjection::from_preset(
                light_dynamics::ProjectionPreset::Top,
                Position3d::default(),
            ),
            shape: SpatialSelectionShape::Grid {
                angle_degrees: 0.0,
                direction: RankDirection::Ascending,
            },
        }),
        ..Default::default()
    }
}

fn reference_group(id: &str, referenced: &str) -> GroupDefinition {
    GroupDefinition {
        id: id.into(),
        name: id.into(),
        source: Some(GroupFixtureSource::References {
            references: vec![GroupReference {
                group_id: referenced.into(),
                rule: SelectionRule::All,
            }],
        }),
        ..Default::default()
    }
}

fn group_template(group_id: &str, pan: Vec<f32>) -> DynamicPresetTemplate {
    DynamicPresetTemplate {
        groups: vec![DynamicPresetGroupTemplate {
            group_id: group_id.into(),
            value: AttributeValue::Position(Arc::new(PositionIntent::Angles {
                pan_degrees: ScalarIntent::Spread(pan),
                tilt_degrees: ScalarIntent::Value(0.0),
            })),
        }],
        ..Default::default()
    }
}

fn pan_definition(targets: &[FixtureId], template: DynamicPresetTemplate) -> DynamicDefinition {
    let address = DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Angles,
        component: Some(ProgrammingComponent::Pan),
    };
    preset_definition(targets, address, template)
}

fn direct_preset_definition(
    profile: &FixtureProfile,
    target: FixtureId,
    raw: u32,
) -> DynamicDefinition {
    let identity = identity(profile);
    let channel = &profile.modes[0].channels[0];
    let binding = NativeColorBinding {
        channel_id: channel.id,
        function_id: channel.functions[0].id,
    };
    let recipe = NativeColorRecipe {
        source: identity.clone(),
        channels: vec![NativeColorValue {
            channel_id: binding.channel_id,
            function_id: binding.function_id,
            raw,
        }],
        spreads: vec![],
    };
    let catalogue = with_catalogue(&[profile]).native_color_sources;
    let portable = catalogue
        .resolve(&identity)
        .unwrap()
        .predict(&recipe)
        .unwrap();
    let template = DynamicPresetTemplate {
        universal: Some(AttributeValue::ColorProgram(Arc::new(
            ColorProgram::Direct { recipe, portable },
        ))),
        ..Default::default()
    };
    preset_definition(&[target], native_address(profile), template)
}

fn native_value_definition(
    profile: &FixtureProfile,
    target: FixtureId,
    high: u32,
) -> DynamicDefinition {
    let mut definition = preset_definition(
        &[target],
        native_address(profile),
        DynamicPresetTemplate::default(),
    );
    definition.lanes[0].body = DynamicLaneBody::Programming(ProgrammingLaneBody {
        address: native_address(profile),
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

fn native_address(profile: &FixtureProfile) -> DynamicValueAddress {
    let channel = &profile.modes[0].channels[0];
    DynamicValueAddress {
        representation: DynamicFamilyRepresentation::DirectColor {
            source: identity(profile),
        },
        component: Some(ProgrammingComponent::NativeColor(NativeColorBinding {
            channel_id: channel.id,
            function_id: channel.functions[0].id,
        })),
    }
}

fn preset_definition(
    targets: &[FixtureId],
    address: DynamicValueAddress,
    template: DynamicPresetTemplate,
) -> DynamicDefinition {
    let mut definition: DynamicDefinition = serde_json::from_value(serde_json::json!({
        "id": Uuid::new_v4(), "pool_number": 1, "revision": 1, "name": "Restored Preset",
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
    }))
    .unwrap();
    let preset = |id: &str| DynamicValueSource::Preset {
        preset_id: id.into(),
        address: address.clone(),
        last_valid_by_target: Vec::new(),
        retained: Some(Arc::new(template.clone())),
    };
    definition.lanes[0].body = DynamicLaneBody::Programming(ProgrammingLaneBody {
        address: address.clone(),
        configuration: ProgrammingLaneConfiguration::MaxMin(MaxMinConfiguration {
            minimum: preset("3.1"),
            maximum: preset("3.1"),
            function: PeriodicFunction::LinearUp,
            size: 1.0,
            pwm: PwmShape::default(),
        }),
    });
    definition
}

fn identity(profile: &FixtureProfile) -> NativeColorIdentity {
    profile
        .native_color_identity(profile.modes[0].id, profile.modes[0].heads[0].id)
        .unwrap()
}

/// A validated one-channel UV emitter; the real catalogue compiler gives it 8-bit bounds.
fn profile(revision: u32) -> FixtureProfile {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "Restored original UV source".into();
    profile.revision = revision;
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
        reacts_to_sequence_master: false,
        reacts_to_group_master: false,
        reacts_to_grand_master: false,
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
