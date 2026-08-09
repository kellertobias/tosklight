use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const EXTENSIONS_CONFIGURATION_VERSION: u16 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionsConfiguration {
    pub version: u16,
    #[serde(default)]
    pub approved_packages: BTreeMap<String, String>,
    #[serde(default)]
    pub instances: Vec<ExtensionInstanceConfiguration>,
}

impl Default for ExtensionsConfiguration {
    fn default() -> Self {
        Self {
            version: EXTENSIONS_CONFIGURATION_VERSION,
            approved_packages: BTreeMap::new(),
            instances: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionInstanceConfiguration {
    pub id: String,
    pub extension_id: String,
    #[serde(default)]
    pub enabled: bool,
    pub desk_id: Option<String>,
    pub device: Option<DeviceBinding>,
    #[serde(default)]
    pub settings: BTreeMap<String, Value>,
    #[serde(default)]
    pub control_bindings: BTreeMap<String, light_extensions_contract::CanonicalControlIntent>,
    /// Explicit per-instance grants for manifest-declared device actions.
    #[serde(default)]
    pub device_action_permissions: BTreeSet<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceBinding {
    pub identity: String,
    #[serde(default)]
    pub transport: Option<String>,
}

#[derive(Clone, Debug)]
pub struct LoadedExtensionsConfiguration {
    pub path: PathBuf,
    pub configuration: ExtensionsConfiguration,
    pub diagnostic: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExtensionsConfigurationError {
    UnsupportedVersion(u16),
    InvalidPackageId(String),
    InvalidDigest(String),
    InvalidInstanceId(String),
    DuplicateInstanceId(String),
    MissingDeviceIdentity(String),
    InvalidControlBinding(String),
    InvalidDeviceActionPermission(String),
}

impl std::fmt::Display for ExtensionsConfigurationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedVersion(version) => write!(
                f,
                "extensions configuration version {version} is unsupported"
            ),
            Self::InvalidPackageId(id) => write!(f, "invalid extension id `{id}`"),
            Self::InvalidDigest(id) => write!(
                f,
                "approved digest for `{id}` must be 64 lowercase hexadecimal characters"
            ),
            Self::InvalidInstanceId(id) => write!(f, "invalid extension instance id `{id}`"),
            Self::DuplicateInstanceId(id) => write!(f, "duplicate extension instance id `{id}`"),
            Self::MissingDeviceIdentity(id) => write!(
                f,
                "device binding for instance `{id}` has no stable identity"
            ),
            Self::InvalidControlBinding(id) => {
                write!(f, "invalid control binding on instance `{id}`")
            }
            Self::InvalidDeviceActionPermission(id) => {
                write!(f, "invalid device-action permission on instance `{id}`")
            }
        }
    }
}

impl std::error::Error for ExtensionsConfigurationError {}

impl ExtensionsConfiguration {
    pub fn from_json(bytes: &[u8]) -> Result<Self, String> {
        let value: Self = serde_json::from_slice(bytes)
            .map_err(|error| format!("invalid extensions configuration JSON: {error}"))?;
        value.validate().map_err(|error| error.to_string())?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), ExtensionsConfigurationError> {
        if self.version != EXTENSIONS_CONFIGURATION_VERSION {
            return Err(ExtensionsConfigurationError::UnsupportedVersion(
                self.version,
            ));
        }
        for (id, digest) in &self.approved_packages {
            if !valid_reverse_dns_id(id) {
                return Err(ExtensionsConfigurationError::InvalidPackageId(id.clone()));
            }
            if !valid_sha256(digest) {
                return Err(ExtensionsConfigurationError::InvalidDigest(id.clone()));
            }
        }
        let mut ids = BTreeSet::new();
        for instance in &self.instances {
            if !valid_local_id(&instance.id) {
                return Err(ExtensionsConfigurationError::InvalidInstanceId(
                    instance.id.clone(),
                ));
            }
            if !ids.insert(instance.id.clone()) {
                return Err(ExtensionsConfigurationError::DuplicateInstanceId(
                    instance.id.clone(),
                ));
            }
            if !valid_reverse_dns_id(&instance.extension_id) {
                return Err(ExtensionsConfigurationError::InvalidPackageId(
                    instance.extension_id.clone(),
                ));
            }
            if instance
                .device
                .as_ref()
                .is_some_and(|device| device.identity.trim().is_empty())
            {
                return Err(ExtensionsConfigurationError::MissingDeviceIdentity(
                    instance.id.clone(),
                ));
            }
            if instance
                .control_bindings
                .keys()
                .any(|id| !valid_local_id(id))
            {
                return Err(ExtensionsConfigurationError::InvalidControlBinding(
                    instance.id.clone(),
                ));
            }
            if instance
                .device_action_permissions
                .iter()
                .any(|permission| permission.trim().is_empty())
            {
                return Err(ExtensionsConfigurationError::InvalidDeviceActionPermission(
                    instance.id.clone(),
                ));
            }
        }
        Ok(())
    }

    pub fn approved_digest(&self, extension_id: &str) -> Option<&str> {
        self.approved_packages.get(extension_id).map(String::as_str)
    }
}

/// A missing file is a valid empty installation. Malformed or future-version files are preserved
/// byte-for-byte and disable extension launches without preventing server readiness.
pub fn load_extensions_configuration(path: &Path) -> LoadedExtensionsConfiguration {
    match fs::read(path) {
        Ok(bytes) => match ExtensionsConfiguration::from_json(&bytes) {
            Ok(configuration) => LoadedExtensionsConfiguration {
                path: path.to_path_buf(),
                configuration,
                diagnostic: None,
            },
            Err(error) => LoadedExtensionsConfiguration {
                path: path.to_path_buf(),
                configuration: ExtensionsConfiguration::default(),
                diagnostic: Some(error),
            },
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            LoadedExtensionsConfiguration {
                path: path.to_path_buf(),
                configuration: ExtensionsConfiguration::default(),
                diagnostic: None,
            }
        }
        Err(error) => LoadedExtensionsConfiguration {
            path: path.to_path_buf(),
            configuration: ExtensionsConfiguration::default(),
            diagnostic: Some(format!("cannot read extensions configuration: {error}")),
        },
    }
}

pub fn write_extensions_configuration(
    path: &Path,
    configuration: &ExtensionsConfiguration,
) -> Result<(), String> {
    configuration
        .validate()
        .map_err(|error| error.to_string())?;
    let parent = path
        .parent()
        .ok_or_else(|| "extensions configuration path has no parent".to_owned())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temporary = parent.join(format!(".extensions-{}.tmp", Uuid::new_v4()));
    let bytes = serde_json::to_vec_pretty(configuration).map_err(|error| error.to_string())?;
    fs::write(&temporary, bytes).map_err(|error| error.to_string())?;
    fs::rename(&temporary, path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        error.to_string()
    })
}

pub(crate) fn valid_reverse_dns_id(value: &str) -> bool {
    let labels: Vec<_> = value.split('.').collect();
    labels.len() >= 3
        && labels.iter().all(|label| {
            !label.is_empty()
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}

pub(crate) fn valid_local_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 96
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

pub(crate) fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_requires_exact_digest_and_unique_instances() {
        let mut configuration = ExtensionsConfiguration::default();
        configuration
            .approved_packages
            .insert("de.tosklight.example".into(), "a".repeat(64));
        let instance = ExtensionInstanceConfiguration {
            id: "desk-one".into(),
            extension_id: "de.tosklight.example".into(),
            enabled: true,
            desk_id: Some("main".into()),
            device: Some(DeviceBinding {
                identity: "usb:1234:5678:serial".into(),
                transport: Some("hid".into()),
            }),
            settings: BTreeMap::new(),
            control_bindings: BTreeMap::new(),
            device_action_permissions: BTreeSet::new(),
        };
        configuration.instances.push(instance.clone());
        assert_eq!(configuration.validate(), Ok(()));
        configuration.instances.push(instance);
        assert!(matches!(
            configuration.validate(),
            Err(ExtensionsConfigurationError::DuplicateInstanceId(_))
        ));
    }

    #[test]
    fn legacy_or_future_configuration_is_not_guessed() {
        let error =
            ExtensionsConfiguration::from_json(br#"{"version":2}"#).expect_err("unsupported");
        assert!(error.contains("unsupported"));
    }

    #[test]
    fn malformed_configuration_is_preserved_and_atomic_write_replaces_it() {
        let id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            PathBuf::from(".artifacts/tmp/extensions-configuration-tests").join(id.to_string());
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("extensions.json");
        fs::write(&path, b"not json").unwrap();
        let loaded = load_extensions_configuration(&path);
        assert!(loaded.diagnostic.is_some());
        assert_eq!(fs::read(&path).unwrap(), b"not json");
        write_extensions_configuration(&path, &ExtensionsConfiguration::default()).unwrap();
        assert_eq!(load_extensions_configuration(&path).diagnostic, None);
        fs::remove_dir_all(directory).unwrap();
    }
}
