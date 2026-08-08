//! Process-wide library and audio settings.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// The codec assets are normalized to at import.
///
/// Bounce playback normalizes at import rather than requiring every codec to reverse, so this
/// choice governs both the transcode target and which variant playback prefers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TargetCodec {
    /// MP4 / libx264 / AAC, fast-decode with a 30-frame GOP.
    #[default]
    H264,
    /// MOV / ProRes Proxy / PCM.
    ProRes,
}

/// Where the media library lives and how imports normalize into it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LibraryConfiguration {
    /// The library root. Folders `000`–`255` and the dotted metadata files live under it.
    #[serde(default = "default_library_root")]
    pub root: PathBuf,
    #[serde(default)]
    pub target_codec: TargetCodec,
}

fn default_library_root() -> PathBuf {
    PathBuf::from("media")
}

impl Default for LibraryConfiguration {
    fn default() -> Self {
        Self {
            root: default_library_root(),
            target_codec: TargetCodec::default(),
        }
    }
}

/// How the operator picked an audio input.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "by", content = "value")]
pub enum AudioDeviceSelector {
    /// The platform default input.
    #[default]
    SystemDefault,
    /// The legacy application's device index. Kept for migrated configuration.
    Index(i32),
    Name(String),
}

/// Audio capture and analysis tuning. Analysis itself is platform-independent: the band
/// definitions, smoothing, thresholds, and timing are identical on macOS, Windows, and Linux.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AudioConfiguration {
    #[serde(default)]
    pub device: AudioDeviceSelector,
    /// Input gain. Applied through a nonlinear curve so low settings stay precise.
    #[serde(default = "unit_gain")]
    pub input_gain: f32,
    /// Scales the dynamic beat threshold.
    #[serde(default = "unit_gain")]
    pub beat_sensitivity: f32,
    #[serde(default = "unit_gain")]
    pub eq_bass: f32,
    #[serde(default = "unit_gain")]
    pub eq_mid: f32,
    #[serde(default = "unit_gain")]
    pub eq_treble: f32,
}

const fn unit_gain() -> f32 {
    1.0
}

impl Default for AudioConfiguration {
    fn default() -> Self {
        Self {
            device: AudioDeviceSelector::default(),
            input_gain: unit_gain(),
            beat_sensitivity: unit_gain(),
            eq_bass: unit_gain(),
            eq_mid: unit_gain(),
            eq_treble: unit_gain(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_defaults_match_the_legacy_layout_and_codec() {
        let library = LibraryConfiguration::default();
        assert_eq!(library.root, PathBuf::from("media"));
        assert_eq!(library.target_codec, TargetCodec::H264);
    }

    #[test]
    fn audio_defaults_are_neutral() {
        let audio = AudioConfiguration::default();
        assert_eq!(audio.device, AudioDeviceSelector::SystemDefault);
        assert_eq!(audio.input_gain, 1.0);
        assert_eq!(audio.eq_treble, 1.0);
    }
}
