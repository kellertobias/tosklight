//! Counter-based beat events drive layout changes; render cadence never loses a drum hit.
use crate::beat_events::{BeatEvents, BeatTracker, source_index};
use media_domain::LayerState;
use std::collections::BTreeMap;

#[derive(Debug, Default)]
pub(crate) struct BeatSliceGrid {
    tracker: BeatTracker,
    layouts: BTreeMap<(usize, usize), (u32, f32)>,
}
impl BeatSliceGrid {
    pub(crate) fn apply(
        &mut self,
        layers: &[LayerState],
        seconds: f32,
        beats: BeatEvents,
    ) -> Vec<LayerState> {
        let landed = self.tracker.landed(beats);
        let mut active = Vec::new();
        let result = layers
            .iter()
            .enumerate()
            .map(|(layer_index, layer)| {
                let mut layer = layer.clone();
                for (slot, effect) in layer.effects.iter_mut().enumerate() {
                    let Some(parameters) = effect.beat_slice_grid_parameters() else {
                        continue;
                    };
                    let key = (layer_index, slot);
                    active.push(key);
                    let layout = self.layouts.entry(key).or_insert((0, seconds - 1.0));
                    if landed[source_index(effect.beat_source)] {
                        layout.0 = (layout.0 + 1) % 1_000_000;
                        layout.1 = seconds;
                    }
                    effect.text_mask = media_text::packed_line_mask(
                        effect
                            .text
                            .as_deref()
                            .unwrap_or(media_domain::slice_grid::DEFAULT_SLICE_TEXT),
                    )
                    .to_vec();
                    effect.parameters = parameters.as_array().to_vec();
                    effect.parameters.extend([
                        layout.0 as f32,
                        ((seconds - layout.1) / 0.24).clamp(0.0, 1.0),
                    ]);
                }
                layer
            })
            .collect();
        self.layouts.retain(|key, _| active.contains(key));
        result
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use media_domain::EffectSlot;
    #[test]
    fn one_layout_per_beat_and_only_transient_layers_change() {
        let mut effect = BeatSliceGrid::default();
        let mut layer = LayerState::default();
        layer.effects[0] = EffectSlot::beat_slice_grid();
        let layers = [layer];
        let first = effect.apply(&layers, 0.0, 1.0.into());
        assert_eq!(first[0].effects[0].parameters[4..], [1.0, 0.0]);
        let held = effect.apply(&layers, 0.25, 1.0.into());
        assert_eq!(held[0].effects[0].parameters[4..], [1.0, 1.0]);
        effect.apply(&layers, 0.5, 0.0.into());
        let second = effect.apply(&layers, 1.0, 1.0.into());
        assert_eq!(second[0].effects[0].parameters[4], 2.0);
        assert_eq!(layers[0].effects[0].parameters.len(), 4);
    }
}
