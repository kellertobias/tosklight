use super::*;
use light_core::{NativeColorBinding, NativeColorIdentity, NativeColorValue, PhysicalDataQuality};

fn model() -> Arc<NativeModel> {
    Arc::new(NativeModel {
        identity: NativeColorIdentity {
            profile_id: Uuid::new_v4(),
            profile_revision: 1,
            profile_digest: "recorded-original".into(),
            mode_id: Uuid::new_v4(),
            head_id: Uuid::new_v4(),
            path_id: Uuid::new_v4(),
            model_revision: 1,
            native_layout_signature: "complete-layout".into(),
        },
        bindings: [0, 1].map(|_| NativeColorBinding {
            channel_id: Uuid::new_v4(),
            function_id: Uuid::new_v4(),
        }),
    })
}
fn direct(model: &NativeModel) -> AttributeValue {
    AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
        recipe: NativeColorRecipe {
            source: model.identity.clone(),
            channels: model
                .bindings
                .iter()
                .enumerate()
                .map(|(index, binding)| NativeColorValue {
                    channel_id: binding.channel_id,
                    function_id: binding.function_id,
                    raw: u32::MAX - (index as u32 + 3),
                })
                .collect(),
            spreads: vec![],
        },
        portable: PortableColorEstimate {
            model_revision: 1,
            visible: None,
            uv: Some(PortableUv {
                amount: 0.625,
                quality: PhysicalDataQuality::Estimated,
            }),
            quality: PhysicalDataQuality::Estimated,
            limitations: vec!["visible leakage unknown".into()],
        },
    }))
}
fn native_manifest(
    model: &NativeModel,
    targets: Vec<FixtureId>,
    component: bool,
) -> DynamicInstancePresetSources {
    let mut instance = source(
        DynamicPresetTemplate {
            universal: Some(direct(model)),
            ..Default::default()
        },
        targets,
    );
    Arc::make_mut(&mut instance.sources[0]).address = DynamicValueAddress {
        representation: DynamicFamilyRepresentation::DirectColor {
            source: model.identity.clone(),
        },
        component: component.then_some(ProgrammingComponent::NativeColor(model.bindings[0])),
    };
    instance
}
fn checkpoint(instance: &mut DynamicInstancePresetSources, value: DynamicValue) {
    let source = &instance.sources[0];
    instance.last_valid = vec![DynamicPresetSourceValues {
        occurrence: source.occurrence.unwrap(),
        preset_id: source.preset_id.clone(),
        address: source.address.clone(),
        values: vec![DynamicValueFallback {
            target: instance.ordered_targets[0],
            value,
        }],
    }];
}

struct Missing;
impl DynamicNativeModelResolver for Missing {
    fn resolve(
        &self,
        _: &NativeColorIdentity,
    ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, IntentError> {
        panic!("capability-aware materialization must not collapse absence into strict errors")
    }
    fn resolve_capability(
        &self,
        source: &NativeColorIdentity,
    ) -> Result<NativeColorModelCapability, IntentError> {
        source.validate()?;
        Ok(NativeColorModelCapability::Unavailable(
            NativeColorModelUnavailable {
                source: source.clone(),
                reason: NativeColorUnavailableReason::MissingRevision,
                detail: "recorded revision is unavailable".into(),
            },
        ))
    }
}

#[test]
fn unavailable_original_retains_complete_uv_recipe_and_each_occurrence_without_blocking_other_owners()
 {
    let model = model();
    let targets = vec![FixtureId::new(), FixtureId::new()];
    let mut instance = native_manifest(&model, targets.clone(), false);
    checkpoint(&mut instance, DynamicValue::Family(direct(&model)));
    let semantic = source(
        DynamicPresetTemplate {
            universal: Some(angles(vec![90.0])),
            ..Default::default()
        },
        targets,
    );
    instance.sources.extend(semantic.sources);
    let original = instance.last_valid.clone();
    let template = instance.sources[0].retained.clone();
    let compiled =
        compile_dynamic_preset_sources(&instance, &HashMap::new(), &HashMap::new(), Some(&Missing))
            .unwrap();
    assert_eq!(
        compiled.values.len(),
        2,
        "never publish an empty replacement manifest"
    );
    assert_eq!(
        compiled.values[0], original[0],
        "all raw bytes, saved XYZ unknown and UV intent survive"
    );
    assert_eq!(compiled.values[1].values.len(), 2);
    assert!(
        compiled.values[1]
            .values
            .iter()
            .all(|value| value.value == DynamicValue::Scalar(90.0))
    );
    assert_eq!(compiled.issues.len(), 2);
    assert!(compiled.issues.iter().all(|issue| matches!(&issue.reason,
        DynamicPresetSourceIssueReason::NativeUnavailable(reason) if reason.reason == NativeColorUnavailableReason::MissingRevision)));
    assert_eq!(
        instance.sources[0].retained, template,
        "cold projection retains the authored template"
    );
    assert_eq!(
        compiled.values[0].values.len(),
        1,
        "a new member cannot invent a native-source match"
    );
}

#[test]
fn absent_resolver_is_passive_but_malformed_values_and_verified_invalid_bindings_are_not() {
    let model = model();
    let mut instance = native_manifest(&model, vec![FixtureId::new()], true);
    checkpoint(&mut instance, DynamicValue::Native(u32::MAX - 3));
    let compiled =
        compile_dynamic_preset_sources(&instance, &HashMap::new(), &HashMap::new(), None).unwrap();
    assert_eq!(compiled.values, instance.last_valid);
    assert!(
        matches!(&compiled.issues[0].reason, DynamicPresetSourceIssueReason::NativeUnavailable(reason)
        if reason.reason == NativeColorUnavailableReason::MissingResolver)
    );

    let mut malformed = instance.clone();
    malformed.last_valid[0].values[0].value = DynamicValue::Scalar(f32::NAN);
    assert!(
        compile_dynamic_preset_sources(
            &malformed,
            &HashMap::new(),
            &HashMap::new(),
            Some(&Missing)
        )
        .is_err()
    );
    let mut invalid = instance.clone();
    Arc::make_mut(&mut invalid.sources[0]).address.component =
        Some(ProgrammingComponent::NativeColor(NativeColorBinding {
            channel_id: Uuid::new_v4(),
            function_id: Uuid::new_v4(),
        }));
    assert!(
        compile_dynamic_preset_sources(
            &invalid,
            &HashMap::new(),
            &HashMap::new(),
            Some(&Models(model))
        )
        .is_err()
    );
}

#[test]
fn available_source_replaces_checkpoint_with_new_prediction_without_changing_template_or_occurrence()
 {
    let model = model();
    let mut instance = native_manifest(&model, vec![FixtureId::new()], true);
    checkpoint(&mut instance, DynamicValue::Native(17));
    let unavailable =
        compile_dynamic_preset_sources(&instance, &HashMap::new(), &HashMap::new(), Some(&Missing))
            .unwrap();
    assert_eq!(
        unavailable.values[0].values[0].value,
        DynamicValue::Native(17)
    );
    let available = compile_dynamic_preset_sources(
        &instance,
        &HashMap::new(),
        &HashMap::new(),
        Some(&Models(model)),
    )
    .unwrap();
    assert_eq!(
        available.values[0].values[0].value,
        DynamicValue::Native(u32::MAX - 3)
    );
    assert_eq!(
        available.values[0].occurrence,
        unavailable.values[0].occurrence
    );
    assert!(available.issues.is_empty());
}

#[test]
fn runtime_capture_uses_original_pins_after_catalogue_removal_and_keeps_generation_cas() {
    let model = model();
    let target = FixtureId::new();
    let manifest = native_manifest(&model, vec![target], true);
    let binding = &manifest.sources[0];
    let source = DynamicValueSource::Preset {
        preset_id: binding.preset_id.clone(),
        address: binding.address.clone(),
        retained: binding.retained.clone(),
        last_valid_by_target: vec![],
    };
    let definition = DynamicDefinition {
        id: Uuid::new_v4(),
        pool_number: 1,
        revision: 1,
        name: "Pinned preset source".into(),
        color: None,
        icon: None,
        target_binding: DynamicTargetBinding::Targetless,
        lanes: vec![DynamicLane {
            id: Uuid::new_v4(),
            speed_multiplier: Rational::ONE,
            width: 1.0,
            phase: None,
            random_group_id: None,
            body: DynamicLaneBody::Programming(ProgrammingLaneBody {
                address: binding.address.clone(),
                configuration: ProgrammingLaneConfiguration::MaxMin(MaxMinConfiguration {
                    minimum: source.clone(),
                    maximum: source,
                    function: PeriodicFunction::LinearUp,
                    size: 1.0,
                    pwm: PwmShape::default(),
                }),
            }),
        }],
        random_groups: vec![],
        phase_spread_mode: DynamicPhaseSpreadMode::Uniform,
        spatial_mapping: DynamicSpatialMappingOverride::default(),
        phase: PhaseDistribution {
            ordering: PhaseOrdering::Selection,
            offset_degrees: 0.0,
            span_degrees: 360.0,
            block_size: 1,
            repeats: 1,
            wings: false,
            anchors_degrees: vec![],
        },
        speed: DynamicSpeed::Fixed {
            duration_millis: 1000,
        },
        overall_speed_multiplier: Rational::ONE,
        run_mode: DynamicRunMode::Loop,
        default_activation: ActivationPolicy::StartNow,
        activation_boundary: ActivationBoundary::Beat,
    };
    let mut runtime = DynamicRuntime::with_native_color_models(
        PROGRAMMING_CONTRACT_VERSION,
        Arc::new(Models(model)),
    );
    runtime.install_definitions([definition.clone()]).unwrap();
    let instance = runtime
        .start(DynamicStartRequest {
            definition_id: definition.id,
            controller: DynamicController {
                id: Uuid::new_v4(),
                source: DynamicControllerSource::Programmer {
                    programmer_id: Uuid::new_v4(),
                    instance_link: None,
                },
                priority: 1,
                activated_at_millis: 123,
                size: 1.0,
                speed_multiplier: 1.0,
                phase_offset_degrees: 0.0,
                paused: false,
            },
            target_scope: DynamicTargetScope {
                ordered_targets: vec![target],
            },
            stage_positions: HashMap::new(),
            inherited_spatial_mapping: None,
            now_millis: 123,
            activation_delay_millis: 0,
            activation_duration_millis: 0,
            activation_policy_override: None,
            reuse_matching_targetless: false,
        })
        .unwrap();
    runtime
        .refresh_native_color_models(Arc::new(Missing))
        .unwrap();
    let before = runtime.snapshot();
    let captured =
        compile_runtime_dynamic_preset_sources(&runtime, &HashMap::new(), &HashMap::new()).unwrap();
    assert_eq!(
        runtime.snapshot(),
        before,
        "capturing dependencies does not mutate phase or source records"
    );
    assert_eq!(captured.len(), 1);
    let (expected, compiled) = &captured[0];
    assert!(
        compiled.issues.is_empty(),
        "runtime's exact original pin survives catalogue removal"
    );
    assert_eq!(compiled.values.len(), 2);
    assert!(
        compiled
            .values
            .iter()
            .all(|value| value.values[0].value == DynamicValue::Native(u32::MAX - 3))
    );
    runtime.invalidate_preset_source_dependencies(instance);
    assert!(
        !runtime
            .install_preset_source_values(expected, compiled.values.clone())
            .unwrap()
    );
    let current =
        compile_runtime_dynamic_preset_sources(&runtime, &HashMap::new(), &HashMap::new()).unwrap();
    assert!(
        runtime
            .install_preset_source_values(&current[0].0, current[0].1.values.clone())
            .unwrap()
    );
    assert_eq!(runtime.snapshot().instances[0].started_at_millis, 123);
}
