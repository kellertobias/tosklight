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
        let mut resolved =
            self.resolved_attributes_for_render(generation, sampled_at.clone(), sampled);
        finalize_output_parameters(
            generation,
            options,
            &self.group_master_flashes.read(),
            &mut resolved,
        );
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
            crate::ProfileValueIndex::new(&named_values, generation.channel_slots())
        });
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

/// The output-parameter stage, applied exactly once to every resolution that becomes output,
/// visualization or a Freeze capture: the masters scale the level parameters (Intensity,
/// Volume) before DMX, then a Freeze holds its parameters, which no master changes afterwards.
pub(crate) fn finalize_output_parameters(
    generation: &crate::RuntimeGeneration,
    options: RenderOptions,
    flashes: &std::collections::HashMap<String, f32>,
    resolved: &mut super::ResolvedAttributes,
) {
    let fixtures = &generation.snapshot().fixtures;
    apply_level_masters(
        fixtures,
        generation.slots(),
        generation.group_masters(),
        options,
        flashes,
        resolved,
    );
    apply_fixture_freezes(fixtures, resolved);
}

/// Group Master × Grand Master (0 under Blackout) on every level parameter, honoring a patched
/// fixture's opt-outs. A level nobody programmed is its profile default and is mastered too.
/// Changes nothing when every master is at full.
pub(crate) fn apply_level_masters(
    fixtures: &[light_fixture::PatchedFixture],
    slots: &crate::SlotTable,
    group_masters: &crate::GroupMasterIndex,
    options: RenderOptions,
    flashes: &std::collections::HashMap<String, f32>,
    resolved: &mut super::ResolvedAttributes,
) {
    let grand = if options.blackout {
        0.0
    } else {
        options.grand_master.clamp(0.0, 1.0)
    };
    if grand == 1.0 && group_masters.is_empty() {
        return;
    }
    let mut roots: Option<std::collections::HashMap<light_core::FixtureId, usize>> = None;
    let mut factor = |owner: light_core::FixtureId, root: Option<u32>| -> f32 {
        let fixture = match root {
            Some(root) => fixtures.get(root as usize),
            None => {
                let roots = roots.get_or_insert_with(|| {
                    fixtures
                        .iter()
                        .enumerate()
                        .flat_map(|(index, fixture)| {
                            std::iter::once(fixture.fixture_id)
                                .chain(fixture.logical_heads.iter().map(|head| head.fixture_id))
                                .map(move |owner| (owner, index))
                        })
                        .collect()
                });
                roots.get(&owner).and_then(|index| fixtures.get(*index))
            }
        };
        let Some(fixture) = fixture else {
            return 1.0;
        };
        let grand = if fixture.grand_master_enabled || options.blackout {
            grand
        } else {
            1.0
        };
        let group = if fixture.group_masters_enabled {
            group_masters.scale(owner, flashes)
        } else {
            1.0
        };
        grand * group
    };
    resolved.scale_levels(slots, &mut factor);
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
