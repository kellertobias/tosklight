//! Input descriptors. The scene source supplies these explicitly so the renderer never has to
//! guess a mapping from whatever happens to be on the network.

use crate::packet::{ARTNET_PORT, SACN_PORT};
use std::net::{Ipv4Addr, SocketAddr};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Protocol {
    ArtNet,
    Sacn,
}

impl Protocol {
    pub fn label(self) -> &'static str {
        match self {
            Self::ArtNet => "Art-Net",
            Self::Sacn => "sACN",
        }
    }

    pub fn default_port(self) -> u16 {
        match self {
            Self::ArtNet => ARTNET_PORT,
            Self::Sacn => SACN_PORT,
        }
    }

    /// The stable spelling, for anything written down and read back.
    pub fn wire(self) -> &'static str {
        match self {
            Self::ArtNet => "artnet",
            Self::Sacn => "sacn",
        }
    }

    pub fn from_wire(value: &str) -> Option<Self> {
        match value {
            "artnet" => Some(Self::ArtNet),
            "sacn" => Some(Self::Sacn),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Delivery {
    Broadcast,
    Multicast,
    Unicast,
}

impl Delivery {
    pub fn label(self) -> &'static str {
        match self {
            Self::Broadcast => "Broadcast",
            Self::Multicast => "Multicast",
            Self::Unicast => "Unicast",
        }
    }
}

/// One configured input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InputMapping {
    pub id: String,
    pub protocol: Protocol,
    pub logical_universe: u16,
    pub destination_universe: u16,
    pub delivery: Delivery,
    /// Interface address and port this mapping listens on.
    pub bind: SocketAddr,
    /// Higher wins when two mappings carry the same logical universe.
    pub priority: u8,
    pub enabled: bool,
}

/// An operator's statement about where one universe actually arrives, overriding whatever the
/// show's output routes say.
///
/// A visualizer often sits on a different network path from the desk's real output: the show may
/// route universe 3 to a node by multicast while the operator wants to watch it arriving as
/// Art-Net on this machine. The show is not edited to make that work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UniverseInput {
    /// The universe as the patch numbers it.
    pub universe: u16,
    pub protocol: Protocol,
    /// The universe number on the wire, which need not match the patch.
    pub destination_universe: u16,
    pub port: u16,
}

impl UniverseInput {
    /// An override that listens for `universe` on the protocol's own default port.
    pub fn new(universe: u16, protocol: Protocol) -> Self {
        Self {
            universe,
            protocol,
            destination_universe: universe,
            port: protocol.default_port(),
        }
    }
}

/// Replace every mapping for an overridden universe with the operator's own.
///
/// Overriding one universe leaves every other one exactly as the show described it, so an
/// operator can correct a single stubborn universe without taking responsibility for the rest.
pub fn apply_overrides(
    mappings: Vec<InputMapping>,
    overrides: &[UniverseInput],
    bind_interface: Option<std::net::Ipv4Addr>,
) -> Vec<InputMapping> {
    if overrides.is_empty() {
        return mappings;
    }
    let mut result: Vec<InputMapping> = mappings
        .into_iter()
        .filter(|mapping| {
            !overrides
                .iter()
                .any(|input| input.universe == mapping.logical_universe)
        })
        .collect();
    for input in overrides {
        let mut mapping =
            InputMapping::loopback(input.protocol, input.universe, input.destination_universe);
        mapping.id = format!(
            "operator-u{}-{}",
            input.universe,
            match input.protocol {
                Protocol::ArtNet => "artnet",
                Protocol::Sacn => "sacn",
            }
        );
        mapping.bind = SocketAddr::new(
            bind_interface
                .map(std::net::IpAddr::V4)
                .unwrap_or(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED)),
            input.port,
        );
        // An operator's own statement outranks anything derived from the show.
        mapping.priority = 255;
        result.push(mapping);
    }
    result
}

impl InputMapping {
    /// The portable same-host default: explicit loopback unicast, which traverses the operating
    /// system network stack and proves the real encoders and receivers without depending on
    /// broadcast or multicast loopback behaviour.
    pub fn loopback(protocol: Protocol, logical: u16, destination: u16) -> Self {
        Self {
            id: format!(
                "{}-u{destination}-loopback",
                match protocol {
                    Protocol::ArtNet => "artnet",
                    Protocol::Sacn => "sacn",
                }
            ),
            protocol,
            logical_universe: logical,
            destination_universe: destination,
            delivery: Delivery::Unicast,
            bind: SocketAddr::from((Ipv4Addr::UNSPECIFIED, protocol.default_port())),
            priority: 100,
            enabled: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_override_replaces_only_its_own_universe() {
        let mappings = vec![
            InputMapping::loopback(Protocol::ArtNet, 1, 1),
            InputMapping::loopback(Protocol::ArtNet, 2, 2),
        ];
        let result = apply_overrides(mappings, &[UniverseInput::new(2, Protocol::Sacn)], None);
        let one: Vec<_> = result
            .iter()
            .filter(|mapping| mapping.logical_universe == 1)
            .collect();
        let two: Vec<_> = result
            .iter()
            .filter(|mapping| mapping.logical_universe == 2)
            .collect();
        assert_eq!(one.len(), 1, "universe 1 was left alone");
        assert_eq!(one[0].protocol, Protocol::ArtNet);
        assert_eq!(
            two.len(),
            1,
            "universe 2 is carried by the operator's input only"
        );
        assert_eq!(two[0].protocol, Protocol::Sacn);
        assert_eq!(two[0].bind.port(), Protocol::Sacn.default_port());
        assert!(
            two[0].priority > one[0].priority,
            "the operator outranks the show"
        );
    }

    #[test]
    fn no_overrides_leaves_the_show_untouched() {
        let mappings = vec![InputMapping::loopback(Protocol::ArtNet, 4, 9)];
        let result = apply_overrides(mappings.clone(), &[], None);
        assert_eq!(result.len(), mappings.len());
        assert_eq!(result[0].destination_universe, 9);
    }

    #[test]
    fn loopback_defaults_use_each_protocols_standard_port() {
        assert_eq!(
            InputMapping::loopback(Protocol::ArtNet, 1, 1).bind.port(),
            6454
        );
        assert_eq!(
            InputMapping::loopback(Protocol::Sacn, 1, 1).bind.port(),
            5568
        );
    }
}
