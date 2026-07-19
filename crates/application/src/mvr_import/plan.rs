use super::model::{
    ApplyActiveMvrImportCommand, MvrImportResolution, PlannedFixture, PlannedPatchChange,
    PreparedMvrImportState,
};
use super::projection::{mvr_transform, profile_projections, project_fixture};
use crate::{ActionContext, ActionError, ActionErrorKind};
use light_fixture::{FixtureDefinition, PatchedFixture, PatchedHead};
use light_mvr::MvrFixture;
use light_show::{PortableShowDocument, PortableShowTransaction};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

type OccupiedPatch = (u16, u16, u16, String);

struct ImportChanges {
    occupied: Vec<OccupiedPatch>,
    transaction: PortableShowTransaction,
    fixtures: Vec<PlannedFixture>,
    removed_fixture_ids: Vec<light_core::FixtureId>,
    warnings: Vec<String>,
}

impl ImportChanges {
    fn new(document: &PortableShowDocument, existing: &[&light_show::PortableShowObject]) -> Self {
        Self {
            occupied: occupied_patches(existing),
            transaction: document.transaction(),
            fixtures: Vec::new(),
            removed_fixture_ids: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn resolve_address(
        &mut self,
        source: &MvrFixture,
        fixture_id: light_core::FixtureId,
        definition: &FixtureDefinition,
        resolution: Option<&MvrImportResolution>,
    ) -> (Option<u16>, Option<u16>) {
        let (mut universe, mut address) = match resolution {
            Some(MvrImportResolution::Address { universe, address }) => {
                (Some(*universe), Some(*address))
            }
            Some(MvrImportResolution::ImportUnpatched) => (None, None),
            _ => (source.universe, source.address),
        };
        let Some((requested_universe, requested_address)) = universe.zip(address) else {
            return (universe, address);
        };
        let end = requested_address.saturating_add(definition.footprint.saturating_sub(1));
        let conflict = self
            .occupied
            .iter()
            .find(|(other_universe, other_address, footprint, id)| {
                *other_universe == requested_universe
                    && *id != fixture_id.0.to_string()
                    && *other_address <= end
                    && other_address.saturating_add(footprint.saturating_sub(1))
                        >= requested_address
            })
            .cloned();
        if let Some((_, _, _, id)) = conflict {
            if matches!(resolution, Some(MvrImportResolution::Replace)) {
                self.remove_conflicting_fixture(&id);
            } else {
                universe = None;
                address = None;
                self.warnings.push(format!(
                    "{} imported unpatched because its requested address conflicts",
                    source.name
                ));
            }
        }
        (universe, address)
    }

    fn remove_conflicting_fixture(&mut self, id: &str) {
        self.transaction.delete("patched_fixture", id);
        self.occupied.retain(|item| item.3 != id);
        self.fixtures
            .retain(|fixture| fixture.patch.fixture_id.0.to_string() != id);
        if let Ok(id) = Uuid::parse_str(id) {
            let id = light_core::FixtureId(id);
            if !self.removed_fixture_ids.contains(&id) {
                self.removed_fixture_ids.push(id);
            }
        }
    }
}

pub(super) struct PlannedMvrImport {
    pub transaction: PortableShowTransaction,
    pub state: PreparedMvrImportState,
}

pub(super) fn plan_import(
    document: &PortableShowDocument,
    context: ActionContext,
    command: &ApplyActiveMvrImportCommand,
) -> Result<PlannedMvrImport, ActionError> {
    if document.id() != command.show_id {
        return Err(not_found("requested show is not active"));
    }
    validate_unique_source_ids(&command.document.fixtures)?;
    let existing = document
        .objects_of_kind("patched_fixture")
        .collect::<Vec<_>>();
    let metadata = document.objects_of_kind("mvr_fixture").collect::<Vec<_>>();
    let fixture_ids = mvr_fixture_ids(&metadata);
    let mut changes = ImportChanges::new(document, &existing);
    let mut imported_ids = HashSet::new();
    let mut imported = 0;
    let mut unresolved = 0;

    for source in &command.document.fixtures {
        if matches!(
            command.resolutions.get(&source.uuid),
            Some(MvrImportResolution::Skip)
        ) {
            continue;
        }
        let Some(definition) = resolve_mvr_definition(&command.definitions, source) else {
            changes.transaction.put(
                "unresolved_mvr_fixture",
                source.uuid.to_string(),
                serde_json::to_value(source).map_err(invalid)?,
            );
            unresolved += 1;
            changes.warnings.push(format!(
                "{} requires {} mode {}",
                source.name, source.gdtf_spec, source.gdtf_mode
            ));
            continue;
        };
        let fixture_id = fixture_ids
            .get(&source.uuid)
            .and_then(|id| Uuid::parse_str(id).ok())
            .map(light_core::FixtureId)
            .unwrap_or_default();
        if !imported_ids.insert(fixture_id) {
            return Err(invalid(format!(
                "MVR fixtures resolve to duplicate show fixture identity {}",
                fixture_id.0
            )));
        }
        let address = changes.resolve_address(
            source,
            fixture_id,
            &definition,
            command.resolutions.get(&source.uuid),
        );
        let patched = patched_fixture(source, &definition, fixture_id, address, &existing);
        let projection = project_fixture(patched.clone())?;
        changes.transaction.put(
            "patched_fixture",
            fixture_id.0.to_string(),
            serde_json::to_value(patched).map_err(invalid)?,
        );
        changes.transaction.put(
            "mvr_fixture",
            source.uuid.to_string(),
            serde_json::json!({
                "fixture_id": fixture_id.0.to_string(),
                "gdtf_spec": source.gdtf_spec,
                "gdtf_mode": source.gdtf_mode,
            }),
        );
        changes.fixtures.push(projection);
        if let (Some(universe), Some(address)) = address {
            changes.occupied.push((
                universe,
                address,
                definition.footprint,
                fixture_id.0.to_string(),
            ));
        }
        imported += 1;
    }
    if !changes.fixtures.is_empty() || !changes.removed_fixture_ids.is_empty() {
        changes.transaction.mark_patch_changed();
    }
    if !command.document.geometry.is_empty() {
        changes.warnings.push(
            "MVR scene geometry was not imported. Add scenery from the Venue fixture library in Show Patch."
                .into(),
        );
    }
    let profiles = profile_projections(&changes.fixtures);
    Ok(PlannedMvrImport {
        transaction: changes.transaction,
        state: PreparedMvrImportState {
            context,
            imported_fixtures: imported,
            unresolved_fixtures: unresolved,
            warnings: changes.warnings,
            patch: PlannedPatchChange {
                fixtures: changes.fixtures,
                removed_fixture_ids: changes.removed_fixture_ids,
                profiles,
            },
        },
    })
}

pub fn resolve_mvr_definition(
    definitions: &[FixtureDefinition],
    fixture: &MvrFixture,
) -> Option<FixtureDefinition> {
    let spec = fixture
        .gdtf_spec
        .rsplit('/')
        .next()
        .unwrap_or(&fixture.gdtf_spec)
        .trim_end_matches(".gdtf");
    definitions
        .iter()
        .find(|definition| {
            definition.mode.eq_ignore_ascii_case(&fixture.gdtf_mode)
                && (definition.model.eq_ignore_ascii_case(spec)
                    || definition.name.eq_ignore_ascii_case(spec)
                    || format!("{}@{}", definition.manufacturer, definition.model)
                        .eq_ignore_ascii_case(spec))
        })
        .cloned()
}

fn validate_unique_source_ids(fixtures: &[MvrFixture]) -> Result<(), ActionError> {
    let mut seen = HashSet::with_capacity(fixtures.len());
    if fixtures.iter().all(|fixture| seen.insert(fixture.uuid)) {
        Ok(())
    } else {
        Err(invalid("MVR fixture UUIDs must be unique"))
    }
}

fn occupied_patches(objects: &[&light_show::PortableShowObject]) -> Vec<OccupiedPatch> {
    objects
        .iter()
        .filter_map(|object| {
            serde_json::from_value::<PatchedFixture>(object.body().clone())
                .ok()
                .and_then(|fixture| {
                    Some((
                        fixture.universe?,
                        fixture.address?,
                        fixture.definition.footprint,
                        object.key().id().to_owned(),
                    ))
                })
        })
        .collect()
}

fn mvr_fixture_ids(objects: &[&light_show::PortableShowObject]) -> HashMap<Uuid, String> {
    objects
        .iter()
        .filter_map(|object| {
            Uuid::parse_str(object.key().id()).ok().and_then(|uuid| {
                object
                    .body()
                    .get("fixture_id")?
                    .as_str()
                    .map(|id| (uuid, id.to_owned()))
            })
        })
        .collect()
}

fn patched_fixture(
    source: &MvrFixture,
    definition: &FixtureDefinition,
    fixture_id: light_core::FixtureId,
    address: (Option<u16>, Option<u16>),
    existing: &[&light_show::PortableShowObject],
) -> PatchedFixture {
    let (location, rotation) = mvr_transform(source.matrix);
    let existing_mib = existing
        .iter()
        .find(|object| object.key().id() == fixture_id.0.to_string())
        .and_then(|object| serde_json::from_value::<PatchedFixture>(object.body().clone()).ok())
        .map(|fixture| {
            (
                fixture.move_in_black_enabled,
                fixture.move_in_black_delay_millis,
            )
        });
    PatchedFixture {
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
        layer_id: source.layer.clone().unwrap_or_else(|| "default".into()),
        direct_control: None,
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
        move_in_black_enabled: existing_mib.is_none_or(|settings| settings.0),
        move_in_black_delay_millis: existing_mib.map_or(0, |settings| settings.1),
        highlight_overrides: Default::default(),
        multipatch: Vec::new(),
    }
}

fn invalid(error: impl std::fmt::Display) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, error.to_string())
}

fn not_found(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::NotFound, message)
}
