use crate::{FixtureError, PatchedFixture, SplitPatch};
use light_core::Universe;
use std::collections::{BTreeMap, HashMap, HashSet};

pub fn validate_patch(fixtures: &[PatchedFixture]) -> Result<(), FixtureError> {
    let mut validator = PatchValidator::new();
    for fixture in fixtures {
        validator.validate_fixture(fixture)?;
    }
    Ok(())
}

struct PatchValidator {
    used_slots: HashMap<Universe, [bool; 512]>,
    fixture_numbers: HashSet<u32>,
    virtual_fixture_numbers: HashSet<u32>,
    selection_ids: HashSet<uuid::Uuid>,
    multipatch_ids: HashSet<uuid::Uuid>,
}

impl PatchValidator {
    fn new() -> Self {
        Self {
            used_slots: HashMap::new(),
            fixture_numbers: HashSet::new(),
            virtual_fixture_numbers: HashSet::new(),
            selection_ids: HashSet::new(),
            multipatch_ids: HashSet::new(),
        }
    }

    fn validate_fixture(&mut self, fixture: &PatchedFixture) -> Result<(), FixtureError> {
        self.validate_stable_identities(fixture)?;
        self.validate_fixture_numbers(fixture)?;
        fixture.definition.validate()?;
        if !fixture.definition.is_dmx_patchable() {
            return validate_visual_only_fixture(fixture);
        }
        validate_direct_control(fixture)?;
        self.validate_instances(fixture)
    }

    fn validate_stable_identities(&mut self, fixture: &PatchedFixture) -> Result<(), FixtureError> {
        self.reserve_selection_id(fixture.fixture_id.0)?;
        let mut head_indices = HashSet::new();
        for head in &fixture.logical_heads {
            if !head_indices.insert(head.head_index) {
                return Err(invalid(format!(
                    "fixture {} repeats logical head index {}",
                    fixture.fixture_id.0, head.head_index
                )));
            }
            self.reserve_selection_id(head.fixture_id.0)?;
        }
        self.reserve_multipatch_ids(fixture)
    }

    fn reserve_selection_id(&mut self, id: uuid::Uuid) -> Result<(), FixtureError> {
        if !self.multipatch_ids.contains(&id) && self.selection_ids.insert(id) {
            Ok(())
        } else {
            Err(invalid(format!(
                "stable fixture, logical-head, or multipatch identity {id} is already in use"
            )))
        }
    }

    fn reserve_multipatch_ids(&mut self, fixture: &PatchedFixture) -> Result<(), FixtureError> {
        for instance in &fixture.multipatch {
            if self.selection_ids.contains(&instance.id) || !self.multipatch_ids.insert(instance.id)
            {
                return Err(invalid(format!(
                    "multipatch identity {} is already in use",
                    instance.id
                )));
            }
        }
        Ok(())
    }

    fn validate_fixture_numbers(&mut self, fixture: &PatchedFixture) -> Result<(), FixtureError> {
        if fixture.fixture_number.is_some() && fixture.virtual_fixture_number.is_some() {
            return Err(invalid(
                "a fixture cannot have both a regular and virtual fixture ID",
            ));
        }
        if let Some(number) = fixture.fixture_number {
            if number == 0 {
                return Err(invalid("fixture IDs start at 1"));
            }
            if !self.fixture_numbers.insert(number) {
                return Err(invalid(format!("fixture ID {number} is already in use")));
            }
        }
        if let Some(number) = fixture.virtual_fixture_number {
            if number == 0 {
                return Err(invalid("virtual fixture IDs start at 0.1"));
            }
            if fixture.definition.is_dmx_patchable() {
                return Err(invalid(format!(
                    "virtual fixture ID 0.{number} requires a visual-only fixture"
                )));
            }
            if !self.virtual_fixture_numbers.insert(number) {
                return Err(invalid(format!(
                    "virtual fixture ID 0.{number} is already in use"
                )));
            }
        }
        Ok(())
    }

    fn validate_instances(&mut self, fixture: &PatchedFixture) -> Result<(), FixtureError> {
        let footprints = fixture.definition.split_footprints();
        self.validate_instance(
            &fixture.fixture_id.0.to_string(),
            &fixture.split_patches,
            fixture.universe,
            fixture.address,
            &footprints,
        )?;
        for instance in &fixture.multipatch {
            self.validate_instance(
                &instance.id.to_string(),
                &instance.split_patches,
                instance.universe,
                instance.address,
                &footprints,
            )?;
        }
        Ok(())
    }

    fn validate_instance(
        &mut self,
        instance: &str,
        explicit_patches: &[SplitPatch],
        legacy_universe: Option<Universe>,
        legacy_address: Option<u16>,
        footprints: &BTreeMap<u16, u16>,
    ) -> Result<(), FixtureError> {
        let patches = normalized_patches(
            instance,
            explicit_patches,
            legacy_universe,
            legacy_address,
            footprints,
        )?;
        validate_split_assignments(instance, &patches, footprints)?;
        for patch in patches {
            self.reserve_patch(instance, patch, footprints[&patch.split])?;
        }
        Ok(())
    }

    fn reserve_patch(
        &mut self,
        instance: &str,
        patch: SplitPatch,
        footprint: u16,
    ) -> Result<(), FixtureError> {
        if patch.universe.is_some() != patch.address.is_some() {
            return Err(invalid(format!(
                "fixture instance {instance} split {} must set both universe and address or neither",
                patch.split
            )));
        }
        let (Some(universe), Some(address)) = (patch.universe, patch.address) else {
            return Ok(());
        };
        if address == 0 || usize::from(address) + usize::from(footprint) - 1 > 512 {
            return Err(invalid(format!(
                "fixture instance {instance} exceeds universe {universe}"
            )));
        }
        let slots = self.used_slots.entry(universe).or_insert([false; 512]);
        let start = usize::from(address - 1);
        for (offset, slot) in slots[start..start + usize::from(footprint)]
            .iter_mut()
            .enumerate()
        {
            if *slot {
                return Err(invalid(format!(
                    "patch overlap at universe {} address {}",
                    universe,
                    start + offset + 1
                )));
            }
            *slot = true;
        }
        Ok(())
    }
}

fn validate_visual_only_fixture(fixture: &PatchedFixture) -> Result<(), FixtureError> {
    if fixture.virtual_fixture_number.is_none() || fixture.fixture_number.is_some() {
        return Err(invalid(format!(
            "visual-only fixture {} requires an ID in the 0.x namespace",
            fixture.fixture_id.0
        )));
    }
    let has_patch = fixture.direct_control.is_some()
        || fixture.universe.is_some()
        || fixture.address.is_some()
        || has_split_patch(&fixture.split_patches)
        || fixture.multipatch.iter().any(|instance| {
            instance.universe.is_some()
                || instance.address.is_some()
                || has_split_patch(&instance.split_patches)
        });
    if has_patch {
        return Err(invalid(format!(
            "visual-only fixture {} cannot have a DMX or direct-control patch",
            fixture.fixture_id.0
        )));
    }
    Ok(())
}

fn has_split_patch(patches: &[SplitPatch]) -> bool {
    patches
        .iter()
        .any(|patch| patch.universe.is_some() || patch.address.is_some())
}

fn validate_direct_control(fixture: &PatchedFixture) -> Result<(), FixtureError> {
    let Some(endpoint) = &fixture.direct_control else {
        return Ok(());
    };
    if endpoint.port == 0 {
        return Err(invalid(format!(
            "fixture {} has an invalid direct-control port",
            fixture.fixture_id.0
        )));
    }
    if !fixture
        .definition
        .direct_control_protocols
        .contains(&endpoint.protocol)
    {
        return Err(invalid(format!(
            "fixture {} profile does not support {:?} direct control",
            fixture.fixture_id.0, endpoint.protocol
        )));
    }
    Ok(())
}

fn normalized_patches(
    instance: &str,
    explicit_patches: &[SplitPatch],
    legacy_universe: Option<Universe>,
    legacy_address: Option<u16>,
    footprints: &BTreeMap<u16, u16>,
) -> Result<Vec<SplitPatch>, FixtureError> {
    if !explicit_patches.is_empty() {
        return Ok(explicit_patches.to_vec());
    }
    if footprints.len() > 1 {
        return Err(invalid(format!(
            "fixture instance {instance} must assign every split, including unpatched splits"
        )));
    }
    let split = *footprints
        .keys()
        .next()
        .ok_or_else(|| invalid(format!("fixture instance {instance} has no defined splits")))?;
    Ok(vec![SplitPatch {
        split,
        universe: legacy_universe,
        address: legacy_address,
    }])
}

fn validate_split_assignments(
    instance: &str,
    patches: &[SplitPatch],
    footprints: &BTreeMap<u16, u16>,
) -> Result<(), FixtureError> {
    let mut assigned = HashSet::new();
    for patch in patches {
        if !assigned.insert(patch.split) {
            return Err(invalid(format!(
                "fixture instance {instance} assigns split {} more than once",
                patch.split
            )));
        }
        if !footprints.contains_key(&patch.split) {
            return Err(invalid(format!(
                "fixture instance {instance} references unknown split {}",
                patch.split
            )));
        }
    }
    if let Some(missing) = footprints.keys().find(|split| !assigned.contains(split)) {
        return Err(invalid(format!(
            "fixture instance {instance} is missing split {missing}; every split needs an optional assignment entry"
        )));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> FixtureError {
    FixtureError::Invalid(message.into())
}
