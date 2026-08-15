use media_domain::{LayerState, OpacityCycleInterval, OutputState};

use crate::layer_pipeline::Prepared;

const TRANSITION_SECONDS: f32 = 0.1;

#[derive(Debug, Default)]
pub(crate) struct OpacityCycle {
    participants: Vec<usize>,
    interval: Option<OpacityCycleInterval>,
    position: usize,
    previous: Option<usize>,
    last_phase: Option<f32>,
    last_second: Option<u64>,
    transition_started: f32,
}

impl OpacityCycle {
    pub(crate) fn apply(
        &mut self,
        output: &OutputState,
        prepared: &Prepared,
        seconds: f32,
        bpm: f32,
        beat_phase: f32,
    ) -> Vec<LayerState> {
        let prepared_indices: Vec<_> = prepared.layers.iter().map(|layer| layer.index).collect();
        let participants: Vec<_> = output
            .layers
            .iter()
            .enumerate()
            .filter(|(index, layer)| {
                layer.dimmer > 0.0
                    && prepared_indices.contains(index)
                    && cycle_effect(layer).is_some()
            })
            .map(|(index, _)| index)
            .collect();
        let interval = participants
            .first()
            .and_then(|index| cycle_effect(&output.layers[*index]))
            .map(|(_, interval)| interval);

        if participants != self.participants || interval != self.interval {
            self.participants = participants;
            self.interval = interval;
            self.position = 0;
            self.previous = None;
            self.last_phase = (bpm > 0.0).then_some(beat_phase);
            self.last_second = Some(seconds.floor() as u64);
            self.transition_started = seconds - TRANSITION_SECONDS;
        } else if self.should_advance(seconds, bpm, beat_phase) && self.participants.len() > 1 {
            self.previous = self.participants.get(self.position).copied();
            self.position = (self.position + 1) % self.participants.len();
            self.transition_started = seconds;
        }

        let current = self.participants.get(self.position).copied();
        let transition = ((seconds - self.transition_started) / TRANSITION_SECONDS).clamp(0.0, 1.0);
        output
            .layers
            .iter()
            .enumerate()
            .map(|(index, layer)| {
                let mut effective = layer.clone();
                if let Some((mix, _)) = cycle_effect(layer) {
                    let cycle_opacity = if Some(index) == current {
                        transition
                    } else if Some(index) == self.previous {
                        1.0 - transition
                    } else {
                        0.0
                    };
                    effective.dimmer *= (1.0 - mix) + mix * cycle_opacity;
                }
                effective
            })
            .collect()
    }

    fn should_advance(&mut self, seconds: f32, bpm: f32, beat_phase: f32) -> bool {
        match self.interval {
            Some(OpacityCycleInterval::EverySecond) => {
                let second = seconds.floor() as u64;
                let changed = self.last_second.is_some_and(|last| second > last);
                self.last_second = Some(second);
                changed
            }
            Some(OpacityCycleInterval::EveryBeat) if bpm > 0.0 => {
                let changed = self.last_phase.is_some_and(|last| beat_phase + 0.25 < last);
                self.last_phase = Some(beat_phase);
                changed
            }
            Some(OpacityCycleInterval::EveryHalfBeat) if bpm > 0.0 => {
                let bucket = beat_phase >= 0.5;
                let changed = self.last_phase.is_some_and(|last| bucket != (last >= 0.5));
                self.last_phase = Some(beat_phase);
                changed
            }
            _ => false,
        }
    }
}

fn cycle_effect(layer: &LayerState) -> Option<(f32, OpacityCycleInterval)> {
    layer.effects.iter().find_map(|effect| {
        effect
            .opacity_cycle_interval()
            .map(|interval| (effect.mix.clamp(0.0, 1.0), interval))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layer_pipeline::{PreparedLayer, Slot};
    use media_domain::{EffectSlot, LayerPersonality, MediaAddress, OutputId, SourceStatus};

    fn output(count: usize, interval: OpacityCycleInterval) -> (OutputState, Prepared) {
        let mut output = OutputState::new(OutputId::new(), LayerPersonality::EightLayers);
        output.layers.truncate(count);
        let mut prepared = Prepared::default();
        for (index, layer) in output.layers.iter_mut().enumerate() {
            layer.address = MediaAddress::new(1, (index + 1) as u8);
            layer.source_status = SourceStatus::Ready;
            layer.dimmer = 1.0;
            let mut effect = EffectSlot::opacity_cycle();
            effect.parameters = vec![interval.parameter()];
            layer.effects[0] = effect;
            prepared.layers.push(PreparedLayer {
                index,
                source: Slot::Media(index),
                mask: None,
            });
        }
        (output, prepared)
    }

    #[test]
    fn every_beat_cycles_in_stable_order_without_mutating_configured_dimmers() {
        let (output, prepared) = output(3, OpacityCycleInterval::EveryBeat);
        let mut cycle = OpacityCycle::default();
        let first = cycle.apply(&output, &prepared, 0.2, 120.0, 0.4);
        assert_eq!(
            first.iter().map(|layer| layer.dimmer).collect::<Vec<_>>(),
            vec![1.0, 0.0, 0.0]
        );
        let crossing = cycle.apply(&output, &prepared, 0.51, 120.0, 0.02);
        let settled = cycle.apply(&output, &prepared, 0.7, 120.0, 0.4);
        assert!(crossing[0].dimmer > 0.0 && crossing[1].dimmer < 1.0);
        assert_eq!(
            settled.iter().map(|layer| layer.dimmer).collect::<Vec<_>>(),
            vec![0.0, 1.0, 0.0]
        );
        assert!(output.layers.iter().all(|layer| layer.dimmer == 1.0));
    }

    #[test]
    fn participant_changes_reset_to_the_first_eligible_layer() {
        let (mut output, prepared) = output(2, OpacityCycleInterval::EveryHalfBeat);
        let mut cycle = OpacityCycle::default();
        cycle.apply(&output, &prepared, 0.1, 120.0, 0.2);
        cycle.apply(&output, &prepared, 0.3, 120.0, 0.6);
        output.layers[0].dimmer = 0.0;
        let changed = cycle.apply(&output, &prepared, 0.4, 120.0, 0.8);
        assert_eq!(changed[0].dimmer, 0.0);
        assert!((changed[1].dimmer - 1.0).abs() < f32::EPSILON * 2.0);
    }
}
