//! Original GDTF source is portable evidence, never an implicit replacement for edited data.
use super::{FixtureProfile, ProfileError};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub const GDTF_SOURCE_MIME: &str = "application/vnd.gdtf";
const PREFIX: &str = "data:application/vnd.gdtf;base64,";
const MAX_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileGdtfSource {
    pub version: u16,
    /// Shared across clones; package codec replaces this data URL with an archive-relative path.
    pub archive_asset: Arc<str>,
    pub archive_sha256: String,
    /// Association at import/explicit attachment. Edits retain this unchanged; None is unverified.
    pub profile_fingerprint: Option<String>,
}

impl ProfileGdtfSource {
    pub fn associate(profile: &FixtureProfile, bytes: &[u8]) -> Result<Self, ProfileError> {
        crate::gdtf::read::archive_xml(bytes)?;
        let fingerprint = crate::fixture_profile_source_fingerprint(profile)
            .map_err(|error| ProfileError::Invalid(error.to_string()))?;
        Ok(Self {
            version: 1,
            archive_asset: format!("{PREFIX}{}", STANDARD.encode(bytes)).into(),
            archive_sha256: format!("{:x}", Sha256::digest(bytes)),
            profile_fingerprint: Some(fingerprint),
        })
    }

    /// Validate integrity without demanding current-profile equality: stale retained evidence
    /// must remain transferable after an operator edits physical metadata.
    pub fn decoded_archive(&self) -> Result<Vec<u8>, ProfileError> {
        let invalid = |message: &str| ProfileError::Invalid(format!("GDTF source: {message}"));
        if self.version != 1 {
            return Err(invalid("unsupported attachment version"));
        }
        let encoded = self
            .archive_asset
            .strip_prefix(PREFIX)
            .ok_or_else(|| invalid("archive must be a self-contained GDTF data URL"))?;
        if encoded.len() > MAX_BYTES.div_ceil(3) * 4 {
            return Err(invalid("archive exceeds 64 MiB"));
        }
        let data = STANDARD
            .decode(encoded)
            .map_err(|_| invalid("invalid base64 archive"))?;
        if data.len() > MAX_BYTES {
            return Err(invalid("archive exceeds 64 MiB"));
        }
        if self.archive_sha256 != format!("{:x}", Sha256::digest(&data)) {
            return Err(invalid("archive SHA-256 does not match retained bytes"));
        }
        if self
            .profile_fingerprint
            .as_ref()
            .is_some_and(|fingerprint| {
                !fingerprint.strip_prefix("v1:sha256:").is_some_and(|hash| {
                    hash.len() == 64
                        && hash
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                })
            })
        {
            return Err(invalid("invalid profile association fingerprint"));
        }
        crate::gdtf::read::archive_xml(&data)?;
        Ok(data)
    }

    pub fn matches_profile(&self, profile: &FixtureProfile) -> Result<bool, ProfileError> {
        self.decoded_archive()?;
        let current = crate::fixture_profile_source_fingerprint(profile)
            .map_err(|error| ProfileError::Invalid(error.to_string()))?;
        Ok(self.profile_fingerprint.as_deref() == Some(&current))
    }
}
