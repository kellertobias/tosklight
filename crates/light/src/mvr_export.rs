//! Building an MVR document from a show's patch.
//!
//! Export is transport-independent and runtime-independent: it needs the patched fixtures, the
//! MVR metadata retained from any earlier import, and a way to read the source GDTF for a profile
//! revision. The desk supplies those from its installation; a planning application supplies them
//! from the show file and the fixture library. Both must produce the same MVR for the same show,
//! so both call this.

use light_core::FixtureId;
use light_fixture::{
    FixtureDefinition, FixtureProfile, PatchedFixture, PatchedFixtureCompiler,
    PatchedFixtureProfileReference, PortablePatchError, PortablePatchedFixtureRecord,
    ResolvedFixtureProfileRevision, gdtf,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

pub const TOSKLIGHT_MVR_FIXTURE_METADATA_PATH: &str = "tosklight/fixture-metadata.json";

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ToskLightMvrFixtureMetadata {
    version: u32,
    fixtures: Vec<ToskLightMvrFixtureMetadataEntry>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ToskLightMvrFixtureMetadataEntry {
    mvr_uuid: Uuid,
    fixture: PatchedFixture,
}

/// Returns ToskLight's lossless fixture metadata when an MVR was exported by this desk.
///
/// The manifest is an ancillary archive member, so standards-only MVR consumers can ignore it.
/// Invalid or future manifests are ignored and leave the normal standards-based import intact.
pub fn tosklight_mvr_fixture_metadata(
    document: &light_mvr::MvrDocument,
) -> HashMap<Uuid, PatchedFixture> {
    let Some(data) = document.files.get(TOSKLIGHT_MVR_FIXTURE_METADATA_PATH) else {
        return HashMap::new();
    };
    let Ok(metadata) = serde_json::from_slice::<ToskLightMvrFixtureMetadata>(data) else {
        return HashMap::new();
    };
    if metadata.version != 1 {
        return HashMap::new();
    }
    metadata
        .fixtures
        .into_iter()
        .map(|entry| (entry.mvr_uuid, entry.fixture))
        .collect()
}

/// What an export contains, and what it could not embed.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MvrExportSummary {
    pub fixtures: usize,
    pub scenery: usize,
    /// Fixtures whose profile's retained source GDTF is embedded unchanged.
    pub embedded_profiles: usize,
    /// Fixtures whose profile has no retained source GDTF, described by a GDTF generated from the
    /// profile instead.
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
            Ok((id, compiler.compile(&record)?))
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
    /// `None` for a retained source, whose mode names are the ones the profile was imported with.
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
/// embedded once: as its retained source GDTF where one exists, otherwise as a GDTF generated from
/// the profile, because an application opening the archive refuses a fixture whose GDTF is absent.
pub fn build_mvr_document<S: GdtfSource>(
    fixtures: &[(String, PatchedFixture)],
    metadata: &MvrFixtureMetadata,
    layers: Vec<light_mvr::MvrLayer>,
    gdtf: &S,
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
    let mut types: HashMap<(Uuid, u32), Option<ExportedType>> = HashMap::new();
    let mut archive_names = HashSet::new();
    let mut tosklight_fixtures = Vec::with_capacity(fixtures.len());
    for (id, fixture) in fixtures {
        let definition = &fixture.definition;
        let preferred = metadata
            .get(id)
            .and_then(|body| body.get("gdtf_spec"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| {
                let model = match definition.model.trim() {
                    "" => definition.name.trim(),
                    model => model,
                };
                format!("{}@{model}.gdtf", definition.manufacturer)
            });
        let key = (definition.id.0, definition.revision);
        if let std::collections::hash_map::Entry::Vacant(slot) = types.entry(key) {
            slot.insert(export_type(
                definition,
                &preferred,
                gdtf,
                &mut document,
                &mut archive_names,
            )?);
        }
        let (spec, mode) = match &types[&key] {
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
                    definition.manufacturer, definition.model
                ));
                (preferred, definition.mode.clone())
            }
        };
        let uuid = by_fixture
            .get(id.as_str())
            .and_then(|uuid| Uuid::parse_str(uuid).ok())
            .unwrap_or(fixture.fixture_id.0);
        tosklight_fixtures.push(ToskLightMvrFixtureMetadataEntry {
            mvr_uuid: uuid,
            fixture: fixture.clone(),
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
            matrix: transform_matrix(fixture),
            layer: Some(fixture.layer_id.clone()),
            class: None,
        });
    }
    let metadata = ToskLightMvrFixtureMetadata {
        version: 1,
        fixtures: tosklight_fixtures,
    };
    if let Ok(data) = serde_json::to_vec(&metadata) {
        document
            .files
            .insert(TOSKLIGHT_MVR_FIXTURE_METADATA_PATH.into(), data);
    }
    if summary.generated_profiles > 0 {
        summary.warnings.push(
            "Some fixture profiles have no retained source GDTF; ToskLight generated GDTF files \
             for them with their modes and channels, but without wheels, emitters or 3D models"
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
    summary.fixtures = document.fixtures.len();
    summary.scenery = document.geometry.len();
    Ok((document, summary))
}

/// Embeds the GDTF for one profile revision, or returns `None` when there is none to embed.
fn export_type<S: GdtfSource>(
    definition: &FixtureDefinition,
    preferred: &str,
    gdtf: &S,
    document: &mut light_mvr::MvrDocument,
    archive_names: &mut HashSet<String>,
) -> Result<Option<ExportedType>, S::Error> {
    let (data, generated) = match gdtf.source_gdtf(definition.id, definition.revision)? {
        Some(source) => (source, None),
        None => match generated_gdtf(definition) {
            Some((data, modes)) => (data, Some(modes)),
            None => return Ok(None),
        },
    };
    let spec = archive_name(preferred, definition.revision, archive_names);
    document.files.insert(spec.clone(), data);
    Ok(Some(ExportedType { spec, generated }))
}

/// A GDTF describing the profile the fixture was patched from, with its modes' GDTF names.
fn generated_gdtf(definition: &FixtureDefinition) -> Option<(Vec<u8>, GeneratedModes)> {
    let profile = match definition.profile_snapshot.as_deref() {
        Some(profile) => profile.clone(),
        None => FixtureProfile::from_flat_modes(std::slice::from_ref(definition)).ok()?,
    };
    let data = gdtf::profile::package_profile(&profile).ok()?;
    let modes = profile
        .modes
        .iter()
        .zip(gdtf::profile::mode_names(&profile))
        .map(|(mode, gdtf)| (mode.id, mode.name.clone(), gdtf))
        .collect();
    Some((data, modes))
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
/// matrix: another application opening this archive gets the rig as it hangs, not as it would
/// hang with every clamp set level. MVR has no separate place to put it, and a rotation nobody
/// exported is a rotation the other application will never draw.
fn transform_matrix(fixture: &PatchedFixture) -> [f64; 12] {
    let rx = f64::from(fixture.rotation.x + fixture.bracket_angle).to_radians();
    let ry = f64::from(fixture.rotation.y).to_radians();
    let rz = f64::from(fixture.rotation.z).to_radians();
    let (sx, cx) = rx.sin_cos();
    let (sy, cy) = ry.sin_cos();
    let (sz, cz) = rz.sin_cos();
    [
        cy * cz,
        cz * sx * sy - cx * sz,
        sx * sz + cx * cz * sy,
        cy * sz,
        cx * cz + sx * sy * sz,
        cx * sy * sz - cz * sx,
        -sy,
        cy * sx,
        cx * cy,
        f64::from(fixture.location.x),
        f64::from(fixture.location.y),
        f64::from(fixture.location.z),
    ]
}

#[cfg(test)]
#[path = "mvr_export_tests.rs"]
mod tests;
