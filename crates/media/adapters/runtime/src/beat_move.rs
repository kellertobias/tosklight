use std::collections::{BTreeMap, BTreeSet};

use media_domain::{BeatMoveDirection, LayerState};

#[derive(Debug, Clone, Copy, Default)]
struct Envelope {
    value: f32,
}

/// Applies a temporary beat-driven offset without changing the authoritative layer transform.
#[derive(Debug, Default)]
pub(crate) struct BeatMove {
    envelopes: BTreeMap<(usize, usize), Envelope>,
    last_seconds: Option<f32>,
    beat_high: bool,
}

impl BeatMove {
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
                    let Some(parameters) = effect.beat_move_parameters() else {
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
                    let offset = parameters.amount * effect.mix.clamp(0.0, 1.0) * eased;
                    let (x, y) = direction_vector(parameters.direction);
                    effective.position_x += x * offset;
                    effective.position_y += y * offset;
                }
                effective
            })
            .collect();
        self.envelopes.retain(|key, _| active.contains(key));
        effective
    }
}

const fn direction_vector(direction: BeatMoveDirection) -> (f32, f32) {
    match direction {
        BeatMoveDirection::Up => (0.0, -1.0),
        BeatMoveDirection::Down => (0.0, 1.0),
        BeatMoveDirection::Left => (-1.0, 0.0),
        BeatMoveDirection::Right => (1.0, 0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use media_domain::{BeatMoveParameters, EffectSlot};

    fn layer() -> LayerState {
        let mut layer = LayerState {
            position_x: 0.2,
            position_y: -0.1,
            ..Default::default()
        };
        let mut effect = EffectSlot::beat_move();
        effect.parameters = BeatMoveParameters {
            amount: 0.2,
            direction: BeatMoveDirection::Right,
            decay_seconds: 0.5,
        }
        .as_array()
        .to_vec();
        layer.effects[0] = effect;
        layer
    }

    #[test]
    fn a_beat_moves_then_returns_to_the_exact_resting_position() {
        let layer = layer();
        let mut movement = BeatMove::default();
        assert_eq!(
            movement.apply(std::slice::from_ref(&layer), 0.0, 0.0)[0],
            layer
        );

        let hit = movement.apply(std::slice::from_ref(&layer), 0.1, 1.0);
        assert!((hit[0].position_x - 0.4).abs() < 0.0001);
        assert_eq!(hit[0].position_y, layer.position_y);

        let returning = movement.apply(std::slice::from_ref(&layer), 0.35, 0.0);
        assert!((returning[0].position_x - 0.3).abs() < 0.0001);
        let settled = movement.apply(std::slice::from_ref(&layer), 0.61, 0.0);
        assert_eq!(settled[0], layer);
        assert_eq!(
            layer.position_x, 0.2,
            "the configured transform was not mutated"
        );
    }

    #[test]
    fn a_sustained_high_pulse_is_one_beat_and_bypass_clears_motion() {
        let mut layer = layer();
        let mut movement = BeatMove::default();
        movement.apply(std::slice::from_ref(&layer), 0.0, 1.0);
        let held = movement.apply(std::slice::from_ref(&layer), 0.1, 1.0);
        assert!(held[0].position_x < 0.4, "the same beat did not retrigger");

        layer.effects[0].enabled = false;
        assert_eq!(
            movement.apply(std::slice::from_ref(&layer), 0.2, 0.0)[0],
            layer
        );
        assert!(movement.envelopes.is_empty());
    }
}
