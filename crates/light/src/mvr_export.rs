//! Building an MVR document from a show's patch.
//!
//! Export is transport-independent and runtime-independent: it needs the patched fixtures, the
//! MVR metadata retained from any earlier import, and a way to read the source GDTF for a profile
//! revision. The desk supplies those from its installation; a planning application supplies them
//! from the show file and the fixture library. Both must produce the same MVR for the same show,
//! so both call this.

use light_core::FixtureId;
use light_fixture::{
    FixtureDefinition, FixtureGdtfSource, FixtureProfile, PatchedFixture, PatchedFixtureCompiler,
    PatchedFixtureProfileReference, PortablePatchError, PortablePatchedFixtureRecord,
    ResolvedFixtureProfileRevision, fixture_profile_source_fingerprint, gdtf,
};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet};

#[path = "mvr_export_sources.rs"]
mod sources;
use uuid::Uuid;

pub const TOSKLIGHT_MVR_FIXTURE_METADATA_PATH: &str = "tosklight/fixture-metadata.json";

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ToskLightMvrFixtureMetadata {
    version: u32,
    fixtures: Vec<ToskLightMvrFixtureMetadataEntry>,
    /// Archive members are shared by hash; descriptors retain their original association.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    gdtf_sources: BTreeMap<String, light_fixture::ProfileGdtfSource>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ToskLightMvrFixtureMetadataEntry {
    mvr_uuid: Uuid,
    fixture: PatchedFixture,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_gdtf_ref: Option<String>,
    /// The hinge the fixture's matrix turns the body about, in fixture-local desk millimetres.
    /// Absent — as in every archive written before hinges were exported — means the matrix turns
    /// the whole lamp about its origin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bracket_hinge_millimetres: Option<[f32; 3]>,
}

/// A fixture ToskLight wrote into an MVR, as its lossless metadata describes it.
#[derive(Clone, Debug)]
pub struct ToskLightMvrFixture {
    pub fixture: PatchedFixture,
    /// The bracket hinge its matrix was written with; see [`crate::mvr_transform`].
    pub bracket_hinge_millimetres: Option<[f32; 3]>,
}

impl ToskLightMvrFixture {
    /// The mount placement `matrix` describes for a fixture that keeps `bracket_degrees`.
    ///
    /// The matrix carries the lamp as it hung when exported, with this entry's bracket and hinge
    /// folded in. Those are taken back out, so the fixture's own location and rotation return, and
    /// the bracket it keeps — its own, or the one already on the desk — turns it from there.
    pub fn placement(
        &self,
        matrix: [f64; 12],
    ) -> (light_fixture::FixtureLocation, light_fixture::FixtureVector) {
        crate::mvr_transform::placement_from_mvr_unbracketed(
            matrix,
            self.fixture.bracket_angle,
            self.bracket_hinge_millimetres,
        )
    }
}

/// The placement an imported fixture gets from its MVR matrix: unfolded from ToskLight's own
/// bracket and hinge when this desk wrote it, read as a whole otherwise.
pub fn mvr_fixture_placement(
    matrix: [f64; 12],
    embedded: Option<&ToskLightMvrFixture>,
) -> (light_fixture::FixtureLocation, light_fixture::FixtureVector) {
    match embedded {
        Some(embedded) => embedded.placement(matrix),
        None => crate::mvr_transform::placement_from_mvr(matrix),
    }
}

/// Returns ToskLight's lossless fixture metadata when an MVR was exported by this desk.
///
/// The manifest is an ancillary archive member, so standards-only MVR consumers can ignore it.
/// Invalid or future manifests are ignored and leave the normal standards-based import intact.
pub fn tosklight_mvr_fixture_metadata(
    document: &light_mvr::MvrDocument,
) -> HashMap<Uuid, ToskLightMvrFixture> {
    let Some(data) = document.files.get(TOSKLIGHT_MVR_FIXTURE_METADATA_PATH) else {
        return HashMap::new();
    };
    let Ok(metadata) = serde_json::from_slice::<ToskLightMvrFixtureMetadata>(data) else {
        return HashMap::new();
    };
    if metadata.version != 1 {
        return HashMap::new();
    }
    let sources = sources::read_sources(document, metadata.gdtf_sources);
    metadata
        .fixtures
        .into_iter()
        .filter_map(|mut entry| {
            if let Some(reference) = entry.source_gdtf_ref {
                // A missing or corrupt retained archive invalidates this native entry. Fall back
                // to the standard MVR description, rather than silently certify different bytes.
                let source = sources.get(&reference)?.clone();
                entry
                    .fixture
                    .definition
                    .profile_snapshot
                    .as_mut()
                    .map(std::sync::Arc::make_mut)?
                    .source_gdtf = Some(source);
            }
            Some((
                entry.mvr_uuid,
                ToskLightMvrFixture {
                    fixture: entry.fixture,
                    bracket_hinge_millimetres: entry.bracket_hinge_millimetres,
                },
            ))
        })
        .collect()
}

/// What an export contains, and what it could not embed.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MvrExportSummary {
    pub fixtures: usize,
    pub scenery: usize,
    /// Fixtures using original archives verified against their actual profile snapshot.
    pub embedded_profiles: usize,
    /// Fixtures described by GDTF generated from their current immutable profile snapshot.
    pub generated_profiles: usize,
    /// Fixtures whose profile could not be embedded either way. They are referenced, not embedded.
    pub missing_profiles: Vec<String>,
    pub warnings: Vec<String>,
}

/// Retained `mvr_fixture` bodies, keyed by the patched fixture id they describe.
///
/// The body carries the GDTF file name the rig arrived with, so exporting a rig that came from an
/// MVR does not rename its profiles.
pub type MvrFixtureMetadata = HashMap<String, serde_json::Value>;

/// Reads the source GDTF retained for one immutable profile revision.
pub trait GdtfSource {
    type Error;

    /// `revision` is the immutable fixture-library revision the patch references.
    fn source_gdtf(
        &self,
        profile: FixtureId,
        revision: u32,
    ) -> Result<Option<Vec<u8>>, Self::Error>;

    /// Bytes and evidence must come from the same source record. Providers without an explicit
    /// association remain unverified; having the same profile id or revision is insufficient.
    fn source_gdtf_with_evidence(
        &self,
        profile: FixtureId,
        revision: u32,
    ) -> Result<Option<FixtureGdtfSource>, Self::Error> {
        Ok(self
            .source_gdtf(profile, revision)?
            .map(|data| FixtureGdtfSource {
                data,
                profile_fingerprint: None,
            }))
    }
}

/// Resolves stored `patched_fixture` objects into fixtures an export can describe.
///
/// A stored patch is a reference to an immutable profile revision, not an inline definition, so
/// the manufacturer, model, mode and footprint an MVR needs only exist once the reference is
/// resolved. Skipping that step silently produces an MVR with no fixtures in it.
pub fn compile_export_fixtures<R>(
    objects: impl IntoIterator<Item = (String, serde_json::Value)>,
    resolve: R,
) -> Result<Vec<(String, PatchedFixture)>, PortablePatchError>
where
    R: Fn(PatchedFixtureProfileReference) -> Option<ResolvedFixtureProfileRevision>,
{
    let mut compiler = PatchedFixtureCompiler::new(resolve);
    objects
        .into_iter()
        .map(|(id, body)| {
            let record = PortablePatchedFixtureRecord::decode(body)?;
            Ok((id, compiler.compile_for_export(&record)?))
        })
        .collect()
}

/// The show's patch layers as MVR layers, in the order the patch sheet lists them.
///
/// `objects` are the stored `patch_layer` objects as `(object id, body)`. Each layer keeps the name
/// the operator gave it, so another application groups the rig the way the show does.
pub fn mvr_layers(
    objects: impl IntoIterator<Item = (String, serde_json::Value)>,
) -> Vec<light_mvr::MvrLayer> {
    let mut layers: Vec<(i64, light_mvr::MvrLayer)> = objects
        .into_iter()
        .map(|(id, body)| {
            let text = |key: &str| {
                body.get(key)
                    .and_then(serde_json::Value::as_str)
                    .map(str::trim)
                    .unwrap_or_default()
                    .to_owned()
            };
            let stored_id = text("id");
            let layer = light_mvr::MvrLayer {
                id: if stored_id.is_empty() { id } else { stored_id },
                name: text("name"),
            };
            let order = body
                .get("order")
                .and_then(serde_json::Value::as_i64)
                .unwrap_or(i64::MAX);
            (order, layer)
        })
        .collect();
    layers.sort_by(|(left_order, left), (right_order, right)| {
        left_order
            .cmp(right_order)
            .then_with(|| left.name.cmp(&right.name))
    });
    layers.into_iter().map(|(_, layer)| layer).collect()
}

/// Each profile mode's id, its own name and its name in a generated GDTF.
type GeneratedModes = Vec<(Uuid, String, String)>;

/// The GDTF file one profile revision is exported as.
struct ExportedType {
    /// The archive member, which is also what every fixture of this revision names as its spec.
    spec: String,
    /// For a generated file: each profile mode's id, its own name and its name in the GDTF.
    /// Absent when a verified original archive keeps the source mode names.
    generated: Option<GeneratedModes>,
}

impl ExportedType {
    fn mode(&self, definition: &FixtureDefinition) -> String {
        let Some(modes) = &self.generated else {
            return definition.mode.clone();
        };
        modes
            .iter()
            .find(|(id, _, _)| Some(*id) == definition.mode_id)
            .or_else(|| modes.iter().find(|(_, name, _)| *name == definition.mode))
            .map_or_else(
                || gdtf::gdtf_name(&definition.mode),
                |(_, _, gdtf)| gdtf.clone(),
            )
    }
}

/// Builds the MVR document for a show's patched fixtures.
///
/// `fixtures` is `(stored object id, fixture)` in stored order, and `layers` the patch layers they
/// belong to, from [`mvr_layers`]. Every fixture's profile revision is
/// described once from its current profile snapshot. Original source bytes may predate profile
/// edits, so they cannot replace current data without a verified source/profile association.
/// Export limitations and missing GDTF files are reported in the existing summary warnings.
///
/// `bracket_hinge` answers where a fixture's body turns in its bracket, in fixture-local desk
/// millimetres — `viz_project::patched_bracket_hinge_millimetres`, the hinge the Visualizer and the
/// CAD turn it about — so the exported lamp's light leaves where it does locally. `None` turns the
/// whole lamp about its origin.
pub fn build_mvr_document<S: GdtfSource>(
    fixtures: &[(String, PatchedFixture)],
    metadata: &MvrFixtureMetadata,
    layers: Vec<light_mvr::MvrLayer>,
    gdtf: &S,
    bracket_hinge: impl Fn(&PatchedFixture) -> Option<[f32; 3]>,
) -> Result<(light_mvr::MvrDocument, MvrExportSummary), S::Error> {
    // The stored association is looked up by the fixture the body names, which is also how the
    // metadata is keyed; an MVR fixture whose key does not parse as a UUID falls back to the
    // fixture's own identity below.
    let by_fixture: HashMap<&str, &str> = metadata
        .iter()
        .filter_map(|(key, body)| Some((body.get("fixture_id")?.as_str()?, key.as_str())))
        .collect();
    let mut document = light_mvr::MvrDocument {
        layers,
        ..Default::default()
    };
    let mut summary = MvrExportSummary::default();
    let mut types: HashMap<(Uuid, u32, String, Option<String>), Option<ExportedType>> =
        HashMap::new();
    let mut archive_names = HashSet::new();
    let mut tosklight_fixtures = Vec::with_capacity(fixtures.len());
    let mut source_archives = sources::SourceArchiveWriter::default();
    for (id, fixture) in fixtures {
        let definition = &fixture.definition;
        let mut native_fixture = fixture.clone();
        let source_gdtf_ref =
            source_archives.detach(&mut native_fixture, &mut document, &mut summary.warnings);
        // The canonical fingerprint intentionally excludes source evidence. Source-only optical
        // data can differ for otherwise identical profiles, so it must also distinguish exports.
        let source_cache_key = source_gdtf_ref.clone().or_else(|| {
            definition.profile_snapshot.as_ref().and_then(|profile| {
                profile
                    .source_gdtf
                    .as_ref()
                    .map(|_| format!("invalid:{id}"))
            })
        });
        let preferred = metadata
            .get(id)
            .and_then(|body| body.get("gdtf_spec"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| {
                let model = profile_name(definition);
                format!("{}@{model}.gdtf", definition.manufacturer)
            });
        // A portable show can carry different snapshots with identical library identifiers
        // (including a selected-mode subset). Never reuse evidence or generated bytes by id alone.
        let prepared = prepare_profile(definition);
        let fingerprint = match &prepared {
            Ok(prepared) => prepared.fingerprint.clone(),
            Err(error) => format!("unavailable:{error}"),
        };
        let key = (
            definition.id.0,
            definition.revision,
            fingerprint,
            source_cache_key,
        );
        if let std::collections::hash_map::Entry::Vacant(slot) = types.entry(key.clone()) {
            slot.insert(export_type(
                definition,
                &prepared,
                &preferred,
                gdtf,
                &mut document,
                &mut archive_names,
                &mut summary.warnings,
            )?);
        }
        // Verified source bytes may already be retained as an ancillary member. Once standard
        // export has written the same bytes at the MVR root, point descriptors there and remove
        // the redundant copy. Generated output cannot take the original source's place.
        if let Some(exported) = &types[&key]
            && exported.generated.is_none()
            && let Some(reference) = &source_gdtf_ref
        {
            source_archives.use_standard_member(reference, &exported.spec, &mut document);
        }
        let (spec, mode) =
            exported_spec_and_mode(types[&key].as_ref(), definition, preferred, &mut summary);
        let uuid = by_fixture
            .get(id.as_str())
            .and_then(|uuid| Uuid::parse_str(uuid).ok())
            .unwrap_or(fixture.fixture_id.0);
        // Only a turned bracket moves the body off its mount; a level one exports as it always did.
        let hinge = (fixture.bracket_angle != 0.0)
            .then(|| bracket_hinge(fixture))
            .flatten();
        tosklight_fixtures.push(ToskLightMvrFixtureMetadataEntry {
            mvr_uuid: uuid,
            fixture: native_fixture,
            source_gdtf_ref,
            bracket_hinge_millimetres: hinge,
        });
        document.fixtures.push(light_mvr::MvrFixture {
            uuid,
            name: if fixture.name.is_empty() {
                definition.name.clone()
            } else {
                fixture.name.clone()
            },
            fixture_id: Some(display_fixture_id(id, fixture)),
            gdtf_spec: spec,
            gdtf_mode: mode,
            universe: fixture.universe,
            address: fixture.address,
            matrix: transform_matrix(fixture, hinge),
            layer: Some(fixture.layer_id.clone()),
            class: None,
        });
    }
    embed_tosklight_metadata(
        &mut document,
        ToskLightMvrFixtureMetadata {
            version: 1,
            fixtures: tosklight_fixtures,
            gdtf_sources: source_archives.descriptors,
        },
    );
    add_profile_warnings(&mut summary);
    summary.fixtures = document.fixtures.len();
    summary.scenery = document.geometry.len();
    Ok((document, summary))
}

/// The GDTF spec and mode a fixture references, counting how its profile was exported.
fn exported_spec_and_mode(
    exported: Option<&ExportedType>,
    definition: &FixtureDefinition,
    preferred: String,
    summary: &mut MvrExportSummary,
) -> (String, String) {
    match exported {
        Some(exported) => {
            if exported.generated.is_some() {
                summary.generated_profiles += 1;
            } else {
                summary.embedded_profiles += 1;
            }
            (exported.spec.clone(), exported.mode(definition))
        }
        None => {
            summary.missing_profiles.push(format!(
                "{} · {}",
                definition.manufacturer,
                profile_name(definition)
            ));
            (preferred, definition.mode.clone())
        }
    }
}

/// Stores ToskLight's own fixture metadata beside the standard MVR content.
fn embed_tosklight_metadata(
    document: &mut light_mvr::MvrDocument,
    metadata: ToskLightMvrFixtureMetadata,
) {
    if let Ok(data) = serde_json::to_vec(&metadata) {
        document
            .files
            .insert(TOSKLIGHT_MVR_FIXTURE_METADATA_PATH.into(), data);
    }
}

/// Explains generated and missing GDTF profiles to the operator.
fn add_profile_warnings(summary: &mut MvrExportSummary) {
    if summary.generated_profiles > 0 {
        summary.warnings.push(
            "ToskLight generated GDTF files from the current fixture profiles with their modes, \
             channels, supported physical functions and colour emitters, filters and wheels, but \
             without native calibration provenance, Position geometry, Zoom conventions or \
             detailed 3D models"
                .to_owned(),
        );
    }
    if !summary.missing_profiles.is_empty() {
        summary.warnings.push(
            "Some fixture profiles could not be described as GDTF and are referenced but not \
             embedded"
                .to_owned(),
        );
    }
}

fn profile_name(definition: &FixtureDefinition) -> &str {
    match definition.model.trim() {
        "" => definition.name.trim(),
        model => model,
    }
}

/// Embeds the GDTF for one profile revision, or returns `None` when there is none to embed.
fn export_type<S: GdtfSource>(
    definition: &FixtureDefinition,
    prepared: &Result<PreparedProfile<'_>, light_fixture::ProfileError>,
    preferred: &str,
    gdtf: &S,
    document: &mut light_mvr::MvrDocument,
    archive_names: &mut HashSet<String>,
    warnings: &mut Vec<String>,
) -> Result<Option<ExportedType>, S::Error> {
    let label = format!(
        "{} · {} (revision {})",
        definition.manufacturer,
        profile_name(definition),
        definition.revision
    );
    let embedded = prepared
        .as_ref()
        .ok()
        .and_then(|prepared| prepared.profile.source_gdtf.as_ref());
    let source = if let Some(embedded) = embedded {
        match embedded.decoded_archive() {
            Ok(data) => Some(FixtureGdtfSource {
                data,
                profile_fingerprint: embedded.profile_fingerprint.clone(),
            }),
            Err(error) => {
                warnings.push(format!(
                    "{label}: embedded GDTF source was not used: {error}"
                ));
                None
            }
        }
    } else {
        gdtf.source_gdtf_with_evidence(definition.id, definition.revision)?
    };
    if let Some(source) = source {
        let matches = prepared.as_ref().is_ok_and(|prepared| {
            source.profile_fingerprint.as_ref() == Some(&prepared.fingerprint)
        });
        if matches {
            let existing = document
                .files
                .iter()
                .filter(|(path, data)| {
                    !path.contains('/')
                        && path.to_ascii_lowercase().ends_with(".gdtf")
                        && **data == source.data
                })
                .map(|(path, _)| path.clone())
                .min();
            let spec = existing
                .unwrap_or_else(|| archive_name(preferred, definition.revision, archive_names));
            document.files.entry(spec.clone()).or_insert(source.data);
            return Ok(Some(ExportedType {
                spec,
                generated: None,
            }));
        }
        let reason = if source.profile_fingerprint.is_some() {
            "its associated profile does not match the actual export snapshot"
        } else {
            "it is not verified against the actual export snapshot"
        };
        warnings.push(format!(
            "{label}: the retained source GDTF was not used because {reason}. \
             The original source remains retained with its original evidence."
        ));
    }
    let generated = match prepared {
        Ok(prepared) => generated_gdtf(&prepared.profile).map_err(|error| error.to_string()),
        Err(error) => Err(error.to_string()),
    };
    let (data, modes) = match generated {
        Ok(generated) => generated,
        Err(error) => {
            warnings.push(format!(
                "{label}: GDTF could not be generated: {error}. No GDTF is embedded for this \
                 profile; other applications may not load these fixtures. Export its native \
                 .toskfixture package to preserve the complete profile."
            ));
            return Ok(None);
        }
    };
    let generated = Some(modes);
    let spec = archive_name(preferred, definition.revision, archive_names);
    document.files.insert(spec.clone(), data);
    Ok(Some(ExportedType { spec, generated }))
}

struct PreparedProfile<'a> {
    profile: Cow<'a, FixtureProfile>,
    fingerprint: String,
}

fn prepare_profile(
    definition: &FixtureDefinition,
) -> Result<PreparedProfile<'_>, light_fixture::ProfileError> {
    let profile = match definition.profile_snapshot.as_deref() {
        Some(profile) => Cow::Borrowed(profile),
        None => Cow::Owned(FixtureProfile::from_flat_modes(std::slice::from_ref(
            definition,
        ))?),
    };
    let fingerprint = fixture_profile_source_fingerprint(&profile).map_err(|error| {
        light_fixture::ProfileError::Invalid(format!("source fingerprint: {error}"))
    })?;
    Ok(PreparedProfile {
        profile,
        fingerprint,
    })
}

/// A GDTF describing the exact export snapshot, with its modes' GDTF names.
fn generated_gdtf(
    profile: &FixtureProfile,
) -> Result<(Vec<u8>, GeneratedModes), light_fixture::ProfileError> {
    let data = gdtf::profile::package_profile(profile)?;
    let modes = profile
        .modes
        .iter()
        .zip(gdtf::profile::mode_names(profile))
        .map(|(mode, gdtf)| (mode.id, mode.name.clone(), gdtf))
        .collect();
    Ok((data, modes))
}

/// A GDTF file name at the archive root that no other member shares, even by case.
///
/// MVR requires every GDTF at the root and forbids names that differ only by case. Two revisions
/// of one fixture share a name, so the later one is told apart by its revision.
fn archive_name(preferred: &str, revision: u32, used: &mut HashSet<String>) -> String {
    let file = preferred
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(preferred)
        .trim();
    let stem = if file.to_ascii_lowercase().ends_with(".gdtf") {
        &file[..file.len() - ".gdtf".len()]
    } else {
        file
    };
    let stem: String = stem
        .chars()
        .map(|character| match character {
            ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            character if character.is_control() => '_',
            character => character,
        })
        .collect();
    let stem = match stem.trim() {
        "" => "Fixture",
        stem => stem,
    };
    let mut candidate = format!("{stem}.gdtf");
    let mut attempt = 1;
    while !used.insert(candidate.to_ascii_lowercase()) {
        candidate = if attempt == 1 {
            format!("{stem} r{revision}.gdtf")
        } else {
            format!("{stem} r{revision}-{attempt}.gdtf")
        };
        attempt += 1;
    }
    candidate
}

fn display_fixture_id(stored_id: &str, fixture: &PatchedFixture) -> String {
    if let Some(number) = fixture.virtual_fixture_number {
        format!("0.{number}")
    } else if let Some(number) = fixture.fixture_number {
        number.to_string()
    } else {
        stored_id.to_owned()
    }
}

/// The fixture's rotation and location as an MVR transform matrix.
///
/// The bracket angle is part of where the fixture actually points, so it is composed into the
/// matrix — after the placement rotation, in the fixture's own frame, about the lamp's bracket
/// hinge where it has one, exactly as the Stage, the CAD and the visualizer turn it. The matrix is
/// then the turned body's frame, so the lens and the beam leave where they do locally. Another application opening this archive gets the rig as it hangs, not as it
/// would hang with every clamp set level. MVR has no separate place to put it, and a rotation nobody
/// exported is a rotation the other application will never draw. The convention itself is stated
/// once, in [`crate::mvr_transform`].
fn transform_matrix(fixture: &PatchedFixture, hinge: Option<[f32; 3]>) -> [f64; 12] {
    crate::mvr_transform::mvr_matrix_hinged(
        fixture.location,
        fixture.rotation,
        fixture.bracket_angle,
        hinge,
    )
}

#[cfg(test)]
#[path = "mvr_export_tests.rs"]
mod tests;
