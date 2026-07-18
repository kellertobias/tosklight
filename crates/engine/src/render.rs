use std::collections::HashMap;

use light_core::Universe;

use super::{
    Engine, EngineError, RenderOptions, RenderResult, render_fixture, render_profile_split,
};

impl Engine {
    pub fn render(&self, options: RenderOptions) -> Result<RenderResult, EngineError> {
        let snapshot = self.snapshot.load_full();
        let resolved = self.resolved_attributes_for_render(&snapshot, self.clock.now());
        let groups = snapshot
            .groups
            .iter()
            .map(|group| (group.id.clone(), group.clone()))
            .collect::<HashMap<_, _>>();
        let group_master_flashes = self.group_master_flashes.read();
        let highlighted_fixtures = self.highlighted_fixtures.read();
        let mut universes = HashMap::new();
        let mut patched_slots: HashMap<Universe, u16> = HashMap::new();
        for fixture in &snapshot.fixtures {
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
                let footprints = fixture.definition.split_footprints();
                let mut patches = fixture.effective_split_patches();
                for instance in &fixture.multipatch {
                    patches.extend(instance.effective_split_patches());
                }
                for patch in patches {
                    let (Some(universe), Some(address)) = (patch.universe, patch.address) else {
                        continue;
                    };
                    let footprint = footprints.get(&patch.split).copied().ok_or_else(|| {
                        EngineError::Invalid(format!(
                            "fixture split {} has no footprint",
                            patch.split
                        ))
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
                    render_profile_split(
                        frame,
                        fixture,
                        mode,
                        patch.split,
                        address,
                        &resolved,
                        options,
                        &groups,
                        &group_master_flashes,
                        &highlighted_fixtures,
                    )?;
                }
                continue;
            }
            render_legacy_fixture(
                fixture,
                &resolved,
                options,
                &groups,
                &group_master_flashes,
                &mut universes,
                &mut patched_slots,
            )?;
        }
        Ok(RenderResult {
            universes,
            patched_slots,
            revision: snapshot.revision,
            automatic_playback_transitions: resolved.automatic_playback_transitions,
        })
    }
}

fn render_legacy_fixture(
    fixture: &light_fixture::PatchedFixture,
    resolved: &super::ResolvedAttributes,
    options: RenderOptions,
    groups: &HashMap<String, light_programmer::GroupDefinition>,
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
            groups,
            group_master_flashes,
        )?;
    }
    Ok(())
}
