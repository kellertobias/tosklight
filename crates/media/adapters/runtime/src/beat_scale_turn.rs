use std::collections::{BTreeMap, BTreeSet};

use media_domain::LayerState;

#[derive(Debug, Clone, Copy, Default)]
struct Envelope {
    value: f32,
}

/// Applies a beat pulse over the configured transform without mutating the show state.
#[derive(Debug, Default)]
pub(crate) struct BeatScaleTurn {
    envelopes: BTreeMap<(usize, usize), Envelope>,
    last_seconds: Option<f32>,
    beat_high: bool,
}

impl BeatScaleTurn {
    pub(crate) fn apply(
        &mut self,
        layers: &[LayerState],
        seconds: f32,
        beat: f32,
    ) -> Vec<LayerState> {
        let delta = self
            .last_seconds
            .map(|previous| (seconds - previous).clamp(0.0, 0.25))
            .unwrap_or(0.0);
        self.last_seconds = Some(seconds);
        let high = beat >= 0.95;
        let landed = high && !self.beat_high;
        self.beat_high = high;

        let mut active = BTreeSet::new();
        let effective = layers
            .iter()
            .enumerate()
            .map(|(layer_index, layer)| {
                let mut effective = layer.clone();
                for (slot, effect) in layer.effects.iter().enumerate() {
                    let Some(parameters) = effect.beat_scale_turn_parameters() else {
                        continue;
                    };
                    let key = (layer_index, slot);
                    active.insert(key);
                    let envelope = self.envelopes.entry(key).or_default();
                    envelope.value = if landed {
                        1.0
                    } else {
                        (envelope.value - delta / parameters.decay_seconds).max(0.0)
                    };
                    let eased = envelope.value * envelope.value * (3.0 - 2.0 * envelope.value);
                    let mix = effect.mix.clamp(0.0, 1.0) * eased;
                    let scale = 1.0 + parameters.scale_amount * mix;
                    effective.scale_x *= scale;
                    effective.scale_y *= scale;
                    if parameters.turn_enabled {
                        effective.rotation += parameters.rotation_degrees * mix;
                    }
                }
                effective
            })
            .collect();
        self.envelopes.retain(|key, _| active.contains(key));
        effective
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use media_domain::{BeatScaleTurnParameters, EffectSlot};

    fn layer(turn_enabled: bool) -> LayerState {
        let mut layer = LayerState {
            scale_x: 1.2,
            scale_y: 0.8,
            rotation: 11.0,
            ..Default::default()
        };
        let mut effect = EffectSlot::beat_scale_turn();
        effect.parameters = BeatScaleTurnParameters {
            scale_amount: 0.25,
            turn_enabled,
            rotation_degrees: -6.0,
            decay_seconds: 0.5,
        }
        .as_array()
        .to_vec();
        layer.effects[0] = effect;
        layer
    }

    #[test]
    fn a_beat_scales_and_optionally_turns_from_the_exact_resting_transform() {
        let resting = layer(true);
        let mut pulse = BeatScaleTurn::default();
        assert_eq!(
            pulse.apply(std::slice::from_ref(&resting), 0.0, 0.0)[0],
            resting
        );
        let hit = pulse.apply(std::slice::from_ref(&resting), 0.1, 1.0);
        assert!((hit[0].scale_x - 1.5).abs() < 0.0001);
        assert!((hit[0].scale_y - 1.0).abs() < 0.0001);
        assert!((hit[0].rotation - 5.0).abs() < 0.0001);

        let returning = pulse.apply(std::slice::from_ref(&resting), 0.35, 0.0);
        assert!(returning[0].scale_x > resting.scale_x);
        let settled = pulse.apply(std::slice::from_ref(&resting), 0.61, 0.0);
        assert_eq!(settled[0], resting);
    }

    #[test]
    fn turning_is_independent_and_bypass_clears_the_transient_envelope() {
        let mut resting = layer(false);
        let mut pulse = BeatScaleTurn::default();
        let hit = pulse.apply(std::slice::from_ref(&resting), 0.0, 1.0);
        assert!(hit[0].scale_x > resting.scale_x);
        assert_eq!(hit[0].rotation, resting.rotation);

        resting.effects[0].enabled = false;
        assert_eq!(
            pulse.apply(std::slice::from_ref(&resting), 0.1, 0.0)[0],
            resting
        );
        assert!(pulse.envelopes.is_empty());
    }
}
