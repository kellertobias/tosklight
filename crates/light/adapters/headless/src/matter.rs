//! Matter-facing playback bridge model.
//!
//! The transport-independent adapter in this module deliberately models explicit
//! `page/playback` addresses. A controller must never inherit the current page of any
//! particular control desk. The production `rs-matter` transport consumes
//! [`MatterBridgeAdapter::status`] as its endpoint/attribute snapshot and routes controller writes
//! through [`MatterBridgeAdapter::resolve_write`].

use light_playback::{PlaybackAddress, PlaybackPage};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[path = "matter/transport.rs"]
mod transport;

pub use transport::{
    MatterPairingData, MatterTransport, MatterTransportLifecycle, MatterTransportSnapshot,
};

/// Matter reserves `0xff` for an unknown/null CurrentLevel value.
pub const MAX_MATTER_LEVEL: u8 = 0xfe;

const TRANSPORT_LIMITATION: &str =
    "The playback bridge adapter is ready, but the Matter commissioning transport has not started";

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlaybackValue {
    pub level: f32,
    pub active: bool,
    pub color: Option<light_core::Xyz>,
}

impl PlaybackValue {
    pub fn new(level: f32, active: bool) -> Self {
        Self {
            level,
            active,
            color: None,
        }
    }

    pub fn with_color(mut self, color: Option<light_core::Xyz>) -> Self {
        self.color = color;
        self
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MatterLightKind {
    Dimmable,
    Color,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MatterColorMode {
    HueSaturation,
    ColorTemperature,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct MatterColorState {
    pub hue: u8,
    pub saturation: u8,
    pub color_temperature_mireds: u16,
    pub mode: MatterColorMode,
}

impl MatterColorState {
    pub fn from_xyz(color: light_core::Xyz, mode: MatterColorMode) -> Self {
        let (x, y) = xyz_to_matter_xy(color);
        let (enhanced_hue, saturation) =
            rs_matter::dm::clusters::app::color_control::SetDeviceColor::Xy { x, y }
                .to_hue_saturation();
        Self {
            hue: (enhanced_hue >> 8).min(254) as u8,
            saturation,
            color_temperature_mireds: xyz_to_mireds(color),
            mode,
        }
    }
}

impl Default for MatterColorState {
    fn default() -> Self {
        Self::from_xyz(
            light_core::Xyz {
                x: 0.950_47,
                y: 1.0,
                z: 1.088_83,
            },
            MatterColorMode::ColorTemperature,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MatterTransportState {
    Disabled,
    AdapterReady,
    Starting,
    Running,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MatterPlaybackLight {
    /// Stable Matter endpoint ID derived from the explicit page/playback address.
    pub endpoint_id: u16,
    pub address: PlaybackAddress,
    pub name: String,
    pub on: bool,
    /// Matter Level Control value in the inclusive range `0..=254`.
    pub level: u8,
    pub kind: MatterLightKind,
    /// True only while the engine owns an explicit Group color contribution for this endpoint.
    pub color_active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<MatterColorState>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MatterBridgeStatus {
    pub enabled: bool,
    pub transport: MatterTransportState,
    /// True only while a successfully started IP transport advertises a commissioning window.
    pub commissionable: bool,
    /// True only after the Matter UDP and mDNS sockets have both been opened successfully.
    pub network_running: bool,
    pub commissioned: bool,
    pub commissioning_window_open: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pairing: Option<MatterPairingData>,
    /// Changes whenever the endpoint list or any mirrored value changes.
    pub revision: u64,
    pub lights: Vec<MatterPlaybackLight>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limitation: Option<String>,
}

impl Default for MatterBridgeStatus {
    fn default() -> Self {
        Self {
            enabled: false,
            transport: MatterTransportState::Disabled,
            commissionable: false,
            network_running: false,
            commissioned: false,
            commissioning_window_open: false,
            pairing: None,
            revision: 0,
            lights: Vec::new(),
            limitation: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MatterPlaybackWrite {
    pub on: Option<bool>,
    pub level: Option<u8>,
    pub color: Option<MatterColorWrite>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MatterColorWrite {
    HueSaturation { hue: u8, saturation: u8 },
    ColorTemperature { mireds: u16 },
}

impl MatterColorWrite {
    pub fn xyz(self) -> light_core::Xyz {
        let target = match self {
            Self::HueSaturation { hue, saturation } => {
                rs_matter::dm::clusters::app::color_control::SetDeviceColor::HueSaturation {
                    enhanced_hue: u16::from(hue) * 257,
                    saturation,
                }
            }
            Self::ColorTemperature { mireds } => {
                rs_matter::dm::clusters::app::color_control::SetDeviceColor::ColorTemperature {
                    mireds,
                }
            }
        };
        let (x, y) = target.to_xy();
        matter_xy_to_xyz(x, y)
    }

    pub const fn mode(self) -> MatterColorMode {
        match self {
            Self::HueSaturation { .. } => MatterColorMode::HueSaturation,
            Self::ColorTemperature { .. } => MatterColorMode::ColorTemperature,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedMatterPlaybackWrite {
    pub endpoint_id: u16,
    pub address: PlaybackAddress,
    pub level: f32,
    pub color: Option<MatterColorWrite>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MatterBridgeError {
    Disabled,
    EndpointNotExposed(u16),
    MissingValue,
    ReservedLevel,
}

impl std::fmt::Display for MatterBridgeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disabled => formatter.write_str("Matter playback control is disabled"),
            Self::EndpointNotExposed(endpoint) => {
                write!(formatter, "Matter endpoint {endpoint} is not exposed")
            }
            Self::MissingValue => formatter.write_str("Matter write requires on or level"),
            Self::ReservedLevel => formatter.write_str("Matter level 255 is reserved"),
        }
    }
}

/// A stateful seam between the desk runtime and a Matter protocol implementation.
///
/// Reconciliation is idempotent and revisions are monotonic. That lets a transport publish
/// bidirectional attribute changes without coupling the lighting engine to a particular Matter
/// crate or executor.
#[derive(Debug, Default)]
pub struct MatterBridgeAdapter {
    status: RwLock<MatterBridgeStatus>,
    allocations: RwLock<Vec<MatterEndpointAllocation>>,
}

impl MatterBridgeAdapter {
    /// Persist reservations before exposing newly allocated endpoints to the transport.
    pub fn reconcile_persisted(
        &self,
        enabled: bool,
        pages: &[PlaybackPage],
        values: &HashMap<PlaybackAddress, PlaybackValue>,
        saved: Option<&str>,
        persist: impl FnOnce(&str) -> Result<(), String>,
    ) -> MatterBridgeStatus {
        if !enabled {
            return self.reconcile(false, pages, values);
        }
        let result = (|| {
            let previous: Vec<MatterEndpointAllocation> = saved
                .map(serde_json::from_str)
                .transpose()
                .map_err(|error| format!("Cannot read saved Matter endpoint allocation: {error}"))?
                .unwrap_or_default();
            let mut cached = self.allocations.write();
            let mut merged = previous.clone();
            for allocation in cached.iter() {
                if !merged.contains(allocation) {
                    merged.push(allocation.clone());
                }
            }
            let next = allocate_endpoints(pages, values, &merged)?;
            if next != previous {
                let serialized = serde_json::to_string(&next).map_err(|error| error.to_string())?;
                persist(&serialized)?;
            }
            *cached = next;
            Ok::<_, String>(())
        })();
        match result {
            Ok(()) => {
                {
                    let mut status = self.status.write();
                    if status.transport == MatterTransportState::Failed {
                        status.transport = MatterTransportState::AdapterReady;
                        status.limitation = None;
                    }
                }
                self.reconcile(true, pages, values)
            }
            Err(error) => self.fail(error),
        }
    }

    pub fn fail(&self, error: String) -> MatterBridgeStatus {
        let mut status = self.status.write();
        let changed = status.transport != MatterTransportState::Failed
            || !status.lights.is_empty()
            || status.limitation.as_deref() != Some(error.as_str());
        status.enabled = true;
        status.transport = MatterTransportState::Failed;
        status.lights.clear();
        status.commissionable = false;
        status.network_running = false;
        if changed {
            status.revision = status.revision.saturating_add(1);
        }
        status.limitation = Some(error);
        status.clone()
    }

    pub fn reconcile(
        &self,
        enabled: bool,
        pages: &[PlaybackPage],
        values: &HashMap<PlaybackAddress, PlaybackValue>,
    ) -> MatterBridgeStatus {
        if enabled {
            let next = allocate_endpoints(pages, values, &self.allocations.read());
            match next {
                Ok(next) => *self.allocations.write() = next,
                Err(error) => return self.fail(error),
            }
        }
        let mut lights = if enabled {
            build_lights(pages, values, &self.allocations.read())
        } else {
            Vec::new()
        };
        let mut current = self.status.write();
        for light in &mut lights {
            if let (Some(color), Some(current_color)) = (
                light.color.as_mut(),
                current
                    .lights
                    .iter()
                    .find(|current| current.endpoint_id == light.endpoint_id)
                    .and_then(|current| current.color),
            ) {
                color.mode = current_color.mode;
            }
        }
        let transport = if !enabled {
            MatterTransportState::Disabled
        } else if current.enabled
            && matches!(
                current.transport,
                MatterTransportState::Starting
                    | MatterTransportState::Running
                    | MatterTransportState::Failed
            )
        {
            current.transport
        } else {
            MatterTransportState::AdapterReady
        };
        let limitation = if !enabled {
            None
        } else if transport == MatterTransportState::AdapterReady {
            Some(TRANSPORT_LIMITATION.to_owned())
        } else {
            current.limitation.clone()
        };
        let (commissionable, network_running, commissioned, commissioning_window_open, pairing) =
            if enabled {
                (
                    current.commissionable,
                    current.network_running,
                    current.commissioned,
                    current.commissioning_window_open,
                    current.pairing.clone(),
                )
            } else {
                (false, false, false, false, None)
            };
        let changed = current.enabled != enabled
            || current.transport != transport
            || current.commissionable != commissionable
            || current.network_running != network_running
            || current.commissioned != commissioned
            || current.commissioning_window_open != commissioning_window_open
            || current.pairing != pairing
            || current.lights != lights
            || current.limitation != limitation;
        if changed {
            current.revision = current.revision.saturating_add(1);
        }
        current.enabled = enabled;
        current.transport = transport;
        current.commissionable = commissionable;
        current.network_running = network_running;
        current.commissioned = commissioned;
        current.commissioning_window_open = commissioning_window_open;
        current.pairing = pairing;
        current.lights = lights;
        current.limitation = limitation;
        current.clone()
    }

    /// Merge the production transport's lifecycle and pairing state into the adapter status.
    ///
    /// The transport owns network truth: in particular, `commissionable` can only become true
    /// after both the Matter UDP and mDNS sockets are running and a commissioning window is open.
    pub fn apply_transport_snapshot(
        &self,
        transport: &MatterTransportSnapshot,
    ) -> MatterBridgeStatus {
        let mut current = self.status.write();
        let transport_state = if !current.enabled {
            MatterTransportState::Disabled
        } else {
            match transport.lifecycle {
                MatterTransportLifecycle::Disabled => MatterTransportState::AdapterReady,
                MatterTransportLifecycle::Starting => MatterTransportState::Starting,
                MatterTransportLifecycle::Running => MatterTransportState::Running,
                MatterTransportLifecycle::Failed => MatterTransportState::Failed,
            }
        };
        let limitation = if !current.enabled {
            None
        } else {
            match transport.lifecycle {
                MatterTransportLifecycle::Disabled => Some(TRANSPORT_LIMITATION.to_owned()),
                MatterTransportLifecycle::Starting => {
                    Some("Matter network transport is starting".into())
                }
                MatterTransportLifecycle::Failed => transport.last_error.clone(),
                MatterTransportLifecycle::Running => None,
            }
        };
        let commissionable = current.enabled
            && transport.network_running
            && transport.commissioning_window_open
            && transport.commissionable;
        let network_running = current.enabled && transport.network_running;
        let commissioned = current.enabled && transport.commissioned;
        let commissioning_window_open = current.enabled && transport.commissioning_window_open;
        let pairing = if current.enabled {
            transport.pairing.clone()
        } else {
            None
        };
        let changed = current.transport != transport_state
            || current.commissionable != commissionable
            || current.network_running != network_running
            || current.commissioned != commissioned
            || current.commissioning_window_open != commissioning_window_open
            || current.pairing != pairing
            || current.limitation != limitation;
        if changed {
            current.revision = current.revision.saturating_add(1);
        }
        current.transport = transport_state;
        current.commissionable = commissionable;
        current.network_running = network_running;
        current.commissioned = commissioned;
        current.commissioning_window_open = commissioning_window_open;
        current.pairing = pairing;
        current.limitation = limitation;
        current.clone()
    }

    pub fn status(&self) -> MatterBridgeStatus {
        self.status.read().clone()
    }

    /// Record the controller's chosen ColorControl mode and value exactly once before ordinary
    /// authoritative reconciliation. Desk-side changes retain that mode while updating values.
    pub fn apply_color_write(&self, endpoint_id: u16, write: MatterColorWrite) {
        let mut current = self.status.write();
        let next = MatterColorState::from_xyz(write.xyz(), write.mode());
        let changed = current
            .lights
            .iter_mut()
            .find(|light| light.endpoint_id == endpoint_id)
            .is_some_and(|light| {
                if light.color == Some(next) && light.color_active {
                    false
                } else {
                    light.color = Some(next);
                    light.color_active = true;
                    true
                }
            });
        if changed {
            current.revision = current.revision.saturating_add(1);
        }
    }

    /// Resolve a Matter On/Off or Level Control mutation to the assigned global playback.
    ///
    /// `Off` wins when a combined write contains contradictory fields. `On` without a level
    /// retains a currently non-zero value and otherwise activates the playback at full.
    pub fn resolve_write(
        &self,
        endpoint_id: u16,
        write: MatterPlaybackWrite,
    ) -> Result<ResolvedMatterPlaybackWrite, MatterBridgeError> {
        let status = self.status.read();
        if !status.enabled {
            return Err(MatterBridgeError::Disabled);
        }
        let light = status
            .lights
            .iter()
            .find(|light| light.endpoint_id == endpoint_id)
            .ok_or(MatterBridgeError::EndpointNotExposed(endpoint_id))?;
        if write.on.is_none() && write.level.is_none() && write.color.is_none() {
            return Err(MatterBridgeError::MissingValue);
        }
        if write.level == Some(u8::MAX) {
            return Err(MatterBridgeError::ReservedLevel);
        }
        if write.color.is_some() && light.kind != MatterLightKind::Color {
            return Err(MatterBridgeError::EndpointNotExposed(endpoint_id));
        }
        let matter_level = if write.on == Some(false) {
            0
        } else if let Some(level) = write.level {
            level
        } else if write.on == Some(true) {
            if light.level > 0 {
                light.level
            } else {
                MAX_MATTER_LEVEL
            }
        } else if write.color.is_some() {
            light.level
        } else {
            return Err(MatterBridgeError::MissingValue);
        };
        Ok(ResolvedMatterPlaybackWrite {
            endpoint_id,
            address: light.address,
            level: f32::from(matter_level) / f32::from(MAX_MATTER_LEVEL),
            color: write.color,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MatterEndpointAllocation {
    pub address: PlaybackAddress,
    pub endpoint_id: u16,
}

/// Keep removed addresses reserved so removing and restoring an assignment cannot redirect a
/// commissioned controller to another playback. The root, aggregator and null endpoint IDs are reserved.
fn allocate_endpoints(
    pages: &[PlaybackPage],
    values: &HashMap<PlaybackAddress, PlaybackValue>,
    saved: &[MatterEndpointAllocation],
) -> Result<Vec<MatterEndpointAllocation>, String> {
    let mut addresses = std::collections::BTreeSet::new();
    let mut endpoints = std::collections::BTreeSet::new();
    for allocation in saved {
        if allocation.endpoint_id == 0
            || allocation.endpoint_id >= 0xfffe
            || !addresses.insert(allocation.address)
            || !endpoints.insert(allocation.endpoint_id)
        {
            return Err(
                "Saved Matter endpoint allocation contains a reserved ID or duplicate address/ID"
                    .into(),
            );
        }
    }
    let required = pages
        .iter()
        .flat_map(|page| {
            page.assignments
                .keys()
                .filter_map(move |position| PlaybackAddress::new(page.number, *position).ok())
        })
        .filter(|address| values.contains_key(address))
        .collect::<std::collections::BTreeSet<_>>();
    let mut result = saved.to_vec();
    let mut available = (1..0xfffe).filter(|id| !endpoints.contains(id));
    for address in required {
        if addresses.contains(&address) {
            continue;
        }
        let endpoint_id = available.next().ok_or_else(||
            "Matter endpoint capacity is exhausted; remove unused saved endpoint reservations or reduce exposed assignments".to_string())?;
        result.push(MatterEndpointAllocation {
            address,
            endpoint_id,
        });
    }
    Ok(result)
}

fn build_lights(
    pages: &[PlaybackPage],
    values: &HashMap<PlaybackAddress, PlaybackValue>,
    allocations: &[MatterEndpointAllocation],
) -> Vec<MatterPlaybackLight> {
    let endpoints = allocations
        .iter()
        .map(|entry| (entry.address, entry.endpoint_id))
        .collect::<HashMap<_, _>>();
    let mut lights = BTreeMap::new();
    for page in pages {
        for (&position, definition) in &page.assignments {
            let Ok(address) = PlaybackAddress::new(page.number, position) else {
                continue;
            };
            let Some(&endpoint_id) = endpoints.get(&address) else {
                continue;
            };
            let Some(value) = values.get(&address).copied() else {
                continue;
            };
            let normalized = if value.active && value.level.is_finite() {
                value.level.clamp(0.0, 1.0)
            } else {
                0.0
            };
            let level = (normalized * f32::from(MAX_MATTER_LEVEL)).round() as u8;
            let kind = if matches!(
                definition.target,
                light_playback::PlaybackTarget::Group { .. }
            ) {
                MatterLightKind::Color
            } else {
                MatterLightKind::Dimmable
            };
            lights.insert(
                endpoint_id,
                MatterPlaybackLight {
                    endpoint_id,
                    address,
                    name: format!(
                        "Page {} Playback {}: {}",
                        page.number, position, definition.name
                    ),
                    on: value.active && level > 0,
                    level,
                    kind,
                    color_active: kind == MatterLightKind::Color && value.color.is_some(),
                    color: (kind == MatterLightKind::Color).then(|| match value.color {
                        Some(color) => {
                            MatterColorState::from_xyz(color, MatterColorMode::HueSaturation)
                        }
                        None => MatterColorState::default(),
                    }),
                },
            );
        }
    }
    lights.into_values().collect()
}

fn xyz_to_matter_xy(color: light_core::Xyz) -> (u16, u16) {
    let total = color.x + color.y + color.z;
    if !total.is_finite() || total <= f32::EPSILON {
        return (0, 0);
    }
    let scale = 0xFEFF as f32;
    (
        (color.x / total * scale).round().clamp(0.0, scale) as u16,
        (color.y / total * scale).round().clamp(0.0, scale) as u16,
    )
}

fn matter_xy_to_xyz(x: u16, y: u16) -> light_core::Xyz {
    let scale = 0xFEFF as f32;
    let x = f32::from(x) / scale;
    let y = (f32::from(y) / scale).max(0.000_1);
    let z = (1.0 - x - y).max(0.0);
    light_core::Xyz {
        x: x / y,
        y: 1.0,
        z: z / y,
    }
}

fn xyz_to_mireds(color: light_core::Xyz) -> u16 {
    let total = color.x + color.y + color.z;
    if total <= f32::EPSILON {
        return 370;
    }
    let x = color.x / total;
    let y = color.y / total;
    let denominator = 0.1858 - y;
    if denominator.abs() < 0.000_1 {
        return 370;
    }
    let n = (x - 0.3320) / denominator;
    let kelvin = 449.0 * n.powi(3) + 3525.0 * n.powi(2) + 6823.3 * n + 5520.33;
    if !kelvin.is_finite() || kelvin <= 0.0 {
        return 370;
    }
    (1_000_000.0 / kelvin).round().clamp(153.0, 500.0) as u16
}

#[cfg(test)]
#[path = "matter/tests.rs"]
mod tests;
