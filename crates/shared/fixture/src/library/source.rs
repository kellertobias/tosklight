use crate::FixtureProfile;
use sha2::{Digest, Sha256};

/// An original archive and the complete typed profile it was explicitly attached to.
///
/// The association establishes that the profile is unchanged since attachment, not that the
/// archive's physical data has been independently measured or validated. Older archives have no
/// evidence and must not replace an edited snapshot when exporting.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixtureGdtfSource {
    pub data: Vec<u8>,
    pub profile_fingerprint: Option<String>,
}

/// Versioned identity for source reuse. Excludes the library revision and source attachment itself.
///
/// Object keys are recursively sorted; arrays retain their order, including modes, functions and
/// calibration samples. Every other field of the typed profile participates, including provenance
/// and authored assets. The source association is excluded to avoid a self-referential hash. Callers must fingerprint the actual export snapshot, not a newer library revision.
/// The hash covers the runtime-derived projection, so a stored profile and its patched snapshot
/// (which carries the derived nominal models) fingerprint identically.
pub fn fixture_profile_source_fingerprint(
    profile: &FixtureProfile,
) -> Result<String, serde_json::Error> {
    // Drop the shared archive before serialization, so hashing 300 fixture snapshots does not
    // allocate and copy their potentially large base64 archive 300 times.
    let mut content = profile.clone();
    content.source_gdtf = None;
    // A patched fixture's snapshot carries the runtime-derived nominal models; deriving here too
    // makes the stored profile and its patched snapshot agree. Derivation leaves authored models
    // untouched and is idempotent, so a profile it does not change keeps its earlier fingerprint.
    crate::apply_derived_color_physical(&mut content);
    crate::apply_derived_position_physical(&mut content);
    crate::apply_derived_zoom_physical(&mut content);
    let mut document = serde_json::to_value(content)?;
    document
        .as_object_mut()
        .expect("profile serializes as an object")
        .remove("revision");
    document.sort_all_objects();
    let digest = Sha256::digest(serde_json::to_vec(&document)?);
    Ok(format!("v1:sha256:{digest:x}"))
}
