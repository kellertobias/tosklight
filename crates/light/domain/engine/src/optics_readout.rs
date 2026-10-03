//! Runtime-only Zoom adoption from final physical output (TL-637 follow-up). The opening is the
//! compiled optics forward evaluation (`CompiledOpticsForward`) of the accepted frame's native
//! output, the same model the frame was published with. No fitting, mutation or conversion of a
//! percentage into degrees happens here.
use crate::{Engine, PhysicalForwardFrame, PhysicalInstanceOutput, PhysicalModelSupport};
use light_core::FixtureId;
use light_fixture::forward::{OpticsForwardStatus, ZoomForwardValue};
use std::collections::HashMap;

impl Engine {
    /// The measured Zoom opening of each programming owner from its supplied accepted physical
    /// output, in caller order. The outer `None` means the frame no longer belongs to the running
    /// generation/layout. An owner reads `None` when its root instance is missing, incomplete or
    /// has no compiled optics model, when it owns no Zoom or more than one Zoom head (ambiguous,
    /// as for the Zoom adapter), or when the forward model cannot resolve the current output.
    /// Multipatch copies replay the root's raw, so the root instance is authoritative.
    pub fn zoom_readouts_from_physical(
        &self,
        generation: u64,
        physical: &PhysicalForwardFrame,
        owners: &[FixtureId],
    ) -> Option<Vec<Option<ZoomForwardValue>>> {
        let current = self.generation.load_full();
        if current.identity() != generation
            || !physical.belongs_to_generation(generation)
            || !current.physical_projection().layout_matches(physical)
        {
            return None;
        }
        let mut roots: HashMap<FixtureId, Option<&PhysicalInstanceOutput>> = HashMap::new();
        for instance in &physical.instances {
            if instance.instance_id == instance.fixture_id.0 {
                roots
                    .entry(instance.fixture_id)
                    .and_modify(|entry| *entry = None)
                    .or_insert(Some(instance));
            }
        }
        Some(
            owners
                .iter()
                .map(|&owner| {
                    let (root, index) = current.profile_owner(owner)?;
                    let fixture = &current.snapshot().fixtures[index];
                    let mode = crate::fixture::profile_mode(fixture)?;
                    let instance = roots.get(&root).copied().flatten()?;
                    if !instance.complete
                        || instance.optics_support != PhysicalModelSupport::Compiled
                    {
                        return None;
                    }
                    let mut found = None;
                    for (head_index, head) in mode.heads.iter().enumerate() {
                        if crate::fixture::profile_head_owner(fixture, head_index, head) != owner {
                            continue;
                        }
                        let Some(result) = instance.optics().iter().find(|r| r.head_id == head.id)
                        else {
                            continue;
                        };
                        if result.zoom_status == OpticsForwardStatus::Unsupported {
                            continue;
                        }
                        if found.is_some() {
                            return None;
                        }
                        found = Some(result);
                    }
                    let result = found?;
                    (result.zoom_status == OpticsForwardStatus::Resolved)
                        .then_some(result.zoom)
                        .flatten()
                })
                .collect(),
        )
    }
}
