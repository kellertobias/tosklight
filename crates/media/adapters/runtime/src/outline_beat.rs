//! Beat modulation of the Outline effect's Intensity (TL-426).
//!
//! It shares the analysis beat every other beat effect follows. A landed beat pushes Intensity
//! toward one by Beat depth, and the push falls back over Beat decay. The result is written only
//! into the frame's effective layers, never into the show state.

use std::collections::{BTreeMap, BTreeSet};

use media_domain::{LayerState, OutlineParameters};

/// The beat level that counts as a landed beat, as for Beat Scale & Turn.
const BEAT_LANDED: f32 = 0.95;

#[derive(Debug, Default)]
pub(crate) struct OutlineBeat {
    envelopes: BTreeMap<(usize, usize), f32>,
    last_seconds: Option<f32>,
    beat_high: bool,
}

impl OutlineBeat {
    pub(crate) fn apply(
        &mut self,
        layers: &[LayerState],
        seconds: f32,
        beat: f32,
    ) -> Vec<LayerState> {
        let delta = self
            .last_seconds
            .map_or(0.0, |previous| (seconds - previous).clamp(0.0, 0.25));
        self.last_seconds = Some(seconds);
        let high = beat >= BEAT_LANDED;
        let landed = high && !self.beat_high;
        self.beat_high = high;

        let mut active = BTreeSet::new();
        let effective = layers
            .iter()
            .enumerate()
            .map(|(layer_index, layer)| {
                let mut effective = layer.clone();
                for (slot, effect) in layer.effects.iter().enumerate() {
                    let Some(parameters) = effect.outline_parameters() else {
                        continue;
                    };
                    if parameters.beat_depth <= 0.0 {
                        continue;
                    }
                    active.insert((layer_index, slot));
                    let envelope = self.envelopes.entry((layer_index, slot)).or_default();
                    *envelope = if landed {
                        1.0
                    } else {
                        (*envelope - delta / parameters.beat_decay_seconds).max(0.0)
                    };
                    let eased = *envelope * *envelope * (3.0 - 2.0 * *envelope);
                    // Bank resolution normalizes every preset, so Intensity is always stored.
                    if let Some(stored) = effective.effects[slot].parameters.first_mut() {
                        *stored = modulated(parameters, eased);
                    }
                }
                effective
            })
            .collect();
        self.envelopes.retain(|key, _| active.contains(key));
        effective
    }
}

/// Resting Intensity pushed toward one by `depth × envelope`.
fn modulated(parameters: OutlineParameters, envelope: f32) -> f32 {
    let pulse = (parameters.beat_depth * envelope).clamp(0.0, 1.0);
    parameters.intensity + (1.0 - parameters.intensity) * pulse
}

#[cfg(test)]
mod tests {
    use super::*;
    use media_domain::EffectSlot;

    fn layer(beat_depth: f32) -> LayerState {
        let mut effect = EffectSlot::outline();
        effect.parameters = OutlineParameters {
            intensity: 0.5,
            beat_depth,
            beat_decay_seconds: 0.5,
            ..Default::default()
        }
        .as_array()
        .to_vec();
        let mut layer = LayerState::default();
        layer.effects[1] = effect;
        layer
    }

    fn intensity(layers: &[LayerState]) -> f32 {
        layers[0].effects[1].parameters[0]
    }

    #[test]
    fn without_beat_depth_the_intensity_stays_static() {
        let resting = layer(0.0);
        let mut beat = OutlineBeat::default();
        let hit = beat.apply(std::slice::from_ref(&resting), 0.0, 1.0);
        assert_eq!(hit[0], resting);
        assert!(beat.envelopes.is_empty());
    }

    #[test]
    fn a_beat_pushes_toward_outlines_only_and_decays_back() {
        let resting = layer(1.0);
        let mut beat = OutlineBeat::default();
        assert_eq!(
            intensity(&beat.apply(std::slice::from_ref(&resting), 0.0, 0.0)),
            0.5
        );
        let hit = beat.apply(std::slice::from_ref(&resting), 0.05, 1.0);
        assert_eq!(intensity(&hit), 1.0, "full depth reaches outlines only");
        // The beat is still high: that is the same beat, not a new one.
        let held = beat.apply(std::slice::from_ref(&resting), 0.3, 1.0);
        let falling = intensity(&held);
        assert!(falling > 0.5 && falling < 1.0, "{falling}");
        let settled = beat.apply(std::slice::from_ref(&resting), 0.56, 0.0);
        assert_eq!(settled[0], resting);
        assert_eq!(
            resting.effects[1].parameters[0], 0.5,
            "the show is untouched"
        );
    }

    #[test]
    fn half_depth_pushes_half_way() {
        let resting = layer(0.5);
        let mut beat = OutlineBeat::default();
        let hit = beat.apply(std::slice::from_ref(&resting), 0.0, 1.0);
        assert!((intensity(&hit) - 0.75).abs() < 1e-6);
    }
}
