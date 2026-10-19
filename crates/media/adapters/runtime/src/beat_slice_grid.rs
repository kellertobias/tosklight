//! Counter-based beat events drive layout changes; render cadence never loses a drum hit.
use crate::beat_events::{BeatEvents, BeatTracker, source_index};
use media_domain::LayerState;
use std::collections::BTreeMap;

#[derive(Debug, Default)]
pub(crate) struct BeatSliceGrid {
    tracker: BeatTracker,
    layouts: BTreeMap<(usize, usize), (u32, f32)>,
    rasters: BTreeMap<
        (usize, usize),
        (
            media_domain::slice_grid::SliceGridTypography,
            std::sync::Arc<media_domain::slice_grid::SliceTextAtlas>,
        ),
    >,
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
                    let typography = effect.slice_typography();
                    if self
                        .rasters
                        .get(&key)
                        .is_none_or(|(stored, _)| stored != &typography)
                    {
                        self.rasters.insert(
                            key,
                            (
                                typography.clone(),
                                media_text::slice_text_atlas(&typography),
                            ),
                        );
                    }
                    effect.text_atlas = self
                        .rasters
                        .get(&key)
                        .map(|(_, atlas)| std::sync::Arc::clone(atlas));
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
        self.rasters.retain(|key, _| active.contains(key));
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
        assert!(
            std::sync::Arc::ptr_eq(
                first[0].effects[0].text_atlas.as_ref().unwrap(),
                held[0].effects[0].text_atlas.as_ref().unwrap()
            ),
            "steady frames reuse the same CPU atlas"
        );
        effect.apply(&layers, 0.5, 0.0.into());
        let second = effect.apply(&layers, 1.0, 1.0.into());
        assert_eq!(second[0].effects[0].parameters[4], 2.0);
        assert_eq!(layers[0].effects[0].parameters.len(), 4);
        effect.apply(&[], 2.0, 0.0.into());
        assert!(
            effect.rasters.is_empty(),
            "inactive layer atlases are released"
        );
    }
}
