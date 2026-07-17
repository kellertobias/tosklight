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
mod tests {
    use super::*;
    use light_playback::{
        FlashReleaseMode, PlaybackButtonAction, PlaybackFaderMode, PlaybackTarget,
    };

    fn definition(number: u16, name: &str, has_fader: bool) -> PlaybackDefinition {
        PlaybackDefinition {
            number,
            name: name.into(),
            target: PlaybackTarget::CueList {
                cue_list_id: light_core::CueListId::new(),
            },
            buttons: [PlaybackButtonAction::None; 3],
            button_count: 3,
            fader: PlaybackFaderMode::Master,
            has_fader,
            go_activates: true,
            auto_off: true,
            xfade_millis: 0,
            color: "#20c997".into(),
            flash_release: FlashReleaseMode::ReleaseAll,
            protect_from_swap: false,
            presentation_icon: None,
            presentation_image: None,
        }
    }

    #[test]
    fn endpoint_ids_are_stable_global_page_playback_addresses() {
        assert_eq!(endpoint_id(1, 1), Some(1));
        assert_eq!(endpoint_id(1, 127), Some(127));
        assert_eq!(endpoint_id(2, 1), Some(128));
        assert_eq!(endpoint_id(127, 127), Some(16_129));
        assert_eq!(endpoint_id(0, 1), None);
        assert_eq!(endpoint_id(1, 0), None);
    }

    #[test]
    fn disabled_bridge_exposes_nothing_and_does_not_churn_revision() {
        let adapter = MatterBridgeAdapter::default();
        let page = PlaybackPage {
            number: 1,
            name: "Main".into(),
            slots: HashMap::from([(1, 7)]),
        };
        let first = adapter.reconcile(
            false,
            std::slice::from_ref(&page),
            &[definition(7, "Look", true)],
            &HashMap::new(),
        );
        let second = adapter.reconcile(
            false,
            &[page],
            &[definition(7, "Look", true)],
            &HashMap::new(),
        );
        assert_eq!(first, MatterBridgeStatus::default());
        assert_eq!(second.revision, first.revision);
        assert_eq!(
            adapter.resolve_write(
                1,
                MatterPlaybackWrite {
                    on: Some(true),
                    level: None
                }
            ),
            Err(MatterBridgeError::Disabled)
        );
    }

    #[test]
    fn every_assigned_playback_is_exposed_in_global_address_order() {
        let adapter = MatterBridgeAdapter::default();
        let pages = [
            PlaybackPage {
                number: 2,
                name: "Second".into(),
                slots: HashMap::from([(1, 20), (3, 30)]),
            },
            PlaybackPage {
                number: 1,
                name: "First".into(),
                slots: HashMap::from([(2, 10)]),
            },
        ];
        let values = HashMap::from([
            (10, PlaybackValue::new(0.5, true)),
            (20, PlaybackValue::new(1.0, true)),
        ]);
        let status = adapter.reconcile(
            true,
            &pages,
            &[
                definition(10, "Half", true),
                definition(20, "Full", true),
                definition(30, "Button only", false),
                definition(40, "Pool only", true),
            ],
            &values,
        );

        assert_eq!(status.transport, MatterTransportState::AdapterReady);
        assert!(!status.commissionable);
        assert_eq!(
            status
                .lights
                .iter()
                .map(|light| (
                    light.page,
                    light.playback,
                    light.playback_number,
                    light.level
                ))
                .collect::<Vec<_>>(),
            vec![(1, 2, 10, 127), (2, 1, 20, 254), (2, 3, 30, 0)]
        );
        assert_eq!(status.lights[2].name, "Page 2 Playback 3: Button only");
        assert_eq!(status.lights[2].endpoint_id, endpoint_id(2, 3).unwrap());
        assert!(
            status
                .lights
                .iter()
                .all(|light| light.playback_number != 40)
        );
        assert!(status.limitation.is_some());
    }

    #[test]
    fn runtime_reconciliation_publishes_remote_and_tracking_changes_bidirectionally() {
        let adapter = MatterBridgeAdapter::default();
        let pages = [PlaybackPage {
            number: 4,
            name: "Looks".into(),
            slots: HashMap::from([(7, 25)]),
        }];
        let definitions = [definition(25, "Tracked", true)];
        let active = adapter.reconcile(
            true,
            &pages,
            &definitions,
            &HashMap::from([(25, PlaybackValue::new(0.5, true))]),
        );
        assert!(active.lights[0].on);
        assert_eq!(active.lights[0].level, 127);

        // Automatic tracking can disable a playback while retaining its old master internally.
        let tracked_off = adapter.reconcile(
            true,
            &pages,
            &definitions,
            &HashMap::from([(25, PlaybackValue::new(0.5, false))]),
        );
        assert!(!tracked_off.lights[0].on);
        assert_eq!(tracked_off.lights[0].level, 0);
        assert!(tracked_off.revision > active.revision);
    }

    #[test]
    fn transport_status_is_commissionable_only_after_network_start_and_open_window() {
        let adapter = MatterBridgeAdapter::default();
        adapter.reconcile(
            true,
            &[PlaybackPage {
                number: 1,
                name: "Main".into(),
                slots: HashMap::from([(1, 7)]),
            }],
            &[definition(7, "Look", true)],
            &HashMap::new(),
        );
        let pairing = MatterPairingData {
            qr_code: "MT:TEST".into(),
            manual_code: "1234-567-8901".into(),
            discriminator: 42,
        };

        let starting = adapter.apply_transport_snapshot(&MatterTransportSnapshot {
            lifecycle: MatterTransportLifecycle::Starting,
            pairing: Some(pairing.clone()),
            ..MatterTransportSnapshot::default()
        });
        assert_eq!(starting.transport, MatterTransportState::Starting);
        assert!(!starting.commissionable);
        assert!(!starting.network_running);
        assert_eq!(starting.pairing, Some(pairing.clone()));

        let running_without_window = adapter.apply_transport_snapshot(&MatterTransportSnapshot {
            lifecycle: MatterTransportLifecycle::Running,
            network_running: true,
            commissioned: true,
            commissioning_window_open: false,
            commissionable: true,
            pairing: Some(pairing.clone()),
            ..MatterTransportSnapshot::default()
        });
        assert_eq!(
            running_without_window.transport,
            MatterTransportState::Running
        );
        assert!(running_without_window.network_running);
        assert!(running_without_window.commissioned);
        assert!(!running_without_window.commissionable);

        let commissionable = adapter.apply_transport_snapshot(&MatterTransportSnapshot {
            lifecycle: MatterTransportLifecycle::Running,
            network_running: true,
            commissioned: false,
            commissioning_window_open: true,
            commissionable: true,
            pairing: Some(pairing),
            ..MatterTransportSnapshot::default()
        });
        assert!(commissionable.commissionable);
        assert!(commissionable.commissioning_window_open);

        adapter.reconcile(false, &[], &[], &HashMap::new());
        let disabled = adapter.apply_transport_snapshot(&MatterTransportSnapshot {
            lifecycle: MatterTransportLifecycle::Running,
            network_running: true,
            commissioning_window_open: true,
            commissionable: true,
            ..MatterTransportSnapshot::default()
        });
        assert_eq!(disabled.transport, MatterTransportState::Disabled);
        assert!(!disabled.network_running);
        assert!(!disabled.commissionable);
        assert!(disabled.pairing.is_none());
    }

    #[test]
    fn matter_writes_resolve_the_explicit_address_without_a_desk_page() {
        let adapter = MatterBridgeAdapter::default();
        let pages = [PlaybackPage {
            number: 4,
            name: "Looks".into(),
            slots: HashMap::from([(7, 25)]),
        }];
        adapter.reconcile(
            true,
            &pages,
            &[definition(25, "Look", true)],
            &HashMap::from([(25, PlaybackValue::new(0.5, true))]),
        );
        let endpoint = endpoint_id(4, 7).unwrap();
        let half = adapter
            .resolve_write(
                endpoint,
                MatterPlaybackWrite {
                    on: None,
                    level: Some(127),
                },
            )
            .unwrap();
        assert_eq!((half.page, half.playback, half.playback_number), (4, 7, 25));
        assert!((half.level - 0.5).abs() < 0.001);

        let off = adapter
            .resolve_write(
                endpoint,
                MatterPlaybackWrite {
                    on: Some(false),
                    level: Some(254),
                },
            )
            .unwrap();
        assert_eq!(off.level, 0.0);
        assert_eq!(
            adapter.resolve_write(
                endpoint,
                MatterPlaybackWrite {
                    on: None,
                    level: Some(255)
                }
            ),
            Err(MatterBridgeError::ReservedLevel)
        );
    }
}
