//! Audio: what is being heard, and how the analysis is tuned.
//!
//! Two different things travel here, and they travel by different routes for a reason. The
//! analysis is volatile — it changes many times a second and is *pushed* over the telemetry
//! socket. The tuning is stored configuration, read as a snapshot and changed by an edit that
//! carries a request id.

use media_application::configuration::{AudioConfiguration, AudioDeviceSelector};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::diagnostics::AudioTelemetry;

/// The three bands an operator mixes, and what they are reading right now.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct AudioBandsView {
    pub bass: f32,
    pub mid: f32,
    pub treble: f32,
}

/// The window a meter draws.
///
/// Downsampled for display and explicitly not for measurement, which is why it is named for what
/// it is rather than offered as "the samples".
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct WaveformView {
    pub points: Vec<f32>,
}

/// One instant of analysis, as the API reports it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct AudioView {
    /// Whether an input device is open at all. A flat meter on a capturing device means a quiet
    /// room; a flat meter on a closed one means something to fix.
    pub capturing: bool,
    pub device: String,
    pub detail: Option<String>,
    pub waveform: WaveformView,
    pub spectrum: Vec<f32>,
    pub bands: AudioBandsView,
    /// Root-mean-square of the window.
    pub energy: f32,
    /// The largest absolute sample in the window, which is what tells an operator they are clipping.
    pub peak: f32,
    /// `1.0` on the pass a beat landed, falling afterwards.
    pub beat: f32,
    /// Zero until enough beats have been seen to mean anything.
    pub bpm: f32,
    pub beat_phase: f32,
}

impl AudioView {
    pub fn of(telemetry: &AudioTelemetry) -> Self {
        Self {
            capturing: telemetry.capturing,
            device: telemetry.device.clone(),
            detail: telemetry.detail.clone(),
            waveform: WaveformView {
                points: telemetry.waveform.clone(),
            },
            spectrum: telemetry.spectrum.clone(),
            bands: AudioBandsView {
                bass: telemetry.bass,
                mid: telemetry.mid,
                treble: telemetry.treble,
            },
            energy: telemetry.energy,
            peak: telemetry.peak,
            beat: telemetry.beat,
            bpm: telemetry.bpm,
            beat_phase: telemetry.beat_phase,
        }
    }
}

/// One frame of pushed telemetry.
///
/// A frame rather than a bare analysis, so the socket can grow another volatile subject later
/// without every client having to be taught a second message shape.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryFrame {
    pub audio: AudioView,
    /// Every import this run has seen. Pushed rather than polled for the same reason as the
    /// meters: a progress bar that has to ask is a progress bar that stutters.
    pub imports: Vec<crate::wire::ImportJobView>,
}

/// The audio settings, as the API reports them.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct AudioSettingsView {
    /// `system-default`, `name`, or `index`.
    pub device_by: String,
    /// The name or index the operator chose, when they chose one.
    pub device_value: Option<String>,
    pub input_gain: f32,
    pub beat_sensitivity: f32,
    pub eq_bass: f32,
    pub eq_mid: f32,
    pub eq_treble: f32,
    /// This machine's inputs, so an operator picks from what exists rather than typing a name.
    pub available_devices: Vec<String>,
    /// Gain, sensitivity, and the bands reach the running analysis immediately. Choosing a
    /// different device reopens a stream, which happens on the next start.
    pub device_takes_effect_on_restart: bool,
}

impl AudioSettingsView {
    pub fn of(audio: &AudioConfiguration, available_devices: Vec<String>) -> Self {
        let (device_by, device_value) = match &audio.device {
            AudioDeviceSelector::SystemDefault => ("system-default", None),
            AudioDeviceSelector::Name(name) => ("name", Some(name.clone())),
            AudioDeviceSelector::Index(index) => ("index", Some(index.to_string())),
        };
        Self {
            device_by: device_by.to_owned(),
            device_value,
            input_gain: audio.input_gain,
            beat_sensitivity: audio.beat_sensitivity,
            eq_bass: audio.eq_bass,
            eq_mid: audio.eq_mid,
            eq_treble: audio.eq_treble,
            available_devices,
            device_takes_effect_on_restart: true,
        }
    }
}

/// One read of everything an audio panel needs before its telemetry socket is up.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct AudioPanelView {
    pub settings: AudioSettingsView,
    pub analysis: AudioView,
}

/// An intent-shaped audio edit: only the fields being changed.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAudio {
    pub request_id: String,
    /// `system-default`, `name`, or `index`. Naming a device without a value is refused, because
    /// falling back to the default input would give an operator the laptop microphone instead of
    /// the desk feed they asked for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_gain: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beat_sensitivity: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eq_bass: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eq_mid: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eq_treble: Option<f32>,
}

/// Why an audio edit was refused.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum AudioEditError {
    #[error("no device selection is called {by}")]
    UnknownSelector { by: String },
    #[error("choosing a device by {by} needs deviceValue")]
    MissingDeviceValue { by: &'static str },
    #[error("{field} must be between {low} and {high}")]
    OutOfRange {
        field: &'static str,
        low: f32,
        high: f32,
    },
}

/// The widest gain that is still a gain rather than a fault. Ten is enough to bring a quiet line
/// feed up; beyond it an operator is amplifying noise, and a typed 1000 is a mistake.
const MAX_GAIN: f32 = 10.0;

impl UpdateAudio {
    /// The configuration this edit describes, or why it was refused.
    pub fn applied(
        &self,
        current: &AudioConfiguration,
    ) -> Result<AudioConfiguration, AudioEditError> {
        let mut next = current.clone();
        if let Some(by) = &self.device_by {
            next.device = self.selector(by)?;
        }
        next.input_gain = gain("inputGain", self.input_gain, next.input_gain)?;
        next.beat_sensitivity = gain(
            "beatSensitivity",
            self.beat_sensitivity,
            next.beat_sensitivity,
        )?;
        next.eq_bass = gain("eqBass", self.eq_bass, next.eq_bass)?;
        next.eq_mid = gain("eqMid", self.eq_mid, next.eq_mid)?;
        next.eq_treble = gain("eqTreble", self.eq_treble, next.eq_treble)?;
        Ok(next)
    }

    fn selector(&self, by: &str) -> Result<AudioDeviceSelector, AudioEditError> {
        match by {
            "system-default" => Ok(AudioDeviceSelector::SystemDefault),
            "name" => {
                let name = self
                    .device_value
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or(AudioEditError::MissingDeviceValue { by: "name" })?;
                Ok(AudioDeviceSelector::Name(name.to_owned()))
            }
            "index" => {
                let index = self
                    .device_value
                    .as_deref()
                    .and_then(|value| value.trim().parse().ok())
                    .ok_or(AudioEditError::MissingDeviceValue { by: "index" })?;
                Ok(AudioDeviceSelector::Index(index))
            }
            other => Err(AudioEditError::UnknownSelector {
                by: other.to_owned(),
            }),
        }
    }
}

fn gain(field: &'static str, edited: Option<f32>, current: f32) -> Result<f32, AudioEditError> {
    let Some(value) = edited else {
        return Ok(current);
    };
    if !value.is_finite() || !(0.0..=MAX_GAIN).contains(&value) {
        return Err(AudioEditError::OutOfRange {
            field,
            low: 0.0,
            high: MAX_GAIN,
        });
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(body: &str) -> UpdateAudio {
        serde_json::from_str(body).expect("an audio edit")
    }

    #[test]
    fn a_machine_with_no_input_reports_silence_and_says_why() {
        let view = AudioView::of(&AudioTelemetry::default());
        assert!(!view.capturing);
        assert_eq!(view.energy, 0.0);
        assert!(view.detail.is_some());
        assert!(view.waveform.points.is_empty());
    }

    #[test]
    fn the_settings_report_the_machines_own_inputs() {
        let view = AudioSettingsView::of(
            &AudioConfiguration::default(),
            vec!["Built-in".to_owned(), "Desk feed".to_owned()],
        );
        assert_eq!(view.device_by, "system-default");
        assert_eq!(view.device_value, None);
        assert_eq!(view.available_devices.len(), 2);
        assert!(view.device_takes_effect_on_restart);
    }

    #[test]
    fn a_migrated_index_selection_is_reported_as_what_it_is() {
        let configuration = AudioConfiguration {
            device: AudioDeviceSelector::Index(3),
            ..Default::default()
        };
        let view = AudioSettingsView::of(&configuration, Vec::new());
        assert_eq!(view.device_by, "index");
        assert_eq!(view.device_value.as_deref(), Some("3"));
    }

    #[test]
    fn an_edit_changes_only_what_it_carries() {
        let current = AudioConfiguration::default();
        let next = edit(r#"{"requestId":"a","eqBass":1.5}"#)
            .applied(&current)
            .expect("accepted");

        assert_eq!(next.eq_bass, 1.5);
        assert_eq!(next.eq_mid, current.eq_mid);
        assert_eq!(next.device, current.device);
    }

    #[test]
    fn naming_a_device_without_naming_it_is_refused() {
        let error = edit(r#"{"requestId":"a","deviceBy":"name"}"#)
            .applied(&AudioConfiguration::default())
            .expect_err("refused");
        assert_eq!(error, AudioEditError::MissingDeviceValue { by: "name" });

        let named = edit(r#"{"requestId":"b","deviceBy":"name","deviceValue":" Desk feed "}"#)
            .applied(&AudioConfiguration::default())
            .expect("accepted");
        assert_eq!(
            named.device,
            AudioDeviceSelector::Name("Desk feed".to_owned())
        );
    }

    #[test]
    fn a_gain_outside_its_range_is_refused_by_name() {
        let error = edit(r#"{"requestId":"a","inputGain":50}"#)
            .applied(&AudioConfiguration::default())
            .expect_err("refused");
        assert_eq!(
            error,
            AudioEditError::OutOfRange {
                field: "inputGain",
                low: 0.0,
                high: MAX_GAIN
            }
        );
        assert!(error.to_string().contains("inputGain"));
    }

    #[test]
    fn an_unknown_selector_is_refused_rather_than_guessed_at() {
        assert_eq!(
            edit(r#"{"requestId":"a","deviceBy":"whatever is loudest"}"#)
                .applied(&AudioConfiguration::default())
                .expect_err("refused"),
            AudioEditError::UnknownSelector {
                by: "whatever is loudest".to_owned()
            }
        );
    }
}
