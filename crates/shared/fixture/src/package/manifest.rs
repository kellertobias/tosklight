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
/// A scan script is source text that has to be compiled every time a laser is loaded, and a
/// quarter of a megabyte is already far more than a scan engine needs. The limit is here to keep
/// an accidental bundle out of a fixture package rather than to constrain honest authoring.
pub const MAX_FIXTURE_SCAN_SCRIPT_BYTES: usize = 256 * 1024;
pub const MAX_FIXTURE_EFFECT_SCRIPT_BYTES: usize = 256 * 1024;
/// A gobo is a mask, and a wheel of them travels inside every patched show that uses the fixture.
/// One megabyte is a generous greyscale image and mean enough to keep a photograph out.
pub const MAX_FIXTURE_GOBO_BYTES: usize = 1024 * 1024;
pub const MAX_FIXTURE_PROJECTION_BYTES: usize = 2 * 1024 * 1024;

pub(super) const MAX_PHOTOGRAPH_DIMENSION: u32 = 8_192;
pub(super) const MAX_ICON_DIMENSION: u32 = 2_048;
/// The gate is a disc a few hundred pixels across on screen at most, and every slot is resampled
/// to one size before it reaches the GPU anyway.
pub(super) const MAX_GOBO_DIMENSION: u32 = 2_048;

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
    /// The JavaScript scan engine a laser fixture projects with.
    ScanScript,
    /// The JavaScript engine an Effect fixture uses to project its DMX programs.
    EffectScript,
    /// One slot's artwork on a gobo wheel.
    Gobo,
    /// Safe, package-owned vector artwork for one named orthographic view.
    Projection,
}

impl AssetKind {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Photograph => "photograph",
            Self::Icon => "stage icon",
            Self::Model => "3D model",
            Self::ScanScript => "scan script",
            Self::EffectScript => "effect script",
            Self::Gobo => "gobo artwork",
            Self::Projection => "SVG projection",
        }
    }

    pub(super) fn max_bytes(self) -> usize {
        match self {
            Self::Photograph => MAX_FIXTURE_PHOTOGRAPH_BYTES,
            Self::Icon => MAX_FIXTURE_ICON_BYTES,
            Self::Model => MAX_FIXTURE_MODEL_BYTES,
            Self::ScanScript => MAX_FIXTURE_SCAN_SCRIPT_BYTES,
            Self::EffectScript => MAX_FIXTURE_EFFECT_SCRIPT_BYTES,
            Self::Gobo => MAX_FIXTURE_GOBO_BYTES,
            Self::Projection => MAX_FIXTURE_PROJECTION_BYTES,
        }
    }
}

pub(super) struct PackageAsset {
    pub(super) path: String,
    pub(super) bytes: Vec<u8>,
}
