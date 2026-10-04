//! Deferred typed evaluation of the lanes pinned in stage one, against the final Current and
//! adoption sources. Unresolved lanes become family preparation requirements.
use super::*;

/// One typed lane's evaluation: its value or error, and the keyframe transition it compiled
/// for its lane's cache. Workers produce these off the frame's thread (TL-639 round 6); the
/// frame keeps the transition and applies the value in frame order either way.
pub(in crate::runtime) struct Evaluated {
    result: Result<Option<DynamicSampleExpression>, TransitionError>,
    compiled: Option<crate::programming::KeyframeTransition>,
}

/// Evaluate the typed lane `pinned` of `work` against `sources` without changing anything.
/// `checked` reads Current through [`CheckedCurrentSources`] (a typed frame that collects
/// preparation requirements). Must only be called for a [`PinnedValue::Typed`] lane.
pub(super) fn evaluate(
    compiled: &crate::CompiledProgrammingLane,
    plan: &PinnedInstance,
    work: &PinnedController,
    pinned: &PinnedLane,
    sources: &dyn DynamicValueSourceResolver,
    checked: bool,
) -> Evaluated {
    let PinnedValue::Typed {
        phase,
        random_envelope,
        authored_occurrence,
    } = pinned.value
    else {
        unreachable!("typed work");
    };
    let controller = &plan.frame.controllers[work.controller_index];
    let lane = &plan.frame.definition.lanes[pinned.lane_index];
    let current = CheckedCurrentSources::new(sources);
    let input = if checked {
        &current as &dyn DynamicValueSourceResolver
    } else {
        sources
    };
    let operation = work
        .emission
        .as_ref()
        .map(|emission| crate::DynamicOperationContext {
            emission,
            target: pinned.target,
            lane_id: lane.id,
        });
    let context = ProgrammingEvaluationContext {
        instance_id: plan.instance_id,
        controller_id: controller.id,
        authored_occurrence,
        target: pinned.target,
        elapsed_millis: plan.frame.elapsed,
        cycle_duration_millis: plan.cycle_duration_millis,
        phase_degrees: phase,
        random_envelope,
        sources: input,
    };
    let mut kept = None;
    let result = sample_typed_lane(
        compiled,
        context,
        controller.size,
        pinned.target,
        input,
        operation,
        &mut kept,
    );
    let result = match (result, current.take_error()) {
        (Err(error @ TransitionError::Invalid(_)), _)
        | (_, Some(error @ TransitionError::Invalid(_))) => Err(error),
        (_, Some(error)) => Err(error),
        (result, None) => result,
    };
    Evaluated {
        result,
        compiled: kept,
    }
}

/// Resolve every typed lane of `plan` in frame order. `recorded` answers each lane's
/// evaluation when a worker already made it (`None`: evaluate it here against `sources`).
pub(super) fn resolve_deferred(
    instance: &mut DynamicInstance,
    plan: &mut PinnedInstance,
    sources: &dyn DynamicValueSourceResolver,
    mut requirements: Option<&mut Vec<DynamicFamilyPreparationRequirement>>,
    recorded: &mut dyn FnMut() -> Option<Evaluated>,
) -> Result<(), DynamicRuntimeError> {
    let checked = requirements.is_some();
    for work_index in 0..plan.controllers.len() {
        let controller = &plan.frame.controllers[plan.controllers[work_index].controller_index];
        let mut position_required = HashSet::new();
        for typed in 0..plan.controllers[work_index].typed_indices.len() {
            let work = &plan.controllers[work_index];
            let index = work.typed_indices[typed];
            let pinned = &work.lanes[index];
            let PinnedValue::Typed {
                authored_occurrence,
                ..
            } = pinned.value
            else {
                continue;
            };
            let lane = &plan.frame.definition.lanes[pinned.lane_index];
            let key = (controller.id, pinned.target, lane.id);
            let compiled = instance
                .programming_lanes
                .get_mut(&lane.id)
                .expect("pinned compiled lane");
            let evaluated = match recorded() {
                Some(evaluated) => evaluated,
                None => evaluate(compiled, plan, work, pinned, sources, checked),
            };
            compiled.keep_transition(evaluated.compiled);
            let target = pinned.target;
            let preserve_angle_branches = pinned.preserve_angle_branches;
            let value = match evaluated.result {
                Ok(Some(mut expression)) => {
                    expression.bind_fresh_authored_occurrence(authored_occurrence);
                    held_or_live_value(
                        instance,
                        key,
                        expression,
                        plan.frame.synchronized_resume_mix,
                        preserve_angle_branches,
                    )
                    .map_or(PinnedValue::Absent, PinnedValue::Ready)
                }
                Ok(None) => PinnedValue::Absent,
                Err(TransitionError::Requires(reason)) if requirements.is_some() => {
                    let crate::DynamicLaneBody::Programming(body) = &lane.body else {
                        unreachable!("typed work");
                    };
                    let owner = body.address.owner();
                    requirements
                        .as_deref_mut()
                        .unwrap()
                        .push(preparation_requirement(
                            controller,
                            plan.instance_id,
                            target,
                            lane.id,
                            owner,
                            reason,
                        ));
                    if owner == ProgrammingOwner::Position {
                        position_required.insert(target);
                    }
                    plan.controllers[work_index].required_keys.insert(key);
                    PinnedValue::Required
                }
                Err(error) => return Err(DynamicRuntimeError::InvalidSample(error.to_string())),
            };
            plan.controllers[work_index].lanes[index].value = value;
        }
        let work = &mut plan.controllers[work_index];
        if work.required_keys.is_empty() {
            continue;
        }
        retain_position_cohort(
            instance,
            &plan.frame.definition,
            controller.id,
            work,
            &position_required,
        )?;
    }
    Ok(())
}

fn sample_typed_lane(
    compiled: &crate::CompiledProgrammingLane,
    context: ProgrammingEvaluationContext<'_>,
    controller_size: f32,
    target: FixtureId,
    input: &dyn DynamicValueSourceResolver,
    operation: Option<crate::DynamicOperationContext<'_>>,
    kept: &mut Option<crate::programming::KeyframeTransition>,
) -> Result<Option<DynamicSampleExpression>, TransitionError> {
    compiled
        .pin_angle_numeric_with_operations(&context, controller_size, operation)
        .and_then(|numeric| match numeric {
            crate::AngleNumericSample::Program(program) => {
                Ok(Some(DynamicSampleExpression::AngleNumeric { program }))
            }
            crate::AngleNumericSample::Absent => Ok(None),
            crate::AngleNumericSample::NotApplicable => compiled
                .sample_with_cache(context, operation, kept)
                .and_then(|value| {
                    value
                        .map(|value| {
                            compiled
                                .apply_controller_size_with_operations(
                                    value,
                                    controller_size,
                                    target,
                                    input,
                                    operation,
                                )
                                .map_err(TransitionError::from)
                        })
                        .transpose()
                }),
        })
}

fn preparation_requirement(
    controller: &DynamicController,
    instance_id: Uuid,
    target: FixtureId,
    lane_id: Uuid,
    owner: ProgrammingOwner,
    reason: light_core::programming::TransitionRequirement,
) -> DynamicFamilyPreparationRequirement {
    DynamicFamilyPreparationRequirement {
        target,
        owner,
        rank: FamilySampleRank {
            priority: controller.priority,
            changed_at_millis: controller.activated_at_millis,
            changed_at_submillis_nanos: 0,
            stable_order: controller.id.as_u128(),
            identity: crate::FamilySampleIdentity::Dynamic {
                instance_id,
                controller_id: controller.id,
                lane_id,
            },
        },
        reason: DynamicFamilyPreparationRequirementReason::Transition(reason),
    }
}

/// Retain both sides of a correlated Position history. Independent owners and legacy output
/// still use this frame's prepared expressions; only retained records are held.
fn retain_position_cohort(
    instance: &DynamicInstance,
    definition: &DynamicDefinition,
    controller_id: Uuid,
    work: &mut PinnedController,
    position_required: &HashSet<FixtureId>,
) -> Result<(), DynamicRuntimeError> {
    for pinned in &work.lanes {
        let lane = &definition.lanes[pinned.lane_index];
        let key = (controller_id, pinned.target, lane.id);
        if position_required.contains(&pinned.target) {
            let mut position = lane.output_owner() == ProgrammingOwner::Position.key();
            if let PinnedValue::Ready(expression) = &pinned.value {
                position |= expression.contains_angles();
                expression
                    .visit_programming_values(&mut |address, _| {
                        position |= address.owner() == ProgrammingOwner::Position;
                        Ok(())
                    })
                    .map_err(|error| DynamicRuntimeError::InvalidSample(error.to_string()))?;
            }
            if position {
                work.required_keys.insert(key);
            }
        }
    }
    // A partner can already have been deleted from the definition. Its retained branch
    // still belongs to this unresolved cohort and must survive a completed Resume.
    if !position_required.is_empty() {
        for (key, held) in &instance.synchronized_hold_values {
            if key.0 != controller_id || !position_required.contains(&key.1) {
                continue;
            }
            let mut position = held.contains_angles();
            held.visit_programming_values(&mut |address, _| {
                position |= address.owner() == ProgrammingOwner::Position;
                Ok(())
            })
            .map_err(|error| DynamicRuntimeError::InvalidSample(error.to_string()))?;
            if position {
                work.required_keys.insert(*key);
            }
        }
    }
    for key in &work.required_keys {
        if let Some(previous) = instance.last_sample_values.get(key) {
            work.required_last.push((*key, previous.clone()));
        }
    }
    Ok(())
}
