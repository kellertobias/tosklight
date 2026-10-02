use std::{collections::HashMap, sync::Arc};

use light_core::Universe;

use super::{
    AxisInversion, ContributionBatch, Engine, EngineError, RenderOptions, RenderResult,
    RuntimeGeneration, encode_profile_split, resolve_profile_fixture,
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
        let inputs = ProjectionInputs {
            position_native,
            values: &profile_values,
            options,
            group_masters,
            group_master_flashes: &group_master_flashes,
            highlight_layers: &highlight_layers,
            highlight_look: &highlight_look,
        };
        crate::timed(
            crate::RenderPhase::FixtureProjection,
            || -> Result<(), EngineError> {
                for fixture in snapshot.fixtures.iter() {
                    project_fixture(
                        fixture,
                        generation,
                        &inputs,
                        &mut output,
                        &mut universes,
                        &mut patched_slots,
                        &mut profile_visualization_values,
                        &mut physical,
                    )?;
                }
                Ok(())
            },
        )?;
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

/// Everything a fixture's projection reads and none of what it writes. Bundled because the render
/// resolves each of these once for the whole frame, and threading seven borrows through one call
/// per fixture said nothing the frame did not already say.
struct ProjectionInputs<'a> {
    position_native: &'a crate::native_position_projection::NativePositionProjection,
    values: &'a crate::ProfileValueIndex<'a>,
    options: RenderOptions,
    group_masters: &'a crate::GroupMasterIndex,
    group_master_flashes: &'a HashMap<String, f32>,
    highlight_layers: &'a HashMap<light_core::FixtureId, light_programmer::HighlightOutputLayer>,
    highlight_look: &'a light_fixture::HighlightLook,
}

/// Resolve one patched fixture and write it to every destination it is patched to.
fn project_fixture(
    fixture: &light_fixture::PatchedFixture,
    generation: &RuntimeGeneration,
    inputs: &ProjectionInputs<'_>,
    output: &mut crate::ResolvedProfileFixtureOutput,
    universes: &mut HashMap<Universe, light_output::DmxFrame>,
    patched_slots: &mut HashMap<Universe, u16>,
    visualization: &mut crate::ResolvedValues,
    physical: &mut crate::PhysicalForwardFrame,
) -> Result<(), EngineError> {
    let profile = fixture
        .definition
        .profile_snapshot
        .as_deref()
        .ok_or_else(|| {
            EngineError::Invalid("schema-v2 fixture is missing its profile snapshot".into())
        })?;
    let mode_id = fixture.definition.mode_id.ok_or_else(|| {
        EngineError::Invalid("schema-v2 fixture is missing its mode identity".into())
    })?;
    let mode = profile
        .mode(mode_id)
        .ok_or_else(|| EngineError::Invalid("schema-v2 fixture mode is missing".into()))?;
    let projection = generation
        .profile_projection(fixture.fixture_id)
        .ok_or_else(|| {
            EngineError::Invalid("schema-v2 fixture projection plan is missing".into())
        })?;
    let resolve = |inversion, instance, output: &mut crate::ResolvedProfileFixtureOutput| {
        resolve_profile_fixture(
            fixture,
            mode,
            projection,
            None,
            inputs.values,
            inputs.options,
            inputs.group_masters,
            inputs.group_master_flashes,
            inputs.highlight_layers,
            inputs.highlight_look,
            inversion,
            instance,
            inputs
                .position_native
                .instance(fixture.fixture_id, instance),
            output,
        )
    };
    if profile.patch_policy != light_fixture::PatchPolicy::Dmx {
        resolve(AxisInversion::default(), fixture.fixture_id.0, output)?;
        insert_profile_visualization_values(visualization, output);
        insert_raw_channel_values(visualization, fixture, mode, output);
        generation.physical_projection().evaluate(
            fixture.fixture_id,
            0,
            &output.channels,
            physical,
        )?;
        for (index, copy) in fixture.multipatch.iter().enumerate() {
            if inputs
                .position_native
                .instance(fixture.fixture_id, copy.id)
                .is_some()
            {
                resolve(AxisInversion::default(), copy.id, output)?;
            }
            generation.physical_projection().evaluate(
                fixture.fixture_id,
                index + 1,
                &output.channels,
                physical,
            )?;
        }
        return Ok(());
    }
    let encoding = generation
        .profile_encoding(fixture.fixture_id)
        .ok_or_else(|| EngineError::Invalid("schema-v2 fixture encoding plan is missing".into()))?;
    resolve(
        AxisInversion {
            pan: fixture.invert_pan,
            tilt: fixture.invert_tilt,
        },
        fixture.fixture_id.0,
        output,
    )?;
    insert_profile_visualization_values(visualization, output);
    generation
        .physical_projection()
        .evaluate(fixture.fixture_id, 0, &output.channels, physical)?;
    encode_profile_destination(
        &fixture.split_patches,
        fixture.universe,
        fixture.address,
        encoding,
        output,
        universes,
        patched_slots,
    )?;
    for (index, instance) in fixture.multipatch.iter().enumerate() {
        resolve(
            AxisInversion {
                pan: instance.invert_pan,
                tilt: instance.invert_tilt,
            },
            instance.id,
            output,
        )?;
        generation.physical_projection().evaluate(
            fixture.fixture_id,
            index + 1,
            &output.channels,
            physical,
        )?;
        encode_profile_destination(
            &instance.split_patches,
            instance.universe,
            instance.address,
            encoding,
            output,
            universes,
            patched_slots,
        )?;
    }
    Ok(())
}

/// A non-DMX profile publishes its resolved channels for visualization, since nothing encodes them.
fn insert_raw_channel_values(
    visualization: &mut crate::ResolvedValues,
    fixture: &light_fixture::PatchedFixture,
    mode: &light_fixture::FixtureMode,
    output: &crate::ResolvedProfileFixtureOutput,
) {
    for (channel_index, raw) in &output.channels {
        // The resolved channel says which one of the mode it is, so this is an index rather than a
        // scan of every channel per channel.
        let Some(channel) = mode.channels.get(*channel_index as usize) else {
            continue;
        };
        let Some((head_index, head)) = mode
            .heads
            .iter()
            .enumerate()
            .find(|(_, head)| head.id == channel.head_id)
        else {
            continue;
        };
        visualization.insert(
            (
                crate::fixture::profile_head_owner(fixture, head_index, head),
                channel.attribute.clone(),
            ),
            light_core::AttributeValue::RawDmxExact(*raw),
        );
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

fn encode_profile_destination(
    patches: &[light_fixture::SplitPatch],
    legacy_universe: Option<Universe>,
    legacy_address: Option<light_core::DmxAddress>,
    encoding: &light_fixture::FixtureModeEncodingPlan,
    output: &crate::profile_projection::ResolvedProfileFixtureOutput,
    universes: &mut HashMap<Universe, light_output::DmxFrame>,
    patched_slots: &mut HashMap<Universe, u16>,
) -> Result<(), EngineError> {
    if patches.is_empty() {
        return encode_profile_patch(
            1,
            legacy_universe,
            legacy_address,
            encoding,
            output,
            universes,
            patched_slots,
        );
    }
    for patch in patches {
        encode_profile_patch(
            patch.split,
            patch.universe,
            patch.address,
            encoding,
            output,
            universes,
            patched_slots,
        )?;
    }
    Ok(())
}

fn encode_profile_patch(
    split: u16,
    universe: Option<Universe>,
    address: Option<light_core::DmxAddress>,
    encoding: &light_fixture::FixtureModeEncodingPlan,
    output: &crate::profile_projection::ResolvedProfileFixtureOutput,
    universes: &mut HashMap<Universe, light_output::DmxFrame>,
    patched_slots: &mut HashMap<Universe, u16>,
) -> Result<(), EngineError> {
    let (Some(universe), Some(address)) = (universe, address) else {
        return Ok(());
    };
    let footprint = encoding
        .split_footprint(split)
        .ok_or_else(|| EngineError::Invalid(format!("fixture split {split} has no footprint")))?;
    let frame = universes.entry(universe).or_insert([0; 512]);
    let last_slot = address
        .saturating_sub(1)
        .saturating_add(footprint)
        .min(light_output::DMX_SLOTS as u16);
    patched_slots
        .entry(universe)
        .and_modify(|current| *current = (*current).max(last_slot))
        .or_insert(last_slot);
    encode_profile_split(frame, encoding, split, address, output)?;
    Ok(())
}

fn insert_profile_visualization_values(
    values: &mut crate::ResolvedValues,
    output: &crate::profile_projection::ResolvedProfileFixtureOutput,
) {
    for head in &output.heads {
        values.insert(
            (head.owner, light_core::AttributeKey::intensity()),
            light_core::AttributeValue::Normalized(head.intensity),
        );
        if let Some(color) = head.color {
            values.insert(
                (head.owner, light_core::AttributeKey::color()),
                light_core::AttributeValue::ColorXyz(color),
            );
        }
    }
}
