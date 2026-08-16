//! Beat-driven scan-line event history.
//!
//! The configured effect parameters remain authoritative. This coordinator clones the frame's
//! layer state and appends transient event positions for the compositor; it never writes those
//! positions back into the show configuration.

use std::collections::{BTreeMap, BTreeSet};

use media_domain::LayerState;

/// Three lines per beat at the detector's 200 ms minimum gap for the longest three-second travel.
const MAX_EVENTS: usize = 16;

#[derive(Debug, Clone, Copy)]
struct Event {
    started_at: f32,
    line_count: u8,
}

#[derive(Debug, Default)]
pub(crate) struct BeatScan {
    events: BTreeMap<(usize, usize), Vec<Event>>,
    beat_high: bool,
}

impl BeatScan {
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
        let line_count = line_count(strength);

        let mut active = BTreeSet::new();
        let effective = layers
            .iter()
            .enumerate()
            .map(|(layer_index, layer)| {
                let mut effective = layer.clone();
                for (slot, effect) in effective.effects.iter_mut().enumerate() {
                    let Some(parameters) = effect.beat_scan_parameters() else {
                        continue;
                    };
                    let key = (layer_index, slot);
                    active.insert(key);
                    let events = self.events.entry(key).or_default();
                    events.retain(|event| seconds - event.started_at < parameters.duration_seconds);
                    if landed && events.len() < MAX_EVENTS {
                        events.push(Event {
                            started_at: seconds,
                            line_count,
                        });
                    }

                    effect.parameters = parameters.as_array().to_vec();
                    for event in events.iter() {
                        let progress = ((seconds - event.started_at) / parameters.duration_seconds)
                            .clamp(0.0, 1.0);
                        let spacing = parameters.width * 1.4;
                        let spread = spacing * f32::from(event.line_count.saturating_sub(1));
                        let base = progress * (1.0 + parameters.width * 2.0 + spread)
                            - parameters.width
                            - spread;
                        effect.parameters.push(base);
                        effect.parameters.push(f32::from(event.line_count));
                    }
                }
                effective
            })
            .collect();
        self.events.retain(|key, _| active.contains(key));
        effective
    }
}

fn line_count(strength: f32) -> u8 {
    let strength = if strength.is_finite() {
        strength.clamp(0.0, 1.0)
    } else {
        0.0
    };
    1 + u8::from(strength >= 0.40) + u8::from(strength >= 0.75)
}

#[cfg(test)]
mod tests {
    use super::*;
    use media_domain::{BeatScanEdge, BeatScanParameters, EffectSlot};

    fn layer() -> LayerState {
        let mut layer = LayerState::default();
        let mut effect = EffectSlot::beat_scan();
        effect.parameters = BeatScanParameters {
            width: 0.05,
            edge: BeatScanEdge::Soft,
            falloff: 0.5,
            duration_seconds: 1.0,
        }
        .as_array()
        .to_vec();
        layer.effects[0] = effect;
        layer
    }

    #[test]
    fn beat_strength_spawns_one_to_three_lines_without_a_spawn_control() {
        for (strength, expected) in [(0.1, 1.0), (0.5, 2.0), (0.9, 3.0)] {
            let mut scan = BeatScan::default();
            let effective = scan.apply(&[layer()], 0.0, 1.0, strength);
            assert_eq!(effective[0].effects[0].parameters[5], expected);
        }
    }

    #[test]
    fn consecutive_beats_preserve_lines_that_are_still_travelling() {
        let layer = layer();
        let mut scan = BeatScan::default();
        scan.apply(std::slice::from_ref(&layer), 0.0, 1.0, 0.9);
        scan.apply(std::slice::from_ref(&layer), 0.1, 0.0, 0.0);
        let second = scan.apply(std::slice::from_ref(&layer), 0.2, 1.0, 0.5);
        assert_eq!(scan.events[&(0, 0)].len(), 2);
        assert_eq!(second[0].effects[0].parameters.len(), 8);
        assert_eq!(second[0].effects[0].parameters[5], 3.0);
        assert_eq!(second[0].effects[0].parameters[7], 2.0);
    }

    #[test]
    fn a_held_pulse_does_not_retrigger_and_bypass_clears_transient_state() {
        let mut layer = layer();
        let mut scan = BeatScan::default();
        scan.apply(std::slice::from_ref(&layer), 0.0, 1.0, 1.0);
        scan.apply(std::slice::from_ref(&layer), 0.1, 1.0, 1.0);
        assert_eq!(scan.events[&(0, 0)].len(), 1);

        layer.effects[0].enabled = false;
        let bypassed = scan.apply(std::slice::from_ref(&layer), 0.2, 0.0, 0.0);
        assert_eq!(bypassed[0], layer);
        assert!(scan.events.is_empty());
    }

    #[test]
    fn a_line_cluster_crosses_the_image_and_leaves_at_the_configured_duration() {
        let layer = layer();
        let mut scan = BeatScan::default();
        let start = scan.apply(std::slice::from_ref(&layer), 0.0, 1.0, 0.9);
        let middle = scan.apply(std::slice::from_ref(&layer), 0.5, 0.0, 0.0);
        let finished = scan.apply(std::slice::from_ref(&layer), 1.0, 0.0, 0.0);
        assert!(start[0].effects[0].parameters[4] < 0.0);
        assert!((0.0..1.0).contains(&middle[0].effects[0].parameters[4]));
        assert_eq!(finished[0].effects[0].parameters.len(), 4);
    }
}
