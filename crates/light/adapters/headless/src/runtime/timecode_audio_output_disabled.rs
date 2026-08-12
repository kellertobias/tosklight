//! Timecode audio boundary for builds without a native device adapter.
//!
//! Cross-compiled headless artifacts remain useful for lighting output without requiring a target
//! ALSA sysroot. Native Desk builds enable `native-audio-output` and use the CPAL implementation.

use std::sync::Arc;

use light_application::timeline::TimecodeClock;
use light_application::{ManagedAssetStore, TimecodeAudioCommand, TimecodeAudioOutput};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum OutputDeviceSelector {
    SystemDefault,
    Name(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct NativeTimecodeAudioConfig {
    pub device: OutputDeviceSelector,
    pub latency_trim_micros: i64,
}

const OUTPUT_DEVICE_PROBE_ARGUMENT: &str = "--probe-timecode-audio-outputs";

pub(super) fn run_output_device_probe_from_process() -> anyhow::Result<bool> {
    if std::env::args().nth(1).as_deref() != Some(OUTPUT_DEVICE_PROBE_ARGUMENT) {
        return Ok(false);
    }
    serde_json::to_writer(std::io::stdout().lock(), &Vec::<String>::new())?;
    Ok(true)
}

pub(super) fn output_devices() -> Result<Vec<String>, String> {
    Ok(Vec::new())
}

pub(super) struct NativeTimecodeAudioOutput;

impl NativeTimecodeAudioOutput {
    pub(super) fn open_with_timeout(
        _store: Arc<dyn ManagedAssetStore>,
        _clock: Arc<dyn TimecodeClock>,
        configuration: &NativeTimecodeAudioConfig,
    ) -> Result<Self, String> {
        let _ = (&configuration.device, configuration.latency_trim_micros);
        Err("this build does not include native Timecode audio output".into())
    }
}

impl TimecodeAudioOutput for NativeTimecodeAudioOutput {
    fn output_latency_micros(&self) -> u64 {
        0
    }

    fn apply(&self, _command: TimecodeAudioCommand) -> Result<(), String> {
        Err("this build does not include native Timecode audio output".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_build_reports_no_native_devices() {
        assert!(output_devices().unwrap().is_empty());
    }
}
