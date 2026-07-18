use crate::FixtureProfile;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FIXTURE_PACKAGE_EXTENSION: &str = "toskfixture";
pub const FIXTURE_PACKAGE_MIME_TYPE: &str = "application/vnd.tosklight.fixture+zip";
pub const FIXTURE_PACKAGE_FORMAT: &str = "tosklight.fixture";
pub const FIXTURE_PACKAGE_FORMAT_VERSION: u16 = 1;
pub const FIXTURE_PACKAGE_MANIFEST_PATH: &str = "fixture.json";
pub const MAX_FIXTURE_PACKAGE_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_FIXTURE_PACKAGE_EXPANDED_BYTES: u64 = 128 * 1024 * 1024;
pub const MAX_FIXTURE_PACKAGE_ENTRIES: usize = 32;
pub const MAX_FIXTURE_MANIFEST_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_FIXTURE_PHOTOGRAPH_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_FIXTURE_ICON_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_FIXTURE_MODEL_BYTES: usize = 64 * 1024 * 1024;

pub(super) const MAX_PHOTOGRAPH_DIMENSION: u32 = 8_192;
pub(super) const MAX_ICON_DIMENSION: u32 = 2_048;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixturePackageManifest {
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    pub format: String,
    pub format_version: u16,
    pub profile: FixtureProfile,
}

impl FixturePackageManifest {
    pub fn new(profile: FixtureProfile) -> Self {
        Self {
            schema: Some("https://tosklight.app/schemas/fixture-package-v1.json".into()),
            format: FIXTURE_PACKAGE_FORMAT.into(),
            format_version: FIXTURE_PACKAGE_FORMAT_VERSION,
            profile,
        }
    }
}

#[derive(Debug, Error)]
pub enum FixturePackageError {
    #[error("invalid fixture package: {0}")]
    Invalid(String),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Zip(#[from] zip::result::ZipError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Clone, Copy)]
pub(super) enum AssetKind {
    Photograph,
    Icon,
    Model,
}

impl AssetKind {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Photograph => "photograph",
            Self::Icon => "stage icon",
            Self::Model => "3D model",
        }
    }

    pub(super) fn max_bytes(self) -> usize {
        match self {
            Self::Photograph => MAX_FIXTURE_PHOTOGRAPH_BYTES,
            Self::Icon => MAX_FIXTURE_ICON_BYTES,
            Self::Model => MAX_FIXTURE_MODEL_BYTES,
        }
    }
}

pub(super) struct PackageAsset {
    pub(super) path: String,
    pub(super) bytes: Vec<u8>,
}
