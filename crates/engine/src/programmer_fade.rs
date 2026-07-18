use crate::Engine;
use light_core::{AttributeKey, AttributeValue, FixtureId, ProgrammerId, TimedValue};
use std::sync::atomic::Ordering;

#[derive(Clone)]
pub(crate) struct ProgrammerTransition {
    changed_at: chrono::DateTime<chrono::Utc>,
    from: AttributeValue,
    target: AttributeValue,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct ProgrammerTransitionKey {
    programmer_id: ProgrammerId,
    source: ProgrammerTransitionSource,
    fixture_id: FixtureId,
    attribute: AttributeKey,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum ProgrammerTransitionSource {
    Programmer,
    Preload,
    Group(String),
    PreloadGroup(String),
}

impl Engine {
    pub(crate) fn faded_programmer_value(
        &self,
        mut value: TimedValue,
        now: chrono::DateTime<chrono::Utc>,
        underlying: Option<&AttributeValue>,
        programmer_id: ProgrammerId,
        source: ProgrammerTransitionSource,
        snap: bool,
    ) -> TimedValue {
        let key = ProgrammerTransitionKey {
            programmer_id,
            source,
            fixture_id: value.fixture_id,
            attribute: value.attribute.clone(),
        };
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
