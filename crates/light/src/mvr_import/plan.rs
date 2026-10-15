use super::candidates::{MvrPatchCandidates, conflicting_owners, primary_conflicting_owner};
use super::layers::MvrLayerPlan;
use super::model::{
    ApplyActiveMvrImportCommand, MvrImportResolution, PlannedFixture, PlannedPatchChange,
    PreparedMvrImportState,
};
use super::projection::{ProjectionCache, profile_projections, project_fixture};
use crate::{ActionContext, ActionError, ActionErrorKind};
use light_fixture::{FixtureDefinition, PatchedFixture};
use light_mvr::MvrFixture;
use light_show::{PortableShowDocument, PortableShowTransaction};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use super::occupied::{
    OccupiedPatch, apply_mvr_primary_address, fixture_occupied_patches, occupied_patches,
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

    fn resolve_candidate(
        &mut self,
        candidates: &MvrPatchCandidates<'_>,
        source: &MvrFixture,
        definition: &FixtureDefinition,
        fixture_id: light_core::FixtureId,
        resolution: Option<&MvrImportResolution>,
        layer: String,
    ) -> Result<(PatchedFixture, Vec<OccupiedPatch>), ActionError> {
        let address = match resolution {
            Some(MvrImportResolution::Address { universe, address }) => {
                (Some(*universe), Some(*address))
            }
            Some(MvrImportResolution::ImportUnpatched) => (None, None),
            _ => (source.universe, source.address),
        };
        let mut patched = candidates.patch(
            source,
            &definition,
            fixture_id,
            address,
            layer.clone(),
            imports_unpatched(source, resolution, address),
        );
        let mut occupied = fixture_occupied_patches(&patched, &fixture_id.0.to_string());
        let conflicts = conflicting_owners(&occupied, &self.occupied, &fixture_id.0.to_string());
        if !conflicts.is_empty() {
            if matches!(resolution, Some(MvrImportResolution::Replace)) {
                // Preserve the existing single primary-owner replacement only. Additional
                // secondary/copy ownership requires a separate explicit operator decision.
                let primary_owner = primary_conflicting_owner(
                    address,
                    &definition,
                    &self.occupied,
                    &fixture_id.0.to_string(),
                );
                if conflicts.len() != 1 || primary_owner.as_deref() != Some(conflicts[0].as_str()) {
                    return Err(ActionError::new(
                        ActionErrorKind::Conflict,
                        "Cannot replace conflicts on secondary/copy outputs; choose Import unpatched, a safe address or Skip",
                    ));
                }
                self.remove_conflicting_fixture(&conflicts[0]);
            } else {
                patched =
                    candidates.patch(source, &definition, fixture_id, (None, None), layer, true);
                occupied.clear();
                self.warnings.push(format!(
                    "{} imported unpatched because its requested address conflicts",
                    source.name
                ));
            }
        }
        Ok((patched, occupied))
    }

    /// Puts one imported fixture and its MVR association, replacing its occupied patch rows.
    fn record_fixture(
        &mut self,
        source: &MvrFixture,
        fixture_id: light_core::FixtureId,
        projection: PlannedFixture,
        occupied: Vec<OccupiedPatch>,
    ) {
        self.transaction.put(
            "patched_fixture",
            fixture_id.0.to_string(),
            projection.record.clone(),
        );
        self.transaction.put(
            "mvr_fixture",
            source.uuid.to_string(),
            serde_json::json!({
                "fixture_id": fixture_id.0.to_string(),
                "gdtf_spec": source.gdtf_spec,
                "gdtf_mode": source.gdtf_mode,
            }),
        );
        self.fixtures.push(projection);
        self.occupied
            .retain(|row| row.3 != fixture_id.0.to_string());
        self.occupied.extend(occupied);
    }

    fn put_retained_archives(
        &mut self,
        document: &PortableShowDocument,
        retained_sources: &super::RetainedMvrSources,
    ) -> Result<(), ActionError> {
        for (id, archive) in retained_sources.archives() {
            if document
                .object(super::MVR_SOURCE_ARCHIVE_KIND, id)
                .is_some_and(|object| object.body() != archive)
            {
                return Err(invalid("retained MVR source archive hash conflicts"));
            }
            self.transaction
                .put(super::MVR_SOURCE_ARCHIVE_KIND, id, archive.clone());
        }
        Ok(())
    }

    fn put_profile_revisions(
        &mut self,
        profiles: &[crate::PatchProfileRevisionProjection],
    ) -> Result<(), ActionError> {
        for profile in profiles {
            let retained =
                light_show::FixtureProfileRevision::from_profile(profile.profile_snapshot.clone())
                    .map_err(invalid)?;
            self.transaction
                .put_fixture_profile_revision(retained)
                .map_err(invalid)?;
        }
        Ok(())
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
    let candidates = MvrPatchCandidates::new(Some(document), source_document);
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
        let embedded = candidates.embedded(&source.uuid);
        // The trusted preview binding may contain an explicitly copied immutable profile.
        // Native metadata still supplies fixture placement/settings, but must not undo that
        // profile identity choice at apply time. Callers without a prepared binding keep the
        // original native fallback.
        let Some(definition) = command_definitions
            .get(&source.uuid)
            .cloned()
            .or_else(|| embedded.map(|embedded| embedded.fixture.definition.clone()))
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
        let fixture_id = candidates.fixture_id(source);
        if !imported_ids.insert(fixture_id) {
            return Err(invalid(format!(
                "MVR fixtures resolve to duplicate show fixture identity {}",
                fixture_id.0
            )));
        }
        let (patched, occupied) = changes.resolve_candidate(
            &candidates,
            source,
            &definition,
            fixture_id,
            command_resolutions.get(&source.uuid),
            layers.layer_for(source.layer.as_deref()),
        )?;
        let projection = project_fixture(patched, &mut projection_cache)?;
        changes.record_fixture(source, fixture_id, projection, occupied);
        imported += 1;
    }
    changes.put_retained_archives(document, &retained_sources)?;
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
    changes.put_profile_revisions(&profiles)?;
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

/// Whether a fixture imports unpatched: on request, or when its requested address was dropped.
fn imports_unpatched(
    source: &MvrFixture,
    resolution: Option<&MvrImportResolution>,
    address: (Option<u16>, Option<u16>),
) -> bool {
    matches!(resolution, Some(MvrImportResolution::ImportUnpatched))
        || ((source.universe.zip(source.address).is_some()
            || matches!(resolution, Some(MvrImportResolution::Address { .. })))
            && address.0.zip(address.1).is_none())
}

pub fn resolve_mvr_definition(
    definitions: &[FixtureDefinition],
    fixture: &MvrFixture,
) -> Option<FixtureDefinition> {
    let spec = mvr_source_spec(fixture);
    let mut matches = definitions.iter().filter(|definition| {
        matches_mvr_definition_metadata(
            &definition.manufacturer,
            &definition.model,
            &definition.name,
            &definition.mode,
            &spec,
            &fixture.gdtf_mode,
        )
    });
    let first = matches.next()?;
    // A name is only a fallback when it has one meaning in this library.
    matches.next().is_none().then(|| first.clone())
}

/// Preserve the installed-definition fallback's basename and alias rules without projection.
pub(super) fn mvr_source_spec(fixture: &MvrFixture) -> String {
    let normalized = fixture.gdtf_spec.replace('\\', "/").to_ascii_lowercase();
    normalized
        .rsplit('/')
        .next()
        .unwrap_or(&normalized)
        .trim_end_matches(".gdtf")
        .to_owned()
}

pub(super) fn matches_mvr_definition_metadata(
    manufacturer: &str,
    model: &str,
    name: &str,
    mode: &str,
    spec: &str,
    requested_mode: &str,
) -> bool {
    mode.eq_ignore_ascii_case(requested_mode)
        && (model.eq_ignore_ascii_case(spec)
            || name.eq_ignore_ascii_case(spec)
            || format!("{manufacturer}@{model}").eq_ignore_ascii_case(spec))
}

pub(super) fn validate_unique_source_ids(fixtures: &[MvrFixture]) -> Result<(), ActionError> {
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
