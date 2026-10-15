//! Import candidates shared by preview and Apply; no fixture/profile projection is performed.
use super::*;
use crate::{ActionError, ActionErrorKind};
use light_fixture::{FixtureDefinition, PatchedFixture, PatchedHead, PortablePatchedFixtureRecord};
use light_mvr::MvrFixture;
use light_show::{PortableShowDocument, PortableShowObject};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

pub(super) struct MvrPatchCandidates<'a> {
    existing: Vec<&'a PortableShowObject>,
    native: HashMap<Uuid, crate::mvr_export::ToskLightMvrFixture>,
    owners: HashMap<Uuid, light_core::FixtureId>,
}
impl<'a> MvrPatchCandidates<'a> {
    pub(super) fn new(
        destination: Option<&'a PortableShowDocument>,
        source: &light_mvr::MvrDocument,
    ) -> Self {
        let native = crate::mvr_export::tosklight_mvr_fixture_metadata(source);
        let owners = destination
            .map(|document| super::plan::mvr_destination_fixture_ids(document, &native))
            .unwrap_or_else(|| {
                native
                    .iter()
                    .map(|(uuid, data)| (*uuid, data.fixture.fixture_id))
                    .collect()
            });
        Self {
            existing: destination
                .map(|document| document.objects_of_kind("patched_fixture").collect())
                .unwrap_or_default(),
            native,
            owners,
        }
    }
    pub(super) fn embedded(&self, uuid: &Uuid) -> Option<&crate::mvr_export::ToskLightMvrFixture> {
        self.native.get(uuid)
    }
    pub(super) fn fixture_id(&self, source: &light_mvr::MvrFixture) -> light_core::FixtureId {
        self.owners.get(&source.uuid).copied().unwrap_or_default()
    }
    pub(super) fn patch(
        &self,
        source: &light_mvr::MvrFixture,
        definition: &FixtureDefinition,
        fixture_id: light_core::FixtureId,
        address: (Option<u16>, Option<u16>),
        layer: String,
        suppress: bool,
    ) -> PatchedFixture {
        patched_fixture(
            source,
            definition,
            fixture_id,
            address,
            layer,
            &self.existing,
            self.embedded(&source.uuid),
            suppress,
        )
    }
}

pub(super) fn conflicting_owners(
    candidate: &[OccupiedPatch],
    occupied: &[OccupiedPatch],
    owner: &str,
) -> Vec<String> {
    let mut seen = HashSet::new();
    occupied
        .iter()
        .filter(|other| other.3 != owner && candidate.iter().any(|row| overlaps(row, other)))
        .filter_map(|row| seen.insert(row.3.clone()).then(|| row.3.clone()))
        .collect()
}
fn overlaps(left: &OccupiedPatch, right: &OccupiedPatch) -> bool {
    left.0 == right.0
        && left.1 <= right.1.saturating_add(right.2.saturating_sub(1))
        && right.1 <= left.1.saturating_add(left.2.saturating_sub(1))
}
/// The original replacement contract addresses the first foreign owner at the primary patch.
pub(super) fn primary_conflicting_owner(
    address: (Option<u16>, Option<u16>),
    definition: &FixtureDefinition,
    occupied: &[OccupiedPatch],
    owner: &str,
) -> Option<String> {
    let (universe, address) = address.0.zip(address.1)?;
    let primary = (
        universe,
        address,
        primary_footprint(definition),
        owner.to_owned(),
    );
    occupied
        .iter()
        .find(|other| other.3 != owner && overlaps(&primary, other))
        .map(|row| row.3.clone())
}

pub struct MvrAddressConflict {
    pub fixture_uuid: Uuid,
    pub message: String,
}

/// Simulate default sequential occupancy without compiling profiles or publishing data.
pub fn preview_mvr_address_conflicts(
    destination: Option<&PortableShowDocument>,
    source: &light_mvr::MvrDocument,
    definitions: &HashMap<Uuid, FixtureDefinition>,
) -> Result<Vec<MvrAddressConflict>, ActionError> {
    super::plan::validate_unique_source_ids(&source.fixtures)?;
    let candidates = MvrPatchCandidates::new(destination, source);
    let mut occupied = destination
        .map(occupied_patches)
        .transpose()?
        .unwrap_or_default();
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for fixture in &source.fixtures {
        let Some(definition) = definitions.get(&fixture.uuid).or_else(|| {
            candidates
                .embedded(&fixture.uuid)
                .map(|data| &data.fixture.definition)
        }) else {
            continue;
        };
        let id = candidates.fixture_id(fixture);
        if !seen.insert(id) {
            return Err(ActionError::new(
                ActionErrorKind::Invalid,
                format!(
                    "MVR fixtures resolve to duplicate show fixture identity {}",
                    id.0
                ),
            ));
        }
        let owner = id.0.to_string();
        let candidate = candidates.patch(
            fixture,
            definition,
            id,
            (fixture.universe, fixture.address),
            "default".into(),
            false,
        );
        let rows = fixture_occupied_patches(&candidate, &owner);
        let conflicts = conflicting_owners(&rows, &occupied, &owner);
        if let Some(row) = rows.iter().find(|row| {
            occupied
                .iter()
                .any(|other| other.3 != owner && overlaps(row, other))
        }) {
            result.push(MvrAddressConflict {
                fixture_uuid: fixture.uuid,
                message: format!(
                    "{} conflicts at universe {} address {}-{}",
                    fixture.name,
                    row.0,
                    row.1,
                    row.1.saturating_add(row.2.saturating_sub(1))
                ),
            });
        }
        occupied.retain(|row| row.3 != owner);
        if conflicts.is_empty() {
            occupied.extend(rows);
        }
    }
    Ok(result)
}

pub(super) fn patched_fixture(
    source: &MvrFixture,
    definition: &FixtureDefinition,
    fixture_id: light_core::FixtureId,
    address: (Option<u16>, Option<u16>),
    layer_id: String,
    existing: &[&light_show::PortableShowObject],
    embedded: Option<&crate::mvr_export::ToskLightMvrFixture>,
    suppress_all_outputs: bool,
) -> PatchedFixture {
    // A matrix this desk wrote carries its bracket about its hinge; that comes back out here.
    let (location, rotation) = crate::mvr_export::mvr_fixture_placement(source.matrix, embedded);
    let existing_patch = existing
        .iter()
        .find(|object| object.key().id() == fixture_id.0.to_string())
        .and_then(|object| PortablePatchedFixtureRecord::decode(object.body().clone()).ok())
        .and_then(|record| record.patch().ok());
    let mut patched = embedded
        .map(|embedded| embedded.fixture.clone())
        .unwrap_or_else(|| PatchedFixture {
            model_scale: None,
            scenery_options: Default::default(),
            scenery_size_metres: None,
            fixture_id,
            fixture_number: source
                .fixture_id
                .as_deref()
                .and_then(|value| value.parse().ok()),
            virtual_fixture_number: None,
            name: source.name.clone(),
            definition: definition.clone(),
            universe: address.0,
            address: address.1,
            split_patches: Vec::new(),
            layer_id: layer_id.clone(),
            // An imported rig is placed against the stage; nothing in MVR describes a 3D Point.
            note: None,
            position_master: None,
            direct_control: None,
            internal_bindings: Default::default(),
            location,
            rotation,
            logical_heads: definition
                .heads
                .iter()
                .filter(|head| !head.shared)
                .map(|head| PatchedHead {
                    profile_head_id: None,
                    head_index: head.index,
                    fixture_id: light_core::FixtureId::new(),
                })
                .collect(),
            move_in_black_enabled: existing_patch
                .as_ref()
                .is_none_or(|fixture| fixture.move_in_black_enabled),
            move_in_black_delay_millis: existing_patch
                .as_ref()
                .map_or(0, |fixture| fixture.move_in_black_delay_millis),
            group_masters_enabled: existing_patch
                .as_ref()
                .is_none_or(|fixture| fixture.group_masters_enabled),
            grand_master_enabled: existing_patch
                .as_ref()
                .is_none_or(|fixture| fixture.grand_master_enabled),
            invert_pan: existing_patch
                .as_ref()
                .is_some_and(|fixture| fixture.invert_pan),
            invert_tilt: existing_patch
                .as_ref()
                .is_some_and(|fixture| fixture.invert_tilt),
            position_calibration: existing_patch
                .as_ref()
                .and_then(|fixture| fixture.position_calibration.clone()),
            color_calibration: existing_patch
                .as_ref()
                .and_then(|fixture| fixture.color_calibration.clone()),
            // An MVR source owns the root transform and address, but knows nothing about installed
            // lamp/filter/mechanical settings or desk-owned physical copies. Retain those exact
            // values across a reference-only portable record as well as a legacy inline record.
            bracket_angle: existing_patch
                .as_ref()
                .map_or(0.0, |fixture| fixture.bracket_angle),
            shaper_angle: existing_patch
                .as_ref()
                .and_then(|fixture| fixture.shaper_angle),
            installed_appearance: existing_patch
                .as_ref()
                .map_or_else(Default::default, |fixture| {
                    fixture.installed_appearance.clone()
                }),
            highlight_overrides: Default::default(),
            freeze: existing_patch
                .as_ref()
                .map_or_else(Default::default, |fixture| fixture.freeze.clone()),
            multipatch: existing_patch
                .as_ref()
                .map_or_else(Vec::new, |fixture| fixture.multipatch.clone()),
        });
    patched.fixture_id = fixture_id;
    patched.name = source.name.clone();
    patched.definition = definition.clone();
    patched.universe = address.0;
    patched.address = address.1;
    patched.layer_id = layer_id;
    patched.location = location;
    patched.rotation = rotation;
    if let Some(existing_patch) = existing_patch {
        patched.move_in_black_enabled = existing_patch.move_in_black_enabled;
        patched.move_in_black_delay_millis = existing_patch.move_in_black_delay_millis;
        patched.group_masters_enabled = existing_patch.group_masters_enabled;
        patched.grand_master_enabled = existing_patch.grand_master_enabled;
        patched.invert_pan = existing_patch.invert_pan;
        patched.invert_tilt = existing_patch.invert_tilt;
        patched.position_calibration = existing_patch.position_calibration.clone();
        patched.color_calibration = existing_patch.color_calibration.clone();
        patched.bracket_angle = existing_patch.bracket_angle;
        patched.shaper_angle = existing_patch.shaper_angle;
        patched.installed_appearance = existing_patch.installed_appearance;
        patched.freeze = existing_patch.freeze;
        patched.multipatch = existing_patch.multipatch;
    }
    apply_mvr_primary_address(&mut patched, address, suppress_all_outputs);
    patched
}
