use std::{collections::HashMap, sync::Arc};

use light_core::Universe;

use super::{
    AxisInversion, ContributionBatch, Engine, EngineError, GroupMasterIndex, RenderOptions,
    RenderResult, RuntimeGeneration, encode_profile_split, render_fixture, resolve_profile_fixture,
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
        apply_fixture_freezes(&snapshot.fixtures, &mut resolved);
        let profile_values = crate::ProfileValueIndex::new(&resolved);
        let group_masters = generation.group_masters();
        let group_master_flashes = self.group_master_flashes.read();
        let highlight_layers = self.highlight_layers.read();
        let highlight_look = self.highlight_look.read();
        let mut universes = HashMap::new();
        let mut patched_slots: HashMap<Universe, u16> = HashMap::new();
        let mut profile_visualization_values =
            HashMap::with_capacity(snapshot.fixtures.len().saturating_mul(2));
        for fixture in snapshot.fixtures.iter() {
            if fixture.definition.schema_version == light_fixture::FIXTURE_PROFILE_SCHEMA_VERSION {
                let profile = fixture
                    .definition
                    .profile_snapshot
                    .as_deref()
                    .ok_or_else(|| {
                        EngineError::Invalid(
                            "schema-v2 fixture is missing its profile snapshot".into(),
                        )
                    })?;
                let mode_id = fixture.definition.mode_id.ok_or_else(|| {
                    EngineError::Invalid("schema-v2 fixture is missing its mode identity".into())
                })?;
                let mode = profile.mode(mode_id).ok_or_else(|| {
                    EngineError::Invalid("schema-v2 fixture mode is missing".into())
                })?;
                let projection = generation
                    .profile_projection(fixture.fixture_id)
                    .ok_or_else(|| {
                        EngineError::Invalid("schema-v2 fixture projection plan is missing".into())
                    })?;
                if profile.patch_policy == light_fixture::PatchPolicy::Internal {
                    let output = resolve_profile_fixture(
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
                    )?;
                    insert_profile_visualization_values(&mut profile_visualization_values, &output);
                    for (channel_id, raw) in &output.channels {
                        let Some(channel) = mode
                            .channels
                            .iter()
                            .find(|channel| channel.id == *channel_id)
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
                                crate::fixture::profile_head_owner(fixture, head_index, head),
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
                let root_output = resolve_profile_fixture(
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
                )?;
                insert_profile_visualization_values(
                    &mut profile_visualization_values,
                    &root_output,
                );
                encode_profile_destination(
                    &fixture.split_patches,
                    fixture.universe,
                    fixture.address,
                    encoding,
                    &root_output,
                    &mut universes,
                    &mut patched_slots,
                )?;
                for instance in &fixture.multipatch {
                    let instance_output = resolve_profile_fixture(
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
                    )?;
                    encode_profile_destination(
                        &instance.split_patches,
                        instance.universe,
                        instance.address,
                        encoding,
                        &instance_output,
                        &mut universes,
                        &mut patched_slots,
                    )?;
                }
                continue;
            }
            render_legacy_fixture(
                fixture,
                &resolved,
                options,
                group_masters,
                &group_master_flashes,
                &mut universes,
                &mut patched_slots,
            )?;
        }
        Ok(RenderResult {
            universes,
            resolved_values: Arc::new(resolved.values),
            resolved_changed_at: Arc::new(resolved.changed_at),
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
                let key = (*fixture_id, attribute.clone());
                resolved.values.insert(key.clone(), value.clone());
                // A Freeze is the final LTP hold. Retaining an underlying sequence-master scale
                // would allow a Cue master to alter the held value after the Freeze was taken.
                resolved.sequence_masters.remove(&key);
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
    values: &mut HashMap<
        (light_core::FixtureId, light_core::AttributeKey),
        light_core::AttributeValue,
    >,
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

fn render_legacy_fixture(
    fixture: &light_fixture::PatchedFixture,
    resolved: &super::ResolvedAttributes,
    options: RenderOptions,
    group_masters: &GroupMasterIndex,
    group_master_flashes: &HashMap<String, f32>,
    universes: &mut HashMap<Universe, light_output::DmxFrame>,
    patched_slots: &mut HashMap<Universe, u16>,
) -> Result<(), EngineError> {
    let mut patches = vec![(
        fixture.universe,
        fixture.address,
        fixture.invert_pan,
        fixture.invert_tilt,
    )];
    patches.extend(fixture.multipatch.iter().map(|instance| {
        (
            instance.universe,
            instance.address,
            instance.invert_pan,
            instance.invert_tilt,
        )
    }));
    for (universe, address, invert_pan, invert_tilt) in patches {
        let (Some(universe), Some(address)) = (universe, address) else {
            continue;
        };
        let frame = universes.entry(universe).or_insert([0; 512]);
        let last_slot = address
            .saturating_sub(1)
            .saturating_add(fixture.definition.footprint)
            .min(light_output::DMX_SLOTS as u16);
        patched_slots
            .entry(universe)
            .and_modify(|current| *current = (*current).max(last_slot))
            .or_insert(last_slot);
        let mut instance = fixture.clone();
        instance.universe = Some(universe);
        instance.address = Some(address);
        instance.invert_pan = invert_pan;
        instance.invert_tilt = invert_tilt;
        render_fixture(
            frame,
            &instance,
            &resolved.values,
            options,
            group_masters,
            group_master_flashes,
        )?;
    }
    Ok(())
}
