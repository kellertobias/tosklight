//! The versioned Media configuration document.
//!
//! Configuration parses and validates before any subsystem starts. Invalid required
//! configuration prevents startup with an actionable error rather than silently choosing a
//! dangerous or incompatible value.

mod legacy_text;
mod migration;
mod network;
mod output;
mod service;
mod validate;

pub use legacy_text::{
    LEGACY_TEXT_DOCUMENT, LegacyTextError, Migrated as MigratedText, Note as TextMigrationNote,
    migrate as migrate_legacy_text,
};
pub use media_domain::output::MonitorSelector;
pub use migration::{CURRENT_VERSION, MigrationError, migrate_to_current};
pub use network::{
    ART_NET_PORT, CITP_PORT, HTTP_PORT, LOOPBACK, NetworkConfiguration, ResolvedNetwork, SACN_PORT,
};
pub use output::{
    CitpIdentity, DmxProtocol, OutputConfiguration, OutputTarget, Resolution, SoundOutput,
};
pub use service::{
    AudioConfiguration, AudioDeviceSelector, LibraryConfiguration, PlaybackConfiguration,
    TargetCodec,
};
pub use validate::ConfigurationError;

use media_domain::OutputId;
use media_domain::text_catalog::TextCatalog;
use media_domain::visualizer::GeneratedCatalog;
use serde::{Deserialize, Serialize};

/// Distinguishes two Media Server processes on one host in logs, CITP announcements, and
/// bind-conflict diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct InstanceId(String);

impl InstanceId {
    pub fn new(value: impl Into<String>) -> Self {
        let trimmed = value.into().trim().to_owned();
        Self(if trimmed.is_empty() {
            "media".to_owned()
        } else {
            trimmed
        })
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for InstanceId {
    fn default() -> Self {
        Self::new("media")
    }
}

/// The whole configuration of one Media Server process.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MediaConfiguration {
    #[serde(default)]
    pub instance_id: InstanceId,
    #[serde(default)]
    pub network: NetworkConfiguration,
    #[serde(default)]
    pub library: LibraryConfiguration,
    #[serde(default)]
    pub audio: AudioConfiguration,
    #[serde(default)]
    pub playback: PlaybackConfiguration,
    /// Which generated visualizer answers at which address.
    ///
    /// Configuration rather than a constant, because moving a visualizer is an operator decision
    /// their show depends on — a new build must never renumber what a cue already points at.
    #[serde(default)]
    pub visualizers: GeneratedCatalog,
    /// Which text entry answers at which address, and how it is drawn.
    #[serde(default)]
    pub text: TextCatalog,
    /// One or more logical outputs. The first release ships one; the collection is never
    /// collapsed into singleton state.
    pub outputs: Vec<OutputConfiguration>,
}

impl Default for MediaConfiguration {
    fn default() -> Self {
        Self {
            instance_id: InstanceId::default(),
            network: NetworkConfiguration::default(),
            library: LibraryConfiguration::default(),
            audio: AudioConfiguration::default(),
            playback: PlaybackConfiguration::default(),
            visualizers: GeneratedCatalog::default(),
            text: TextCatalog::default(),
            outputs: vec![OutputConfiguration::new("Main")],
        }
    }
}

impl MediaConfiguration {
    pub fn output(&self, id: OutputId) -> Option<&OutputConfiguration> {
        self.outputs.iter().find(|candidate| candidate.id == id)
    }
}

/// A configuration document as it appears on disk: a version and the configuration it describes.
///
/// The version is a sibling of the configuration rather than a field inside it, so both levels
/// can refuse unknown fields instead of quietly absorbing a typo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfigurationDocument {
    pub version: u32,
    pub configuration: MediaConfiguration,
}

impl ConfigurationDocument {
    pub fn current(configuration: MediaConfiguration) -> Self {
        Self {
            version: CURRENT_VERSION,
            configuration,
        }
    }
}

impl Default for ConfigurationDocument {
    fn default() -> Self {
        Self::current(MediaConfiguration::default())
    }
}

/// Reads a configuration document of any supported version, migrates it forward, and validates
/// it. This is the only supported way to turn stored bytes into a `MediaConfiguration`.
pub fn load(serialized: &str) -> Result<MediaConfiguration, ConfigurationError> {
    let value: serde_json::Value =
        serde_json::from_str(serialized).map_err(|error| ConfigurationError::Malformed {
            detail: error.to_string(),
        })?;
    let migrated = migrate_to_current(value)?;
    let document: ConfigurationDocument =
        serde_json::from_value(migrated).map_err(|error| ConfigurationError::Malformed {
            detail: error.to_string(),
        })?;
    validate::validate(&document.configuration)?;
    Ok(document.configuration)
}

/// Serializes the current configuration for storage.
pub fn save(configuration: &MediaConfiguration) -> String {
    serde_json::to_string_pretty(&ConfigurationDocument::current(configuration.clone()))
        .expect("the configuration document is always serializable")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_document_round_trips() {
        let configuration = MediaConfiguration::default();
        let loaded = load(&save(&configuration)).unwrap();
        assert_eq!(loaded, configuration);
    }

    #[test]
    fn the_default_configuration_ships_exactly_one_output() {
        let configuration = MediaConfiguration::default();
        assert_eq!(configuration.outputs.len(), 1);
        assert_eq!(configuration.outputs[0].name.as_str(), "Main");
    }

    #[test]
    fn an_output_is_addressable_by_its_stable_identity() {
        let configuration = MediaConfiguration::default();
        let id = configuration.outputs[0].id;
        assert_eq!(configuration.output(id).map(|output| output.id), Some(id));
        assert!(configuration.output(OutputId::new()).is_none());
    }

    #[test]
    fn malformed_json_reports_an_actionable_error() {
        let error = load("{ not json").unwrap_err();
        assert!(
            matches!(error, ConfigurationError::Malformed { .. }),
            "{error:?}"
        );
    }

    #[test]
    fn instance_ids_never_collapse_to_an_empty_string() {
        assert_eq!(InstanceId::new("   ").as_str(), "media");
        assert_eq!(InstanceId::new(" stage-left ").as_str(), "stage-left");
    }
}
