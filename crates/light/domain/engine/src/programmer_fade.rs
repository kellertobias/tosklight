use crate::Engine;
use light_core::{AttributeKey, AttributeValue, FixtureId, ProgrammerId, TimedValue};
use std::sync::{Arc, atomic::Ordering};

#[derive(Clone)]
pub(crate) struct ProgrammerTransition {
    changed_at: chrono::DateTime<chrono::Utc>,
    from: AttributeValue,
    target: AttributeValue,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct ProgrammerTransitionKey {
    pub(crate) programmer_id: ProgrammerId,
    pub(crate) source: ProgrammerTransitionSource,
    pub(crate) fixture_id: FixtureId,
    pub(crate) attribute: AttributeKey,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum ProgrammerTransitionSource {
    Programmer,
    Preload,
    Transient(Arc<str>),
    Group(Arc<str>),
    PreloadGroup(Arc<str>),
}

impl Engine {
    pub(crate) fn programmer_transition_key(
        &self,
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
        &self,
        key: ProgrammerTransitionKey,
        value: &TimedValue,
    ) {
        self.programmer_transitions.lock().insert(
            key,
            ProgrammerTransition {
                changed_at: value.changed_at,
                from: value.value.clone(),
                target: value.value.clone(),
            },
        );
    }

    pub(crate) fn faded_programmer_value(
        &self,
        mut value: TimedValue,
        now: chrono::DateTime<chrono::Utc>,
        underlying: Option<&AttributeValue>,
        programmer_id: ProgrammerId,
        source: ProgrammerTransitionSource,
        snap: bool,
    ) -> TimedValue {
        let key = self.programmer_transition_key(&value, programmer_id, source);
        if snap {
            self.programmer_transitions.lock().remove(&key);
            let elapsed = (now - value.changed_at).num_milliseconds().max(0) as u64;
            if elapsed < value.delay_millis.unwrap_or(0) {
                value.value = underlying
                    .cloned()
                    .unwrap_or(AttributeValue::Normalized(0.0));
            }
            return value;
        }
        let duration = value
            .fade_millis
            .unwrap_or_else(|| self.programmer_fade_millis.load(Ordering::Relaxed));
        if duration == 0 || value.value.normalized().is_none() {
            self.programmer_transitions.lock().remove(&key);
            return value;
        }
        let mut transitions = self.programmer_transitions.lock();
        let transition = transitions
            .entry(key)
            .or_insert_with(|| ProgrammerTransition {
                changed_at: value.changed_at,
                from: underlying
                    .cloned()
                    .unwrap_or(AttributeValue::Normalized(0.0)),
                target: value.value.clone(),
            });
        let interpolate = |transition: &ProgrammerTransition| {
            let elapsed = (now - transition.changed_at).num_milliseconds().max(0) as u64;
            let elapsed = elapsed.saturating_sub(value.delay_millis.unwrap_or(0));
            let progress = (elapsed as f32 / duration as f32).clamp(0.0, 1.0);
            match (transition.from.normalized(), transition.target.normalized()) {
                (Some(from), Some(target)) => {
                    AttributeValue::Normalized(from + (target - from) * progress)
                }
                _ => transition.target.clone(),
            }
        };
        if transition.changed_at != value.changed_at || transition.target != value.value {
            let from = interpolate(transition);
            *transition = ProgrammerTransition {
                changed_at: value.changed_at,
                from,
                target: value.value.clone(),
            };
        }
        value.value = interpolate(transition);
        value
    }
}
