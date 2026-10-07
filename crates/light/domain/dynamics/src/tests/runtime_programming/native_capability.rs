use super::*;

mod cold_candidate;
mod prepared_definitions;

fn models() -> Arc<NativeModels> {
    Arc::new(NativeModels {
        model: Arc::new(NativeModel {
            source: NativeColorIdentity {
                profile_id: Uuid::from_u128(1),
                profile_revision: 2,
                profile_digest: "original".into(),
                mode_id: Uuid::from_u128(2),
                head_id: Uuid::from_u128(3),
                path_id: Uuid::from_u128(4),
                model_revision: 1,
                native_layout_signature: "layout".into(),
            },
            bindings: [5, 6].map(|id| NativeColorBinding {
                channel_id: Uuid::from_u128(id),
                function_id: Uuid::from_u128(id + 10),
            }),
        }),
        calls: AtomicUsize::new(0),
    })
}
fn native_address(models: &NativeModels, index: usize) -> DynamicValueAddress {
    DynamicValueAddress {
        representation: DynamicFamilyRepresentation::DirectColor {
            source: models.model.source.clone(),
        },
        component: Some(ProgrammingComponent::NativeColor(
            models.model.bindings[index],
        )),
    }
}
fn sources() -> TypedSources {
    TypedSources {
        current: None,
        preset: None,
        calls: Cell::new(0),
    }
}
fn focus() -> DynamicLane {
    ramp(
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Focus,
            component: Some(ProgrammingComponent::Focus),
        },
        value(DynamicValue::Scalar(0.0)),
        value(DynamicValue::Scalar(1.0)),
    )
}

struct Unavailable;
impl DynamicNativeModelResolver for Unavailable {
    fn resolve(
        &self,
        source: &NativeColorIdentity,
    ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, IntentError> {
        self.resolve_capability(source)?.require_available()
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
                detail: "original is not retained".into(),
            },
        ))
    }
}

#[test]
fn unavailable_original_suspends_only_its_lane_and_cold_rebind_preserves_phase_and_pins() {
    let models = models();
    let native = ramp(
        native_address(&models, 0),
        value(DynamicValue::Native(u32::MAX - 4)),
        value(DynamicValue::Native(u32::MAX)),
    );
    let focus = focus();
    let mut definition = definition(native.clone());
    definition.lanes.push(focus.clone());
    let provider: Arc<dyn DynamicNativeModelResolver> = Arc::new(Unavailable);
    let mut runtime = DynamicRuntime::with_native_color_models(1, provider.clone());
    runtime.install_definitions([definition.clone()]).unwrap();
    let instance = runtime
        .start(start_request(
            definition.id,
            controller(1, 1, false),
            FixtureId::new(),
            0,
            false,
        ))
        .unwrap();
    let first = sampled(&mut runtime, instance, 250, &sources());
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].lane_id, focus.id);
    assert_eq!(typed_value(&first[0]), &DynamicValue::Scalar(0.25));
    assert_eq!(runtime.unavailable_native_sources().len(), 1);
    let before = runtime.snapshot();
    let unavailable_view = runtime.captured_native_color_models();
    runtime.refresh_native_color_models(provider).unwrap();
    assert!(
        matches!(
            runtime
                .captured_native_color_models()
                .resolve_capability(&models.model.source)
                .unwrap(),
            NativeColorModelCapability::Unavailable(_)
        ),
        "an explicit retry keeps a still-unavailable source passive"
    );
    runtime.refresh_native_color_models(models.clone()).unwrap();
    assert_eq!(
        runtime.snapshot(),
        before,
        "cold rebind does not reset transport or stored payloads"
    );
    assert!(runtime.unavailable_native_sources().is_empty());
    let verified_view = runtime.captured_native_color_models();
    runtime.refresh_native_color_models(models.clone()).unwrap();
    assert!(
        Arc::ptr_eq(&verified_view, &runtime.captured_native_color_models()),
        "fully verified unchanged providers keep their warm no-op"
    );
    let next = sampled(&mut runtime, instance, 750, &sources());
    assert_eq!(
        typed_value(next.iter().find(|s| s.lane_id == native.id).unwrap()),
        &DynamicValue::Native(u32::MAX - 1)
    );
    assert_eq!(
        typed_value(next.iter().find(|s| s.lane_id == focus.id).unwrap()),
        &DynamicValue::Scalar(0.75)
    );
    let captured = runtime.captured_native_color_models();
    let original = captured.resolve(&models.model.source).unwrap();
    assert!(Arc::ptr_eq(
        &captured,
        &runtime.captured_native_color_models()
    ));
    runtime
        .refresh_native_color_models(Arc::new(Unavailable))
        .unwrap();
    let retained = runtime
        .captured_native_color_models()
        .resolve(&models.model.source)
        .unwrap();
    assert!(Arc::ptr_eq(&original, &retained));
    assert!(
        matches!(
            unavailable_view
                .resolve_capability(&models.model.source)
                .unwrap(),
            NativeColorModelCapability::Unavailable(_)
        ),
        "old captured capability is immutable"
    );
    assert_eq!(models.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn newly_available_bounds_are_validated_transactionally_and_invalid_identity_is_never_passive() {
    let models = models();
    let definition = definition(ramp(
        native_address(&models, 1),
        value(DynamicValue::Native(u32::MAX - 1)),
        value(DynamicValue::Native(u32::MAX)),
    ));
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    let instance = runtime
        .start(start_request(
            definition.id,
            controller(1, 1, false),
            FixtureId::new(),
            0,
            false,
        ))
        .unwrap();
    let before = runtime.snapshot();
    let resolver = runtime.captured_native_color_models();
    assert!(runtime.refresh_native_color_models(models.clone()).is_err());
    assert_eq!(runtime.snapshot(), before);
    assert!(Arc::ptr_eq(
        &resolver,
        &runtime.captured_native_color_models()
    ));
    assert!(sampled(&mut runtime, instance, 300, &sources()).is_empty());
    let mut malformed = definition;
    let DynamicLaneBody::Programming(body) = &mut malformed.lanes[0].body else {
        unreachable!()
    };
    let DynamicFamilyRepresentation::DirectColor { source } = &mut body.address.representation
    else {
        unreachable!()
    };
    source.profile_id = Uuid::nil();
    assert!(runtime.install_definitions([malformed]).is_err());
}

#[test]
fn unavailable_preset_fallback_survives_restore_and_rebind_with_all_integer_bits() {
    let models = models();
    let target = FixtureId::new();
    let address = native_address(&models, 0);
    let preset = DynamicValueSource::Preset {
        preset_id: "retained-native".into(),
        address: address.clone(),
        retained: None,
        last_valid_by_target: vec![DynamicValueFallback {
            target,
            value: DynamicValue::Native(u32::MAX - 7),
        }],
    };
    let definition = definition(ramp(address, preset.clone(), preset));
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    let instance = runtime
        .start(start_request(
            definition.id,
            controller(1, 1, false),
            target,
            0,
            false,
        ))
        .unwrap();
    let manifest = runtime.preset_source_instances().remove(0);
    let records = manifest
        .sources
        .iter()
        .map(|binding| DynamicPresetSourceValues {
            occurrence: binding.occurrence.unwrap(),
            preset_id: binding.preset_id.clone(),
            address: binding.address.clone(),
            values: vec![DynamicValueFallback {
                target,
                value: DynamicValue::Native(u32::MAX - 7),
            }],
        })
        .collect();
    runtime
        .install_preset_source_values(&manifest, records)
        .unwrap();
    let saved = runtime.snapshot();
    let mut restored = DynamicRuntime::default();
    restored
        .restore_snapshot(serde_json::from_value(serde_json::to_value(&saved).unwrap()).unwrap())
        .unwrap();
    assert_eq!(restored.snapshot(), saved);
    assert_eq!(restored.preset_source_instances()[0].sources.len(), 2);
    assert!(sampled(&mut restored, instance, 500, &sources()).is_empty());
    restored.refresh_native_color_models(models).unwrap();
    assert_eq!(
        typed_value(&sampled(&mut restored, instance, 500, &sources())[0]),
        &DynamicValue::Native(u32::MAX - 7)
    );
    assert_eq!(
        restored.snapshot().instances[0].preset_source_values,
        saved.instances[0].preset_source_values
    );
}

#[test]
fn unavailable_held_original_survives_pause_but_does_not_block_exact_resume_endpoint() {
    let models = models();
    let native = ramp(
        native_address(&models, 0),
        value(DynamicValue::Native(u32::MAX - 4)),
        value(DynamicValue::Native(u32::MAX)),
    );
    let mut definition = definition(native.clone());
    let mut runtime = DynamicRuntime::with_native_color_models(1, models);
    runtime.install_definitions([definition.clone()]).unwrap();
    let c = controller(1, 1, false);
    let mut request = start_request(definition.id, c.clone(), FixtureId::new(), 0, false);
    request.activation_policy_override = Some(ActivationPolicy::JoinSyncNow);
    request.activation_duration_millis = 1000;
    let instance = runtime.start(request).unwrap();
    sampled(&mut runtime, instance, 250, &sources());
    runtime
        .set_controller_paused(instance, c.id, true, 250)
        .unwrap();
    let mut replacement = focus();
    replacement.id = native.id;
    definition.lanes = vec![replacement];
    runtime.install_definitions([definition]).unwrap();
    let saved = runtime.snapshot();
    let mut restored = DynamicRuntime::default();
    restored.restore_snapshot(saved.clone()).unwrap();
    assert!(sampled(&mut restored, instance, 400, &sources()).is_empty());
    assert_eq!(
        restored.snapshot().instances[0].synchronized_hold_values,
        saved.instances[0].synchronized_hold_values
    );
    restored
        .set_controller_paused_with_resume(
            instance,
            c.id,
            false,
            500,
            Some(ActivationPolicy::JoinSyncNow),
        )
        .unwrap();
    assert!(sampled(&mut restored, instance, 1000, &sources()).is_empty());
    let resumed = sampled(&mut restored, instance, 1500, &sources());
    assert_eq!(resumed.len(), 1);
    // This helper supplies no synchronized transport. Its clock therefore subtracts the
    // 250 ms pause: (1500 - (500 - 250)) % 1000 = 250 ms, or one quarter cycle.
    assert_eq!(restored.snapshot().instances[0].paused_elapsed_millis, 250);
    assert_eq!(typed_value(&resumed[0]), &DynamicValue::Scalar(0.25));
    assert_eq!(
        typed_value(&sampled(&mut restored, instance, 1750, &sources())[0]),
        &DynamicValue::Scalar(0.5)
    );
    assert!(restored.unavailable_native_sources().is_empty());
    assert!(
        restored.snapshot().instances[0]
            .synchronized_hold_values
            .is_empty()
    );
}

#[test]
fn missing_whole_direct_original_preserves_complete_recipe_uv_and_unknown_visible_estimate() {
    use light_core::{
        NativeColorValue, PhysicalDataQuality,
        programming::{ColorProgram, PortableUv},
    };
    let models = models();
    let source = models.model.source.clone();
    let family = AttributeValue::ColorProgram(Arc::new(ColorProgram::Direct {
        recipe: NativeColorRecipe {
            source: source.clone(),
            channels: models
                .model
                .bindings
                .iter()
                .enumerate()
                .map(|(index, binding)| NativeColorValue {
                    channel_id: binding.channel_id,
                    function_id: binding.function_id,
                    raw: if index == 0 { u32::MAX - 3 } else { 45678 },
                })
                .collect(),
            spreads: vec![],
        },
        portable: PortableColorEstimate {
            model_revision: source.model_revision,
            visible: None,
            uv: Some(PortableUv {
                amount: 0.65,
                quality: PhysicalDataQuality::Estimated,
            }),
            quality: PhysicalDataQuality::Unknown,
            limitations: vec!["visible leakage unknown".into()],
        },
    }));
    let definition = definition(DynamicLane {
        body: DynamicLaneBody::Programming(ProgrammingLaneBody {
            address: DynamicValueAddress {
                representation: DynamicFamilyRepresentation::DirectColor { source },
                component: None,
            },
            configuration: ProgrammingLaneConfiguration::Keyframes(KeyframeConfiguration {
                // The cycle wraps to the first keyframe at 1.0, which is not a stored point.
                points: [0.0, 0.5]
                    .into_iter()
                    .map(|position| DynamicKeyframe {
                        position,
                        source: value(DynamicValue::Family(family.clone())),
                        interpolation: ScalarInterpolation::Linear,
                    })
                    .collect(),
                size: 1.0,
            }),
        }),
        ..lane()
    });
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    let instance = runtime
        .start(start_request(
            definition.id,
            controller(1, 1, false),
            FixtureId::new(),
            0,
            false,
        ))
        .unwrap();
    let saved = runtime.snapshot();
    let mut restored = DynamicRuntime::default();
    restored
        .restore_snapshot(serde_json::from_value(serde_json::to_value(&saved).unwrap()).unwrap())
        .unwrap();
    assert!(sampled(&mut restored, instance, 500, &sources()).is_empty());
    assert_eq!(
        restored.snapshot().instances[0].definition,
        saved.instances[0].definition
    );
    let DynamicLaneBody::Programming(body) =
        &restored.snapshot().instances[0].definition.lanes[0].body
    else {
        unreachable!()
    };
    let ProgrammingLaneConfiguration::Keyframes(config) = &body.configuration else {
        unreachable!()
    };
    assert_eq!(config.points[0].source, value(DynamicValue::Family(family)));
}

#[test]
fn exact_retained_endpoints_prune_unavailable_original_but_still_validate_inactive_payloads() {
    use crate::runtime::DynamicHeldPayload;
    let models = models();
    let focus = focus();
    let definition = definition(focus.clone());
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    let c = controller(1, 1, false);
    let mut request = start_request(definition.id, c.clone(), FixtureId::new(), 0, false);
    request.activation_policy_override = Some(ActivationPolicy::JoinSyncNow);
    let instance = runtime.start(request).unwrap();
    sampled(&mut runtime, instance, 250, &sources());
    runtime
        .set_controller_paused(instance, c.id, true, 250)
        .unwrap();
    let saved = runtime.snapshot();
    let native = DynamicSampleExpression::Programming {
        address: Arc::new(native_address(&models, 0)),
        value: DynamicValue::Native(u32::MAX - 3),
        occurrence: None,
        dependency_occurrence: None,
    };
    let focused = DynamicSampleExpression::Programming {
        address: Arc::new(DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Focus,
            component: Some(ProgrammingComponent::Focus),
        }),
        value: DynamicValue::Scalar(0.25),
        occurrence: None,
        dependency_occurrence: None,
    };
    for progress in [0.0, 1.0] {
        let mut snapshot = saved.clone();
        let (from, to) = if progress == 0.0 {
            (focused.clone(), native.clone())
        } else {
            (native.clone(), focused.clone())
        };
        let expression = DynamicSampleExpression::Transition {
            from: Some(Arc::new(from)),
            to: Some(Arc::new(to)),
            progress,
            reason: DynamicTransitionReason::Resume {
                occurrence_id: Uuid::new_v4(),
            },
        };
        let stored = &mut snapshot.instances[0];
        for sample in stored
            .last_sample_values
            .iter_mut()
            .chain(stored.synchronized_hold_values.iter_mut())
        {
            sample.payload = DynamicHeldPayload::Expression {
                expression: expression.clone(),
            };
        }
        let mut restored = DynamicRuntime::default();
        restored.restore_snapshot(snapshot.clone()).unwrap();
        assert!(restored.unavailable_native_sources().is_empty());
        assert_eq!(sampled(&mut restored, instance, 400, &sources()).len(), 1);

        let DynamicHeldPayload::Expression { expression } =
            &mut snapshot.instances[0].last_sample_values[0].payload
        else {
            unreachable!()
        };
        let DynamicSampleExpression::Transition { from, to, .. } = expression else {
            unreachable!()
        };
        let inactive = if progress == 0.0 { to } else { from };
        let DynamicSampleExpression::Programming { address, .. } =
            Arc::make_mut(inactive.as_mut().unwrap())
        else {
            unreachable!()
        };
        let DynamicFamilyRepresentation::DirectColor { source } =
            &mut Arc::make_mut(address).representation
        else {
            unreachable!()
        };
        source.profile_id = Uuid::nil();
        assert!(
            restored.restore_snapshot(snapshot).is_err(),
            "inactive stored payloads must remain structurally valid"
        );
    }
}
