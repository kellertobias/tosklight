//! Gathers each tick's Dynamic samples and fixed (FAT) Programmer and Cue rows into one
//! candidate stack per addressed attribute, and resolves a stack by its activation mixes.

use super::*;

pub(super) struct DynamicCandidate {
    pub(super) value: AttributeValue,
    pub(super) priority: i16,
    pub(super) changed_at_millis: u64,
    /// Cue capture retains submillisecond action order. Legacy Dynamic/programmer rows only
    /// carry milliseconds; absence here must not be presented as exact producer provenance.
    pub(super) exact_changed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub(super) stable_order: u128,
    pub(super) activation_mix: f32,
    pub(super) dynamic: bool,
}

pub(super) fn resolve_dynamic_stack(
    stack: &[DynamicCandidate],
    resolve_underlay: impl FnOnce() -> Option<AttributeValue>,
) -> AttributeValue {
    let (first, remaining) = stack
        .split_first()
        .expect("one Dynamic/FAT candidate exists for every stack");
    let mut resolved = if first.activation_mix >= 1.0 {
        first.value.clone()
    } else {
        blend_attribute_value(
            resolve_underlay().unwrap_or_else(|| first.value.clone()),
            first.value.clone(),
            first.activation_mix,
        )
    };
    for candidate in remaining {
        resolved =
            blend_attribute_value(resolved, candidate.value.clone(), candidate.activation_mix);
    }
    resolved
}

/// How candidates for one attribute are gathered into one stack.
///
/// A pair the patch numbered is keyed by its number, so a Dynamic's samples cost no hash of the
/// attribute name; a pair it did not is keyed by name. Both sides ask the same resolver, so a
/// Programmer value and a Dynamic sample for one pair always land in the same stack.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) enum CandidateKey {
    Address(light_core::FrameAddress),
    Name(FixtureId, AttributeKey),
}

/// One attribute's candidates and the pair they are for.
pub(super) struct CandidateStack {
    pub(super) fixture_id: FixtureId,
    pub(super) attribute: AttributeKey,
    pub(super) address: Option<light_core::FrameAddress>,
    pub(super) candidates: Vec<DynamicCandidate>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn collect_dynamic_candidates(
    addresser: &dyn light_core::FrameAddressResolver,
    programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    cue_values: &[light_playback::ActiveCueDynamicValue],
    extra_programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    samples: &[light_dynamics::DynamicRuntimeSample],
    playback_controls: &HashMap<Uuid, DynamicPlaybackControl>,
    cue_controls: &HashMap<Uuid, CueDynamicOutputControl>,
    sources: &impl DynamicTickSource,
    now_millis: u64,
) -> FxHashMap<CandidateKey, CandidateStack> {
    collect_dynamic_candidates_with_fixed_rows(
        addresser,
        programmer_values,
        cue_values,
        extra_programmer_values,
        samples,
        playback_controls,
        cue_controls,
        sources,
        now_millis,
        |_, _| true,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn collect_dynamic_candidates_with_fixed_rows(
    addresser: &dyn light_core::FrameAddressResolver,
    programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    cue_values: &[light_playback::ActiveCueDynamicValue],
    extra_programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    samples: &[light_dynamics::DynamicRuntimeSample],
    playback_controls: &HashMap<Uuid, DynamicPlaybackControl>,
    cue_controls: &HashMap<Uuid, CueDynamicOutputControl>,
    sources: &impl DynamicTickSource,
    now_millis: u64,
    include_fixed_row: impl Fn(light_dynamics::FamilyFixedSampleSource, usize) -> bool,
) -> FxHashMap<CandidateKey, CandidateStack> {
    // One entry per addressed attribute, rebuilt every tick: the hash is the cost, so it is the
    // cheap one rather than the DoS-resistant one nothing here needs.
    let mut candidates = FxHashMap::<CandidateKey, CandidateStack>::default();
    let mut consider = |fixture_id: FixtureId,
                        attribute: &AttributeKey,
                        address: Option<light_core::FrameAddress>,
                        candidate: DynamicCandidate| {
        let key = match address {
            Some(address) => CandidateKey::Address(address),
            None => CandidateKey::Name(fixture_id, attribute.clone()),
        };
        candidates
            .entry(key)
            .or_insert_with(|| CandidateStack {
                fixture_id,
                attribute: attribute.clone(),
                address,
                candidates: Vec::new(),
            })
            .candidates
            .push(candidate);
    };
    consider_runtime_samples(
        &mut consider,
        samples,
        playback_controls,
        cue_controls,
        sources,
    );
    consider_programmer_rows(
        &mut consider,
        addresser,
        programmer_values,
        extra_programmer_values,
        &include_fixed_row,
        now_millis,
    );
    consider_cue_rows(
        &mut consider,
        addresser,
        cue_values,
        &include_fixed_row,
        now_millis,
    );
    candidates
}

/// Casts one candidate per legacy contribution of every live, enabled Dynamic sample.
fn consider_runtime_samples(
    consider: &mut impl FnMut(
        FixtureId,
        &AttributeKey,
        Option<light_core::FrameAddress>,
        DynamicCandidate,
    ),
    samples: &[light_dynamics::DynamicRuntimeSample],
    playback_controls: &HashMap<Uuid, DynamicPlaybackControl>,
    cue_controls: &HashMap<Uuid, CueDynamicOutputControl>,
    sources: &impl DynamicTickSource,
) {
    for sample in samples {
        // A retained, fully covered source still samples to maintain history, but it must not
        // cast a zero-gain LTP vote or manufacture an underlying Current contribution.
        if sample.activation_mix <= 0.0
            || cue_controls
                .get(&sample.controller_id)
                .is_some_and(|control| !control.enabled)
        {
            continue;
        }
        sample
            .expression
            .visit_legacy_contributions(|attribute, value, influence| {
                let value = cue_controls
                    .get(&sample.controller_id)
                    .map_or(value, |control| {
                        if attribute.is_level() {
                            value * control.sequence_master
                        } else {
                            value
                        }
                    });
                let dynamic_value =
                    playback_controls
                        .get(&sample.controller_id)
                        .map_or(value, |control| {
                            if attribute.is_level() {
                                value * control.master
                            } else if control.crossfade_non_intensity {
                                sources
                                    .current(sample.target, &attribute)
                                    .map_or(value, |base| base + (value - base) * control.master)
                            } else {
                                value
                            }
                        });
                if playback_controls
                    .get(&sample.controller_id)
                    .is_some_and(|control| {
                        control.master == 0.0
                            && !attribute.is_level()
                            && !control.crossfade_non_intensity
                    })
                {
                    return;
                }
                consider(
                    sample.target,
                    &attribute,
                    if influence == 1.0 {
                        sample.address
                    } else {
                        None
                    },
                    DynamicCandidate {
                        value: AttributeValue::Normalized(dynamic_value),
                        priority: sample.priority,
                        changed_at_millis: sample.activated_at_millis,
                        exact_changed_at: None,
                        stable_order: sample.controller_id.as_u128(),
                        activation_mix: sample.activation_mix * influence,
                        dynamic: true,
                    },
                );
            });
    }
}

/// Casts one candidate per fixed (Static or FixAt) Programmer and extra Programmer row.
fn consider_programmer_rows(
    consider: &mut impl FnMut(
        FixtureId,
        &AttributeKey,
        Option<light_core::FrameAddress>,
        DynamicCandidate,
    ),
    addresser: &dyn light_core::FrameAddressResolver,
    programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    extra_programmer_values: &[(Uuid, i16, light_dynamics::DynamicAddressValue)],
    include_fixed_row: &impl Fn(light_dynamics::FamilyFixedSampleSource, usize) -> bool,
    now_millis: u64,
) {
    let programmer_rows = programmer_values.iter().enumerate().map(|(index, row)| {
        (
            light_dynamics::FamilyFixedSampleSource::Programmer,
            index,
            row,
        )
    });
    let extra_rows = extra_programmer_values
        .iter()
        .enumerate()
        .map(|(index, row)| {
            (
                light_dynamics::FamilyFixedSampleSource::ExtraProgrammer,
                index,
                row,
            )
        });
    for (source, index, (_, priority, stored)) in programmer_rows.chain(extra_rows) {
        if !include_fixed_row(source, index) {
            continue;
        }
        let (value, timing) = match &stored.value {
            light_dynamics::DynamicSemanticValue::Static { value, timing } => {
                (value.clone(), *timing)
            }
            light_dynamics::DynamicSemanticValue::FixAt { value, timing } => {
                (AttributeValue::Normalized(*value), *timing)
            }
            light_dynamics::DynamicSemanticValue::DynamicOn { .. }
            | light_dynamics::DynamicSemanticValue::DynamicOff { .. }
            // Contract support remains disabled until the typed family compositor is wired.
            | light_dynamics::DynamicSemanticValue::ProgrammingFixAt { .. }
            | light_dynamics::DynamicSemanticValue::ProgrammingRelease { .. }
            | light_dynamics::DynamicSemanticValue::Release => continue,
        };
        consider(
            stored.fixture_id,
            &stored.attribute,
            addresser.frame_address(stored.fixture_id, &stored.attribute),
            DynamicCandidate {
                value,
                priority: *priority,
                changed_at_millis: stored.changed_at_millis,
                exact_changed_at: None,
                stable_order: u128::from(stored.programmer_order),
                activation_mix: authored_activation_mix(
                    stored.changed_at_millis,
                    timing,
                    now_millis,
                ),
                dynamic: false,
            },
        );
    }
}

/// Casts one candidate per output-enabled fixed (Static or FixAt) Cue row.
fn consider_cue_rows(
    consider: &mut impl FnMut(
        FixtureId,
        &AttributeKey,
        Option<light_core::FrameAddress>,
        DynamicCandidate,
    ),
    addresser: &dyn light_core::FrameAddressResolver,
    cue_values: &[light_playback::ActiveCueDynamicValue],
    include_fixed_row: &impl Fn(light_dynamics::FamilyFixedSampleSource, usize) -> bool,
    now_millis: u64,
) {
    for (index, stored) in cue_values
        .iter()
        .enumerate()
        .filter(|(_, row)| row.output_enabled)
    {
        if !include_fixed_row(light_dynamics::FamilyFixedSampleSource::Cue, index) {
            continue;
        }
        let (value, timing) = match &stored.value {
            light_dynamics::DynamicSemanticValue::Static { value, timing } => {
                (value.clone(), *timing)
            }
            light_dynamics::DynamicSemanticValue::FixAt { value, timing } => {
                (AttributeValue::Normalized(*value), *timing)
            }
            light_dynamics::DynamicSemanticValue::DynamicOn { .. }
            | light_dynamics::DynamicSemanticValue::DynamicOff { .. }
            | light_dynamics::DynamicSemanticValue::ProgrammingFixAt { .. }
            | light_dynamics::DynamicSemanticValue::ProgrammingRelease { .. }
            | light_dynamics::DynamicSemanticValue::Release => continue,
        };
        consider(
            stored.fixture_id,
            &stored.attribute,
            addresser.frame_address(stored.fixture_id, &stored.attribute),
            DynamicCandidate {
                value: if stored.attribute.is_level() {
                    value.normalized().map_or(value.clone(), |value| {
                        AttributeValue::Normalized(value * stored.sequence_master)
                    })
                } else {
                    value
                },
                priority: stored.priority,
                changed_at_millis: stored.changed_at_millis,
                exact_changed_at: Some(stored.changed_at),
                stable_order: u128::from(stored.transition_ordinal),
                activation_mix: authored_activation_mix(
                    stored.changed_at_millis,
                    timing,
                    now_millis,
                ),
                dynamic: false,
            },
        );
    }
}

pub(super) fn authored_activation_mix(
    changed_at_millis: u64,
    timing: light_dynamics::DynamicValueTiming,
    now_millis: u64,
) -> f32 {
    let delay = timing.delay_millis.unwrap_or_default();
    if now_millis < changed_at_millis.saturating_add(delay) {
        return 0.0;
    }
    let fade = timing.fade_millis.unwrap_or_default();
    if fade == 0 {
        return 1.0;
    }
    (now_millis
        .saturating_sub(changed_at_millis)
        .saturating_sub(delay) as f32
        / fade as f32)
        .clamp(0.0, 1.0)
}

pub(super) fn blend_attribute_value(
    underlying: AttributeValue,
    contribution: AttributeValue,
    mix: f32,
) -> AttributeValue {
    if underlying.programming_owner().is_some() || contribution.programming_owner().is_some() {
        return match light_core::programming::interpolate_programming_value(
            &underlying,
            &contribution,
            mix,
        ) {
            Ok(value) => value,
            // Typed frame activation retains this unresolved transition, rather than emitting
            // only its source. This compatibility path deliberately keeps the old owner.
            Err(light_core::programming::TransitionError::Requires(_)) => underlying,
            // Reject an invalid contribution and retain the eligible underlay.
            Err(light_core::programming::TransitionError::Invalid(_)) => underlying,
        };
    }
    match (underlying.normalized(), contribution.normalized()) {
        (Some(underlying), Some(contribution)) => AttributeValue::Normalized(
            underlying + (contribution - underlying) * mix.clamp(0.0, 1.0),
        ),
        _ if mix >= 0.5 => contribution,
        _ => underlying,
    }
}
