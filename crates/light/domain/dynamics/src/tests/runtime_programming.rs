use super::*;
mod current_dependencies;
mod history;
mod native_capability;
mod operation_origins;
mod preset_batch;
mod preset_reconciliation;
mod staged;
use light_core::{
    AttributeValue, NativeColorBinding, NativeColorIdentity,
    programming::{
        IntentError, NativeColorComponentDescriptor, NativeColorEditModel, NativeColorRecipe,
        PortableColorEstimate, PositionIntent, ProgrammingComponent, TargetReference,
    },
};
use std::sync::atomic::{AtomicUsize, Ordering};

struct TypedSources {
    current: Option<DynamicValue>,
    preset: Option<DynamicValue>,
    calls: Cell<usize>,
}
impl DynamicValueSourceResolver for TypedSources {
    fn current(&self, _: FixtureId, _: &DynamicValueAddress) -> Option<DynamicValue> {
        self.calls.set(self.calls.get() + 1);
        self.current.clone()
    }
    fn preset(
        &self,
        _: &DynamicPresetSourceBinding,
        _: Uuid,
        _: FixtureId,
    ) -> Option<DynamicValue> {
        self.preset.clone()
    }
}
fn value(value: DynamicValue) -> DynamicValueSource {
    DynamicValueSource::Value { value }
}
fn pan_address() -> DynamicValueAddress {
    DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Angles,
        component: Some(ProgrammingComponent::Pan),
    }
}
fn ramp(
    address: DynamicValueAddress,
    low: DynamicValueSource,
    high: DynamicValueSource,
) -> DynamicLane {
    DynamicLane {
        body: DynamicLaneBody::Programming(ProgrammingLaneBody {
            address,
            configuration: ProgrammingLaneConfiguration::MaxMin(MaxMinConfiguration {
                minimum: low,
                maximum: high,
                function: PeriodicFunction::LinearUp,
                size: 1.0,
                pwm: PwmShape::default(),
            }),
        }),
        ..lane()
    }
}
fn sampled(
    runtime: &mut DynamicRuntime,
    instance: Uuid,
    at: u64,
    sources: &TypedSources,
) -> Vec<DynamicRuntimeSample> {
    runtime
        .sample_programming(instance, at, 1000, 10, &Sources { current: 0.99 }, sources)
        .unwrap()
}
fn typed_value(sample: &DynamicRuntimeSample) -> &DynamicValue {
    sample
        .expression
        .programming_leaf()
        .expect("expected typed leaf")
        .1
}

/// Numeric inspection is deliberately test-only: production retains Current until complete
/// Angle pair assembly against the captured static frame.
fn inspected_typed_value(
    sample: &DynamicRuntimeSample,
    sources: &dyn DynamicValueSourceResolver,
) -> DynamicValue {
    match sample.expression.angle_current_address() {
        Some(address) => sources
            .current(sample.target, address)
            .expect("available Current"),
        None => typed_value(sample).clone(),
    }
}

fn assert_angle_expression(expression: &DynamicSampleExpression, pan: f32, tilt: f32) {
    let (address, value) = expression
        .programming_leaf()
        .unwrap_or_else(|| panic!("expected complete Angle endpoint: {expression:?}"));
    assert_eq!(address.component, None);
    assert_eq!(
        value,
        &DynamicValue::Family(AttributeValue::Position(Arc::new(PositionIntent::angles(
            pan, tilt
        )))),
    );
}

#[test]
fn paused_axis_hot_edit_keeps_original_live_partner_and_zips_resume_pairs_after_restore() {
    let target = FixtureId::new();
    let pan = ramp(
        pan_address(),
        value(DynamicValue::Scalar(90.0)),
        value(DynamicValue::Scalar(90.0)),
    );
    let mut definition = definition(pan.clone());
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    let c = controller(1, 1, false);
    let mut request = start_request(definition.id, c.clone(), target, 0, false);
    request.activation_policy_override = Some(ActivationPolicy::JoinSyncNow);
    request.activation_duration_millis = 1000;
    let instance = runtime.start(request).unwrap();
    runtime
        .set_controller_lane_selection(
            instance,
            c.id,
            DynamicLaneSelection::Uniform {
                lanes: vec![pan.id],
            },
        )
        .unwrap();
    let mut sources = TypedSources {
        current: Some(DynamicValue::Scalar(10.0)),
        preset: None,
        calls: Cell::new(0),
    };
    sampled(&mut runtime, instance, 200, &sources);
    runtime
        .set_controller_paused(instance, c.id, true, 200)
        .unwrap();

    let mut tilt_address = pan_address();
    tilt_address.component = Some(ProgrammingComponent::Tilt);
    let mut tilt = ramp(
        tilt_address,
        value(DynamicValue::Scalar(60.0)),
        value(DynamicValue::Scalar(60.0)),
    );
    tilt.id = pan.id;
    definition.lanes = vec![tilt];
    runtime.install_definitions([definition.clone()]).unwrap();
    sources.current = Some(DynamicValue::Scalar(25.0));
    let paused = sampled(&mut runtime, instance, 400, &sources);
    assert_eq!(
        paused.len(),
        2,
        "new Pan partner must not enter the held pair"
    );
    let bundle = bundle_position_sample_expressions(&paused, &sources).unwrap();
    assert_angle_expression(&bundle.position.unwrap().expression, 90.0, 25.0);
    let saved = serde_json::from_value(serde_json::to_value(runtime.snapshot()).unwrap()).unwrap();
    runtime.restore_snapshot(saved).unwrap();
    let valid = runtime.snapshot();
    let mut forged = valid.clone();
    let tape = forged.instances[0].expression_tape.clone().unwrap();
    let partner = forged.instances[0]
        .synchronized_hold_values
        .iter_mut()
        .find(|sample| match &sample.payload {
            crate::runtime::DynamicHeldPayload::TapeRoot { tape_root } => matches!(
                tape.node(*tape_root),
                Some(RetainedExpressionNode::AngleCurrent { .. })
            ),
            crate::runtime::DynamicHeldPayload::Expression { expression } => {
                expression.angle_current_address().is_some()
            }
            _ => false,
        })
        .unwrap();
    partner.lane_id = Uuid::new_v4();
    assert!(
        runtime.restore_snapshot(forged).is_err(),
        "arbitrary out-of-selection Current IDs remain invalid"
    );
    assert_eq!(runtime.snapshot(), valid);
    runtime
        .set_controller_paused_with_resume(
            instance,
            c.id,
            false,
            500,
            Some(ActivationPolicy::JoinSyncNow),
        )
        .unwrap();
    sources.current = Some(DynamicValue::Scalar(40.0));
    let resumed = sampled(&mut runtime, instance, 1000, &sources);
    assert_eq!(
        resumed.len(),
        3,
        "old and new automatic partners keep distinct identities"
    );
    let occurrence = runtime.snapshot().instances[0]
        .synchronized_resume_transition
        .unwrap()
        .occurrence_id;
    assert!(resumed.iter().all(|sample| matches!(sample.expression,
        DynamicSampleExpression::Transition { reason: DynamicTransitionReason::Resume { occurrence_id }, progress: 0.5, .. }
        if occurrence_id == occurrence)));
    let bundle = bundle_position_sample_expressions(&resumed, &sources).unwrap();
    let DynamicSampleExpression::Transition {
        from: Some(from),
        to: Some(to),
        progress: 0.5,
        ..
    } = bundle.position.unwrap().expression.shallow().unwrap()
    else {
        panic!("complete paired resume")
    };
    assert_angle_expression(&from, 90.0, 40.0);
    assert_angle_expression(&to, 40.0, 60.0);

    // An interruption preserves the old nested branch identity while both Current
    // partners are resolved again from today's one immutable frame.
    runtime
        .set_controller_paused(instance, c.id, true, 1000)
        .unwrap();
    definition.lanes = vec![ramp(
        pan_address(),
        value(DynamicValue::Scalar(120.0)),
        value(DynamicValue::Scalar(120.0)),
    )];
    definition.lanes[0].id = pan.id;
    runtime.install_definitions([definition]).unwrap();
    runtime
        .set_controller_paused_with_resume(
            instance,
            c.id,
            false,
            1200,
            Some(ActivationPolicy::JoinSyncNow),
        )
        .unwrap();
    sources.current = Some(DynamicValue::Scalar(50.0));
    let mut resumed = sampled(&mut runtime, instance, 1450, &sources);
    let bundle = bundle_position_sample_expressions(&resumed, &sources).unwrap();
    let DynamicSampleExpression::Transition {
        from: Some(old),
        to: Some(new),
        progress: 0.25,
        reason: DynamicTransitionReason::Resume {
            occurrence_id: second,
        },
    } = bundle.position.unwrap().expression.shallow().unwrap()
    else {
        panic!("outer resume")
    };
    assert_ne!(second, occurrence);
    let DynamicSampleExpression::Transition {
        from: Some(from),
        to: Some(to),
        progress: 0.5,
        reason: DynamicTransitionReason::Resume { occurrence_id },
    } = &old.shallow().unwrap()
    else {
        panic!("retained inner resume")
    };
    assert_eq!(*occurrence_id, occurrence);
    assert_angle_expression(from, 90.0, 50.0);
    assert_angle_expression(to, 50.0, 60.0);
    assert_angle_expression(&new, 120.0, 50.0);
    let snapshot = runtime.snapshot();
    runtime.restore_snapshot(snapshot).unwrap();
    let mut restored = sampled(&mut runtime, instance, 1450, &sources);
    resumed.sort_by_key(|sample| sample.lane_id);
    restored.sort_by_key(|sample| sample.lane_id);
    assert_eq!(restored, resumed);
}

#[test]
fn deleted_held_angle_pair_releases_to_absence_and_missing_current_never_reuses_old_angle() {
    let target = FixtureId::new();
    let mut definition = definition(ramp(
        pan_address(),
        value(DynamicValue::Scalar(90.0)),
        value(DynamicValue::Scalar(90.0)),
    ));
    definition.lanes.push(lane());
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    let c = controller(1, 1, false);
    let mut request = start_request(definition.id, c.clone(), target, 0, false);
    request.activation_policy_override = Some(ActivationPolicy::JoinSyncNow);
    request.activation_duration_millis = 1000;
    let instance = runtime.start(request).unwrap();
    let mut sources = TypedSources {
        current: Some(DynamicValue::Scalar(25.0)),
        preset: None,
        calls: Cell::new(0),
    };
    sampled(&mut runtime, instance, 200, &sources);
    runtime
        .set_controller_paused(instance, c.id, true, 200)
        .unwrap();
    definition.lanes.remove(0);
    runtime.install_definitions([definition]).unwrap();
    let snapshot = runtime.snapshot();
    runtime.restore_snapshot(snapshot).unwrap();
    sources.current = None;
    let paused = sampled(&mut runtime, instance, 400, &sources);
    assert!(
        bundle_position_sample_expressions(&paused, &sources)
            .unwrap()
            .position
            .is_none()
    );
    sources.current = Some(DynamicValue::Scalar(-45.0));
    let paused = sampled(&mut runtime, instance, 450, &sources);
    assert_angle_expression(
        &bundle_position_sample_expressions(&paused, &sources)
            .unwrap()
            .position
            .unwrap()
            .expression,
        90.0,
        -45.0,
    );
    runtime
        .set_controller_paused_with_resume(
            instance,
            c.id,
            false,
            500,
            Some(ActivationPolicy::JoinSyncNow),
        )
        .unwrap();
    let resumed = sampled(&mut runtime, instance, 1000, &sources);
    let bundle = bundle_position_sample_expressions(&resumed, &sources).unwrap();
    let DynamicSampleExpression::Transition {
        from: Some(from),
        to: None,
        progress: 0.5,
        ..
    } = bundle.position.unwrap().expression.shallow().unwrap()
    else {
        panic!("released old pair")
    };
    assert_angle_expression(&from, 90.0, -45.0);
    sources.current = None;
    let missing = sampled(&mut runtime, instance, 1000, &sources);
    assert!(
        bundle_position_sample_expressions(&missing, &sources)
            .unwrap()
            .position
            .is_none()
    );
    let completed = sampled(&mut runtime, instance, 1500, &sources);
    assert_eq!(completed.len(), 1, "only unrelated scalar lane remains");
}

#[test]
fn angle_current_tokens_validate_without_fabricated_values_and_initial_pause_captures_once() {
    let token = DynamicSampleExpression::AngleCurrent {
        address: Arc::new(pan_address()),
    };
    token.validate().unwrap();
    assert_eq!(token.required_programming_contract(), 1);
    let mut visited = false;
    token
        .visit_programming_values(&mut |_, _| {
            visited = true;
            Ok(())
        })
        .unwrap();
    assert!(
        !visited,
        "a live Current token has no numeric source to validate"
    );
    for address in [
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Angles,
            component: Some(ProgrammingComponent::Focus),
        },
        DynamicValueAddress {
            representation: DynamicFamilyRepresentation::Focus,
            component: Some(ProgrammingComponent::Focus),
        },
    ] {
        assert!(
            DynamicSampleExpression::AngleCurrent {
                address: Arc::new(address)
            }
            .validate()
            .is_err()
        );
    }

    let target = FixtureId::new();
    let pan = ramp(
        pan_address(),
        value(DynamicValue::Scalar(90.0)),
        value(DynamicValue::Scalar(90.0)),
    );
    let mut definition = definition(pan.clone());
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    let c = controller(1, 1, true);
    let mut request = start_request(definition.id, c.clone(), target, 0, false);
    request.activation_policy_override = Some(ActivationPolicy::JoinSyncNow);
    let instance = runtime.start(request).unwrap();
    runtime
        .set_controller_paused(instance, c.id, true, 0)
        .unwrap();
    let sources = TypedSources {
        current: Some(DynamicValue::Scalar(15.0)),
        preset: None,
        calls: Cell::new(0),
    };
    let first = sampled(&mut runtime, instance, 100, &sources);
    assert_eq!(
        first.len(),
        2,
        "pause before first sample captures the original pair once"
    );
    let DynamicLaneBody::Programming(body) = &mut definition.lanes[0].body else {
        panic!()
    };
    body.address.component = Some(ProgrammingComponent::Tilt);
    runtime.install_definitions([definition]).unwrap();
    let next = sampled(&mut runtime, instance, 200, &sources);
    assert_angle_expression(
        &bundle_position_sample_expressions(&next, &sources)
            .unwrap()
            .position
            .unwrap()
            .expression,
        90.0,
        15.0,
    );
}

#[test]
fn emitted_angle_partner_retains_static_current_until_pair_assembly_and_after_restore() {
    let target = FixtureId::new();
    let definition = definition(ramp(
        pan_address(),
        value(DynamicValue::Scalar(90.0)),
        value(DynamicValue::Scalar(90.0)),
    ));
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    let controller = controller(1, 1, false);
    let mut request = start_request(definition.id, controller.clone(), target, 0, false);
    request.activation_policy_override = Some(ActivationPolicy::JoinSyncNow);
    let instance = runtime.start(request).unwrap();
    let mut sources = TypedSources {
        current: Some(DynamicValue::Scalar(10.0)),
        preset: None,
        calls: Cell::new(0),
    };
    let running = sampled(&mut runtime, instance, 100, &sources);
    assert_eq!(running.len(), 2);
    assert_eq!(
        sources.calls.get(),
        0,
        "running partners defer Current adoption"
    );
    let token = &running
        .iter()
        .find(|sample| sample.expression.angle_current_address().is_some())
        .expect("the generated partner must remain an explicit Current source")
        .expression;
    assert_eq!(
        token.angle_current_address().unwrap().component,
        Some(ProgrammingComponent::Tilt),
    );
    assert!(
        token.programming_leaf().is_none(),
        "Current must not become an authored scalar"
    );

    // Retained samples carry the dependency without reading the preliminary frame. The
    // pair assembler's final captured frame is the authority when the source is evaluated.
    sources.current = Some(DynamicValue::Scalar(25.0));
    assert_angle_expression(
        &bundle_position_sample_expressions(&running, &sources)
            .unwrap()
            .position
            .unwrap()
            .expression,
        90.0,
        25.0,
    );
    runtime
        .set_controller_paused(instance, controller.id, true, 100)
        .unwrap();
    let saved = serde_json::from_value(serde_json::to_value(runtime.snapshot()).unwrap()).unwrap();
    runtime.restore_snapshot(saved).unwrap();
    let reads_before_sampling = sources.calls.get();
    let paused = sampled(&mut runtime, instance, 200, &sources);
    assert_eq!(
        sources.calls.get(),
        reads_before_sampling,
        "paused partners also defer adoption"
    );
    let paused_token = &paused
        .iter()
        .find(|sample| sample.expression.angle_current_address().is_some())
        .unwrap()
        .expression;
    assert_eq!(paused_token, token);
    sources.current = Some(DynamicValue::Scalar(-45.0));
    assert_angle_expression(
        &bundle_position_sample_expressions(&paused, &sources)
            .unwrap()
            .position
            .unwrap()
            .expression,
        90.0,
        -45.0,
    );

    for invalid in [
        DynamicValue::Scalar(f32::NAN),
        DynamicValue::Scalar(f32::INFINITY),
        DynamicValue::Family(AttributeValue::Position(Arc::new(PositionIntent::angles(
            5.0, 10.0,
        )))),
    ] {
        sources.current = Some(invalid);
        let pending = sampled(&mut runtime, instance, 250, &sources);
        assert!(matches!(
            prepare_dynamic_family_samples(
                &pending,
                &sources,
                None,
                &mut DynamicFamilyPreparationScratch::default(),
            ),
            Err(light_core::programming::TransitionError::Invalid(_)),
        ));
    }
    sources.current = None;
    let missing = sampled(&mut runtime, instance, 300, &sources);
    assert_eq!(
        missing.len(),
        2,
        "unavailable Current remains symbolic until preparation"
    );
    assert!(
        bundle_position_sample_expressions(&missing, &sources)
            .unwrap()
            .position
            .is_none()
    );
}

#[test]
fn running_and_paused_angle_partner_adoption_first_occurs_in_final_family_preparation() {
    let target = FixtureId::new();
    let definition = definition(ramp(
        pan_address(),
        value(DynamicValue::Scalar(90.0)),
        value(DynamicValue::Scalar(90.0)),
    ));
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    let controller = controller(1, 1, false);
    let mut request = start_request(definition.id, controller.clone(), target, 0, false);
    request.activation_policy_override = Some(ActivationPolicy::JoinSyncNow);
    let instance = runtime.start(request).unwrap();
    let mut preparation = DynamicFamilyPreparationScratch::default();
    for (at, final_tilt) in [(100, 25.0), (200, -45.0)] {
        let mut sources = TypedSources {
            current: None,
            preset: None,
            calls: Cell::new(0),
        };
        let pending = sampled(&mut runtime, instance, at, &sources);
        assert_eq!(pending.len(), 2);
        assert_eq!(
            sources.calls.get(),
            0,
            "preliminary geometry must not be queried"
        );
        sources.current = Some(DynamicValue::Scalar(final_tilt));
        let prepared =
            prepare_dynamic_family_samples(&pending, &sources, None, &mut preparation).unwrap();
        assert!(prepared.requirements.is_empty());
        assert_eq!(prepared.families.len(), 1);
        let FamilyCompositionSample::CoupledExpression { expression, .. } =
            &prepared.families[0].samples[0]
        else {
            panic!("one complete Position cohort")
        };
        assert!(matches!(
            expression.footprint(CoupledExpressionRole::Base),
            CoupledExpressionFootprint::Exact { value, .. }
                if value == &DynamicValue::Family(AttributeValue::Position(Arc::new(PositionIntent::angles(90.0, final_tilt))))
        ));
        assert_eq!(
            sources.calls.get(),
            1,
            "one late value/adoption read per address"
        );
        if at == 100 {
            runtime
                .set_controller_paused(instance, controller.id, true, at)
                .unwrap();
            let saved =
                serde_json::from_value(serde_json::to_value(runtime.snapshot()).unwrap()).unwrap();
            runtime.restore_snapshot(saved).unwrap();
        }
    }
}

#[test]
fn captured_empty_source_set_stays_empty_through_paused_hot_edit_and_restore() {
    let target = FixtureId::new();
    let address = DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Focus,
        component: Some(ProgrammingComponent::Focus),
    };
    let original = ramp(
        address.clone(),
        DynamicValueSource::Current,
        DynamicValueSource::Current,
    );
    let mut definition = definition(original.clone());
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    let c = controller(1, 1, false);
    let mut request = start_request(definition.id, c.clone(), target, 0, false);
    request.activation_policy_override = Some(ActivationPolicy::JoinSyncNow);
    let instance = runtime.start(request).unwrap();
    let sources = TypedSources {
        current: None,
        preset: None,
        calls: Cell::new(0),
    };
    assert!(sampled(&mut runtime, instance, 100, &sources).is_empty());
    runtime
        .set_controller_paused(instance, c.id, true, 100)
        .unwrap();
    definition.lanes = vec![ramp(
        address,
        value(DynamicValue::Scalar(0.5)),
        value(DynamicValue::Scalar(0.5)),
    )];
    definition.lanes[0].id = original.id;
    runtime.install_definitions([definition]).unwrap();
    let saved = runtime.snapshot();
    assert!(saved.instances[0].synchronized_hold_captured);
    runtime.restore_snapshot(saved).unwrap();
    assert!(sampled(&mut runtime, instance, 200, &sources).is_empty());
}

#[test]
fn whole_angles_current_follows_the_live_frame_while_paused_and_restored() {
    let target = FixtureId::new();
    let address = DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Angles,
        component: None,
    };
    let token = DynamicSampleExpression::AngleCurrent {
        address: Arc::new(address.clone()),
    };
    token.validate().unwrap();
    let definition = definition(DynamicLane {
        body: DynamicLaneBody::Programming(ProgrammingLaneBody {
            address,
            configuration: ProgrammingLaneConfiguration::Keyframes(KeyframeConfiguration {
                points: [0.0, 0.5]
                    .into_iter()
                    .map(|position| DynamicKeyframe {
                        position,
                        source: DynamicValueSource::Current,
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
    let c = controller(1, 1, false);
    let mut request = start_request(definition.id, c.clone(), target, 0, false);
    request.activation_policy_override = Some(ActivationPolicy::JoinSyncNow);
    let instance = runtime.start(request).unwrap();
    let mut sources = TypedSources {
        current: Some(DynamicValue::Family(AttributeValue::Position(Arc::new(
            PositionIntent::angles(90.0, 25.0),
        )))),
        preset: None,
        calls: Cell::new(0),
    };
    let running = sampled(&mut runtime, instance, 100, &sources);
    assert_eq!(running[0].expression, token);
    assert_angle_expression(
        &bundle_position_sample_expressions(&running, &sources)
            .unwrap()
            .position
            .unwrap()
            .expression,
        90.0,
        25.0,
    );
    runtime
        .set_controller_paused(instance, c.id, true, 100)
        .unwrap();
    let saved = runtime.snapshot();
    let crate::runtime::DynamicHeldPayload::TapeRoot { tape_root } =
        saved.instances[0].synchronized_hold_values[0].payload
    else {
        panic!("shared history root");
    };
    assert_eq!(
        DynamicSampleExpression::Retained {
            tape: saved.instances[0].expression_tape.clone().unwrap(),
            root: tape_root
        },
        token
    );
    runtime.restore_snapshot(saved).unwrap();
    sources.current = Some(DynamicValue::Family(AttributeValue::Position(Arc::new(
        PositionIntent::angles(450.0, -45.0),
    ))));
    let restored = sampled(&mut runtime, instance, 200, &sources);
    assert_eq!(restored[0].expression, token);
    assert_angle_expression(
        &bundle_position_sample_expressions(&restored, &sources)
            .unwrap()
            .position
            .unwrap()
            .expression,
        450.0,
        -45.0,
    );
    sources.current = None;
    let pending = sampled(&mut runtime, instance, 300, &sources);
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].expression, token);
    let mut preparation = DynamicFamilyPreparationScratch::default();
    let prepared =
        prepare_dynamic_family_samples(&pending, &sources, None, &mut preparation).unwrap();
    assert!(prepared.families.is_empty());
    assert_eq!(prepared.requirements.len(), 1);
}

#[test]
fn recorded_pan_selection_restores_live_tilt_current_even_during_synchronized_pause() {
    let target = FixtureId::new();
    let pan = ramp(
        pan_address(),
        value(DynamicValue::Scalar(-180.0)),
        value(DynamicValue::Scalar(180.0)),
    );
    let mut definition = definition(pan.clone());
    definition.default_activation = ActivationPolicy::JoinSyncNow;
    definition.speed = DynamicSpeed::SpeedGroup {
        group: SpeedGroup::A,
        beats_per_cycle: Rational::ONE,
    };
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    let c = controller(1, 1, false);
    let instance = runtime
        .start(start_request(definition.id, c.clone(), target, 0, false))
        .unwrap();
    runtime
        .set_controller_lane_selection(
            instance,
            c.id,
            DynamicLaneSelection::PerTarget {
                targets: vec![DynamicTargetLanes {
                    target,
                    lanes: vec![pan.id],
                }],
            },
        )
        .unwrap();
    let mut sources = TypedSources {
        current: Some(DynamicValue::Scalar(25.0)),
        preset: None,
        calls: Cell::new(0),
    };
    let samples = sampled(&mut runtime, instance, 750, &sources);
    assert_eq!(samples.len(), 2);
    assert!(samples[1].expression.angle_current_address().is_some());
    assert_eq!(
        inspected_typed_value(&samples[1], &sources),
        DynamicValue::Scalar(25.0)
    );
    runtime
        .set_controller_paused(instance, c.id, true, 750)
        .unwrap();
    let snapshot = serde_json::to_value(runtime.snapshot()).unwrap();
    runtime
        .restore_snapshot(serde_json::from_value(snapshot).unwrap())
        .unwrap();
    sources.current = Some(DynamicValue::Scalar(-45.0));
    let paused = sampled(&mut runtime, instance, 1250, &sources);
    assert_eq!(
        typed_value(&paused[0]),
        typed_value(&samples[0]),
        "animated Pan holds its phase"
    );
    assert_eq!(
        inspected_typed_value(&paused[1], &sources),
        DynamicValue::Scalar(-45.0),
        "static Tilt remains live"
    );
    sources.current = None;
    let unavailable = sampled(&mut runtime, instance, 1500, &sources);
    assert_eq!(
        unavailable.len(),
        2,
        "the held Tilt dependency remains symbolic"
    );
    assert_eq!(unavailable[0].lane_id, pan.id);
    let mut preparation = DynamicFamilyPreparationScratch::default();
    let prepared =
        prepare_dynamic_family_samples(&unavailable, &sources, None, &mut preparation).unwrap();
    assert!(
        prepared.families.is_empty(),
        "no stale held Tilt may replace missing Current"
    );
    assert_eq!(prepared.requirements.len(), 1);
}

#[test]
fn runtime_uses_typed_current_presets_and_size_without_legacy_normalization_or_feedback() {
    let target = FixtureId::new();
    let address = pan_address();
    let high = DynamicValueSource::Preset {
        retained: None,
        preset_id: "pan-preset".into(),
        address: address.clone(),
        last_valid_by_target: vec![DynamicValueFallback {
            target,
            value: DynamicValue::Scalar(720.0),
        }],
    };
    let definition = definition(ramp(address, DynamicValueSource::Current, high));
    let sources = TypedSources {
        current: Some(DynamicValue::Scalar(-90.0)),
        preset: None,
        calls: Cell::new(0),
    };
    for (size, expected) in [
        (0.0, None),
        (0.5, Some(213.75)),
        (1.0, Some(517.5)),
        (2.0, Some(1125.0)),
    ] {
        let mut runtime = DynamicRuntime::default();
        runtime.install_definitions([definition.clone()]).unwrap();
        let mut c = controller(1, 1, false);
        c.size = size;
        let instance = runtime
            .start(start_request(definition.id, c, target, 0, false))
            .unwrap();
        for _ in 0..2 {
            let samples = sampled(&mut runtime, instance, 750, &sources);
            assert_eq!(
                samples.first().map(typed_value),
                expected
                    .as_ref()
                    .map(|value| DynamicValue::Scalar(*value))
                    .as_ref()
            );
            if size == 1.0 {
                assert_eq!(samples[0].activation_mix, 1.0);
            }
        }
    }
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
    let sources = TypedSources {
        preset: Some(DynamicValue::Scalar(270.0)),
        ..sources
    };
    assert_eq!(
        typed_value(&sampled(&mut runtime, instance, 750, &sources)[0]),
        &DynamicValue::Scalar(180.0)
    );
    let unavailable = TypedSources {
        current: None,
        ..sources
    };
    let pending = sampled(&mut runtime, instance, 750, &unavailable);
    assert_eq!(pending.len(), 1, "only the symbolic partner remains");
    let mut preparation = DynamicFamilyPreparationScratch::default();
    assert!(
        prepare_dynamic_family_samples(&pending, &unavailable, None, &mut preparation)
            .unwrap()
            .families
            .is_empty()
    );
}

#[test]
fn typed_hot_edits_preload_pin_and_lane_selection_preserve_runtime_clocks() {
    let target = FixtureId::new();
    let pan = ramp(
        pan_address(),
        value(DynamicValue::Scalar(-720.0)),
        value(DynamicValue::Scalar(720.0)),
    );
    let mut definition = definition(pan.clone());
    let mut tilt = pan.clone();
    tilt.id = Uuid::new_v4();
    if let DynamicLaneBody::Programming(body) = &mut tilt.body {
        body.address.component = Some(ProgrammingComponent::Tilt);
    }
    definition.lanes.push(tilt);
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    let c = controller(1, 1, false);
    let instance = runtime
        .start(start_request(definition.id, c.clone(), target, 0, false))
        .unwrap();
    runtime
        .set_controller_lane_selection(
            instance,
            c.id,
            DynamicLaneSelection::Uniform {
                lanes: vec![pan.id],
            },
        )
        .unwrap();
    let sources = TypedSources {
        current: None,
        preset: None,
        calls: Cell::new(0),
    };
    assert_eq!(
        typed_value(&sampled(&mut runtime, instance, 750, &sources)[0]),
        &DynamicValue::Scalar(360.0)
    );
    assert_eq!(
        sources.calls.get(),
        0,
        "full-size literal lanes do not sample Current"
    );
    runtime.set_definitions_pinned(true);
    if let DynamicLaneBody::Programming(body) = &mut definition.lanes[0].body {
        let ProgrammingLaneConfiguration::MaxMin(config) = &mut body.configuration else {
            panic!()
        };
        config.maximum = value(DynamicValue::Scalar(1440.0));
    }
    runtime.install_definitions([definition]).unwrap();
    let pinned = runtime.snapshot();
    runtime.restore_snapshot(pinned).unwrap();
    assert_eq!(
        typed_value(&sampled(&mut runtime, instance, 750, &sources)[0]),
        &DynamicValue::Scalar(360.0)
    );
    runtime.set_definitions_pinned(false);
    let samples = sampled(&mut runtime, instance, 750, &sources);
    assert_eq!(
        samples.len(),
        2,
        "selecting Pan includes Tilt from the same Dynamic"
    );
    assert_eq!(typed_value(&samples[0]), &DynamicValue::Scalar(900.0));
    assert_eq!(typed_value(&samples[1]), &DynamicValue::Scalar(360.0));
    assert_eq!(runtime.snapshot().instances[0].started_at_millis, 0);
}

#[test]
fn pending_target_transitions_and_whole_family_size_survive_hold_and_restore() {
    let target = FixtureId::new();
    let target_value = |reference| {
        DynamicValue::Family(AttributeValue::Position(Arc::new(PositionIntent::target(
            reference,
            [1.0, 2.0, 3.0],
        ))))
    };
    let from = target_value(TargetReference::Origin);
    let to = target_value(TargetReference::Point {
        point_id: Uuid::from_u128(9),
    });
    let lane = DynamicLane {
        body: DynamicLaneBody::Programming(ProgrammingLaneBody {
            address: DynamicValueAddress {
                representation: DynamicFamilyRepresentation::Target { reference: None },
                component: None,
            },
            configuration: ProgrammingLaneConfiguration::Keyframes(KeyframeConfiguration {
                points: vec![
                    DynamicKeyframe {
                        position: 0.0,
                        source: value(from.clone()),
                        interpolation: ScalarInterpolation::Linear,
                    },
                    DynamicKeyframe {
                        position: 0.5,
                        source: value(to),
                        interpolation: ScalarInterpolation::Linear,
                    },
                ],
                size: 1.0,
            }),
        }),
        ..lane()
    };
    let definition = definition(lane);
    for baseline in [
        from.clone(),
        DynamicValue::Family(AttributeValue::Position(Arc::new(PositionIntent::angles(
            90.0, 45.0,
        )))),
    ] {
        let sources = TypedSources {
            current: Some(baseline.clone()),
            preset: None,
            calls: Cell::new(0),
        };
        for factor in [0.0, 0.5, 1.0, 2.0] {
            let mut runtime = DynamicRuntime::default();
            runtime.install_definitions([definition.clone()]).unwrap();
            let mut c = controller(1, 1, false);
            c.size = factor;
            let mut request = start_request(definition.id, c.clone(), target, 0, false);
            request.activation_policy_override = Some(ActivationPolicy::JoinSyncNow);
            let instance = runtime.start(request).unwrap();
            let samples = sampled(&mut runtime, instance, 250, &sources);
            if factor == 0.0 {
                assert!(samples.is_empty());
                continue;
            }
            let expression = &samples[0].expression;
            expression.validate().unwrap();
            if factor == 1.0 {
                assert!(matches!(
                    expression.unannotated(),
                    DynamicSampleExpression::Transition { progress: 0.5, .. }
                ));
            } else {
                assert!(
                    matches!(expression.unannotated(), DynamicSampleExpression::Scale { base, value, factor: saved, .. } if base == &baseline && *saved == factor && matches!(value.unannotated(), DynamicSampleExpression::Transition { progress: 0.5, .. }))
                );
            }
            runtime
                .set_controller_paused(instance, c.id, true, 250)
                .unwrap();
            assert_eq!(
                sampled(&mut runtime, instance, 450, &sources)[0].expression,
                *expression
            );
            let saved =
                serde_json::from_value(serde_json::to_value(runtime.snapshot()).unwrap()).unwrap();
            let mut restored = DynamicRuntime::default();
            restored.restore_snapshot(saved).unwrap();
            assert_eq!(
                sampled(&mut restored, instance, 900, &sources)[0].expression,
                *expression
            );
        }
    }
}

struct NativeModel {
    source: NativeColorIdentity,
    bindings: [NativeColorBinding; 2],
}
impl NativeColorEditModel for NativeModel {
    fn source(&self) -> &NativeColorIdentity {
        &self.source
    }
    fn descriptor(&self, binding: NativeColorBinding) -> Option<NativeColorComponentDescriptor> {
        self.bindings
            .iter()
            .position(|candidate| *candidate == binding)
            .map(|index| NativeColorComponentDescriptor {
                binding,
                raw_from: 0,
                raw_to: if index == 0 { u32::MAX } else { 65535 },
                continuous: true,
            })
    }
    fn predict(&self, _: &NativeColorRecipe) -> Result<PortableColorEstimate, IntentError> {
        panic!("predict only after composition")
    }
}
struct NativeModels {
    model: Arc<NativeModel>,
    calls: AtomicUsize,
}
impl DynamicNativeModelResolver for NativeModels {
    fn resolve(
        &self,
        source: &NativeColorIdentity,
    ) -> Result<Arc<dyn NativeColorEditModel + Send + Sync>, IntentError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if source != &self.model.source {
            return Err(IntentError("pinned source unavailable".into()));
        }
        Ok(self.model.clone())
    }
}

#[test]
fn native_runtime_retains_low_bits_and_compiles_models_only_at_cold_boundaries() {
    let models = Arc::new(NativeModels {
        model: Arc::new(NativeModel {
            source: NativeColorIdentity {
                profile_id: Uuid::from_u128(1),
                profile_revision: 2,
                profile_digest: "pinned".into(),
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
    });
    let address = DynamicValueAddress {
        representation: DynamicFamilyRepresentation::DirectColor {
            source: models.model.source.clone(),
        },
        component: Some(ProgrammingComponent::NativeColor(models.model.bindings[0])),
    };
    let mut definition = definition(ramp(
        address,
        value(DynamicValue::Native(u32::MAX - 4)),
        value(DynamicValue::Native(u32::MAX)),
    ));
    DynamicRuntime::default()
        .install_definitions([definition.clone()])
        .unwrap();
    let mut runtime = DynamicRuntime::with_native_color_models(1, models.clone());
    runtime.install_definitions([definition.clone()]).unwrap();
    runtime.install_definitions([definition.clone()]).unwrap();
    assert_eq!(models.calls.load(Ordering::SeqCst), 1);
    let target = FixtureId::new();
    let mut c = controller(1, 1, false);
    c.size = 0.5;
    let instance = runtime
        .start(start_request(definition.id, c, target, 0, false))
        .unwrap();
    let sources = TypedSources {
        current: Some(DynamicValue::Native(u32::MAX - 4)),
        preset: None,
        calls: Cell::new(0),
    };
    for _ in 0..3 {
        assert_eq!(
            typed_value(&sampled(&mut runtime, instance, 500, &sources)[0]),
            &DynamicValue::Native(u32::MAX - 3)
        );
    }
    assert_eq!(
        models.calls.load(Ordering::SeqCst),
        1,
        "no profile lookup per target/tick"
    );
    let before = runtime.snapshot();
    if let DynamicLaneBody::Programming(body) = &mut definition.lanes[0].body {
        body.address.component = Some(ProgrammingComponent::NativeColor(models.model.bindings[1]));
    }
    assert!(
        runtime.install_definitions([definition]).is_err(),
        "narrower native function cannot consume full-u32 sources"
    );
    assert_eq!(runtime.snapshot(), before, "invalid hot edit is atomic");
    let mut restored = DynamicRuntime::with_native_color_models(1, models.clone());
    restored.restore_snapshot(before.clone()).unwrap();
    assert_eq!(
        sampled(&mut restored, instance, 500, &sources),
        sampled(&mut runtime, instance, 500, &sources)
    );
    let mut unavailable = DynamicRuntime::default();
    unavailable.restore_snapshot(before.clone()).unwrap();
    assert!(!unavailable.unavailable_native_sources().is_empty());
    assert!(sampled(&mut unavailable, instance, 500, &sources).is_empty());
    let mut invalid = before.clone();
    let stored = &mut invalid.instances[0];
    stored.definition.lanes = vec![lane()];
    let crate::runtime::DynamicHeldPayload::TapeRoot { tape_root } =
        stored.last_sample_values[0].payload
    else {
        panic!()
    };
    let tape = Arc::make_mut(stored.expression_tape.as_mut().unwrap());
    let RetainedExpressionNode::Programming { address, .. } = &mut tape.nodes[tape_root.0 as usize]
    else {
        panic!()
    };
    address.component = Some(ProgrammingComponent::NativeColor(models.model.bindings[1]));
    assert!(
        restored.restore_snapshot(invalid).is_err(),
        "retained native values must validate their original function even after a definition edit"
    );
    assert_eq!(restored.snapshot(), before);
    let mut random = before.instances[0].definition.clone();
    let group_id = Uuid::new_v4();
    random.lanes[0].random_group_id = Some(group_id);
    if let DynamicLaneBody::Programming(body) = &mut random.lanes[0].body {
        body.configuration = ProgrammingLaneConfiguration::Random;
    }
    let mut second = random.lanes[0].clone();
    second.id = Uuid::new_v4();
    if let DynamicLaneBody::Programming(body) = &mut second.body {
        body.address.component = Some(ProgrammingComponent::NativeColor(models.model.bindings[1]));
    }
    random.lanes.push(second);
    random.random_groups = vec![DynamicRandomGroup {
        id: group_id,
        seed: 1,
        range: DynamicRandomRange::Programming {
            low: value(DynamicValue::Native(0)),
            high: value(DynamicValue::Native(1)),
        },
        decision_interval_millis: 100,
        start_probability: 0.5,
        mean_duration_millis: 100,
        duration_spread_millis: 0,
        attack_ratio: 0.2,
        decay_ratio: 0.2,
    }];
    validate_definition(&random).unwrap();
    assert!(
        restored
            .install_definitions([random])
            .unwrap_err()
            .to_string()
            .contains("incompatible compiled native/unit domains")
    );
    assert_eq!(restored.snapshot(), before);
}

#[test]
fn held_scope_is_validated_and_repeated_component_resumes_remain_flat() {
    let target = FixtureId::new();
    let definition = definition(ramp(
        pan_address(),
        value(DynamicValue::Scalar(-720.0)),
        value(DynamicValue::Scalar(720.0)),
    ));
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    let c = controller(1, 1, false);
    let mut request = start_request(definition.id, c.clone(), target, 0, false);
    request.activation_policy_override = Some(ActivationPolicy::JoinSyncNow);
    request.activation_duration_millis = 1000;
    let instance = runtime.start(request).unwrap();
    let sources = TypedSources {
        current: None,
        preset: None,
        calls: Cell::new(0),
    };
    sampled(&mut runtime, instance, 100, &sources);
    for index in 0..100 {
        let at = 100 + index * 200;
        runtime
            .set_controller_paused(instance, c.id, true, at)
            .unwrap();
        sampled(&mut runtime, instance, at + 10, &sources);
        runtime
            .set_controller_paused_with_resume(
                instance,
                c.id,
                false,
                at + 20,
                Some(ActivationPolicy::JoinSyncNow),
            )
            .unwrap();
        let samples = sampled(&mut runtime, instance, at + 100, &sources);
        assert!(matches!(
            samples[0].expression,
            DynamicSampleExpression::Programming { .. }
        ));
    }
    let saved = runtime.snapshot();
    runtime.restore_snapshot(saved.clone()).unwrap();
    let mut invalid = saved.clone();
    invalid.instances[0].synchronized_hold_values[0].target = FixtureId::new();
    assert!(runtime.restore_snapshot(invalid).is_err());
    assert_eq!(runtime.snapshot(), saved);
    let mut invalid = saved.clone();
    invalid.instances[0].lane_selections = vec![DynamicControllerLaneSelection {
        controller_id: c.id,
        selection: DynamicLaneSelection::Uniform { lanes: vec![] },
    }];
    assert!(runtime.restore_snapshot(invalid).is_err());
    assert_eq!(runtime.snapshot(), saved);
}

#[test]
fn missing_or_deleted_live_lane_does_not_resurrect_when_paused_later() {
    let target = FixtureId::new();
    for delete_lane in [false, true] {
        let mut definition = definition(ramp(
            pan_address(),
            DynamicValueSource::Current,
            value(DynamicValue::Scalar(720.0)),
        ));
        let pan_id = definition.lanes[0].id;
        definition.lanes.push(lane());
        let mut runtime = DynamicRuntime::default();
        runtime.install_definitions([definition.clone()]).unwrap();
        let c = controller(1, 1, false);
        let mut request = start_request(definition.id, c.clone(), target, 0, false);
        request.activation_policy_override = Some(ActivationPolicy::JoinSyncNow);
        let instance = runtime.start(request).unwrap();
        let mut sources = TypedSources {
            current: Some(DynamicValue::Scalar(-90.0)),
            preset: None,
            calls: Cell::new(0),
        };
        assert_eq!(sampled(&mut runtime, instance, 250, &sources).len(), 3);
        if delete_lane {
            definition.lanes.remove(0);
            runtime.install_definitions([definition]).unwrap();
        } else {
            sources.current = None;
        }
        let samples = sampled(&mut runtime, instance, 300, &sources);
        assert_eq!(samples.len(), if delete_lane { 1 } else { 2 });
        assert!(samples.iter().all(|sample| sample.lane_id != pan_id));
        runtime
            .set_controller_paused(instance, c.id, true, 300)
            .unwrap();
        let samples = sampled(&mut runtime, instance, 400, &sources);
        assert_eq!(
            samples.len(),
            if delete_lane { 1 } else { 2 },
            "a later pause must not revive an unavailable earlier sample"
        );
        assert!(samples.iter().all(|sample| sample.lane_id != pan_id));
        let mut preparation = DynamicFamilyPreparationScratch::default();
        let prepared =
            prepare_dynamic_family_samples(&samples, &sources, None, &mut preparation).unwrap();
        assert!(prepared.families.is_empty());
        assert_eq!(prepared.legacy.len(), 1);
    }
}

#[test]
fn preset_occurrences_keep_per_instance_selection_ranks_and_pinned_generations() {
    struct ScopedSources(HashMap<(Uuid, Uuid, FixtureId), DynamicValue>);
    impl DynamicValueSourceResolver for ScopedSources {
        fn current(&self, _: FixtureId, _: &DynamicValueAddress) -> Option<DynamicValue> {
            None
        }
        fn preset(
            &self,
            source: &DynamicPresetSourceBinding,
            instance: Uuid,
            target: FixtureId,
        ) -> Option<DynamicValue> {
            self.0.get(&(source.id, instance, target)).cloned()
        }
    }
    let shared = FixtureId::new();
    let other = FixtureId::new();
    let third = FixtureId::new();
    let source = DynamicValueSource::Preset {
        retained: None,
        preset_id: "spread".into(),
        address: pan_address(),
        last_valid_by_target: vec![],
    };
    let mut definition = definition(ramp(pan_address(), source.clone(), source));
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    let mut instances = Vec::new();
    for (index, targets) in [vec![shared, other], vec![other, shared, third]]
        .into_iter()
        .enumerate()
    {
        let mut request = start_request(
            definition.id,
            controller(index as u128 + 1, 1, false),
            shared,
            0,
            false,
        );
        request.target_scope.ordered_targets = targets;
        instances.push(runtime.start(request).unwrap());
    }
    let manifest = runtime.preset_source_instances();
    assert_eq!(manifest.len(), 2);
    assert_ne!(
        manifest[0].sources[0].id, manifest[0].sources[1].id,
        "Min and Max are distinct source occurrences"
    );
    assert_eq!(
        manifest[0].sources[0].id, manifest[1].sources[0].id,
        "instances clone the immutable source generation"
    );
    let values = manifest
        .iter()
        .flat_map(|instance| {
            instance.sources.iter().flat_map(move |source| {
                instance
                    .ordered_targets
                    .iter()
                    .enumerate()
                    .map(move |(rank, target)| {
                        (
                            (source.id, instance.instance_id, *target),
                            DynamicValue::Scalar(
                                rank as f32 * 100.0 / (instance.ordered_targets.len() - 1) as f32,
                            ),
                        )
                    })
            })
        })
        .collect();
    let sources = ScopedSources(values);
    for instance in &manifest {
        let records = instance
            .sources
            .iter()
            .map(|source| DynamicPresetSourceValues {
                occurrence: source.occurrence.unwrap(),
                preset_id: source.preset_id.clone(),
                address: source.address.clone(),
                values: instance
                    .ordered_targets
                    .iter()
                    .map(|target| DynamicValueFallback {
                        target: *target,
                        value: sources.0[&(source.id, instance.instance_id, *target)].clone(),
                    })
                    .collect(),
            })
            .collect();
        assert!(
            runtime
                .install_preset_source_values(instance, records)
                .unwrap()
        );
    }
    let stored: DynamicRuntimeSnapshot =
        serde_json::from_slice(&serde_json::to_vec(&runtime.snapshot()).unwrap()).unwrap();
    let mut restored = DynamicRuntime::default();
    restored.restore_snapshot(stored.clone()).unwrap();
    let empty = ScopedSources(HashMap::new());
    for (instance, expected) in instances.iter().zip([0.0, 50.0]) {
        let samples = restored
            .sample_programming(*instance, 250, 1000, 10, &Sources { current: 0.0 }, &empty)
            .unwrap();
        assert_eq!(
            typed_value(
                samples
                    .iter()
                    .find(|sample| sample.target == shared)
                    .unwrap()
            ),
            &DynamicValue::Scalar(expected)
        );
    }
    let older = restored.preset_source_instances()[0].clone();
    assert!(restored.invalidate_preset_source_dependencies(older.instance_id));
    let newer = restored.preset_source_instances()[0].clone();
    let mut recalculated = newer.last_valid.clone();
    recalculated[0].values[0].value = DynamicValue::Scalar(123.0);
    assert!(
        restored
            .install_preset_source_values(&newer, recalculated.clone())
            .unwrap()
    );
    assert!(
        !restored
            .install_preset_source_values(&older, older.last_valid.clone())
            .unwrap(),
        "delayed Group/geometry compilation must not replace a newer result"
    );
    assert_eq!(
        restored.preset_source_instances()[0].last_valid,
        recalculated
    );
    let mut invalid = stored;
    invalid.instances[0].preset_source_values[0].values[0].target = FixtureId::new();
    assert!(restored.restore_snapshot(invalid).is_err());
    for (instance, expected) in instances.iter().zip([0.0, 50.0]) {
        let samples = runtime
            .sample_programming(
                *instance,
                250,
                1000,
                10,
                &Sources { current: 0.0 },
                &sources,
            )
            .unwrap();
        assert_eq!(
            typed_value(
                samples
                    .iter()
                    .find(|sample| sample.target == shared)
                    .unwrap()
            ),
            &DynamicValue::Scalar(expected)
        );
    }
    runtime.set_definitions_pinned(true);
    if let DynamicLaneBody::Programming(body) = &mut definition.lanes[0].body {
        let ProgrammingLaneConfiguration::MaxMin(config) = &mut body.configuration else {
            panic!()
        };
        let DynamicValueSource::Preset { preset_id, .. } = &mut config.maximum else {
            panic!()
        };
        *preset_id = "new-source".into();
    }
    runtime.install_definitions([definition]).unwrap();
    assert_eq!(
        runtime.preset_source_instances()[0].sources[1].id,
        manifest[0].sources[1].id
    );
    runtime.set_definitions_pinned(false);
    assert_ne!(
        runtime.preset_source_instances()[0].sources[1].id,
        manifest[0].sources[1].id
    );
    assert!(
        !runtime
            .install_preset_source_values(&manifest[0], vec![])
            .unwrap(),
        "stale source generation cannot overwrite current values"
    );
    assert!(
        runtime
            .snapshot()
            .instances
            .iter()
            .all(|instance| instance.preset_source_values.len() == 1),
        "changing maximum's source leaves minimum's independent fallback intact"
    );
    assert!(
        runtime
            .snapshot()
            .instances
            .iter()
            .all(|instance| instance.started_at_millis == 0)
    );
}

#[test]
fn inserting_a_keyframe_does_not_attach_an_old_same_preset_checkpoint_to_the_new_point() {
    let target = FixtureId::new();
    let preset = DynamicValueSource::Preset {
        preset_id: "shared".into(),
        address: pan_address(),
        retained: None,
        last_valid_by_target: vec![],
    };
    let point = |position| DynamicKeyframe {
        position,
        source: preset.clone(),
        interpolation: ScalarInterpolation::Linear,
    };
    let mut definition = definition(DynamicLane {
        body: DynamicLaneBody::Programming(ProgrammingLaneBody {
            address: pan_address(),
            configuration: ProgrammingLaneConfiguration::Keyframes(KeyframeConfiguration {
                points: vec![point(0.0), point(0.5)],
                size: 1.0,
            }),
        }),
        ..lane()
    });
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    runtime
        .start(start_request(
            definition.id,
            controller(1, 1, false),
            target,
            0,
            false,
        ))
        .unwrap();
    let manifest = runtime.preset_source_instances()[0].clone();
    let records = manifest
        .sources
        .iter()
        .enumerate()
        .map(|(index, source)| DynamicPresetSourceValues {
            occurrence: source.occurrence.unwrap(),
            preset_id: source.preset_id.clone(),
            address: source.address.clone(),
            values: vec![DynamicValueFallback {
                target,
                value: DynamicValue::Scalar(10.0 + index as f32),
            }],
        })
        .collect();
    runtime
        .install_preset_source_values(&manifest, records)
        .unwrap();
    let DynamicLaneBody::Programming(body) = &mut definition.lanes[0].body else {
        panic!()
    };
    let ProgrammingLaneConfiguration::Keyframes(config) = &mut body.configuration else {
        panic!()
    };
    config.points.insert(1, point(0.25));
    runtime.install_definitions([definition]).unwrap();
    let retained = &runtime.snapshot().instances[0].preset_source_values;
    assert_eq!(retained.len(), 1);
    assert_eq!(
        retained[0].occurrence,
        manifest.sources[0].occurrence.unwrap()
    );
    assert_eq!(retained[0].values[0].value, DynamicValue::Scalar(10.0));
}
