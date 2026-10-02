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
pub fn fixture_profile_source_fingerprint(
    profile: &FixtureProfile,
) -> Result<String, serde_json::Error> {
    // Drop the shared archive before serialization, so hashing 300 fixture snapshots does not
    // allocate and copy their potentially large base64 archive 300 times.
    let mut content = profile.clone();
    content.source_gdtf = None;
    let mut document = serde_json::to_value(content)?;
    document
        .as_object_mut()
        .expect("profile serializes as an object")
        .remove("revision");
    document.sort_all_objects();
    let digest = Sha256::digest(serde_json::to_vec(&document)?);
    Ok(format!("v1:sha256:{digest:x}"))
}
