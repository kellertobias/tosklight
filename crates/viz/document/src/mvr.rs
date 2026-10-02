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
        compile_export_fixtures, mvr_layers,
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
#[derive(Clone)]
pub struct MvrPreview {
    pub warnings: Vec<String>,
    pub fixtures: Vec<MvrPreviewFixture>,
    /// Scenery objects the archive carries.
    pub scenery: usize,
    /// GDTF spec and mode pairs no profile in this document's library matches.
    pub missing_profiles: Vec<String>,
    /// Fixtures whose address range overlaps something already patched here.
    pub address_conflicts: Vec<String>,
}

/// One fixture in an archive, as this document sees it.
#[derive(Clone)]
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

/// Exact preview inputs. File and library names are never resolved again at Apply.
#[derive(Clone)]
pub struct PreparedMvrImport {
    pub preview: MvrPreview,
    document: light_mvr::MvrDocument,
    definitions: light_application::mvr_import::MvrDefinitions,
    show_id: light_core::ShowId,
    patch_revision: u64,
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

    fn source_gdtf_with_evidence(
        &self,
        profile: FixtureId,
        revision: u32,
    ) -> Result<Option<light_fixture::FixtureGdtfSource>, Self::Error> {
        let Some(library) = self.0 else {
            return Ok(None);
        };
        Ok(library
            .lock()
            .source_gdtf_with_evidence(profile, revision)?)
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
        let layers = mvr_layers(
            store
                .objects("patch_layer")?
                .into_iter()
                .map(|object| (object.id, object.body)),
        );
        let (document, summary) = build_mvr_document(
            &fixtures,
            &metadata,
            layers,
            &LibraryGdtf(self.ports().library()),
            // The hinge the Visualizer and the CAD turn the lamp's body about.
            viz_project::patched_bracket_hinge_millimetres,
        )?;
        let data =
            light_mvr::write(&document).map_err(|error| DocumentError::Mvr(error.to_string()))?;
        Ok(MvrExport { data, summary })
    }

    /// Reads an MVR archive without changing anything, so the operator can see what it contains
    /// and how its fixtures resolve before committing to the import.
    pub fn read_mvr(data: &[u8]) -> Result<light_mvr::MvrDocument, DocumentError> {
        light_mvr::read(data).map_err(|error| DocumentError::Mvr(error.to_string()))
    }

    /// Capture exact source/mode bindings and the destination revision without writes.
    pub fn prepare_mvr_import(
        &self,
        document: light_mvr::MvrDocument,
    ) -> Result<PreparedMvrImport, DocumentError> {
        use light_application::mvr_import::{
            bind_mvr_sources, occupied_patches, primary_footprint,
        };
        let patch_revision = self.patch_revision()?;
        let (profiles, legacy) = match self.ports().library() {
            Some(library) => {
                let library = library.lock();
                (library.profiles()?, library.definitions()?)
            }
            None => (Vec::new(), Vec::new()),
        };
        let mut definitions = bind_mvr_sources(
            &document,
            &profiles,
            &legacy,
            |id, revision| {
                let Some(library) = self.ports().library() else {
                    return Ok(None);
                };
                library.lock().profile(id, revision).map_err(|e| {
                    light_application::ActionError::new(
                        light_application::ActionErrorKind::Invalid,
                        e.to_string(),
                    )
                })
            },
            |profile| {
                let mut attributes = std::collections::BTreeSet::new();
                for channel in profile.modes.iter().flat_map(|m| &m.channels) {
                    for attribute in std::iter::once(&channel.attribute)
                        .chain(channel.functions.iter().map(|f| &f.attribute))
                    {
                        if !light_core::ATTRIBUTE_REGISTRY.iter().any(|d| {
                            d.id == &*attribute.0
                                && !light_core::built_in_attribute_is_retired(d.id)
                        }) {
                            attributes.insert(attribute.0.to_string());
                        }
                    }
                }
                attributes.into_iter().collect()
            },
        )?;
        if !document.geometry.is_empty() {
            definitions.warnings.push(
                "Scenery import is not supported on this surface; fixture source data is retained."
                    .into(),
            );
        }
        let destination = self.store()?.portable_document()?;
        let occupied = occupied_patches(&destination)?;
        let native = light_application::mvr_export::tosklight_mvr_fixture_metadata(&document);
        let owners =
            light_application::mvr_import::mvr_destination_fixture_ids(&destination, &native);
        let mut missing = std::collections::BTreeSet::new();
        let mut conflicts = Vec::new();
        let mut fixtures = Vec::new();
        for fixture in &document.fixtures {
            let definition = definitions.definitions.get(&fixture.uuid);
            if definition.is_none() {
                missing.insert(format!("{} · {}", fixture.gdtf_spec, fixture.gdtf_mode));
            }
            let conflicted = match (fixture.universe, fixture.address, definition) {
                (Some(universe), Some(address), Some(definition)) => {
                    let end =
                        address.saturating_add(primary_footprint(definition).saturating_sub(1));
                    let owner = owners.get(&fixture.uuid).map(|id| id.0.to_string());
                    let overlap = occupied.iter().any(|(other, start, footprint, id)| {
                        owner.as_ref() != Some(id)
                            && *other == universe
                            && *start <= end
                            && start.saturating_add(footprint.saturating_sub(1)) >= address
                    });
                    if overlap {
                        conflicts.push(format!(
                            "{} conflicts at universe {universe} address {address}-{end}",
                            fixture.name
                        ));
                    }
                    overlap
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
        let preview = MvrPreview {
            fixtures,
            scenery: document.geometry.len(),
            missing_profiles: missing.into_iter().collect(),
            address_conflicts: conflicts,
            warnings: definitions.warnings.clone(),
        };
        Ok(PreparedMvrImport {
            preview,
            document,
            definitions,
            show_id: self.show_id(),
            patch_revision,
        })
    }

    pub fn preview_mvr(
        &self,
        document: &light_mvr::MvrDocument,
    ) -> Result<MvrPreview, DocumentError> {
        Ok(self.prepare_mvr_import(document.clone())?.preview)
    }

    /// Apply the exact preview. A changed destination needs a new preview.
    pub fn import_prepared_mvr(
        &self,
        mut prepared: PreparedMvrImport,
        resolutions: HashMap<Uuid, MvrImportResolution>,
    ) -> Result<MvrImportOutcome, DocumentError> {
        use light_application::mvr_import::{
            MvrProfileSlots, mvr_profile_identity, reserve_mvr_profiles,
        };
        if prepared.show_id != self.show_id() || prepared.patch_revision != self.patch_revision()? {
            return Err(DocumentError::Mvr(
                "The destination changed after MVR preview. Preview the archive again.".into(),
            ));
        }
        let ids = prepared
            .definitions
            .definitions
            .values()
            .filter_map(|d| d.profile_id)
            .map(|id| id.0)
            .collect::<std::collections::BTreeSet<_>>();
        let mut slots = MvrProfileSlots::new();
        if let Some(library) = self.ports().library() {
            let library = library.lock();
            for id in ids {
                for revision in library.profile_revisions(FixtureId(id))? {
                    let value = library
                        .profile_revision_document(FixtureId(id), revision)?
                        .ok_or_else(|| {
                            DocumentError::Mvr(
                                "A library revision disappeared during MVR import".into(),
                            )
                        })?;
                    slots
                        .entry((id, u64::from(revision)))
                        .or_default()
                        .insert(mvr_profile_identity(value)?);
                }
            }
        }
        let document = self.store()?.portable_document()?;
        let legacy = document.canonical_legacy_fixture_profile_revisions()?;
        for profile in document
            .fixture_profile_revisions()
            .iter()
            .chain(legacy.iter())
        {
            slots
                .entry((profile.id().profile_id().0, profile.id().revision()))
                .or_default()
                .insert(mvr_profile_identity(profile.profile().clone())?);
        }
        reserve_mvr_profiles(&mut prepared.definitions, &mut slots, &resolutions)?;
        let mut warnings = prepared.definitions.warnings;
        let context = self
            .context()
            .with_request_id(Uuid::new_v4().to_string())
            .with_expected_revision(prepared.patch_revision);
        let command = ApplyActiveMvrImportCommand {
            show_id: self.show_id(),
            document: prepared.document,
            definitions: prepared.definitions.definitions,
            resolutions,
        };
        let service = MvrImportService::new(light_application::ActiveShowService::new(
            light_application::EventBus::default(),
        ));
        let result = service.apply(ActionEnvelope { context, command }, self.ports())?;
        warnings.extend(result.warnings);
        Ok(MvrImportOutcome {
            imported_fixtures: result.imported_fixtures,
            unresolved_fixtures: result.unresolved_fixtures,
            warnings,
        })
    }

    /// Non-interactive callers prepare and apply the same decoded archive in one operation.
    pub fn import_mvr(
        &self,
        document: light_mvr::MvrDocument,
        resolutions: HashMap<Uuid, MvrImportResolution>,
    ) -> Result<MvrImportOutcome, DocumentError> {
        self.import_prepared_mvr(self.prepare_mvr_import(document)?, resolutions)
    }
}
