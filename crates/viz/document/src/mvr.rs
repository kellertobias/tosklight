//! MVR in and out of a planning document.
//!
//! Both directions reuse the desk's implementations: import runs `MvrImportService` against this
//! document's patch boundary, and export calls the shared builder the desk's own export endpoint
//! calls. A rig exchanged with another application therefore behaves the same whichever ToskLight
//! surface produced it.

use crate::{DocumentError, PlanningDocument};
use light_application::{
    ActionEnvelope, ApplyActiveMvrImportCommand, MvrImportResolution, MvrImportService,
    mvr_export::{
        GdtfSource, MvrExportSummary, MvrFixtureMetadata, build_mvr_document,
        compile_export_fixtures,
    },
};
use light_core::FixtureId;
use light_fixture::{FixtureLibrary, ResolvedFixtureProfileRevision};
use parking_lot::Mutex;
use std::collections::HashMap;
use uuid::Uuid;

/// One exported MVR archive and what went into it.
pub struct MvrExport {
    pub data: Vec<u8>,
    pub summary: MvrExportSummary,
}

/// What an import changed.
pub struct MvrImportOutcome {
    pub imported_fixtures: usize,
    pub unresolved_fixtures: usize,
    pub warnings: Vec<String>,
}

/// What an archive contains and how it lands in this document, read before anything is committed.
///
/// The operator decides what to do about a fixture the document cannot place on its own; without
/// this there is nothing to decide from, and an unresolved fixture is only ever counted after the
/// fact.
pub struct MvrPreview {
    pub fixtures: Vec<MvrPreviewFixture>,
    /// Scenery objects the archive carries.
    pub scenery: usize,
    /// GDTF spec and mode pairs no profile in this document's library matches.
    pub missing_profiles: Vec<String>,
    /// Fixtures whose address range overlaps something already patched here.
    pub address_conflicts: Vec<String>,
}

/// One fixture in an archive, as this document sees it.
pub struct MvrPreviewFixture {
    pub uuid: Uuid,
    pub name: String,
    pub gdtf_spec: String,
    pub gdtf_mode: String,
    pub universe: Option<u16>,
    pub address: Option<u16>,
    /// Whether a profile in the library matches this fixture's GDTF spec and mode.
    pub matched: bool,
    /// Whether importing it at its own address would overlap a fixture already patched here.
    pub conflicted: bool,
}

/// Reads retained source GDTF from the fixture library the document patches from.
struct LibraryGdtf<'a>(Option<&'a Mutex<FixtureLibrary>>);

impl GdtfSource for LibraryGdtf<'_> {
    type Error = DocumentError;

    fn source_gdtf(
        &self,
        profile: FixtureId,
        revision: u32,
    ) -> Result<Option<Vec<u8>>, Self::Error> {
        let Some(library) = self.0 else {
            return Ok(None);
        };
        Ok(library.lock().profile_source_gdtf(profile, revision)?)
    }
}

impl PlanningDocument {
    /// Writes the rig as an MVR archive for another application to open.
    pub fn export_mvr(&self) -> Result<MvrExport, DocumentError> {
        let store = self.store()?;
        let metadata: MvrFixtureMetadata = store
            .objects("mvr_fixture")?
            .into_iter()
            .filter_map(|object| {
                let id = object.body.get("fixture_id")?.as_str()?.to_owned();
                Some((id, object.body))
            })
            .collect();
        let objects = store
            .objects("patched_fixture")?
            .into_iter()
            .map(|object| (object.id, object.body));
        let fixtures = compile_export_fixtures(objects, |reference| {
            store
                .resolve_fixture_profile_revision(reference.profile_id, reference.profile_revision)
                .ok()
                .flatten()
                .map(|profile| {
                    ResolvedFixtureProfileRevision::new(
                        profile.id().profile_id(),
                        profile.id().revision(),
                        profile.digest().as_str(),
                        profile.profile().clone(),
                    )
                })
        })
        .map_err(|error| DocumentError::Mvr(error.to_string()))?;
        let (document, summary) =
            build_mvr_document(&fixtures, &metadata, &LibraryGdtf(self.ports().library()))?;
        let data =
            light_mvr::write(&document).map_err(|error| DocumentError::Mvr(error.to_string()))?;
        Ok(MvrExport { data, summary })
    }

    /// Reads an MVR archive without changing anything, so the operator can see what it contains
    /// and how its fixtures resolve before committing to the import.
    pub fn read_mvr(data: &[u8]) -> Result<light_mvr::MvrDocument, DocumentError> {
        light_mvr::read(data).map_err(|error| DocumentError::Mvr(error.to_string()))
    }

    /// Describes what importing `document` into this document would do.
    ///
    /// Nothing is written and nothing is staged: the archive is read again when the operator
    /// commits, so a preview cannot expire and there is no import token to keep alive.
    pub fn preview_mvr(
        &self,
        document: &light_mvr::MvrDocument,
    ) -> Result<MvrPreview, DocumentError> {
        let definitions = match self.ports().library() {
            Some(library) => library.lock().patchable_definitions()?,
            None => Vec::new(),
        };
        let patched: Vec<light_fixture::PatchedFixture> = self
            .store()?
            .objects("patched_fixture")?
            .into_iter()
            .filter_map(|object| serde_json::from_value(object.body).ok())
            .collect();

        let mut missing = std::collections::BTreeSet::new();
        let mut conflicts = Vec::new();
        let mut fixtures = Vec::with_capacity(document.fixtures.len());
        for fixture in &document.fixtures {
            let definition = light_application::resolve_mvr_definition(&definitions, fixture);
            if definition.is_none() {
                missing.insert(format!("{} · {}", fixture.gdtf_spec, fixture.gdtf_mode));
            }
            let conflicted = match (fixture.universe, fixture.address, definition.as_ref()) {
                (Some(universe), Some(address), Some(definition)) => {
                    let end = address.saturating_add(definition.footprint.saturating_sub(1));
                    let overlapping = patched.iter().any(|existing| {
                        existing.universe == Some(universe)
                            && existing.address.is_some_and(|start| {
                                start <= end
                                    && start.saturating_add(
                                        existing.definition.footprint.saturating_sub(1),
                                    ) >= address
                            })
                    });
                    if overlapping {
                        conflicts.push(format!(
                            "{} conflicts at universe {universe} address {address}-{end}",
                            fixture.name
                        ));
                    }
                    overlapping
                }
                _ => false,
            };
            fixtures.push(MvrPreviewFixture {
                uuid: fixture.uuid,
                name: fixture.name.clone(),
                gdtf_spec: fixture.gdtf_spec.clone(),
                gdtf_mode: fixture.gdtf_mode.clone(),
                universe: fixture.universe,
                address: fixture.address,
                matched: definition.is_some(),
                conflicted,
            });
        }
        Ok(MvrPreview {
            fixtures,
            scenery: document.geometry.len(),
            missing_profiles: missing.into_iter().collect(),
            address_conflicts: conflicts,
        })
    }

    /// Imports an MVR archive into this document.
    ///
    /// `resolutions` decides what each MVR fixture becomes; an MVR fixture with no resolution is
    /// retained as unresolved rather than dropped, exactly as on the desk, so nothing silently
    /// disappears from an imported rig.
    pub fn import_mvr(
        &self,
        document: light_mvr::MvrDocument,
        resolutions: HashMap<Uuid, MvrImportResolution>,
    ) -> Result<MvrImportOutcome, DocumentError> {
        let definitions = match self.ports().library() {
            Some(library) => library.lock().patchable_definitions()?,
            None => Vec::new(),
        };
        let context = self
            .context()
            .with_request_id(Uuid::new_v4().to_string())
            .with_expected_revision(self.patch_revision()?);
        let command = ApplyActiveMvrImportCommand {
            show_id: self.show_id(),
            document,
            definitions,
            resolutions,
        };
        let service = MvrImportService::new(light_application::ActiveShowService::new(
            light_application::EventBus::default(),
        ));
        let result = service.apply(ActionEnvelope { context, command }, self.ports())?;
        Ok(MvrImportOutcome {
            imported_fixtures: result.imported_fixtures,
            unresolved_fixtures: result.unresolved_fixtures,
            warnings: result.warnings,
        })
    }
}
