//! Matter-facing playback bridge model.
//!
//! The transport-independent adapter in this module deliberately models explicit
//! `page/playback` addresses. A controller must never inherit the current page of any
//! particular control desk. The production `rs-matter` transport consumes
//! [`MatterBridgeAdapter::status`] as its endpoint/attribute snapshot and routes controller writes
//! through [`MatterBridgeAdapter::resolve_write`].

use light_playback::{MAX_PAGE_SLOTS, MAX_PLAYBACK_PAGES, PlaybackDefinition, PlaybackPage};
use parking_lot::RwLock;
use serde::Serialize;
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
}

impl PlaybackValue {
    pub fn new(level: f32, active: bool) -> Self {
        Self { level, active }
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
    pub page: u8,
    pub playback: u8,
    /// Playback-pool number currently assigned to this explicit address.
    pub playback_number: u16,
    pub name: String,
    pub on: bool,
    /// Matter Level Control value in the inclusive range `0..=254`.
    pub level: u8,
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
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedMatterPlaybackWrite {
    pub endpoint_id: u16,
    pub page: u8,
    pub playback: u8,
    pub playback_number: u16,
    pub level: f32,
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
}

impl MatterBridgeAdapter {
    pub fn reconcile(
        &self,
        enabled: bool,
        pages: &[PlaybackPage],
        definitions: &[PlaybackDefinition],
        values: &HashMap<u16, PlaybackValue>,
    ) -> MatterBridgeStatus {
        let lights = if enabled {
            build_lights(pages, definitions, values)
        } else {
            Vec::new()
        };
        let mut current = self.status.write();
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
        if write.on.is_none() && write.level.is_none() {
            return Err(MatterBridgeError::MissingValue);
        }
        if write.level == Some(u8::MAX) {
            return Err(MatterBridgeError::ReservedLevel);
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
        } else {
            return Err(MatterBridgeError::MissingValue);
        };
        Ok(ResolvedMatterPlaybackWrite {
            endpoint_id,
            page: light.page,
            playback: light.playback,
            playback_number: light.playback_number,
            level: f32::from(matter_level) / f32::from(MAX_MATTER_LEVEL),
        })
    }
}

/// Endpoint zero is the Matter root endpoint, so explicit playback addresses start at one.
pub fn endpoint_id(page: u8, playback: u8) -> Option<u16> {
    if !(1..=MAX_PLAYBACK_PAGES).contains(&page) || !(1..=MAX_PAGE_SLOTS).contains(&playback) {
        return None;
    }
    Some(1 + u16::from(page - 1) * u16::from(MAX_PAGE_SLOTS) + u16::from(playback - 1))
}

fn build_lights(
    pages: &[PlaybackPage],
    definitions: &[PlaybackDefinition],
    values: &HashMap<u16, PlaybackValue>,
) -> Vec<MatterPlaybackLight> {
    let definitions = definitions
        .iter()
        .map(|definition| (definition.number, definition))
        .collect::<HashMap<_, _>>();
    let mut lights = BTreeMap::new();
    for page in pages {
        for (&playback, &playback_number) in &page.slots {
            let Some(endpoint_id) = endpoint_id(page.number, playback) else {
                continue;
            };
            let Some(definition) = definitions.get(&playback_number) else {
                continue;
            };
            let value = values.get(&playback_number).copied().unwrap_or_default();
            let normalized = if value.active && value.level.is_finite() {
                value.level.clamp(0.0, 1.0)
            } else {
                0.0
            };
            let level = (normalized * f32::from(MAX_MATTER_LEVEL)).round() as u8;
            lights.insert(
                endpoint_id,
                MatterPlaybackLight {
                    endpoint_id,
                    page: page.number,
                    playback,
                    playback_number,
                    name: format!(
                        "Page {} Playback {}: {}",
                        page.number, playback, definition.name
                    ),
                    on: value.active && level > 0,
                    level,
                },
            );
        }
    }
    lights.into_values().collect()
}

#[cfg(test)]
#[path = "matter/tests.rs"]
mod tests;
