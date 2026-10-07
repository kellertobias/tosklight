//! Ordered segment composition: whole and component writes over one eligible underlay.
use super::*;

pub(super) fn compose_segment(
    base: &AttributeValue,
    base_trace: Option<FamilyTraceNodeId>,
    samples: &[FamilySample],
    ordered: &[usize],
    context: &FamilyCompositionContext<'_>,
    original_base: &AttributeValue,
    scratch: &mut ComponentCompositionScratch,
    mut trace: Option<&mut FamilyTraceArena>,
) -> Result<retained_family::TracedValue, TransitionError> {
    let Some(&highest) = ordered.last() else {
        return Ok(retained_family::TracedValue {
            value: base.clone(),
            trace: base_trace,
        });
    };
    // Orthogonal-only Semantic lanes retain the chosen base, never force a competing Direct
    // Dynamic into Semantic merely because their activation happened more recently.
    let base_writer = ordered
        .iter()
        .rev()
        .copied()
        .find(|index| !orthogonal(samples[*index].address.address()));
    if base_writer.is_none()
        && matches!(base, AttributeValue::ColorProgram(color) if matches!(color.as_ref(), ColorProgram::Direct { .. }))
    {
        // Orthogonal effects cannot silently leave an active Direct representation.
        // An explicit Semantic base writer can request coherent adoption instead.
        return Ok(retained_family::TracedValue {
            value: base.clone(),
            trace: base_trace,
        });
    }
    let winner = base_writer.unwrap_or(highest);
    let mut value = base.clone();
    let mut trace_root = base_trace;
    let last_full_base = ordered
        .iter()
        .rposition(|&index| {
            let sample = &samples[index];
            compatible(sample, &samples[winner])
                && sample.address.address().component.is_none()
                && sample.activation_mix == 1.0
        })
        .unwrap_or(0);
    // Whole semantic keyframes establish a base. Explicit White Blend/CCT/Duv/UV/output
    // lanes override their named components afterward; they still arbitrate amongst themselves.
    for orthogonal_pass in [false, true] {
        let selected = if orthogonal_pass {
            ordered
        } else {
            &ordered[last_full_base..]
        };
        mark_covered_components(samples, selected, winner, orthogonal_pass, scratch);
        for (cursor, &index) in selected.iter().enumerate() {
            let sample = &samples[index];
            if scratch.covered[cursor]
                || !compatible(sample, &samples[winner])
                || orthogonal(sample.address.address()) != orthogonal_pass
            {
                continue;
            }
            if let Some(DynamicValue::Family(target)) = sample.materialized_value() {
                if let Some(arena) = trace.as_deref_mut() {
                    let incoming = sample_trace(sample, arena);
                    trace_root = Some(arena.write(
                        trace_root.expect("traced segment base"),
                        incoming,
                        FamilyTraceFootprint::Whole,
                        sample.activation_mix < 1.0,
                    ));
                }
                value = if sample.activation_mix == 1.0 {
                    // Lower component work has no influence. In particular do not forward
                    // predict a native recipe that this complete sample immediately replaces.
                    scratch.components.clear();
                    scratch.pending_native_model = None;
                    target.clone()
                } else {
                    flush(&mut value, context, scratch)?;
                    // Materialized callers keep their coherent adoption context only.
                    apply_whole(value, sample, context, original_base, None, false)?.value
                };
                continue;
            }
            if let Some(step) = native_fixed::prepare(sample, &value)? {
                // A fixed discrete/function-changing channel keeps its complete eligible
                // underlay until the step boundary. Pending lower controls must be applied
                // first so unrelated emitters and UV never come from the recorded mask.
                flush(&mut value, context, scratch)?;
                let resolved = step.apply(
                    value,
                    sample,
                    context,
                    original_base,
                    trace_root,
                    trace.as_deref_mut(),
                )?;
                value = resolved.value;
                trace_root = resolved.trace;
                continue;
            }
            let written = write_component_sample(
                sample,
                value,
                trace_root,
                trace.as_deref_mut(),
                context,
                original_base,
                scratch,
            )?;
            value = written.value;
            trace_root = written.trace;
        }
        // Finish base edits before orthogonal edits. Hue/Saturation are applied together so
        // a Hue lane is not lost when an independent Saturation lane starts from white.
        flush(&mut value, context, scratch)?;
    }
    Ok(retained_family::TracedValue {
        value,
        trace: trace_root,
    })
}

/// Write one component sample into the pending edit batch, adopting the eligible underlay into
/// the sample's representation first when the batch is empty.
fn write_component_sample(
    sample: &FamilySample,
    mut value: AttributeValue,
    mut trace_root: Option<FamilyTraceNodeId>,
    mut trace: Option<&mut FamilyTraceArena>,
    context: &FamilyCompositionContext<'_>,
    original_base: &AttributeValue,
    scratch: &mut ComponentCompositionScratch,
) -> Result<retained_family::TracedValue, TransitionError> {
    // A native batch can contain several channels whose active functions differ
    // from Current. Commit the earlier controls before adopting the next binding,
    // so the resolver receives the actual complete intermediate recipe.
    if !scratch.components.is_empty()
        && matches!(
            sample.address.address().component,
            Some(ProgrammingComponent::NativeColor(_))
        )
        && !sample.address.address().matches_authored_source(&value)
    {
        flush(&mut value, context, scratch)?;
    }
    if scratch.components.is_empty() {
        let converted = !sample.address.address().matches_authored_source(&value);
        value = adopt(value, sample.address.address(), context, original_base)?;
        if converted {
            // The adoption API returns a value without field transfers. Preserve its
            // successful solve and original graph, but do not reinterpret the old
            // fields as the adopted representation. Later complete writes can still
            // establish exact evidence for the fields they replace.
            if let Some(arena) = trace.as_deref_mut() {
                let prior = trace_root.expect("traced adoption base");
                trace_root = Some(arena.mapped_blend(prior, prior, None));
            }
        }
        scratch.pending_native_model = sample.address.native_model();
    }
    let component = sample
        .address
        .address()
        .component
        .expect("validated component sample");
    let existing = scratch
        .components
        .iter()
        .position(|(key, _)| *key == component);
    let underlay = if sample.component_needs_underlay() {
        Some(if let Some(index) = existing {
            scratch.components[index].1.clone()
        } else {
            extract_compatible_dynamic_value(&value, sample.address.address(), &context.edit)?
                .ok_or_else(|| TransitionError::Requires(requirement(sample.address.address())))?
        })
    } else {
        None
    };
    let (target, source_trace) =
        component_value_and_trace(sample, underlay.as_ref(), trace.as_deref_mut(), trace_root)?;
    let raw_context;
    let output_context = if sample.endpoint_output_exempt || sample.fix_at {
        raw_context = endpoint_output::with_control(context, None);
        &raw_context
    } else {
        context
    };
    let (target, source_trace) = endpoint_output::component(
        sample.rank,
        &sample.address,
        target,
        source_trace,
        output_context,
        trace.as_deref_mut(),
    )?;
    let mixed = if sample.activation_mix == 1.0 {
        target
    } else {
        sample
            .address
            .transition(
                underlay.expect("partial activation resolves its underlay"),
                target,
            )?
            .sample(sample.activation_mix)?
    };
    if let Some(arena) = trace {
        let source = source_trace.expect("traced component source");
        let source = endpoint_output::control_trace(
            sample.rank,
            FamilyTraceFootprint::Component(component),
            source,
            (sample.activation_mix < 1.0).then_some(trace_root.expect("component prefix")),
            output_context,
            arena,
        );
        trace_root = Some(arena.write(
            trace_root.expect("traced component base"),
            source,
            FamilyTraceFootprint::Component(component),
            sample.component_needs_underlay(),
        ));
    }
    if let Some(index) = existing {
        scratch.components[index].1 = mixed;
    } else {
        scratch.components.push((component, mixed));
    }
    Ok(retained_family::TracedValue {
        value,
        trace: trace_root,
    })
}

/// A complete later write to one component makes earlier writes to that same component
/// invisible within the pending edit batch. A whole-family sample is a barrier: its blend
/// can carry lower Hue/recipe changes into other components, so pruning must stop there.
pub(super) fn mark_covered_components(
    samples: &[FamilySample],
    ordered: &[usize],
    winner: usize,
    orthogonal_pass: bool,
    scratch: &mut ComponentCompositionScratch,
) {
    scratch.covered.clear();
    scratch.covered.resize(ordered.len(), false);
    scratch.covered_components.clear();
    scratch.covered_native_functions.clear();
    for (cursor, &index) in ordered.iter().enumerate().rev() {
        let sample = &samples[index];
        if !compatible(sample, &samples[winner])
            || orthogonal(sample.address.address()) != orthogonal_pass
        {
            continue;
        }
        let Some(component) = sample.address.address().component else {
            scratch.covered_components.clear();
            scratch.covered_native_functions.clear();
            continue;
        };
        if let ProgrammingComponent::NativeColor(binding) = component
            && !(native_fixed::is_native_fixed(sample) && sample.activation_mix < 1.0)
        {
            // A partial Fixed step still displays the eligible old function. It must not
            // prune that lower channel merely because its recorded function differs.
            let selected = scratch
                .covered_native_functions
                .entry(binding.channel_id)
                .or_insert(binding.function_id);
            if *selected != binding.function_id {
                // Function identities are discrete. An older function cannot supply a
                // numeric underlay for the selected function, even when another channel
                // happens to be the highest-ranked sample in the complete family.
                scratch.covered[cursor] = true;
                continue;
            }
        }
        if scratch.covered_components.contains(&component) {
            scratch.covered[cursor] = true;
        }
        if !sample.component_needs_underlay() {
            scratch.covered_components.insert(component);
        }
    }
}

pub(super) fn flush(
    value: &mut AttributeValue,
    context: &FamilyCompositionContext<'_>,
    scratch: &mut ComponentCompositionScratch,
) -> Result<(), TransitionError> {
    if scratch.components.is_empty() {
        return Ok(());
    }
    scratch.edits.clear();
    for (component, sample) in &scratch.components {
        scratch.edits.push(match (component, sample) {
            (ProgrammingComponent::NativeColor(binding), DynamicValue::Native(raw)) => {
                ComponentEdit::Native {
                    binding: *binding,
                    operation: NativeColorEdit::Set(*raw),
                }
            }
            (component, DynamicValue::Scalar(value)) => ComponentEdit::Scalar {
                component: *component,
                operation: ScalarEdit::Set(ScalarIntent::Value(*value)),
            },
            _ => return Err(IntentError("invalid materialized Dynamic component".into()).into()),
        });
    }
    // The pending native controls already carry their verified original source model. An
    // intermediate whole expression can change that source, so a frame's initial edit context
    // must not reinterpret the final batch using the original base or destination fixture.
    let edit = FamilyEditContext {
        solved_angles: context.edit.solved_angles,
        semantic_color_adoption: context.edit.semantic_color_adoption,
        color_model: context.edit.color_model,
        native_model: scratch
            .pending_native_model
            .as_ref()
            .map(|model| model.as_ref() as &dyn NativeColorEditModel)
            .or(context.edit.native_model),
    };
    *value = edit_family(value, &scratch.edits, &edit)?;
    scratch.components.clear();
    scratch.pending_native_model = None;
    Ok(())
}
