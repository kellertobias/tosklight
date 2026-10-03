//! Cold native footprints of the Color and Focus/Zoom owners of one patched fixture (TL-548 C2).
//!
//! Every footprint is derived from the same compiled models the physical adapters fit with, never
//! from attribute names:
//! - lamp Color: the head's Color path controls of the instance's compiled Color forward model
//!   (identical to `CompiledColorFitting::controls`, which is built from that path);
//! - Media Color: the Media personality's controls (`MediaColorHead::from_mode`). A head carrying a
//!   Media Color identity never contributes lamp controls, as in the Color router;
//! - Focus and Zoom: the single owning control `CompiledOpticsFitting::control` reports.
//!
//! Position keeps its geometry-derived footprint (`FixtureProjectionPlan::position_footprint`).
//! Footprints are per logical owner and physical instance (root or multipatch copy).
use crate::physical_projection::PhysicalProjectionIndex;
use crate::profile_projection_plan::ProfileHeadPlan;
use light_core::{FixtureId, programming::ProgrammingOwner};
use light_fixture::{CompiledOpticsFitting, FixtureMode, OpticsFamily, PatchedFixture};
// Per-frame lookups by owner: hashed for speed, never for adversaries (TL-553).
use rustc_hash::FxHashMap as HashMap;
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Debug, Default)]
pub(crate) struct FamilyFootprints {
    rows: HashMap<(FixtureId, ProgrammingOwner, Uuid), Box<[usize]>>,
}

impl FamilyFootprints {
    pub(crate) fn compile(
        fixture: &PatchedFixture,
        mode: &FixtureMode,
        heads: &[ProfileHeadPlan],
        physical: &PhysicalProjectionIndex,
    ) -> Self {
        let mut rows: HashMap<_, BTreeSet<usize>> = HashMap::default();
        // Optics bindings are mode-level: one compile serves every instance. A mode the optics
        // model rejects owns no Focus/Zoom control, exactly as the adapter then has no fitter.
        let optics = CompiledOpticsFitting::compile(mode).ok();
        let instances =
            std::iter::once(fixture.fixture_id.0).chain(fixture.multipatch.iter().map(|c| c.id));
        for instance in instances {
            let color = physical.color_forward(fixture.fixture_id, instance);
            for head in heads {
                let controls: Vec<usize> =
                    if light_fixture::media_color::has_media_color_identity(mode, head.head_id) {
                        light_fixture::media_color::MediaColorHead::from_mode(mode, head.head_id)
                            .map(|media| {
                                media
                                    .controls()
                                    .iter()
                                    .map(|c| c.channel_index as usize)
                                    .collect()
                            })
                            .unwrap_or_default()
                    } else {
                        color
                            .and_then(|forward| forward.head_controls(head.head_id))
                            .map(<[usize]>::to_vec)
                            .unwrap_or_default()
                    };
                if !controls.is_empty() {
                    rows.entry((head.owner, ProgrammingOwner::Color, instance))
                        .or_default()
                        .extend(controls);
                }
                let Some(optics) = &optics else {
                    continue;
                };
                let Some(index) = optics.head_index(head.head_id) else {
                    continue;
                };
                for (family, owner) in [
                    (OpticsFamily::Focus, ProgrammingOwner::Focus),
                    (OpticsFamily::Zoom, ProgrammingOwner::Zoom),
                ] {
                    if let Some(Ok(control)) = optics.control(index, family) {
                        rows.entry((head.owner, owner, instance))
                            .or_default()
                            .insert(control.channel_index as usize);
                    }
                }
            }
        }
        Self {
            rows: rows
                .into_iter()
                .map(|(key, channels)| (key, channels.into_iter().collect()))
                .collect(),
        }
    }

    /// Sorted native footprint of one non-Position owner on one physical instance.
    pub(crate) fn get(
        &self,
        target: FixtureId,
        owner: ProgrammingOwner,
        instance: Uuid,
    ) -> Option<&[usize]> {
        self.rows.get(&(target, owner, instance)).map(AsRef::as_ref)
    }
}
