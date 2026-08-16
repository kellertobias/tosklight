//! Beat-driven event history for the three-dimensional grid wave effect.

use std::collections::{BTreeMap, BTreeSet};

use media_domain::LayerState;

const MAX_EVENTS: usize = 16;

#[derive(Debug, Clone, Copy)]
struct Event {
    started_at: f32,
    strength: f32,
}

#[derive(Debug, Default)]
pub(crate) struct BeatGridWave {
    events: BTreeMap<(usize, usize), Vec<Event>>,
    beat_high: bool,
}

impl BeatGridWave {
    pub(crate) fn apply(
        &mut self,
        layers: &[LayerState],
        seconds: f32,
        beat: f32,
        strength: f32,
    ) -> Vec<LayerState> {
        let high = beat >= 0.95;
        let landed = high && !self.beat_high;
        self.beat_high = high;
        let strength = if strength.is_finite() {
            strength.clamp(0.15, 1.0)
        } else {
            0.5
        };

        let mut active = BTreeSet::new();
        let effective = layers
            .iter()
            .enumerate()
            .map(|(layer_index, layer)| {
                let mut effective = layer.clone();
                for (slot, effect) in effective.effects.iter_mut().enumerate() {
                    let Some(parameters) = effect.beat_grid_wave_parameters() else {
                        continue;
                    };
                    let key = (layer_index, slot);
                    active.insert(key);
                    let events = self.events.entry(key).or_default();
                    events.retain(|event| seconds - event.started_at < parameters.duration_seconds);
                    if landed {
                        if events.len() == MAX_EVENTS {
                            events.remove(0);
                        }
                        events.push(Event {
                            started_at: seconds,
                            strength,
                        });
                    }

                    effect.parameters = parameters.as_array().to_vec();
                    for event in events.iter() {
                        effect.parameters.push(
                            ((seconds - event.started_at) / parameters.duration_seconds)
                                .clamp(0.0, 1.0),
                        );
                        effect.parameters.push(event.strength);
                    }
                }
                effective
            })
            .collect();
        self.events.retain(|key, _| active.contains(key));
        effective
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use media_domain::{BeatGridWaveOrigin, BeatGridWaveParameters, EffectSlot};

    fn layer(origin: BeatGridWaveOrigin) -> LayerState {
        let mut layer = LayerState::default();
        let mut effect = EffectSlot::beat_grid_wave();
        effect.parameters = BeatGridWaveParameters {
            origin,
            duration_seconds: 1.0,
            ..BeatGridWaveParameters::default()
        }
        .as_array()
        .to_vec();
        layer.effects[0] = effect;
        layer
    }

    #[test]
    fn consecutive_beats_preserve_independent_waves_until_their_duration_ends() {
        let layer = layer(BeatGridWaveOrigin::Centre);
        let mut waves = BeatGridWave::default();
        waves.apply(std::slice::from_ref(&layer), 0.0, 1.0, 0.4);
        waves.apply(std::slice::from_ref(&layer), 0.1, 0.0, 0.0);
        let overlapping = waves.apply(std::slice::from_ref(&layer), 0.2, 1.0, 0.9);
        assert_eq!(waves.events[&(0, 0)].len(), 2);
        assert_eq!(overlapping[0].effects[0].parameters.len(), 10);
        assert_eq!(overlapping[0].effects[0].parameters[7], 0.4);
        assert_eq!(overlapping[0].effects[0].parameters[9], 0.9);

        let finished = waves.apply(std::slice::from_ref(&layer), 1.2, 0.0, 0.0);
        assert_eq!(finished[0].effects[0].parameters.len(), 6);
    }

    #[test]
    fn held_beat_does_not_retrigger_and_bypass_clears_events() {
        let mut layer = layer(BeatGridWaveOrigin::Left);
        let mut waves = BeatGridWave::default();
        waves.apply(std::slice::from_ref(&layer), 0.0, 1.0, 1.0);
        waves.apply(std::slice::from_ref(&layer), 0.1, 1.0, 1.0);
        assert_eq!(waves.events[&(0, 0)].len(), 1);

        layer.effects[0].enabled = false;
        let bypassed = waves.apply(std::slice::from_ref(&layer), 0.2, 0.0, 0.0);
        assert_eq!(bypassed[0], layer);
        assert!(waves.events.is_empty());
    }
}
