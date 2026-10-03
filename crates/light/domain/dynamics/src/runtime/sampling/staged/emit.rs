//! Emission of one completed controller's pinned and retained samples, and the synchronized
//! hold bookkeeping that closes a completed frame.
use super::*;

/// Emit every resolved pinned lane and every retained value not already emitted live.
#[allow(clippy::too_many_arguments)]
pub(super) fn emit_controller_samples(
    instance: &mut DynamicInstance,
    controller: &DynamicController,
    instance_id: Uuid,
    holding: bool,
    definition: &DynamicDefinition,
    work: &mut PinnedController,
    samples: &mut Vec<DynamicRuntimeSample>,
    mut undo: Option<&mut transaction::OutputFrameUndo>,
) -> Result<(), DynamicRuntimeError> {
    let track_emitted = !work.retained.is_empty();
    work.emitted_keys.clear();
    for pinned in &work.lanes {
        let lane = &definition.lanes[pinned.lane_index];
        let key = (controller.id, pinned.target, lane.id);
        let expression = match &pinned.value {
            PinnedValue::Ready(expression) => expression.clone(),
            PinnedValue::Absent | PinnedValue::Required => continue,
            PinnedValue::Typed { .. } => {
                unreachable!("all numeric work resolved before emission")
            }
        };
        if pinned.fresh && !work.required_keys.contains(&key) {
            if holding && !instance.synchronized_hold_values.contains_key(&key) {
                if let Some(undo) = undo.as_deref_mut() {
                    undo.held(instance);
                }
                if expression.contains_angles() {
                    instance.synchronized_hold_angle_sources.insert(key);
                }
                instance
                    .synchronized_hold_values
                    .insert(key, expression.clone());
            }
            if instance.unavailable_samples.contains_key(&key) {
                if let Some(undo) = undo.as_deref_mut() {
                    undo.unavailable(instance, key);
                }
                instance.unavailable_samples.remove(&key);
            }
        }
        let address = expression_address(&expression, lane, pinned.address);
        if work.required_keys.contains(&key) {
            append_sample(
                controller,
                instance_id,
                pinned.target,
                lane.id,
                expression,
                work.activation_mix,
                address,
                samples,
            )?;
        } else {
            emit_sample(
                instance,
                controller,
                instance_id,
                pinned.target,
                lane.id,
                expression,
                work.activation_mix,
                address,
                samples,
            )?;
        }
        if track_emitted {
            work.emitted_keys.insert(key);
        }
    }
    for (key, expression) in &work.retained {
        if !work.emitted_keys.contains(key) {
            if work.required_keys.contains(key) {
                append_sample(
                    controller,
                    instance_id,
                    key.1,
                    key.2,
                    expression.clone(),
                    work.activation_mix,
                    None,
                    samples,
                )?;
            } else {
                emit_sample(
                    instance,
                    controller,
                    instance_id,
                    key.1,
                    key.2,
                    expression.clone(),
                    work.activation_mix,
                    None,
                    samples,
                )?;
            }
        }
    }
    Ok(())
}

/// Capture a hold, then either keep the unresolved Resume cohort or finish the Resume.
pub(super) fn finish_synchronized_holds(
    instance: &mut DynamicInstance,
    plan: &PinnedInstance,
    undo: Option<&mut transaction::OutputFrameUndo>,
) {
    if plan.holding {
        instance.synchronized_hold_captured = true;
    }
    if plan
        .frame
        .synchronized_resume_mix
        .is_some_and(|mix| mix >= 1.0)
        && let Some(undo) = undo
    {
        undo.held(instance);
    }
    if plan
        .frame
        .synchronized_resume_mix
        .is_some_and(|mix| mix >= 1.0)
        && plan
            .controllers
            .iter()
            .any(|work| !work.required_keys.is_empty())
    {
        // Preserve the exact unresolved branch and clock through retry/restore. Valid lanes
        // retire their old holds now and continue sampling at the already-completed progress.
        let keep = |key: &SampleKey| {
            instance.unavailable_samples.contains_key(key)
                || plan
                    .controllers
                    .iter()
                    .any(|work| work.required_keys.contains(key))
        };
        instance.synchronized_hold_values.retain(|key, _| keep(key));
        instance.synchronized_hold_angle_sources.retain(keep);
    } else {
        finish_synchronized_resume(instance, plan.frame.synchronized_resume_mix);
    }
}
