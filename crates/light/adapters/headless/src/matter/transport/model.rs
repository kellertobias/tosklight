use super::super::{MAX_MATTER_LEVEL, MatterPlaybackLight, MatterPlaybackWrite};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;

pub(super) const AGGREGATOR_ENDPOINT_ID: u16 = 0xfffe;

/// User-facing pairing material. The passcode is represented only through the standard manual
/// code so callers do not need to understand Matter's passcode encoding.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MatterPairingData {
    pub qr_code: String,
    pub manual_code: String,
    pub discriminator: u16,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MatterTransportLifecycle {
    #[default]
    Disabled,
    Starting,
    Running,
    Failed,
}

/// Truthful network status for the production transport. `commissionable` is true only while a
/// commissioning window is advertised by a successfully started IP transport.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MatterTransportSnapshot {
    pub lifecycle: MatterTransportLifecycle,
    pub network_running: bool,
    pub commissioned: bool,
    pub commissioning_window_open: bool,
    pub commissionable: bool,
    pub endpoint_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pairing: Option<MatterPairingData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

impl Default for MatterTransportSnapshot {
    fn default() -> Self {
        Self {
            lifecycle: MatterTransportLifecycle::Disabled,
            network_running: false,
            commissioned: false,
            commissioning_window_open: false,
            commissionable: false,
            endpoint_count: 0,
            pairing: None,
            last_error: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MatterRemoteWrite {
    pub endpoint_id: u16,
    pub write: MatterPlaybackWrite,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) struct MatterIdentity {
    pub(super) passcode: u32,
    pub(super) discriminator: u16,
    pub(super) serial: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) struct EndpointShape {
    pub(super) endpoint_id: u16,
    pub(super) name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct TransportLight {
    pub(super) endpoint_id: u16,
    pub(super) name: String,
    pub(super) on: bool,
    pub(super) level: u8,
}

impl From<&MatterPlaybackLight> for TransportLight {
    fn from(light: &MatterPlaybackLight) -> Self {
        Self {
            endpoint_id: light.endpoint_id,
            name: matter_string(&light.name, 32),
            on: light.on,
            level: light.level.min(MAX_MATTER_LEVEL),
        }
    }
}

pub(super) enum ControlCommand {
    Reconcile(Vec<TransportLight>),
    Shutdown,
}

pub(super) struct RuntimeHandle {
    pub(super) shape: Vec<EndpointShape>,
    pub(super) control: Sender<ControlCommand>,
    pub(super) join: JoinHandle<()>,
}

#[derive(Clone, Copy)]
pub(super) struct StartupReady {
    pub(super) commissioned: bool,
    pub(super) commissioning_window_open: bool,
}

pub(super) fn validate_lights(
    lights: &[MatterPlaybackLight],
) -> Result<Vec<TransportLight>, String> {
    let mut endpoints = BTreeSet::new();
    let mut result = Vec::with_capacity(lights.len());
    for light in lights {
        if light.endpoint_id == 0 || light.endpoint_id == AGGREGATOR_ENDPOINT_ID {
            return Err(format!(
                "Matter playback endpoint {} is reserved",
                light.endpoint_id
            ));
        }
        if !endpoints.insert(light.endpoint_id) {
            return Err(format!(
                "Matter playback endpoint {} is duplicated",
                light.endpoint_id
            ));
        }
        result.push(TransportLight::from(light));
    }
    result.sort_by_key(|light| light.endpoint_id);
    Ok(result)
}

pub(super) fn endpoint_shape(lights: &[TransportLight]) -> Vec<EndpointShape> {
    lights
        .iter()
        .map(|light| EndpointShape {
            endpoint_id: light.endpoint_id,
            name: light.name.clone(),
        })
        .collect()
}

pub(super) fn matter_string(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_owned()
}
