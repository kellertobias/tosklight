//! Building an MVR document from a show's patch.
//!
//! Export is transport-independent and runtime-independent: it needs the patched fixtures, the
//! MVR metadata retained from any earlier import, and a way to read the source GDTF for a profile
//! revision. The desk supplies those from its installation; a planning application supplies them
//! from the show file and the fixture library. Both must produce the same MVR for the same show,
//! so both call this.

use light_core::FixtureId;
use light_fixture::{
    PatchedFixture, PatchedFixtureCompiler, PatchedFixtureProfileReference, PortablePatchError,
    PortablePatchedFixtureRecord, ResolvedFixtureProfileRevision,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    pub embedded_profiles: usize,
    /// Fixtures whose profile has no retained source GDTF. They are referenced, not embedded.
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

/// Builds the MVR document for a show's patched fixtures.
///
/// `fixtures` is `(stored object id, fixture)` in stored order.
pub fn build_mvr_document<S: GdtfSource>(
    fixtures: &[(String, PatchedFixture)],
    metadata: &MvrFixtureMetadata,
    gdtf: &S,
) -> Result<(light_mvr::MvrDocument, MvrExportSummary), S::Error> {
    // The stored association is looked up by the fixture the body names, which is also how the
    // metadata is keyed; an MVR fixture whose key does not parse as a UUID falls back to the
    // fixture's own identity below.
    let by_fixture: HashMap<&str, &str> = metadata
        .iter()
        .filter_map(|(key, body)| Some((body.get("fixture_id")?.as_str()?, key.as_str())))
        .collect();
    let mut document = light_mvr::MvrDocument::default();
    let mut missing = Vec::new();
    let mut embedded = 0;
    let mut tosklight_fixtures = Vec::with_capacity(fixtures.len());
    for (id, fixture) in fixtures {
        let meta = metadata.get(id);
        let spec = meta
            .and_then(|body| body.get("gdtf_spec"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| {
                format!(
                    "{}@{}.gdtf",
                    fixture.definition.manufacturer, fixture.definition.model
                )
            });
        match gdtf.source_gdtf(fixture.definition.id, fixture.definition.revision)? {
            Some(source) => {
                document
                    .files
                    .entry(spec.to_ascii_lowercase())
                    .or_insert(source);
                embedded += 1;
            }
            None => missing.push(format!(
                "{} · {}",
                fixture.definition.manufacturer, fixture.definition.model
            )),
        }
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
                fixture.definition.name.clone()
            } else {
                fixture.name.clone()
            },
            fixture_id: Some(display_fixture_id(id, fixture)),
            gdtf_spec: spec,
            gdtf_mode: fixture.definition.mode.clone(),
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
    let warnings = if missing.is_empty() {
        Vec::new()
    } else {
        vec![
            "Some fixture profiles have no retained source GDTF and are referenced but not embedded"
                .to_owned(),
        ]
    };
    let summary = MvrExportSummary {
        fixtures: document.fixtures.len(),
        scenery: document.geometry.len(),
        embedded_profiles: embedded,
        missing_profiles: missing,
        warnings,
    };
    Ok((document, summary))
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
