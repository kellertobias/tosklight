use std::sync::Arc;

use super::{
    ContributionBatch, Engine, EngineError, RenderOptions, RenderResult, RuntimeGeneration,
};

impl Engine {
    pub fn render(&self, options: RenderOptions) -> Result<RenderResult, EngineError> {
        self.render_with_contribution_batches(options, &[])
    }

    /// Render with immutable semantic samples supplied by stateful sources outside the engine.
    pub fn render_with_contribution_batches(
        &self,
        options: RenderOptions,
        sampled: &[ContributionBatch],
    ) -> Result<RenderResult, EngineError> {
        let frame = self.prepare_output_frame(options);
        self.render_prepared(&frame, sampled)
    }

    /// Commit one final lane evaluation. Speculative observations of this capture never consume
    /// its Live continuity or automatic Playback transitions.
    pub fn render_prepared(
        &self,
        frame: &crate::PreparedOutputFrame,
        sampled: &[ContributionBatch],
    ) -> Result<RenderResult, EngineError> {
        crate::timed(crate::RenderPhase::RenderTotal, || {
            let static_frame = self.prepare_static_family_frame_with_trace(frame, sampled, false);
            self.finish_static_family_frame(frame, static_frame)
        })
    }

    #[cfg(test)]
    fn render_generation(
        &self,
        generation: &RuntimeGeneration,
        options: RenderOptions,
        sampled: &[ContributionBatch],
    ) -> Result<RenderResult, EngineError> {
        crate::timed(crate::RenderPhase::RenderTotal, || {
            self.render_generation_inner(generation, options, sampled)
        })
    }

    #[cfg(test)]
    fn render_generation_inner(
        &self,
        generation: &RuntimeGeneration,
        options: RenderOptions,
        sampled: &[ContributionBatch],
    ) -> Result<RenderResult, EngineError> {
        let sampled_at = self.clock.now();
        let resolved = self.resolved_attributes_for_render(generation, sampled_at.clone(), sampled);
        self.project_resolved_frame(
            generation,
            sampled_at,
            resolved,
            &self.capture_output_overlays(options),
            self.tracking_frame(),
            &mut Default::default(),
            None,
            &Default::default(),
        )
    }

    pub(crate) fn project_resolved_frame(
        &self,
        generation: &RuntimeGeneration,
        sampled_at: chrono::DateTime<chrono::Utc>,
        mut resolved: crate::ResolvedAttributes,
        overlays: &crate::prepared_frame::CapturedOutputOverlays,
        tracking: Arc<crate::TrackedInputFrame>,
        mount_workspace: &mut crate::mount_projection::MountTransformWorkspace,
        geometry: Option<crate::PreparedFrameGeometry>,
        position_native: &crate::native_position_projection::NativePositionProjection,
    ) -> Result<RenderResult, EngineError> {
        let options = overlays.options;
        let snapshot = generation.snapshot();
        crate::timed(crate::RenderPhase::FixtureFreezes, || {
            apply_fixture_freezes(&snapshot.fixtures, &mut resolved)
        });
        // Named values for the boundary and for schema-v1 fixtures. Nothing is materialised until
        // one of them actually asks, and a show of schema-v2 fixtures never asks here at all.
        let sequence_masters = std::mem::take(&mut resolved.sequence_masters);
        let named_values = resolved.named_values();
        let (points, mounts) = match geometry {
            Some(geometry) => (geometry.points, geometry.mounts),
            None => {
                let points = Arc::new(generation.point_projection().resolve(&named_values));
                let mounts = generation
                    .mount_projection()
                    .resolve(&points, mount_workspace);
                (points, mounts)
            }
        };
        let profile_values = crate::timed(crate::RenderPhase::ValueIndexBuild, || {
            crate::ProfileValueIndex::new(
                &named_values,
                &sequence_masters,
                generation.channel_slots(),
            )
        });
        let group_masters = generation.group_masters();
        let group_master_flashes = &overlays.flashes;
        let highlight_layers = &overlays.highlights;
        let highlight_look = &overlays.highlight_look;
        let mut universes = self.universe_pool.take();
        let mut patched_slots = self.patched_slot_pool.take();
        let mut profile_visualization_values = self.visualization_pool.take();
        // One buffer for every fixture of this frame, rather than two vectors per fixture.
        let mut output = self.profile_scratch_pool.take();
        let mut physical = generation.physical_projection().take_frame();
        physical.bind_generation(generation.identity());
        let inputs = crate::render_fixtures::ProjectionInputs {
            position_native,
            values: &profile_values,
            options,
            group_masters,
            group_master_flashes: &group_master_flashes,
            highlight_layers: &highlight_layers,
            highlight_look: &highlight_look,
        };
        crate::timed(crate::RenderPhase::FixtureProjection, || {
            crate::render_fixtures::project_fixtures(
                generation,
                &inputs,
                &mut output,
                &mut crate::render_fixtures::ProjectionWrites {
                    universes: &mut universes,
                    patched_slots: &mut patched_slots,
                    visualization: &mut profile_visualization_values,
                    physical: &mut physical,
                },
                self.output_pool().as_deref(),
                &self.render_chunks,
            )
        })?;
        Ok(RenderResult {
            source_snapshot: generation.snapshot_arc(),
            tracking,
            generation: generation.identity(),
            sampled_at,
            points,
            mounts,
            physical: Arc::new(physical),
            universes,
            resolved_values: named_values,
            profile_visualization_values: Arc::new(profile_visualization_values),
            patched_slots,
            revision: snapshot.revision,
            automatic_playback_transitions: resolved.automatic_playback_transitions,
            routes: generation.routes(),
        })
    }

    #[cfg(test)]
    pub(crate) fn render_with_generation_hook(
        &self,
        options: RenderOptions,
        hook: impl FnOnce(),
    ) -> Result<RenderResult, EngineError> {
        let generation = self.generation.load_full();
        hook();
        self.render_generation(&generation, options, &[])
    }
}

pub(crate) fn apply_fixture_freezes(
    fixtures: &[light_fixture::PatchedFixture],
    resolved: &mut super::ResolvedAttributes,
) {
    for fixture in fixtures {
        for (fixture_id, target) in &fixture.freeze.targets {
            for (attribute, value) in &target.values {
                // A Freeze is the final LTP hold. Retaining an underlying sequence-master scale
                // would allow a Cue master to alter the held value after the Freeze was taken.
                resolved.override_value(*fixture_id, attribute, value.clone(), None);
            }
        }
    }
}
