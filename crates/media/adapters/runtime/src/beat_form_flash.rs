use std::collections::{BTreeMap, BTreeSet};

use media_domain::LayerState;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Flash {
    started_at: f32,
    x: f32,
    y: f32,
    size: f32,
}

/// Adds short-lived, deterministic beat forms to the renderer without persisting transient state.
#[derive(Debug, Default)]
pub(crate) struct BeatFormFlash {
    flashes: BTreeMap<(usize, usize), Vec<Flash>>,
    sequence: u32,
    beat_high: bool,
}

impl BeatFormFlash {
    pub(crate) fn apply(
        &mut self,
        layers: &[LayerState],
        seconds: f32,
        beat: f32,
    ) -> Vec<LayerState> {
        let high = beat >= 0.95;
        let landed = high && !self.beat_high;
        self.beat_high = high;
        if landed {
            self.sequence = self.sequence.wrapping_add(1);
        }

        let mut active = BTreeSet::new();
        let effective = layers
            .iter()
            .enumerate()
            .map(|(layer_index, layer)| {
                let mut effective = layer.clone();
                for (slot, effect) in effective.effects.iter_mut().enumerate() {
                    let Some(parameters) = effect.beat_form_flash_parameters() else {
                        continue;
                    };
                    let key = (layer_index, slot);
                    active.insert(key);
                    let flashes = self.flashes.entry(key).or_default();
                    flashes.retain(|flash| {
                        seconds >= flash.started_at
                            && seconds - flash.started_at < parameters.lifetime_seconds
                    });
                    if landed {
                        for ordinal in 0..parameters.density {
                            let seed = hash(
                                effect.seed
                                    ^ self.sequence.rotate_left(7)
                                    ^ (layer_index as u32).rotate_left(13)
                                    ^ (slot as u32).rotate_left(19)
                                    ^ u32::from(ordinal),
                            );
                            let x = 0.12 + unit(seed) * 0.76;
                            let y = 0.12 + unit(hash(seed ^ 0xa511_e9b3)) * 0.76;
                            let signed = unit(hash(seed ^ 0x63d8_35f1)) * 2.0 - 1.0;
                            flashes.push(Flash {
                                started_at: seconds,
                                x,
                                y,
                                size: (1.0 + signed * parameters.variation).clamp(0.25, 1.75),
                            });
                        }
                        if flashes.len() > 16 {
                            flashes.drain(..flashes.len() - 16);
                        }
                    }
                    effect.parameters.truncate(4);
                    for flash in flashes.iter() {
                        effect.parameters.extend_from_slice(&[
                            ((seconds - flash.started_at) / parameters.lifetime_seconds)
                                .clamp(0.0, 1.0),
                            flash.x,
                            flash.y,
                            flash.size,
                        ]);
                    }
                }
                effective
            })
            .collect();
        self.flashes.retain(|key, _| active.contains(key));
        effective
    }
}

fn hash(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846c_a68b);
    value ^ (value >> 16)
}

fn unit(value: u32) -> f32 {
    (value as f64 / u32::MAX as f64) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use media_domain::{BeatFormFlashParameters, EffectSlot};

    fn layer() -> LayerState {
        let mut layer = LayerState::default();
        let mut effect = EffectSlot::beat_form_flash();
        effect.seed = 17;
        effect.parameters = BeatFormFlashParameters {
            enlargement: 2.0,
            lifetime_seconds: 1.0,
            density: 2,
            variation: 0.4,
        }
        .as_array()
        .to_vec();
        layer.effects[0] = effect;
        layer
    }

    #[test]
    fn held_beats_do_not_retrigger_and_successive_edges_overlap() {
        let layer = layer();
        let mut effect = BeatFormFlash::default();
        let hit = effect.apply(std::slice::from_ref(&layer), 0.0, 1.0);
        assert_eq!(hit[0].effects[0].parameters.len(), 12);
        let held = effect.apply(std::slice::from_ref(&layer), 0.1, 1.0);
        assert_eq!(held[0].effects[0].parameters.len(), 12);
        effect.apply(std::slice::from_ref(&layer), 0.2, 0.0);
        let second = effect.apply(std::slice::from_ref(&layer), 0.3, 1.0);
        assert_eq!(second[0].effects[0].parameters.len(), 20);
    }

    #[test]
    fn events_are_deterministic_and_finish_without_new_beats() {
        let layer = layer();
        let mut first = BeatFormFlash::default();
        let mut second = BeatFormFlash::default();
        let a = first.apply(std::slice::from_ref(&layer), 0.0, 1.0);
        let b = second.apply(std::slice::from_ref(&layer), 0.0, 1.0);
        assert_eq!(a, b);
        assert_ne!(a[0].effects[0].parameters[5], a[0].effects[0].parameters[9]);
        let finished = first.apply(std::slice::from_ref(&layer), 1.1, 0.0);
        assert_eq!(finished[0].effects[0].parameters.len(), 4);
    }

    #[test]
    fn bypass_clears_transient_forms() {
        let mut layer = layer();
        let mut effect = BeatFormFlash::default();
        effect.apply(std::slice::from_ref(&layer), 0.0, 1.0);
        layer.effects[0].enabled = false;
        assert_eq!(
            effect.apply(std::slice::from_ref(&layer), 0.1, 0.0)[0],
            layer
        );
        assert!(effect.flashes.is_empty());
    }
}
