use super::*;
impl ShadedRenderer {
    pub(super) fn prepare_draft_beam_cache(
        &mut self,
        scene: &Scene,
        globals: &Globals,
        beams: bool,
        fused: bool,
        shadows_changed: bool,
    ) -> Option<bool> {
        let draft = self.draft.as_mut()?;
        // Narrow admission to static uniform Draft, using the ordinary unfused upsample path.
        if !super::super::draft::BeamCache::eligible(beams, fused, globals.params5[0]) {
            draft.beam_cache.invalidate();
            return None;
        }
        if shadows_changed {
            draft.beam_cache.invalidate();
        }
        let extent = draft.beam.texture().size();
        let key = super::super::draft::BeamCacheKey::build(
            scene,
            &self.frame,
            globals,
            draft.beam_cache.resource_generation,
            (extent.width, extent.height),
            self.lights.length,
            self.beam_instances.length,
        );
        Some(draft.beam_cache.prepare(key))
    }
}
