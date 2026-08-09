use std::collections::BTreeSet;
use std::path::{Component, Path};

use light_extensions_contract::{
    DeviceActionDeclaration, ExtensionCapability, TelemetryChannelDeclaration,
};
use serde::{Deserialize, Serialize};

use crate::configuration::{valid_local_id, valid_reverse_dns_id, valid_sha256};

pub const EXTENSION_MANIFEST_VERSION: u16 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionManifest {
    pub manifest_version: u16,
    pub id: String,
    pub name: String,
    pub vendor: VendorManifest,
    pub version: String,
    pub description: String,
    pub license: LicenseManifest,
    pub source_url: Option<String>,
    pub protocol: ProtocolRange,
    pub host_api: HostApiRange,
    pub files: Vec<PackageFileManifest>,
    pub artifacts: Vec<ArtifactManifest>,
    pub capabilities: BTreeSet<ExtensionCapability>,
    #[serde(default)]
    pub controls: Vec<ControlDeclaration>,
    #[serde(default)]
    pub telemetry_channels: Vec<TelemetryChannelDeclaration>,
    #[serde(default)]
    pub device_actions: Vec<DeviceActionDeclaration>,
    #[serde(default)]
    pub device_matches: Vec<DeviceMatchManifest>,
    pub transport_metadata: Option<serde_json::Value>,
    #[serde(default)]
    pub feedback_features: Vec<FeedbackFeature>,
    pub configuration_schema_version: u32,
    pub multiplicity: Multiplicity,
    #[serde(default)]
    pub limits: PackageLimits,
    #[serde(default)]
    pub reverse_engineered: bool,
    pub signature: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VendorManifest {
    pub name: String,
    pub url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LicenseManifest {
    pub name: String,
    pub url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProtocolRange {
    pub minimum: u16,
    pub maximum: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostApiRange {
    pub minimum: u16,
    pub maximum: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactManifest {
    pub os: String,
    pub architecture: String,
    pub executable: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageFileManifest {
    pub path: String,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlDeclaration {
    pub id: String,
    pub kind: ControlKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlKind {
    Button,
    AbsoluteFader,
    MotorFader,
    RelativeEncoder,
    AbsoluteEncoder,
    Wheel,
    Lamp,
    RgbLamp,
    EncoderRing,
    TextDisplay,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackFeature {
    Availability,
    Enabled,
    Selected,
    Warning,
    Error,
    Lamp,
    Blink,
    SemanticColor,
    RgbColor,
    MotorValue,
    EncoderRing,
    Text,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceMatchManifest {
    pub transport: String,
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub serial_number: Option<String>,
    pub usage_page: Option<u16>,
    pub usage: Option<u16>,
    pub endpoint_identity: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Multiplicity {
    Single,
    Multiple,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageLimits {
    pub maximum_input_rate_hz: u32,
    pub maximum_telemetry_rate_hz: u32,
}

impl Default for PackageLimits {
    fn default() -> Self {
        Self {
            maximum_input_rate_hz: 500,
            maximum_telemetry_rate_hz: 100,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ManifestError {
    UnsupportedManifestVersion(u16),
    InvalidId,
    MissingField(&'static str),
    InvalidVersionRange(&'static str),
    NoCapabilities,
    DuplicateArtifact(String, String),
    InvalidArtifactPath(String),
    InvalidArtifactDigest(String),
    MissingArtifactFile(String),
    CapabilityDeclarationMismatch(&'static str),
    DuplicateControl(String),
    DuplicateTelemetryChannel(String),
    InvalidDeviceAction(String),
    InvalidDeviceMatch,
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ManifestError {}

pub fn validate_manifest(manifest: &ExtensionManifest) -> Result<(), ManifestError> {
    if manifest.manifest_version != EXTENSION_MANIFEST_VERSION {
        return Err(ManifestError::UnsupportedManifestVersion(
            manifest.manifest_version,
        ));
    }
    if !valid_reverse_dns_id(&manifest.id) {
        return Err(ManifestError::InvalidId);
    }
    for (name, value) in [
        ("name", &manifest.name),
        ("version", &manifest.version),
        ("description", &manifest.description),
        ("vendor.name", &manifest.vendor.name),
        ("license.name", &manifest.license.name),
    ] {
        if value.trim().is_empty() {
            return Err(ManifestError::MissingField(name));
        }
    }
    if manifest.protocol.minimum == 0 || manifest.protocol.minimum > manifest.protocol.maximum {
        return Err(ManifestError::InvalidVersionRange("protocol"));
    }
    if manifest.host_api.minimum == 0 || manifest.host_api.minimum > manifest.host_api.maximum {
        return Err(ManifestError::InvalidVersionRange("host_api"));
    }
    if manifest.capabilities.is_empty() {
        return Err(ManifestError::NoCapabilities);
    }
    if !valid_semver(&manifest.version) {
        return Err(ManifestError::MissingField(
            "version must be semantic versioning",
        ));
    }
    let mut file_paths = BTreeSet::new();
    for file in &manifest.files {
        let path = Path::new(&file.path);
        if path.is_absolute()
            || path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(ManifestError::InvalidArtifactPath(file.path.clone()));
        }
        if !file_paths.insert(file.path.clone()) {
            return Err(ManifestError::InvalidArtifactPath(file.path.clone()));
        }
        if !valid_sha256(&file.sha256) {
            return Err(ManifestError::InvalidArtifactDigest(file.path.clone()));
        }
    }
    let mut platforms = BTreeSet::new();
    for artifact in &manifest.artifacts {
        if !platforms.insert((artifact.os.clone(), artifact.architecture.clone())) {
            return Err(ManifestError::DuplicateArtifact(
                artifact.os.clone(),
                artifact.architecture.clone(),
            ));
        }
        if !file_paths.contains(&artifact.executable) {
            return Err(ManifestError::MissingArtifactFile(
                artifact.executable.clone(),
            ));
        }
    }
    if manifest.artifacts.is_empty() {
        return Err(ManifestError::MissingField("artifacts"));
    }
    if !manifest.controls.is_empty()
        && !manifest
            .capabilities
            .contains(&ExtensionCapability::ControlSurface)
    {
        return Err(ManifestError::CapabilityDeclarationMismatch("controls"));
    }
    if !manifest.telemetry_channels.is_empty()
        && !manifest
            .capabilities
            .contains(&ExtensionCapability::TelemetrySource)
    {
        return Err(ManifestError::CapabilityDeclarationMismatch(
            "telemetry_channels",
        ));
    }
    let mut ids = BTreeSet::new();
    for control in &manifest.controls {
        if !valid_local_id(&control.id) || !ids.insert(control.id.clone()) {
            return Err(ManifestError::DuplicateControl(control.id.clone()));
        }
    }
    ids.clear();
    for channel in &manifest.telemetry_channels {
        if !valid_local_id(&channel.channel_id) || !ids.insert(channel.channel_id.clone()) {
            return Err(ManifestError::DuplicateTelemetryChannel(
                channel.channel_id.clone(),
            ));
        }
    }
    ids.clear();
    for action in &manifest.device_actions {
        if !valid_local_id(&action.action_id)
            || action.label.trim().is_empty()
            || action.required_permission.trim().is_empty()
            || !ids.insert(action.action_id.clone())
        {
            return Err(ManifestError::InvalidDeviceAction(action.action_id.clone()));
        }
    }
    for device in &manifest.device_matches {
        if device.transport.trim().is_empty()
            || (device.vendor_id.is_none()
                && device
                    .endpoint_identity
                    .as_deref()
                    .is_none_or(str::is_empty))
        {
            return Err(ManifestError::InvalidDeviceMatch);
        }
    }
    Ok(())
}

fn valid_semver(value: &str) -> bool {
    let core = value.split_once('+').map_or(value, |(core, _)| core);
    let core = core.split_once('-').map_or(core, |(core, _)| core);
    let segments = core.split('.').collect::<Vec<_>>();
    segments.len() == 3
        && segments.iter().all(|segment| {
            !segment.is_empty()
                && segment.bytes().all(|byte| byte.is_ascii_digit())
                && (*segment == "0" || !segment.starts_with('0'))
        })
}

#[cfg(test)]
pub(crate) fn test_manifest(
    os: &str,
    architecture: &str,
    executable: &str,
    digest: &str,
) -> ExtensionManifest {
    ExtensionManifest {
        manifest_version: 1,
        id: "de.tosklight.example".into(),
        name: "Example".into(),
        vendor: VendorManifest {
            name: "ToskLight".into(),
            url: None,
        },
        version: "1.0.0".into(),
        description: "Test extension".into(),
        license: LicenseManifest {
            name: "MIT".into(),
            url: None,
        },
        source_url: None,
        protocol: ProtocolRange {
            minimum: 1,
            maximum: 1,
        },
        host_api: HostApiRange {
            minimum: 1,
            maximum: 1,
        },
        files: vec![PackageFileManifest {
            path: executable.into(),
            sha256: digest.into(),
        }],
        artifacts: vec![ArtifactManifest {
            os: os.into(),
            architecture: architecture.into(),
            executable: executable.into(),
        }],
        capabilities: BTreeSet::from([ExtensionCapability::ControlSurface]),
        controls: vec![ControlDeclaration {
            id: "go".into(),
            kind: ControlKind::Button,
        }],
        telemetry_channels: vec![],
        device_actions: vec![],
        device_matches: vec![],
        transport_metadata: None,
        feedback_features: vec![FeedbackFeature::Lamp],
        configuration_schema_version: 1,
        multiplicity: Multiplicity::Single,
        limits: PackageLimits::default(),
        reverse_engineered: false,
        signature: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manifest_rejects_path_escape_and_capability_mismatch() {
        let mut manifest = test_manifest("macos", "aarch64", "../escape", &"a".repeat(64));
        assert!(matches!(
            validate_manifest(&manifest),
            Err(ManifestError::InvalidArtifactPath(_))
        ));
        manifest.files[0].path = "bin/extension".into();
        manifest.artifacts[0].executable = "bin/extension".into();
        manifest.capabilities.clear();
        assert_eq!(
            validate_manifest(&manifest),
            Err(ManifestError::NoCapabilities)
        );
    }
}
