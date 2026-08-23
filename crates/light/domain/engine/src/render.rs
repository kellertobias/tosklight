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
        self.advance_group_master_transitions();
        let generation = self.generation.load_full();
        self.render_generation(&generation, options, sampled)
    }

    fn render_generation(
        &self,
        generation: &RuntimeGeneration,
        options: RenderOptions,
        sampled: &[ContributionBatch],
    ) -> Result<RenderResult, EngineError> {
        let snapshot = generation.snapshot();
        let mut resolved =
            self.resolved_attributes_for_render(generation, self.clock.now(), sampled);
        crate::timed(crate::RenderPhase::FixtureFreezes, || {
            apply_fixture_freezes(&snapshot.fixtures, &mut resolved)
        });
        // Named values for the boundary and for schema-v1 fixtures. Nothing is materialised until
        // one of them actually asks, and a show of schema-v2 fixtures never asks here at all.
        let sequence_masters = std::mem::take(&mut resolved.sequence_masters);
        let named_values = resolved.named_values();
        let profile_values = crate::timed(crate::RenderPhase::ValueIndexBuild, || {
            crate::ProfileValueIndex::new(
                &named_values,
                &sequence_masters,
                generation.channel_slots(),
            )
        });
        let group_masters = generation.group_masters();
        let group_master_flashes = self.group_master_flashes.read();
        let highlight_layers = self.highlight_layers.read();
        let highlight_look = self.highlight_look.read();
        let mut universes = self.universe_pool.take();
        let mut patched_slots = self.patched_slot_pool.take();
        let mut profile_visualization_values = self.visualization_pool.take();
        // One buffer for every fixture of this frame, rather than two vectors per fixture.
        let mut output = crate::ResolvedProfileFixtureOutput::default();
        crate::timed(
            crate::RenderPhase::FixtureProjection,
            || -> Result<(), EngineError> {
                for fixture in snapshot.fixtures.iter() {
                    {
                        let profile =
                            fixture
                                .definition
                                .profile_snapshot
                                .as_deref()
                                .ok_or_else(|| {
                                    EngineError::Invalid(
                                        "schema-v2 fixture is missing its profile snapshot".into(),
                                    )
                                })?;
                        let mode_id = fixture.definition.mode_id.ok_or_else(|| {
                            EngineError::Invalid(
                                "schema-v2 fixture is missing its mode identity".into(),
                            )
                        })?;
                        let mode = profile.mode(mode_id).ok_or_else(|| {
                            EngineError::Invalid("schema-v2 fixture mode is missing".into())
                        })?;
                        let projection = generation
                            .profile_projection(fixture.fixture_id)
                            .ok_or_else(|| {
                                EngineError::Invalid(
                                    "schema-v2 fixture projection plan is missing".into(),
                                )
                            })?;
                        if profile.patch_policy != light_fixture::PatchPolicy::Dmx {
                            resolve_profile_fixture(
                                fixture,
                                mode,
                                projection,
                                None,
                                &profile_values,
                                options,
                                group_masters,
                                &group_master_flashes,
                                &highlight_layers,
                                &highlight_look,
                                AxisInversion::default(),
                                &mut output,
                            )?;
                            insert_profile_visualization_values(
                                &mut profile_visualization_values,
                                &output,
                            );
                            for (channel_index, raw) in &output.channels {
                                // The resolved channel says which one of the mode it is, so this
                                // is an index rather than a scan of every channel per channel.
                                let Some(channel) = mode.channels.get(*channel_index as usize)
                                else {
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
                                profile_visualization_values.insert(
                                    (
                                        crate::fixture::profile_head_owner(
                                            fixture, head_index, head,
                                        ),
                                        channel.attribute.clone(),
                                    ),
                                    light_core::AttributeValue::RawDmxExact(*raw),
                                );
                            }
                            continue;
                        }
                        let encoding =
                            generation
                                .profile_encoding(fixture.fixture_id)
                                .ok_or_else(|| {
                                    EngineError::Invalid(
                                        "schema-v2 fixture encoding plan is missing".into(),
                                    )
                                })?;
                        resolve_profile_fixture(
                            fixture,
                            mode,
                            projection,
                            None,
                            &profile_values,
                            options,
                            group_masters,
                            &group_master_flashes,
                            &highlight_layers,
                            &highlight_look,
                            AxisInversion {
                                pan: fixture.invert_pan,
                                tilt: fixture.invert_tilt,
                            },
                            &mut output,
                        )?;
                        insert_profile_visualization_values(
                            &mut profile_visualization_values,
                            &output,
                        );
                        encode_profile_destination(
                            &fixture.split_patches,
                            fixture.universe,
                            fixture.address,
                            encoding,
                            &output,
                            &mut universes,
                            &mut patched_slots,
                        )?;
                        for instance in &fixture.multipatch {
                            resolve_profile_fixture(
                                fixture,
                                mode,
                                projection,
                                None,
                                &profile_values,
                                options,
                                group_masters,
                                &group_master_flashes,
                                &highlight_layers,
                                &highlight_look,
                                AxisInversion {
                                    pan: instance.invert_pan,
                                    tilt: instance.invert_tilt,
                                },
                                &mut output,
                            )?;
                            encode_profile_destination(
                                &instance.split_patches,
                                instance.universe,
                                instance.address,
                                encoding,
                                &output,
                                &mut universes,
                                &mut patched_slots,
                            )?;
                        }
                    }
                }
                Ok(())
            },
        )?;
        Ok(RenderResult {
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

fn apply_fixture_freezes(
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
                (head.owner, light_core::AttributeKey("color".into())),
                light_core::AttributeValue::ColorXyz(color),
            );
        }
    }
}
