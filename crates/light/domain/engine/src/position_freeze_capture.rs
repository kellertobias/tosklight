//! One command-time capture from an accepted physical frame. Per-copy raw words remain distinct;
//! they are output holds, never recording input or replacement-fixture programming intent.
use crate::{Engine, PhysicalForwardFrame, PhysicalModelSupport};
use light_core::FixtureId;
use light_fixture::{FrozenPositionControl, FrozenPositionInstance, FrozenPositionOutput};

impl Engine {
    /// The caller establishes lane acceptance. This checks the exact current generation/layout
    /// and complete absolute motor output; velocity commands cannot freeze a physical position.
    /// No re-render, fitting, normalization, Point/PSN read or root-copy approximation occurs.
    pub fn position_freeze_from_physical(
        &self,
        generation: u64,
        physical: &PhysicalForwardFrame,
        owner: FixtureId,
    ) -> Option<FrozenPositionOutput> {
        let current = self.generation.load_full();
        if current.identity() != generation
            || !physical.belongs_to_generation(generation)
            || !current.physical_projection().layout_matches(physical)
        {
            return None;
        }
        let (root, index) = current.profile_owner(owner)?;
        let fixture = &current.snapshot().fixtures[index];
        let profile = current.profile_projection(root)?;
        let channels = profile.position_footprint(owner)?;
        let emitters = profile.position_adoption_emitters(owner)?;
        let mode = fixture
            .definition
            .profile_snapshot
            .as_deref()?
            .mode(fixture.definition.mode_id?)?;
        if channels.is_empty() || emitters.is_empty() {
            return None;
        }
        let mut captured = Vec::with_capacity(1 + fixture.multipatch.len());
        for instance_id in
            std::iter::once(root.0).chain(fixture.multipatch.iter().map(|copy| copy.id))
        {
            let mut instances = physical.instances.iter().filter(|instance| {
                instance.fixture_id == root && instance.instance_id == instance_id
            });
            let instance = instances.next()?;
            if instances.next().is_some()
                || !instance.complete
                || instance.position_support != PhysicalModelSupport::Compiled
            {
                return None;
            }
            for emitter in emitters {
                let mut lenses = instance
                    .lenses
                    .iter()
                    .filter(|lens| lens.emitter_id == emitter.emitter_id);
                let lens = lenses.next()?;
                if lenses.next().is_some()
                    || lens.local.is_none()
                    || lens.world.is_none()
                    || lens.flags != Default::default()
                {
                    return None;
                }
                for (axis, node) in emitter.commands? {
                    let command = instance.axes.get(axis)?;
                    if command.node_id != node || command.absolute_degrees().is_none() {
                        return None;
                    }
                }
            }
            let mut controls = Vec::with_capacity(channels.len());
            for &index in channels {
                let channel = mode.channels.get(index)?;
                let raw = *instance.native_raw.get(index)?;
                if raw > channel.resolution.max_raw() {
                    return None;
                }
                controls.push(FrozenPositionControl {
                    channel_id: channel.id,
                    signature: profile
                        .position_freeze_signature(instance_id, index)?
                        .to_owned(),
                    raw,
                });
            }
            captured.push(FrozenPositionInstance {
                instance_id,
                controls,
            });
        }
        Some(FrozenPositionOutput {
            version: 1,
            instances: captured,
        })
    }
}
