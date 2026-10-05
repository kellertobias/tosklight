//! Actual coordinator replay, destination native provider and independent Preset materialization.
use super::*;
use crate::runtime::output_scheduler::materialize_cold_preset_dependencies;
use light_core::programming::{ColorProgram, NativeColorRecipe, ProgrammingComponent};
use light_core::{AttributeValue, NativeColorBinding, NativeColorIdentity, NativeColorValue, Xyz};
use light_dynamics::{
    DynamicFamilyRepresentation, DynamicLaneBody, DynamicPresetGroupTemplate,
    DynamicPresetTemplate, DynamicValue, DynamicValueAddress, DynamicValueSource,
    DynamicValueSourceResolver, MaxMinConfiguration, NativeColorModelCapability, PeriodicFunction,
    ProgrammingLaneBody, ProgrammingLaneConfiguration, PwmShape,
};
use light_engine::{NativeColorSourceCatalog, NativeColorSourceRevisionKey};
use light_fixture::{
    CanonicalTransform, ChannelBehavior, ChannelFunction, ChannelResolution, ColorPhysicalModel,
    FixtureChannel, FixtureProfile, HeadOpticalPath, NativeColorBinding as FixtureNativeBinding,
    OpticalEmitter, OpticalEmitterBand, OpticalProvenance, OpticalSource,
};
use light_programmer::{GroupDefinition, GroupFixtureSource, GroupReference, SelectionRule};
use std::collections::HashMap;

struct NoTypedSources;
impl DynamicValueSourceResolver for NoTypedSources {
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
struct NativeEvaluator {
    identity: NativeColorIdentity,
    target: FixtureId,
    calls: usize,
}
impl PendingAttemptEvaluator<DynamicValue> for NativeEvaluator {
    fn evaluate(
        &mut self,
        _: PendingEpisodeKey,
        input: &RetainedInputCapture,
        runtime: &mut DynamicRuntime,
        _: &mut DynamicSourceOrigins,
    ) -> Result<DynamicValue, String> {
        self.calls += 1;
        assert!(matches!(
            runtime
                .captured_native_color_models()
                .resolve_capability(&self.identity),
            Ok(NativeColorModelCapability::Available(_))
        ));
        assert!(
            !runtime.has_pending_preset_sources(),
            "cold replay must materialize before evaluation"
        );
        assert_eq!(
            table(runtime),
            [(self.target, DynamicValue::Native(200))].into()
        );
        // Both resolver methods deliberately return None: only this branch's installed Preset
        // table can supply the first typed sample.
        let samples = runtime
            .sample_all_programming_addressed(
                input.frame.sampled_at().timestamp_millis() as u64,
                1_000 / u64::from(input.rate.max(1)),
                &input.speed_transports,
                &Current(0.0),
                &NoTypedSources,
                None,
            )
            .map_err(|error| error.to_string())?;
        assert_eq!(samples.len(), 1);
        assert_eq!(samples[0].target, self.target);
        Ok(samples[0]
            .expression
            .programming_leaf()
            .expect("native component leaf")
            .1
            .clone())
    }
}
fn table(runtime: &DynamicRuntime) -> HashMap<FixtureId, DynamicValue> {
    runtime
        .preset_source_instances()
        .into_iter()
        .flat_map(|instance| instance.last_valid)
        .flat_map(|record| record.values)
        .map(|entry| (entry.target, entry.value))
        .collect()
}

/// Test-only producer using actual recording/cold preparation. No tables enter the event:
/// Pending must compile its own after replaying Start against the destination provider.
fn native_cold(
    rig: &mut Rig,
    profile: &FixtureProfile,
    malformed_group: bool,
) -> (Vec<Arc<ColdGenerationEvent>>, DynamicDefinition) {
    let previous = rig.engine.snapshot();
    let mut detached = rig.live.fork_for_cold_install();
    let (from, _) = rig
        .publication
        .begin_retained_history(&mut detached, &previous, capacity(64))
        .unwrap();
    let boundary = rig.publication.cold_boundary(&rig.live).unwrap();
    let target = rig.request.target_scope.ordered_targets[0];
    let mut replacement = direct_preset_definition(profile, target, 200);
    if malformed_group {
        let DynamicLaneBody::Programming(body) = &mut replacement.lanes[0].body else {
            unreachable!()
        };
        let ProgrammingLaneConfiguration::MaxMin(config) = &mut body.configuration else {
            unreachable!()
        };
        for source in [&mut config.minimum, &mut config.maximum] {
            let DynamicValueSource::Preset { retained, .. } = source else {
                unreachable!()
            };
            let template = Arc::make_mut(retained.as_mut().unwrap());
            template.groups.push(DynamicPresetGroupTemplate {
                group_id: "front".into(),
                value: template.universal.clone().unwrap(),
            });
        }
    }
    let mut destination = (*previous).clone();
    destination.revision += 1;
    destination.dynamics = vec![replacement.clone()].into();
    destination.native_color_sources = with_catalogue(&[profile]).native_color_sources;
    if malformed_group {
        destination.groups = vec![reference_group("front", "ghost")].into();
    }
    let destination = if malformed_group {
        // Engine correctly rejects invalid Groups. Keep this adversarial intermediate event
        // outside Engine; a following valid cold snapshot supplies the real captured frame.
        Arc::new(destination)
    } else {
        rig.engine.replace_snapshot(destination).unwrap();
        rig.engine.snapshot()
    };
    rig.live
        .install_definitions(destination.dynamics.iter().cloned())
        .unwrap();
    rig.live
        .refresh_native_color_models(destination.native_color_sources.clone())
        .unwrap();
    let mut request = rig.request.clone();
    request.definition_id = replacement.id;
    request.now_millis = 100;
    request.controller.activated_at_millis = 100;
    rig.control(DynamicControl::Start(Box::new(request)));
    if !malformed_group {
        materialize_cold_preset_dependencies(&previous, &destination, &mut rig.live).unwrap();
        assert_eq!(
            table(&rig.live),
            [(target, DynamicValue::Native(200))].into()
        );
    }
    // The malformed case deliberately constructs an adversarial retained event, bypassing
    // producer finalization, to prove the consumer discards its failed private compilation.
    let prepared = boundary.prepare(previous, destination.clone(), &rig.live);
    rig.publication
        .installed_with_cold_event(destination.clone(), Some(prepared));
    if malformed_group {
        let repair_boundary = rig.publication.cold_boundary(&rig.live).unwrap();
        let mut repaired = (*destination).clone();
        repaired.revision += 1;
        repaired.groups = Vec::new().into();
        rig.engine.replace_snapshot(repaired).unwrap();
        let repaired = rig.engine.snapshot();
        let prepared = repair_boundary.prepare(destination, repaired.clone(), &rig.live);
        rig.publication
            .installed_with_cold_event(repaired, Some(prepared));
    }
    (
        rig.publication.cold_generations_since(from).unwrap(),
        replacement,
    )
}

#[test]
fn cold_native_provider_and_start_materialize_pending_own_preset_before_first_sample() {
    let mut rig = Rig::new();
    let original = profile(1);
    let original_identity = identity(&original);
    let target = rig.request.target_scope.ordered_targets[0];
    let mut history = DetachedPendingHistory::new(rig.seed(PreloadBranch::AfterRelease)).unwrap();
    assert!(matches!(
        history
            .runtime
            .captured_native_color_models()
            .resolve_capability(&original_identity),
        Ok(NativeColorModelCapability::Unavailable(_))
    ));
    rig.control(DynamicControl::Off {
        controller: rig.controller,
        delay: 0,
        duration: 0,
    });
    let (cold, replacement) = native_cold(&mut rig, &original, false);
    let input = rig.capture(false);
    let live_before = rig.live.snapshot();
    let live_generation = rig.live.preset_source_instances()[0].dependency_generation;
    let controls = rig.controls(history.position().controls);
    let window = history
        .prepare_window(history.key, &[input.clone()], &cold, &controls, limits())
        .unwrap();
    let mut evaluator = NativeEvaluator {
        identity: original_identity,
        target,
        calls: 0,
    };
    let result = history.consume_window(window, &mut evaluator);
    assert!(result.stopped.is_none(), "{:?}", result.stopped);
    assert_eq!(
        (
            result.consumed_attempts,
            result.successful_attempts,
            evaluator.calls
        ),
        (1, 1, 1)
    );
    assert_eq!(
        history.last_success().unwrap().value,
        DynamicValue::Native(200)
    );
    assert_eq!(history.position().controls, input.controls);
    assert_eq!(history.position().cold, input.cold);
    assert!(Arc::ptr_eq(&history.snapshot, &rig.engine.snapshot()));
    let instance = history.runtime.snapshot().instances[0].id;
    assert_eq!(
        history
            .runtime
            .instance_definition(instance)
            .unwrap()
            .as_ref(),
        &replacement
    );
    assert_ne!(
        history.runtime.preset_source_instances()[0].dependency_generation,
        live_generation,
        "Pending must materialize its own newly started dependency generation"
    );
    assert_eq!(rig.live.snapshot(), live_before);
}

#[test]
fn cold_preset_compilation_failure_rolls_back_prefix_start_provider_and_pins() {
    let mut rig = Rig::new();
    let original = profile(1);
    let original_identity = identity(&original);
    let mut history = DetachedPendingHistory::new(rig.seed(PreloadBranch::AfterRelease)).unwrap();
    let before = history.runtime.snapshot();
    let before_position = history.position();
    let before_snapshot = history.snapshot.clone();
    let before_origins = history.origins.clone();
    rig.control(DynamicControl::Off {
        controller: rig.controller,
        delay: 0,
        duration: 0,
    });
    let (cold, _) = native_cold(&mut rig, &original, true);
    assert!(matches!(
        rig.live
            .captured_native_color_models()
            .resolve_capability(&original_identity),
        Ok(NativeColorModelCapability::Available(_))
    ));
    let input = rig.capture(false);
    let controls = rig.controls(history.position().controls);
    let window = history
        .prepare_window(history.key, &[input], &cold, &controls, limits())
        .unwrap();
    let mut evaluator = NativeEvaluator {
        identity: original_identity.clone(),
        target: rig.request.target_scope.ordered_targets[0],
        calls: 0,
    };
    let result = history.consume_window(window, &mut evaluator);
    assert!(
        matches!(result.stopped,
        Some(PendingHistoryGap::Replay(ref error)) if error.contains("Group front cannot be resolved")),
        "{:?}",
        result.stopped
    );
    assert_eq!(
        (
            result.consumed_attempts,
            result.successful_attempts,
            evaluator.calls
        ),
        (0, 0, 0)
    );
    assert_eq!(history.runtime.snapshot(), before);
    assert_eq!(history.position(), before_position);
    assert!(Arc::ptr_eq(&history.snapshot, &before_snapshot));
    assert!(history.origins.shares_storage(&before_origins));
    assert!(history.last_success().is_none());
    assert!(
        matches!(
            history
                .runtime
                .captured_native_color_models()
                .resolve_capability(&original_identity),
            Ok(NativeColorModelCapability::Unavailable(_))
        ),
        "failed candidate must not leak provider or pins"
    );
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
        "id": Uuid::new_v4(), "pool_number": 1, "revision": 1, "name": "Pending cold Preset",
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

fn profile(revision: u32) -> FixtureProfile {
    let mut profile = FixtureProfile::blank();
    profile.manufacturer = "Test".into();
    profile.name = "Pending original UV source".into();
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
