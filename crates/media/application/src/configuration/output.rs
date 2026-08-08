//! Per-output configuration.
//!
//! The first release ships one output, but the collection model is here from the start so
//! adding output two never means replacing singleton state.

use media_domain::{
    LayerPersonality, OutputId, OutputName, PersonalityVersion, PresentationMode, TempoSource,
};
use serde::{Deserialize, Serialize};

/// Which DMX protocol feeds this output. Both translate into identical domain commands; the
/// selection only says which ingress routes to this output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DmxProtocol {
    #[default]
    ArtNet,
    Sacn,
}

/// How the operator picked a monitor.
///
/// The legacy application stored a plain index. A name survives replugging and reordering, so
/// new configuration prefers it while migrated configuration keeps the index it had.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "by", content = "value")]
pub enum MonitorSelector {
    Index(u32),
    Name(String),
}

/// Where an output presents.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum OutputTarget {
    /// A monitor. The legacy application stored a target monitor but never applied it at window
    /// creation; the rebuild completes that on all supported systems.
    Monitor {
        monitor: MonitorSelector,
        fullscreen: bool,
    },
    /// No window. Serves streaming, preview-only, and headless test use.
    #[default]
    OffScreen,
}

/// The output's pixel dimensions. For a fullscreen monitor target this is the render resolution
/// the compositor works at, which need not equal the monitor's own mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

impl Default for Resolution {
    fn default() -> Self {
        // The certified first-release capacity is one 1080p output.
        Self {
            width: 1920,
            height: 1080,
        }
    }
}

/// How this output identifies itself to CITP peers.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CitpIdentity {
    /// The CITP layer number a desk sees for this output's first layer.
    #[serde(default)]
    pub layer_base: u8,
    /// The name the output publishes as a CITP video source.
    #[serde(default)]
    pub source_name: Option<String>,
}

/// One logical output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OutputConfiguration {
    pub id: OutputId,
    pub name: OutputName,
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
    #[serde(default)]
    pub target: OutputTarget,
    #[serde(default)]
    pub resolution: Resolution,
    #[serde(default)]
    pub presentation: PresentationMode,
    #[serde(default)]
    pub personality: LayerPersonality,
    #[serde(default)]
    pub personality_version: PersonalityVersion,
    #[serde(default)]
    pub protocol: DmxProtocol,
    #[serde(default)]
    pub universe: u16,
    #[serde(default = "first_start_address")]
    pub start_address: u16,
    #[serde(default)]
    pub citp: CitpIdentity,
    #[serde(default)]
    pub tempo_source: TempoSource,
    /// The diagnostic overlay the legacy application drew when no DMX source was active.
    #[serde(default = "enabled_by_default")]
    pub status_overlay: bool,
}

const fn enabled_by_default() -> bool {
    true
}

const fn first_start_address() -> u16 {
    1
}

impl OutputConfiguration {
    /// A new output with the shipped defaults: one enabled off-screen 1080p eight-layer output
    /// at DMX address 1 following its own Playback BPM channels.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: OutputId::new(),
            name: OutputName::new(name),
            enabled: enabled_by_default(),
            target: OutputTarget::default(),
            resolution: Resolution::default(),
            presentation: PresentationMode::default(),
            personality: LayerPersonality::default(),
            personality_version: PersonalityVersion::default(),
            protocol: DmxProtocol::default(),
            universe: 0,
            start_address: first_start_address(),
            citp: CitpIdentity::default(),
            tempo_source: TempoSource::default(),
            status_overlay: enabled_by_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_output_ships_the_certified_defaults() {
        let output = OutputConfiguration::new("Main");
        assert_eq!(output.name.as_str(), "Main");
        assert!(output.enabled);
        assert_eq!(
            output.resolution,
            Resolution {
                width: 1920,
                height: 1080
            }
        );
        assert_eq!(output.presentation, PresentationMode::DisplaySynchronized);
        assert_eq!(output.personality, LayerPersonality::EightLayers);
        assert_eq!(output.personality_version, PersonalityVersion::V2);
        assert_eq!(output.start_address, 1);
    }

    #[test]
    fn outputs_round_trip_through_json() {
        let output = OutputConfiguration::new("Main");
        let encoded = serde_json::to_string(&output).unwrap();
        let decoded: OutputConfiguration = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, output);
    }

    #[test]
    fn unknown_output_fields_are_rejected_rather_than_ignored() {
        let mut value = serde_json::to_value(OutputConfiguration::new("Main")).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("paused".into(), serde_json::Value::Bool(true));
        let error = serde_json::from_value::<OutputConfiguration>(value).unwrap_err();
        assert!(error.to_string().contains("paused"), "{error}");
    }
}
