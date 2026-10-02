use super::layers::MvrLayerPlan;
use super::model::{
    ApplyActiveMvrImportCommand, MvrImportResolution, PlannedFixture, PlannedPatchChange,
    PreparedMvrImportState,
};
use super::projection::{ProjectionCache, profile_projections, project_fixture};
use crate::{ActionContext, ActionError, ActionErrorKind};
use light_fixture::{FixtureDefinition, PatchedFixture, PatchedHead, PortablePatchedFixtureRecord};
use light_mvr::MvrFixture;
use light_show::{PortableShowDocument, PortableShowTransaction};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use super::occupied::{
    OccupiedPatch, apply_mvr_primary_address, fixture_occupied_patches, occupied_patches,
    primary_footprint,
};

struct ImportChanges {
    occupied: Vec<OccupiedPatch>,
    transaction: PortableShowTransaction,
    fixtures: Vec<PlannedFixture>,
    removed_fixture_ids: Vec<light_core::FixtureId>,
    warnings: Vec<String>,
}

impl ImportChanges {
    fn new(document: &PortableShowDocument) -> Result<Self, ActionError> {
        Ok(Self {
            occupied: occupied_patches(document)?,
            transaction: document.transaction(),
            fixtures: Vec::new(),
            removed_fixture_ids: Vec::new(),
            warnings: Vec::new(),
        })
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
        let end = requested_address.saturating_add(primary_footprint(definition).saturating_sub(1));
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
    plan_document(
        document,
        context,
        &command.document,
        &command.definitions,
        &command.resolutions,
    )
}

fn plan_document(
    document: &PortableShowDocument,
    context: ActionContext,
    source_document: &light_mvr::MvrDocument,
    command_definitions: &HashMap<Uuid, FixtureDefinition>,
    command_resolutions: &HashMap<Uuid, MvrImportResolution>,
) -> Result<PlannedMvrImport, ActionError> {
    validate_unique_source_ids(&source_document.fixtures)?;
    let existing = document
        .objects_of_kind("patched_fixture")
        .collect::<Vec<_>>();
    let embedded_fixtures = crate::mvr_export::tosklight_mvr_fixture_metadata(source_document);
    let fixture_ids = mvr_destination_fixture_ids(document, &embedded_fixtures);
    let mut layers = MvrLayerPlan::new(
        source_document,
        document
            .objects_of_kind("patch_layer")
            .map(|object| (object.key().id().to_owned(), object.body().clone())),
    );
    let mut changes = ImportChanges::new(document)?;
    let mut imported_ids = HashSet::new();
    let mut projection_cache = ProjectionCache::default();
    let mut imported = 0;
    let mut unresolved = 0;
    let mut retained_sources = super::RetainedMvrSources::new(source_document);

    for source in &source_document.fixtures {
        if matches!(
            command_resolutions.get(&source.uuid),
            Some(MvrImportResolution::Skip)
        ) {
            continue;
        }
        let embedded = embedded_fixtures.get(&source.uuid);
        let Some(definition) = embedded
            .map(|embedded| embedded.fixture.definition.clone())
            .or_else(|| command_definitions.get(&source.uuid).cloned())
        else {
            changes.transaction.put(
                "unresolved_mvr_fixture",
                source.uuid.to_string(),
                retained_sources.unresolved_fixture(source)?,
            );
            unresolved += 1;
            changes.warnings.push(format!(
                "{} requires {} mode {}",
                source.name, source.gdtf_spec, source.gdtf_mode
            ));
            continue;
        };
        let fixture_id = fixture_ids.get(&source.uuid).copied().unwrap_or_default();
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
            command_resolutions.get(&source.uuid),
        );
        let patched = patched_fixture(
            source,
            &definition,
            fixture_id,
            address,
            layers.layer_for(source.layer.as_deref()),
            &existing,
            embedded,
            matches!(
                command_resolutions.get(&source.uuid),
                Some(MvrImportResolution::ImportUnpatched)
            ) || ((source.universe.zip(source.address).is_some()
                || matches!(
                    command_resolutions.get(&source.uuid),
                    Some(MvrImportResolution::Address { .. })
                ))
                && address.0.zip(address.1).is_none()),
        );
        let occupied = fixture_occupied_patches(&patched, &fixture_id.0.to_string());
        let projection = project_fixture(patched, &mut projection_cache)?;
        changes.transaction.put(
            "patched_fixture",
            fixture_id.0.to_string(),
            projection.record.clone(),
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
        changes
            .occupied
            .retain(|row| row.3 != fixture_id.0.to_string());
        changes.occupied.extend(occupied);
        imported += 1;
    }
    for (id, archive) in retained_sources.archives() {
        if document
            .object(super::MVR_SOURCE_ARCHIVE_KIND, id)
            .is_some_and(|object| object.body() != archive)
        {
            return Err(invalid("retained MVR source archive hash conflicts"));
        }
        changes
            .transaction
            .put(super::MVR_SOURCE_ARCHIVE_KIND, id, archive.clone());
    }
    // Only layers an imported fixture landed on are created, in the same change as the fixtures.
    for (id, body) in layers.created() {
        changes
            .transaction
            .put("patch_layer", id.clone(), body.clone());
    }
    if !changes.fixtures.is_empty() || !changes.removed_fixture_ids.is_empty() {
        changes.transaction.mark_patch_changed();
    }
    if !source_document.geometry.is_empty() {
        changes.warnings.push(
            "MVR scene geometry was not imported. Add scenery from the Venue fixture library in Show Patch."
                .into(),
        );
    }
    let profiles = profile_projections(&changes.fixtures)?;
    for profile in &profiles {
        let retained =
            light_show::FixtureProfileRevision::from_profile(profile.profile_snapshot.clone())
                .map_err(invalid)?;
        changes
            .transaction
            .put_fixture_profile_revision(retained)
            .map_err(invalid)?;
    }
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
    let normalized = fixture.gdtf_spec.replace('\\', "/").to_ascii_lowercase();
    let spec = normalized
        .rsplit('/')
        .next()
        .unwrap_or(&normalized)
        .trim_end_matches(".gdtf");
    let mut matches = definitions.iter().filter(|definition| {
        definition.mode.eq_ignore_ascii_case(&fixture.gdtf_mode)
            && (definition.model.eq_ignore_ascii_case(spec)
                || definition.name.eq_ignore_ascii_case(spec)
                || format!("{}@{}", definition.manufacturer, definition.model)
                    .eq_ignore_ascii_case(spec))
    });
    let first = matches.next()?;
    // A name is only a fallback when it has one meaning in this library.
    matches.next().is_none().then(|| first.clone())
}

fn validate_unique_source_ids(fixtures: &[MvrFixture]) -> Result<(), ActionError> {
    let mut seen = HashSet::with_capacity(fixtures.len());
    if fixtures.iter().all(|fixture| seen.insert(fixture.uuid)) {
        Ok(())
    } else {
        Err(invalid("MVR fixture UUIDs must be unique"))
    }
}

/// Existing source association wins over a native export ID. Preview and Apply share ownership.
pub fn mvr_destination_fixture_ids(
    destination: &PortableShowDocument,
    native: &HashMap<Uuid, crate::mvr_export::ToskLightMvrFixture>,
) -> HashMap<Uuid, light_core::FixtureId> {
    let mut ids = native
        .iter()
        .map(|(uuid, entry)| (*uuid, entry.fixture.fixture_id))
        .collect::<HashMap<_, _>>();
    for object in destination.objects_of_kind("mvr_fixture") {
        if let (Ok(source), Some(id)) = (
            Uuid::parse_str(object.key().id()),
            object
                .body()
                .get("fixture_id")
                .and_then(|v| v.as_str())
                .and_then(|id| Uuid::parse_str(id).ok()),
        ) {
            ids.insert(source, light_core::FixtureId(id));
        }
    }
    ids
}

fn patched_fixture(
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

fn invalid(error: impl std::fmt::Display) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, error.to_string())
}

fn not_found(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::NotFound, message)
}

/// Shared, transport-neutral plan for an inactive/new show. The caller validates and commits the
/// returned transaction to its isolated candidate file before replacing any live destination.
pub struct MvrDocumentImport {
    pub transaction: PortableShowTransaction,
    pub imported_fixtures: usize,
    pub unresolved_fixtures: usize,
    pub warnings: Vec<String>,
}

pub fn plan_mvr_document_import(
    document: &PortableShowDocument,
    context: ActionContext,
    source: &light_mvr::MvrDocument,
    definitions: &HashMap<Uuid, FixtureDefinition>,
    resolutions: &HashMap<Uuid, MvrImportResolution>,
) -> Result<MvrDocumentImport, ActionError> {
    let planned = plan_document(document, context, source, definitions, resolutions)?;
    Ok(MvrDocumentImport {
        transaction: planned.transaction,
        imported_fixtures: planned.state.imported_fixtures,
        unresolved_fixtures: planned.state.unresolved_fixtures,
        warnings: planned.state.warnings,
    })
}
