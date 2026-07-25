//! Persisted output-route models and validation.

use crate::{DMX_SLOTS, artnet_broadcast_destination, sacn_multicast_destination};
use light_core::Universe;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    ArtNet,
    Sacn,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryMode {
    Broadcast,
    Multicast,
    Unicast,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OutputRoute {
    pub protocol: Protocol,
    pub logical_universe: Universe,
    pub destination_universe: Universe,
    /// `None` is accepted so legacy routes can be loaded and migrated.
    #[serde(default)]
    pub delivery_mode: Option<DeliveryMode>,
    pub destination: Option<SocketAddr>,
    pub enabled: bool,
    /// Historical routes omitted this and keep full-universe wire compatibility.
    #[serde(default = "legacy_route_minimum_slots")]
    pub minimum_slots: u16,
}

impl OutputRoute {
    pub fn resolved_delivery_mode(&self) -> DeliveryMode {
        self.delivery_mode.unwrap_or_else(|| {
            if self.destination.is_some() {
                DeliveryMode::Unicast
            } else {
                self.protocol.default_delivery_mode()
            }
        })
    }

    pub fn validate(&self) -> Result<(), String> {
        let mode = self.resolved_delivery_mode();
        validate_delivery_mode(self.protocol, mode)?;
        validate_universe(self.protocol, self.destination_universe)?;
        validate_destination(mode, self.destination)?;
        validate_minimum_slots(self.minimum_slots)
    }

    pub fn resolved_destination(&self) -> Result<SocketAddr, String> {
        self.validate()?;
        match self.resolved_delivery_mode() {
            DeliveryMode::Broadcast => Ok(artnet_broadcast_destination()),
            DeliveryMode::Multicast => Ok(sacn_multicast_destination(self.destination_universe)),
            DeliveryMode::Unicast => self
                .destination
                .ok_or_else(|| "Unicast delivery requires a destination".into()),
        }
    }
}

impl Protocol {
    fn default_delivery_mode(self) -> DeliveryMode {
        match self {
            Self::ArtNet => DeliveryMode::Broadcast,
            Self::Sacn => DeliveryMode::Multicast,
        }
    }

    fn maximum_universe(self) -> Universe {
        match self {
            Self::ArtNet => 32_767,
            Self::Sacn => 63_999,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::ArtNet => "Art-Net",
            Self::Sacn => "sACN",
        }
    }
}

fn validate_delivery_mode(protocol: Protocol, mode: DeliveryMode) -> Result<(), String> {
    match (protocol, mode) {
        (Protocol::ArtNet, DeliveryMode::Broadcast | DeliveryMode::Unicast)
        | (Protocol::Sacn, DeliveryMode::Multicast | DeliveryMode::Unicast) => Ok(()),
        (Protocol::ArtNet, DeliveryMode::Multicast) => {
            Err("Art-Net supports Broadcast or Unicast delivery, not Multicast".into())
        }
        (Protocol::Sacn, DeliveryMode::Broadcast) => {
            Err("sACN supports Multicast or Unicast delivery, not Broadcast".into())
        }
    }
}

fn validate_universe(protocol: Protocol, universe: Universe) -> Result<(), String> {
    let maximum = protocol.maximum_universe();
    if universe == 0 || universe > maximum {
        return Err(format!(
            "{} destination universe must be from 1 to {maximum}",
            protocol.label()
        ));
    }
    Ok(())
}

fn validate_destination(mode: DeliveryMode, destination: Option<SocketAddr>) -> Result<(), String> {
    match (mode, destination) {
        (DeliveryMode::Unicast, None) => {
            Err("Unicast delivery requires a destination IPv4 address and port".into())
        }
        (DeliveryMode::Unicast, Some(destination)) if !destination.is_ipv4() => {
            Err("Unicast output currently requires an IPv4 destination".into())
        }
        (DeliveryMode::Unicast, Some(destination)) if destination.port() == 0 => {
            Err("Unicast destination port must be from 1 to 65535".into())
        }
        (DeliveryMode::Broadcast | DeliveryMode::Multicast, Some(_)) => {
            Err("Only Unicast delivery accepts an explicit destination".into())
        }
        _ => Ok(()),
    }
}

fn validate_minimum_slots(minimum_slots: u16) -> Result<(), String> {
    if !(1..=DMX_SLOTS as u16).contains(&minimum_slots) {
        return Err("minimum universe size must be from 1 to 512".into());
    }
    Ok(())
}

const fn legacy_route_minimum_slots() -> u16 {
    DMX_SLOTS as u16
}
