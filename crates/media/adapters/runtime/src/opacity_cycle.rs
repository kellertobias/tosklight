use media_domain::{BeatRatio, LayerState, OutputState};

use crate::layer_pipeline::Prepared;

const TRANSITION_SECONDS: f32 = 0.1;

#[derive(Debug, Default)]
pub(crate) struct OpacityCycle {
    participants: Vec<usize>,
    ratio: BeatRatio,
    position: usize,
    previous: Option<usize>,
    last_phase: Option<f32>,
    subdivision: Option<u8>,
    beat_count: u64,
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
            .filter(|(index, layer)| layer.dimmer > 0.0 && prepared_indices.contains(index))
            .map(|(index, _)| index)
            .collect();
        let ratio = output.master.opacity_cycle;

        if participants != self.participants || ratio != self.ratio {
            self.participants = participants;
            self.ratio = ratio;
            self.position = 0;
            self.previous = None;
            self.last_phase = (bpm > 0.0).then_some(beat_phase);
            self.subdivision = subdivision(ratio, beat_phase);
            self.beat_count = 0;
            self.transition_started = seconds - TRANSITION_SECONDS;
        } else if self.should_advance(bpm, beat_phase) && self.participants.len() > 1 {
            self.previous = self.participants.get(self.position).copied();
            self.position = (self.position + 1) % self.participants.len();
            self.transition_started = seconds;
        }

        if ratio == BeatRatio::Disabled {
            return output.layers.clone();
        }
        let current = self.participants.get(self.position).copied();
        let transition = ((seconds - self.transition_started) / TRANSITION_SECONDS).clamp(0.0, 1.0);
        output
            .layers
            .iter()
            .enumerate()
            .map(|(index, layer)| {
                let mut effective = layer.clone();
                effective.dimmer *= if Some(index) == current {
                    transition
                } else if Some(index) == self.previous {
                    1.0 - transition
                } else if self.participants.contains(&index) {
                    0.0
                } else {
                    1.0
                };
                effective
            })
            .collect()
    }

    fn should_advance(&mut self, bpm: f32, beat_phase: f32) -> bool {
        if bpm <= 0.0 || self.ratio == BeatRatio::Disabled {
            self.last_phase = None;
            self.subdivision = None;
            return false;
        }
        let wrapped = self.last_phase.is_some_and(|last| beat_phase + 0.25 < last);
        self.last_phase = Some(beat_phase);
        if wrapped {
            self.beat_count = self.beat_count.saturating_add(1);
        }
        match self.ratio {
            BeatRatio::Disabled => false,
            BeatRatio::Unity => wrapped,
            BeatRatio::Divide(divisor) => {
                wrapped && divisor > 0 && self.beat_count.is_multiple_of(u64::from(divisor))
            }
            BeatRatio::Multiply(multiplier) => {
                let current = (beat_phase * f32::from(multiplier)).floor() as u8;
                let changed = self.subdivision.is_some_and(|last| last != current);
                self.subdivision = Some(current);
                changed
            }
        }
    }
}

fn subdivision(ratio: BeatRatio, beat_phase: f32) -> Option<u8> {
    match ratio {
        BeatRatio::Multiply(multiplier) => Some((beat_phase * f32::from(multiplier)).floor() as u8),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layer_pipeline::{PreparedLayer, Slot};
    use media_domain::{LayerPersonality, MediaAddress, OutputId, SourceStatus};

    fn output(count: usize, ratio: BeatRatio) -> (OutputState, Prepared) {
        let mut output = OutputState::new(OutputId::new(), LayerPersonality::EightLayers);
        output.layers.truncate(count);
        output.master.opacity_cycle = ratio;
        let mut prepared = Prepared::default();
        for (index, layer) in output.layers.iter_mut().enumerate() {
            layer.address = MediaAddress::new(1, (index + 1) as u8);
            layer.source_status = SourceStatus::Ready;
            layer.dimmer = 1.0;
            prepared.layers.push(PreparedLayer {
                index,
                source: Slot::Media(index),
                mask: None,
            });
        }
        (output, prepared)
    }

    #[test]
    fn disabled_is_an_exact_bypass() {
        let (output, prepared) = output(3, BeatRatio::Disabled);
        assert_eq!(
            OpacityCycle::default().apply(&output, &prepared, 0.2, 120.0, 0.4),
            output.layers
        );
    }

    #[test]
    fn unity_cycles_all_loaded_dimmer_positive_layers_without_mutating_them() {
        let (output, prepared) = output(3, BeatRatio::Unity);
        let mut cycle = OpacityCycle::default();
        let first = cycle.apply(&output, &prepared, 0.2, 120.0, 0.4);
        assert_eq!(
            first.iter().map(|layer| layer.dimmer).collect::<Vec<_>>(),
            vec![1.0, 0.0, 0.0]
        );
        cycle.apply(&output, &prepared, 0.51, 120.0, 0.02);
        let settled = cycle.apply(&output, &prepared, 0.7, 120.0, 0.4);
        assert_eq!(
            settled.iter().map(|layer| layer.dimmer).collect::<Vec<_>>(),
            vec![0.0, 1.0, 0.0]
        );
        assert!(output.layers.iter().all(|layer| layer.dimmer == 1.0));
    }

    #[test]
    fn multiplied_ratio_advances_inside_a_beat() {
        let (output, prepared) = output(2, BeatRatio::Multiply(4));
        let mut cycle = OpacityCycle::default();
        cycle.apply(&output, &prepared, 0.0, 120.0, 0.1);
        cycle.apply(&output, &prepared, 0.2, 120.0, 0.3);
        let settled = cycle.apply(&output, &prepared, 0.4, 120.0, 0.45);
        assert_eq!(settled[1].dimmer, 1.0);
    }

    #[test]
    fn participant_changes_reset_to_first_eligible_layer() {
        let (mut output, prepared) = output(2, BeatRatio::Unity);
        let mut cycle = OpacityCycle::default();
        cycle.apply(&output, &prepared, 0.1, 120.0, 0.8);
        cycle.apply(&output, &prepared, 0.3, 120.0, 0.1);
        output.layers[0].dimmer = 0.0;
        let changed = cycle.apply(&output, &prepared, 0.4, 120.0, 0.2);
        assert_eq!(changed[0].dimmer, 0.0);
        assert!((changed[1].dimmer - 1.0).abs() < f32::EPSILON);
    }
}
