//! Canonical GDTF import. Native bytes never pass through the legacy flat fixture format.
//!
//! Unsupported executable relationships fail explicitly. Appearance and geometry not yet
//! translated are reported as diagnostics, with the original archive retained by the caller.

mod channels;
mod dmx;
mod functions;
mod optics;
mod xml;

use crate::{FixtureProfile, ProfileError};
use light_core::FixtureId;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, io::Read};
use uuid::Uuid;
use xml::Node;

const MAX_XML_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct GdtfImportDiagnostic {
    pub node: String,
    pub message: String,
}

#[derive(Debug)]
pub struct GdtfImport {
    pub profile: FixtureProfile,
    pub diagnostics: Vec<GdtfImportDiagnostic>,
}

/// Parse an archive without changing any library/show state. Callers retain the original source
/// and present diagnostics before storing the returned profile.
pub fn import_profile(bytes: &[u8]) -> Result<GdtfImport, ProfileError> {
    let mut imported = preview_profile(bytes)?;
    imported.profile.source_gdtf = Some(crate::ProfileGdtfSource::associate(
        &imported.profile,
        bytes,
    )?);
    Ok(imported)
}

/// Convert and validate every supported mode without constructing a retained-source association.
/// Preview callers present diagnostics; storage callers associate the exact final mapped profile
/// and archive atomically. Public import_profile retains its original source-preserving behavior.
pub fn preview_profile(bytes: &[u8]) -> Result<GdtfImport, ProfileError> {
    let xml = archive_xml(bytes)?;
    from_xml(&xml)
}

/// Structural validation is independent of the subset the canonical importer understands.
pub(crate) fn archive_xml(bytes: &[u8]) -> Result<String, ProfileError> {
    if bytes.len() > 64 * 1024 * 1024 {
        return Err(invalid("archive exceeds 64 MiB"));
    }
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(invalid)?;
    if zip.len() > 4096 {
        return Err(invalid("archive contains too many entries"));
    }
    let names = zip
        .file_names()
        .filter(|name| name.eq_ignore_ascii_case("description.xml"))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if names.len() != 1 {
        return Err(invalid("archive requires exactly one root description.xml"));
    }
    let mut entry = zip.by_name(&names[0]).map_err(invalid)?;
    if entry.size() > MAX_XML_BYTES {
        return Err(invalid("description.xml exceeds 16 MiB"));
    }
    let mut xml = String::new();
    entry
        .by_ref()
        .take(MAX_XML_BYTES + 1)
        .read_to_string(&mut xml)
        .map_err(invalid)?;
    if xml.len() as u64 > MAX_XML_BYTES {
        return Err(invalid("description.xml exceeds 16 MiB"));
    }
    let root = xml::parse(&xml)?;
    if root.name != "GDTF" {
        return Err(invalid("source description root must be GDTF"));
    }
    Ok(xml)
}

pub(super) fn invalid(message: impl std::fmt::Display) -> ProfileError {
    ProfileError::Invalid(format!("GDTF import: {message}"))
}

pub(super) fn identity(parent: Uuid, kind: &str, name: &str) -> Uuid {
    // Source node paths are GDTF's reference identity at first import. Once saved, these UUIDs
    // travel with the canonical profile; editing a display label never recalculates them.
    Uuid::new_v5(&parent, format!("{kind}:{name}").as_bytes())
}

pub(super) fn diagnostic(
    output: &mut Vec<GdtfImportDiagnostic>,
    node: impl Into<String>,
    message: impl Into<String>,
) {
    output.push(GdtfImportDiagnostic {
        node: node.into(),
        message: message.into(),
    });
}

/// An explicit vendor alias, not an inferred color name.
pub(crate) fn declares_amber_alias(bytes: &[u8]) -> bool {
    let Ok(xml) = archive_xml(bytes) else {
        return false;
    };
    let Ok(root) = xml::parse(&xml) else {
        return false;
    };
    root.child("FixtureType")
        .and_then(|f| f.child("AttributeDefinitions"))
        .and_then(|d| d.child("Attributes"))
        .is_some_and(|attributes| {
            attributes.children_named("Attribute").any(|a| {
                a.attr("Name") == Some("ColorAdd_RY")
                    && a.attr("Pretty") == Some("Amber")
                    && a.attr("PhysicalUnit") == Some("ColorComponent")
            })
        })
}

pub(crate) fn import_legacy_optical_profile(bytes: &[u8]) -> Result<FixtureProfile, ProfileError> {
    Ok(from_xml_with_optical_scale(&archive_xml(bytes)?, 1.0)?.profile)
}

fn from_xml(xml: &str) -> Result<GdtfImport, ProfileError> {
    from_xml_with_optical_scale(xml, 0.01)
}

fn from_xml_with_optical_scale(xml: &str, optical_scale: f64) -> Result<GdtfImport, ProfileError> {
    let root = xml::parse(xml)?;
    if root.name != "GDTF" || !matches!(root.attr("DataVersion"), Some("1.0" | "1.1" | "1.2")) {
        return Err(invalid("requires a GDTF 1.0, 1.1 or 1.2 document"));
    }
    let fixtures = root.children_named("FixtureType").collect::<Vec<_>>();
    if fixtures.len() != 1 {
        return Err(invalid("requires exactly one FixtureType"));
    }
    let fixture = fixtures[0];
    let fixture_id = Uuid::parse_str(fixture.required("FixtureTypeID")?).map_err(invalid)?;
    if fixture_id.is_nil() {
        return Err(invalid("FixtureTypeID cannot be nil"));
    }
    let attributes = fixture
        .child("AttributeDefinitions")
        .and_then(|node| node.child("Attributes"))
        .map(|node| named_nodes(node, "Attribute"))
        .transpose()?
        .unwrap_or_default();
    for name in attributes.keys() {
        channels::resolve_attribute(name, &attributes)?;
    }
    let mut profile = FixtureProfile::blank();
    profile.id = FixtureId(fixture_id);
    profile.revision = 0;
    profile.manufacturer = fixture.required("Manufacturer")?.to_owned();
    profile.name = fixture.required("Name")?.to_owned();
    profile.short_name = fixture
        .attr("ShortName")
        .unwrap_or(&profile.name)
        .to_owned();
    profile.geometry = Default::default();
    profile.modes.clear();
    profile.notes = format!(
        "Imported GDTF {} / {}. Source physical data is unverified.",
        root.attr("DataVersion").unwrap(),
        fixture_id
    );
    let descriptions = optics::descriptions(fixture, optical_scale);
    optics::beam_optics(&descriptions, &mut profile);
    let mut diagnostics = Vec::new();
    let modes = fixture
        .child("DMXModes")
        .ok_or_else(|| invalid("missing DMXModes"))?;
    let mut mode_names = std::collections::HashSet::new();
    for mode in modes.children_named("DMXMode") {
        let name = mode.required("Name")?;
        if !mode_names.insert(name) {
            return Err(invalid(format!("duplicate DMXMode {name:?}")));
        }
        let mut imported = channels::mode(mode, fixture_id, &attributes, &mut diagnostics)?;
        optics::attach(
            &descriptions,
            mode,
            &mut imported,
            &attributes,
            &mut diagnostics,
        );
        profile.modes.push(imported);
    }
    for (section, message) in [
        (
            "Geometries",
            "Physical geometry and mounting are retained in source only; calibrated positioning is unknown until configured.",
        ),
        (
            "PhysicalDescriptions",
            "Emitters and filters linked from channel functions or beams are imported as nominal, unverified physical colour data; colour spaces, gamuts, DMX profiles, CRIs and unlinked descriptions are retained in source only.",
        ),
        (
            "Wheels",
            "Colour-wheel slots import their linked filters; wheel artwork, slot colours and gobo/prism wheels are retained in source only.",
        ),
        (
            "Models",
            "Source models are not assembled into fixture geometry by this importer.",
        ),
    ] {
        if fixture
            .child(section)
            .is_some_and(|node| !node.children.is_empty())
        {
            diagnostic(&mut diagnostics, section, message);
        }
    }
    for item in &diagnostics {
        profile
            .notes
            .push_str(&format!("\n{}: {}", item.node, item.message));
    }
    profile.validate()?;
    Ok(GdtfImport {
        profile,
        diagnostics,
    })
}

/// Imports a bare `description.xml`, for tests that edit generated XML.
#[cfg(test)]
pub(crate) fn import_xml_for_tests(xml: &str) -> GdtfImport {
    from_xml(xml).unwrap()
}

fn named_nodes<'a>(
    parent: &'a Node,
    kind: &str,
) -> Result<BTreeMap<String, &'a Node>, ProfileError> {
    let mut output = BTreeMap::new();
    for node in &parent.children {
        if node.name != kind {
            continue;
        }
        let name = node.required("Name")?;
        if output.insert(name.to_owned(), node).is_some() {
            return Err(invalid(format!("duplicate {kind} {name:?}")));
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests;
