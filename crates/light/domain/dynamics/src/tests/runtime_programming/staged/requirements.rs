use super::*;
use light_core::programming::{ProgrammingOwner, TransitionError, TransitionRequirement};

struct FallibleSources {
    blocked: Cell<Option<ProgrammingOwner>>,
    reason: TransitionRequirement,
    reads: Cell<usize>,
    base_reads: Cell<usize>,
    malformed_preset: bool,
}

impl FallibleSources {
    fn blocked(owner: ProgrammingOwner) -> Self {
        Self {
            blocked: Cell::new(Some(owner)),
            reason: TransitionRequirement::MaterializedEndpoints,
            reads: Cell::new(0),
            base_reads: Cell::new(0),
            malformed_preset: false,
        }
    }
}

impl DynamicValueSourceResolver for FallibleSources {
    fn current(&self, target: FixtureId, address: &DynamicValueAddress) -> Option<DynamicValue> {
        self.try_current(target, address).ok().flatten()
    }
    fn try_current(
        &self,
        _: FixtureId,
        address: &DynamicValueAddress,
    ) -> Result<Option<DynamicValue>, TransitionError> {
        self.reads.set(self.reads.get() + 1);
        if self.blocked.get() == Some(address.owner()) {
            return Err(TransitionError::Requires(self.reason));
        }
        Ok(Some(if address.component.is_some() {
            DynamicValue::Scalar(0.4)
        } else {
            DynamicValue::Family(AttributeValue::Normalized(0.4))
        }))
    }
    fn try_current_family_base(
        &self,
        _: FixtureId,
        address: &DynamicValueAddress,
    ) -> Result<Option<AttributeValue>, TransitionError> {
        self.base_reads.set(self.base_reads.get() + 1);
        if self.blocked.get() == Some(address.owner()) {
            Err(TransitionError::Requires(self.reason))
        } else {
            Ok(Some(AttributeValue::Normalized(0.4)))
        }
    }
    fn preset(
        &self,
        _: &DynamicPresetSourceBinding,
        _: Uuid,
        _: FixtureId,
    ) -> Option<DynamicValue> {
        self.malformed_preset
            .then_some(DynamicValue::Scalar(f32::NAN))
    }
}

struct PreparedFrame {
    samples: Vec<DynamicRuntimeSample>,
    families: Vec<(ProgrammingOwner, usize)>,
    sampling_requirements: Vec<DynamicFamilyPreparationRequirement>,
    requirements: Vec<DynamicFamilyPreparationRequirement>,
}

fn stage(
    runtime: &mut DynamicRuntime,
    at: u64,
    sources: &dyn DynamicValueSourceResolver,
) -> Result<PreparedFrame, DynamicRuntimeError> {
    let mut transaction = DynamicOutputFrameScratch::default();
    let mut scratch = DynamicSamplingScratch::default();
    let mut preparation = DynamicFamilyPreparationScratch::default();
    runtime.with_output_frame_transaction(&mut transaction, |runtime| {
        runtime.sample_all_programming_staged(
            at,
            10,
            &transports(at),
            &Sources { current: 0.3 },
            sources,
            None,
            &mut scratch,
            |scalar, deferred| {
                let completed = deferred.complete(sources)?;
                assert_eq!(
                    scalar_projection(scalar),
                    scalar_projection(completed.samples())
                );
                let prepared = prepare_dynamic_family_samples_with_requirements(
                    completed.samples(),
                    completed.requirements(),
                    sources,
                    None,
                    &mut preparation,
                )
                .map_err(|error| DynamicRuntimeError::InvalidSample(error.to_string()))?;
                let result = PreparedFrame {
                    samples: completed.samples().to_vec(),
                    families: prepared
                        .families
                        .iter()
                        .map(|group| (group.owner, group.samples.len()))
                        .collect(),
                    sampling_requirements: completed.requirements().to_vec(),
                    requirements: prepared.requirements.to_vec(),
                };
                Ok((completed, result))
            },
        )
    })
}

fn constant_focus() -> DynamicLane {
    let mut lane = focus_lane();
    let DynamicLaneBody::Programming(body) = &mut lane.body else {
        unreachable!()
    };
    body.configuration = ProgrammingLaneConfiguration::MaxMin(MaxMinConfiguration {
        minimum: value(DynamicValue::Scalar(0.8)),
        maximum: value(DynamicValue::Scalar(0.8)),
        function: PeriodicFunction::LinearUp,
        size: 1.0,
        pwm: PwmShape::default(),
    });
    lane
}

#[test]
fn numeric_bounds_middle_pivot_and_controller_size_report_passive_requirements() {
    for configuration in 0..5 {
        let mut affected = focus_lane();
        let DynamicLaneBody::Programming(body) = &mut affected.body else {
            unreachable!()
        };
        match configuration {
            1 => {
                body.configuration =
                    ProgrammingLaneConfiguration::MiddleAmplitude(MiddleAmplitudeConfiguration {
                        middle: DynamicValueSource::Current,
                        amplitude: DynamicValue::Scalar(0.2),
                        function: PeriodicFunction::Sinus,
                        size: 1.0,
                        pwm: PwmShape::default(),
                        invert_waveform: false,
                    })
            }
            2 => {
                body.configuration =
                    ProgrammingLaneConfiguration::Keyframes(KeyframeConfiguration {
                        points: [
                            (0.0, DynamicValueSource::Current),
                            (0.33, value(DynamicValue::Scalar(0.4))),
                            (0.66, value(DynamicValue::Scalar(0.8))),
                        ]
                        .into_iter()
                        .map(|(position, source)| DynamicKeyframe {
                            position,
                            source,
                            interpolation: ScalarInterpolation::Linear,
                        })
                        .collect(),
                        size: 0.5,
                    })
            }
            3 => affected = constant_focus(),
            4 => {
                body.address.component = None;
                body.configuration =
                    ProgrammingLaneConfiguration::Keyframes(KeyframeConfiguration {
                        points: [0.0, 0.5]
                            .into_iter()
                            .map(|position| DynamicKeyframe {
                                position,
                                source: value(DynamicValue::Family(AttributeValue::Normalized(
                                    0.8,
                                ))),
                                interpolation: ScalarInterpolation::Linear,
                            })
                            .collect(),
                        size: 1.0,
                    });
            }
            _ => {}
        }
        let affected_id = affected.id;
        let mut definition = definition(affected);
        definition.lanes.push(point_lane("point.position.x", 0.6));
        if configuration < 3 {
            definition.lanes.push(constant_focus());
        }
        let mut runtime = DynamicRuntime::default();
        runtime.install_definitions([definition.clone()]).unwrap();
        let mut control = controller(131, 1, false);
        if configuration >= 3 {
            control.size = 0.5;
        }
        runtime
            .start(start_request(
                definition.id,
                control,
                FixtureId::new(),
                0,
                false,
            ))
            .unwrap();
        let sources = FallibleSources::blocked(ProgrammingOwner::Focus);
        let result = stage(&mut runtime, 500, &sources).unwrap();
        assert_eq!(
            result.requirements.len(),
            1,
            "configuration {configuration}"
        );
        assert_eq!(result.sampling_requirements, result.requirements);
        assert_eq!(
            result.requirements[0]
                .rank
                .dynamic_identity()
                .unwrap()
                .lane_id,
            affected_id
        );
        assert_eq!(
            result.requirements[0].reason,
            DynamicFamilyPreparationRequirementReason::Transition(sources.reason)
        );
        assert!(
            !result
                .samples
                .iter()
                .any(|sample| sample.lane_id == affected_id),
            "an unresolved Size must not emit unscaled output"
        );
        assert_eq!(scalar_projection(&result.samples).len(), 1);
        assert_eq!(
            result.families,
            if configuration < 3 {
                vec![(ProgrammingOwner::Focus, 1)]
            } else {
                vec![]
            }
        );
        if configuration == 4 {
            assert_eq!(sources.reads.get(), 0);
            assert_eq!(sources.base_reads.get(), 1);
        } else {
            assert!(sources.reads.get() > 0);
            assert_eq!(sources.base_reads.get(), 0);
        }
    }
}

#[test]
fn numeric_position_requirement_withholds_its_partner_but_keeps_independent_owners() {
    let pan = ramp(
        pan_address(),
        DynamicValueSource::Current,
        DynamicValueSource::Current,
    );
    let mut definition = definition(pan);
    definition
        .lanes
        .extend([constant_focus(), point_lane("point.position.x", 0.6)]);
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    runtime
        .start(start_request(
            definition.id,
            controller(132, 1, false),
            FixtureId::new(),
            0,
            false,
        ))
        .unwrap();
    let sources = FallibleSources {
        reason: TransitionRequirement::LiveTargetPoints,
        ..FallibleSources::blocked(ProgrammingOwner::Position)
    };
    let result = stage(&mut runtime, 500, &sources).unwrap();
    assert_eq!(result.families, vec![(ProgrammingOwner::Focus, 1)]);
    assert_eq!(result.requirements.len(), 1);
    assert_eq!(
        result.requirements[0].reason,
        DynamicFamilyPreparationRequirementReason::Transition(
            TransitionRequirement::LiveTargetPoints
        )
    );
    assert_eq!(
        sources.reads.get(),
        2,
        "the withheld symbolic partner is never adopted"
    );
    assert_eq!(scalar_projection(&result.samples).len(), 1);
}

#[test]
fn symbolic_partner_preserves_the_late_adoption_requirement_reason() {
    let pan = ramp(
        pan_address(),
        value(DynamicValue::Scalar(90.0)),
        value(DynamicValue::Scalar(90.0)),
    );
    let mut definition = definition(pan);
    definition
        .lanes
        .extend([constant_focus(), point_lane("point.position.x", 0.6)]);
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    runtime
        .start(start_request(
            definition.id,
            controller(133, 1, false),
            FixtureId::new(),
            0,
            false,
        ))
        .unwrap();
    let sources = FallibleSources {
        reason: TransitionRequirement::LiveTargetPoints,
        ..FallibleSources::blocked(ProgrammingOwner::Position)
    };
    let result = stage(&mut runtime, 500, &sources).unwrap();
    assert!(result.sampling_requirements.is_empty());
    assert_eq!(result.families, vec![(ProgrammingOwner::Focus, 1)]);
    assert_eq!(result.requirements.len(), 1);
    assert_eq!(
        result.requirements[0].reason,
        DynamicFamilyPreparationRequirementReason::Transition(
            TransitionRequirement::LiveTargetPoints
        )
    );
    assert_eq!(sources.reads.get(), 1);
}

fn retained_rows(
    snapshot: &DynamicInstanceSnapshot,
    held: bool,
) -> HashMap<Uuid, DynamicSampleExpression> {
    let rows = if held {
        &snapshot.synchronized_hold_values
    } else {
        &snapshot.last_sample_values
    };
    rows.iter()
        .map(|row| {
            let expression = match &row.payload {
                crate::runtime::DynamicHeldPayload::TapeRoot { tape_root } => {
                    DynamicSampleExpression::Retained {
                        tape: snapshot.expression_tape.clone().unwrap(),
                        root: *tape_root,
                    }
                }
                crate::runtime::DynamicHeldPayload::Expression { expression } => expression.clone(),
                _ => panic!("modern checkpoint"),
            };
            (row.lane_id, expression)
        })
        .collect()
}

#[test]
fn completed_unresolved_resume_survives_restore_then_retires_without_restarting_clocks() {
    let pan = ramp(
        pan_address(),
        value(DynamicValue::Scalar(90.0)),
        value(DynamicValue::Scalar(90.0)),
    );
    let pan_id = pan.id;
    let mut definition = definition(pan);
    definition
        .lanes
        .extend([constant_focus(), point_lane("point.position.x", 0.6)]);
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    let control = controller(134, 1, false);
    let mut request = start_request(definition.id, control.clone(), FixtureId::new(), 0, false);
    request.activation_policy_override = Some(ActivationPolicy::JoinSyncNow);
    request.activation_duration_millis = 1000;
    let instance = runtime.start(request).unwrap();
    let sources = FallibleSources::blocked(ProgrammingOwner::Position);
    sources.blocked.set(None);
    stage(&mut runtime, 2000, &sources).unwrap();
    runtime
        .set_controller_paused(instance, control.id, true, 2000)
        .unwrap();
    let original = runtime.snapshot().instances.remove(0);
    let original_held = retained_rows(&original, true);
    let original_last = retained_rows(&original, false);
    let mut incoming = ramp(
        pan_address(),
        DynamicValueSource::Current,
        DynamicValueSource::Current,
    );
    incoming.id = pan_id;
    definition.lanes[0] = incoming;
    definition.revision += 1;
    runtime.install_definitions([definition.clone()]).unwrap();
    runtime
        .set_controller_paused_with_resume(
            instance,
            control.id,
            false,
            2300,
            Some(ActivationPolicy::JoinSyncNow),
        )
        .unwrap();
    let resumed = runtime.snapshot().instances.remove(0);
    let resume = resumed.synchronized_resume_transition.unwrap();
    sources.blocked.set(Some(ProgrammingOwner::Position));
    for at in [3300, 3400] {
        let result = stage(&mut runtime, at, &sources).unwrap();
        assert_eq!(result.families, vec![(ProgrammingOwner::Focus, 1)]);
        assert_eq!(result.requirements.len(), 1);
        assert_eq!(scalar_projection(&result.samples).len(), 1);
        let pending = runtime.snapshot().instances.remove(0);
        assert_eq!(pending.started_at_millis, resumed.started_at_millis);
        assert_eq!(pending.synchronized_resume_transition, Some(resume));
        let held = retained_rows(&pending, true);
        let last = retained_rows(&pending, false);
        assert_eq!(
            held.len(),
            2,
            "only the unresolved Position pair stays held"
        );
        for (id, expression) in held {
            assert_eq!(expression, original_held[&id]);
            assert_eq!(last[&id], original_last[&id]);
        }
        let saved =
            serde_json::from_value(serde_json::to_value(runtime.snapshot()).unwrap()).unwrap();
        runtime.restore_snapshot(saved).unwrap();
    }
    let mut removed = runtime.fork_for_preview();
    definition.lanes.remove(0);
    definition.revision += 1;
    removed.install_definitions([definition]).unwrap();
    stage(&mut removed, 3500, &sources).unwrap();
    let cleaned = removed.snapshot().instances.remove(0);
    assert!(cleaned.synchronized_resume_transition.is_none());
    assert!(cleaned.synchronized_hold_values.is_empty());
    let mut off = runtime.fork_for_preview();
    off.off_controller(instance, control.id, 3500, 0, 0)
        .unwrap();
    assert_eq!(off.instance_count(), 0);
    sources.blocked.set(None);
    let resolved = stage(&mut runtime, 3500, &sources).unwrap();
    assert!(resolved.requirements.is_empty());
    assert!(resolved.families.contains(&(ProgrammingOwner::Position, 1)));
    let completed = runtime.snapshot().instances.remove(0);
    assert_eq!(completed.started_at_millis, resumed.started_at_millis);
    assert!(completed.synchronized_resume_transition.is_none());
    assert!(completed.synchronized_hold_values.is_empty());
    assert_eq!(
        typed_value(
            resolved
                .samples
                .iter()
                .find(|sample| sample.lane_id == pan_id)
                .unwrap()
        ),
        &DynamicValue::Scalar(0.4)
    );
}

#[test]
fn malformed_live_bound_is_fatal_even_when_another_bound_requires_current() {
    let address = DynamicValueAddress {
        representation: DynamicFamilyRepresentation::Focus,
        component: Some(ProgrammingComponent::Focus),
    };
    let lane = ramp(
        address.clone(),
        DynamicValueSource::Current,
        DynamicValueSource::Preset {
            preset_id: "focus".into(),
            address,
            last_valid_by_target: vec![],
            retained: None,
        },
    );
    let definition = definition(lane);
    let mut runtime = DynamicRuntime::default();
    runtime.install_definitions([definition.clone()]).unwrap();
    runtime
        .start(start_request(
            definition.id,
            controller(135, 1, false),
            FixtureId::new(),
            0,
            false,
        ))
        .unwrap();
    let before = runtime.snapshot();
    let sources = FallibleSources {
        malformed_preset: true,
        ..FallibleSources::blocked(ProgrammingOwner::Focus)
    };
    assert!(matches!(
        stage(&mut runtime, 500, &sources),
        Err(DynamicRuntimeError::InvalidSample(_))
    ));
    assert_eq!(runtime.snapshot(), before);
}
