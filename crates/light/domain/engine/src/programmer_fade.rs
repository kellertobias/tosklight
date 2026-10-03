use light_core::{AttributeKey, AttributeValue, FixtureId, ProgrammerId, TimedValue};
use std::sync::Arc;

type Evidence = Option<Arc<crate::ContributionFamilyEvidence>>;
pub(crate) type ProgrammerSample = (TimedValue, Evidence);

#[derive(Clone)]
pub(crate) struct ProgrammerTransition {
    changed_at: chrono::DateTime<chrono::Utc>,
    programmer_order: u64,
    from: Option<AttributeValue>,
    from_evidence: Evidence,
    target: AttributeValue,
    target_evidence: Evidence,
    blended_evidence: Evidence,
    duration_millis: u64,
    delay_millis: u64,
}

impl ProgrammerTransition {
    fn new(
        key: &ProgrammerTransitionKey,
        value: &TimedValue,
        from: Option<AttributeValue>,
        from_evidence: Evidence,
        duration_millis: u64,
        delay_millis: u64,
    ) -> Self {
        let target_evidence = (value.value.programming_owner().is_some()
            || value.attribute.0.as_ref() == "focus")
            .then(|| {
                let origin = crate::ContributionOrigin::new(key.source_id(), value);
                crate::ContributionFamilyEvidence::authored_endpoint(&origin, value)
            })
            .flatten();
        let blended_evidence = from.as_ref().and_then(|from| {
            blend_evidence(
                &value.attribute,
                from,
                &value.value,
                &from_evidence,
                &target_evidence,
            )
        });
        Self {
            changed_at: value.changed_at,
            programmer_order: value.programmer_order,
            from,
            from_evidence,
            target: value.value.clone(),
            target_evidence,
            blended_evidence,
            duration_millis,
            delay_millis,
        }
    }

    fn matches(&self, value: &TimedValue) -> bool {
        self.changed_at == value.changed_at
            && self.programmer_order == value.programmer_order
            && self.target == value.value
    }

    fn sample(&self, now: chrono::DateTime<chrono::Utc>) -> Option<(AttributeValue, Evidence)> {
        let elapsed = (now - self.changed_at).num_milliseconds().max(0) as u64;
        if elapsed < self.delay_millis {
            return Some((self.from.clone()?, self.from_evidence.clone()));
        }
        let elapsed = elapsed - self.delay_millis;
        if self.duration_millis == 0 || elapsed >= self.duration_millis {
            return Some((self.target.clone(), self.target_evidence.clone()));
        }
        let from = self.from.as_ref()?;
        if elapsed == 0 {
            return Some((from.clone(), self.from_evidence.clone()));
        }
        let progress = (elapsed as f64 / self.duration_millis as f64) as f32;
        match light_core::programming::interpolate_programming_value(from, &self.target, progress) {
            Ok(value) => Some((
                value,
                // Long durations can round to the endpoint before integer elapsed reaches
                // duration. Follow the value evaluator without bypassing its validation.
                if progress >= 1.0 {
                    self.target_evidence.clone()
                } else {
                    self.blended_evidence.clone()
                },
            )),
            // Transitional behavior only: physical-frame activation must carry the unresolved
            // endpoint pair through arbitration. It cannot reconstruct that pair from this hold.
            Err(light_core::programming::TransitionError::Requires(_)) => {
                Some((from.clone(), self.from_evidence.clone()))
            }
            // Malformed runtime input must not become a newly invented owner.
            Err(light_core::programming::TransitionError::Invalid(_)) => None,
        }
    }
}

/// Compile the same field transfer as the value operation at the edit boundary. Sampling
/// reuses this history; it never guesses source participation from an equal output value.
fn blend_evidence(
    attribute: &AttributeKey,
    from: &AttributeValue,
    to: &AttributeValue,
    from_evidence: &Evidence,
    to_evidence: &Evidence,
) -> Evidence {
    use light_core::programming::{ProgrammingOwner, interpolate_programming_trace};
    let owner = to
        .programming_owner()
        .or_else(|| (attribute.0.as_ref() == "focus").then_some(ProgrammingOwner::Focus))?;
    let trace = interpolate_programming_trace(owner, from, to, 0.5).ok()?;
    crate::ContributionFamilyEvidence::transferred(
        owner,
        from,
        to,
        from_evidence.as_ref(),
        to_evidence.as_ref(),
        &trace,
    )
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct ProgrammerTransitionKey {
    pub(crate) programmer_id: ProgrammerId,
    pub(crate) source: ProgrammerTransitionSource,
    pub(crate) fixture_id: FixtureId,
    pub(crate) attribute: AttributeKey,
}

impl ProgrammerTransitionKey {
    fn source_id(&self) -> crate::ContributionSourceId {
        use crate::ContributionSourceId as Id;
        match &self.source {
            ProgrammerTransitionSource::Programmer => Id::programmer(self.programmer_id),
            ProgrammerTransitionSource::Preload => Id::preload(self.programmer_id),
            ProgrammerTransitionSource::Transient(source) => {
                Id::programmer_transient(self.programmer_id, Arc::clone(source))
            }
            ProgrammerTransitionSource::Group(group) => {
                Id::programmer_group(self.programmer_id, Arc::clone(group))
            }
            ProgrammerTransitionSource::PreloadGroup(group) => {
                Id::preload_group(self.programmer_id, Arc::clone(group))
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum ProgrammerTransitionSource {
    Programmer,
    Preload,
    Transient(Arc<str>),
    Group(Arc<str>),
    PreloadGroup(Arc<str>),
}

pub(crate) fn programmer_transition_key(
    value: &TimedValue,
    programmer_id: ProgrammerId,
    source: ProgrammerTransitionSource,
) -> ProgrammerTransitionKey {
    ProgrammerTransitionKey {
        programmer_id,
        source,
        fixture_id: value.fixture_id,
        attribute: value.attribute.clone(),
    }
}

pub(crate) fn track_immediate_programmer_value(
    transitions: &mut crate::programmer_memo::ProgrammerTransitions,
    key: ProgrammerTransitionKey,
    value: &TimedValue,
) -> Evidence {
    if let Some(known) = transitions.get(&key)
        && known.matches(value)
        && known.duration_millis == 0
        && known.delay_millis == 0
    {
        return known.target_evidence.clone();
    }
    let mut transition =
        ProgrammerTransition::new(&key, value, Some(value.value.clone()), None, 0, 0);
    transition.from_evidence = transition.target_evidence.clone();
    let evidence = transition.target_evidence.clone();
    transitions.insert(key, transition);
    evidence
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn faded_programmer_value(
    transitions: &mut crate::programmer_memo::ProgrammerTransitions,
    default_fade_millis: u64,
    mut value: TimedValue,
    now: chrono::DateTime<chrono::Utc>,
    underlying: Option<&AttributeValue>,
    underlying_evidence: impl FnOnce() -> Evidence,
    programmer_id: ProgrammerId,
    source: ProgrammerTransitionSource,
    snap: bool,
) -> Option<ProgrammerSample> {
    let key = programmer_transition_key(&value, programmer_id, source);
    if snap {
        let elapsed = (now - value.changed_at).num_milliseconds().max(0) as u64;
        if elapsed < value.delay_millis.unwrap_or(0) {
            transitions.remove(&key);
            // Delayed state selection leaves the changing underlay live until the switch.
            value.value = underlying.cloned().or_else(|| {
                value
                    .value
                    .normalized()
                    .map(|_| AttributeValue::Normalized(0.0))
            })?;
            return Some((value, underlying_evidence()));
        }
        let evidence = track_immediate_programmer_value(transitions, key, &value);
        return Some((value, evidence));
    }
    // Raw overrides and legacy discrete payloads historically bypass Programmer Fade.
    // Adding complete semantic owners must not turn those controls into delayed jumps.
    if matches!(
        value.value,
        AttributeValue::RawDmx(_)
            | AttributeValue::RawDmxExact(_)
            | AttributeValue::Discrete(_)
            | AttributeValue::Spread(_)
    ) {
        let evidence = track_immediate_programmer_value(transitions, key, &value);
        return Some((value, evidence));
    }
    let duration = value.fade_millis.unwrap_or(default_fade_millis);
    let delay = value.delay_millis.unwrap_or(0);
    if duration == 0 && delay == 0 {
        let evidence = track_immediate_programmer_value(transitions, key, &value);
        return Some((value, evidence));
    }
    let transition = transitions.entry(key.clone()).or_insert_with(|| {
        ProgrammerTransition::new(
            &key,
            &value,
            // Legacy scalar entry keeps its existing zero fallback. Complete Color,
            // Position and Zoom require an actual underlay/default; never invent one.
            underlying.cloned().or_else(|| {
                value
                    .value
                    .normalized()
                    .map(|_| AttributeValue::Normalized(0.0))
            }),
            underlying_evidence(),
            duration,
            delay,
        )
    });
    if !transition.matches(&value) {
        // Sample using the OLD delay/duration before adopting the new timing. Otherwise
        // interruption jumps when the operator changes fade or delay with the destination.
        let (from, evidence) = transition
            .sample(value.changed_at)
            .map_or((None, None), |(from, evidence)| (Some(from), evidence));
        *transition = ProgrammerTransition::new(&key, &value, from, evidence, duration, delay);
    }
    let (sample, evidence) = transition.sample(now)?;
    value.value = sample;
    Some((value, evidence))
}

// Keep the focused fade tests exercising the same Live mutation boundary used by the
// contribution wrapper, while prepared-frame evaluation calls the map-based helper above.
#[cfg(test)]
impl crate::Engine {
    pub(crate) fn faded_programmer_value(
        &self,
        value: TimedValue,
        now: chrono::DateTime<chrono::Utc>,
        underlying: Option<&AttributeValue>,
        programmer_id: ProgrammerId,
        source: ProgrammerTransitionSource,
        snap: bool,
    ) -> Option<TimedValue> {
        let default_fade_millis = self
            .programmer_fade_millis
            .load(std::sync::atomic::Ordering::Relaxed);
        let mut continuity = self.output_continuity.lock();
        continuity.advance_revision();
        faded_programmer_value(
            &mut continuity.programmer_transitions,
            default_fade_millis,
            value,
            now,
            underlying,
            || None,
            programmer_id,
            source,
            snap,
        )
        .map(|(value, _)| value)
    }
}
