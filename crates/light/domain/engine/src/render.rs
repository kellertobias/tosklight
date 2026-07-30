use std::{collections::HashMap, sync::Arc};

use light_core::Universe;

use super::{
    ContributionBatch, Engine, EngineError, GroupMasterIndex, RenderOptions, RenderResult,
    RuntimeGeneration, encode_profile_split, render_fixture, resolve_profile_fixture,
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
        let resolved = self.resolved_attributes_for_render(generation, self.clock.now(), sampled);
        let profile_values = crate::ProfileValueIndex::new(&resolved);
        let group_masters = generation.group_masters();
        let group_master_flashes = self.group_master_flashes.read();
        let highlighted_fixtures = self.highlighted_fixtures.read();
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
                let encoding =
                    generation
                        .profile_encoding(fixture.fixture_id)
                        .ok_or_else(|| {
                            EngineError::Invalid(
                                "schema-v2 fixture encoding plan is missing".into(),
                            )
                        })?;
                let projection = generation
                    .profile_projection(fixture.fixture_id)
                    .ok_or_else(|| {
                        EngineError::Invalid("schema-v2 fixture projection plan is missing".into())
                    })?;
                let footprints = fixture.definition.split_footprints();
                let single_patch = if fixture.multipatch.is_empty() {
                    match fixture.split_patches.as_slice() {
                        [patch] => Some((patch.split, patch.universe, patch.address)),
                        [] => Some((1, fixture.universe, fixture.address)),
                        _ => None,
                    }
                } else {
                    None
                };
                if let Some((split, universe, address)) = single_patch {
                    let output = resolve_profile_fixture(
                        fixture,
                        mode,
                        projection,
                        None,
                        &profile_values,
                        options,
                        group_masters,
                        &group_master_flashes,
                        &highlighted_fixtures,
                    )?;
                    insert_profile_visualization_values(&mut profile_visualization_values, &output);
                    let (Some(universe), Some(address)) = (universe, address) else {
                        continue;
                    };
                    let footprint = footprints.get(&split).copied().ok_or_else(|| {
                        EngineError::Invalid(format!("fixture split {split} has no footprint"))
                    })?;
                    let frame = universes.entry(universe).or_insert([0; 512]);
                    let last_slot = address
                        .saturating_sub(1)
                        .saturating_add(footprint)
                        .min(light_output::DMX_SLOTS as u16);
                    patched_slots
                        .entry(universe)
                        .and_modify(|current| *current = (*current).max(last_slot))
                        .or_insert(last_slot);
                    encode_profile_split(frame, encoding, split, address, &output)?;
                    continue;
                }
                let mut patches = fixture.effective_split_patches();
                for instance in &fixture.multipatch {
                    patches.extend(instance.effective_split_patches());
                }
                let mut destinations = Vec::with_capacity(patches.len());
                for patch in &patches {
                    let (Some(universe), Some(address)) = (patch.universe, patch.address) else {
                        continue;
                    };
                    let footprint = footprints.get(&patch.split).copied().ok_or_else(|| {
                        EngineError::Invalid(format!(
                            "fixture split {} has no footprint",
                            patch.split
                        ))
                    })?;
                    destinations.push((patch.split, universe, address, footprint));
                }
                let included_splits = destinations
                    .iter()
                    .map(|(split, _, _, _)| *split)
                    .collect::<Vec<_>>();
                let output = resolve_profile_fixture(
                    fixture,
                    mode,
                    projection,
                    None,
                    &profile_values,
                    options,
                    group_masters,
                    &group_master_flashes,
                    &highlighted_fixtures,
                )?;
                insert_profile_visualization_values(&mut profile_visualization_values, &output);
                if included_splits.is_empty() {
                    continue;
                }
                for (split, universe, address, footprint) in destinations {
                    let frame = universes.entry(universe).or_insert([0; 512]);
                    let last_slot = address
                        .saturating_sub(1)
                        .saturating_add(footprint)
                        .min(light_output::DMX_SLOTS as u16);
                    patched_slots
                        .entry(universe)
                        .and_modify(|current| *current = (*current).max(last_slot))
                        .or_insert(last_slot);
                    encode_profile_split(frame, encoding, split, address, &output)?;
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
    let mut patches = vec![(fixture.universe, fixture.address)];
    patches.extend(
        fixture
            .multipatch
            .iter()
            .map(|instance| (instance.universe, instance.address)),
    );
    for (universe, address) in patches {
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
