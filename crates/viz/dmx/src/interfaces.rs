//! Which network interface Art-Net and sACN are received on.
//!
//! A machine with a lighting network and an office network has to be able to say which one
//! carries DMX. That choice belongs to the machine and never to the show, and it names an
//! interface (`en0`) rather than an address, so a renewed lease with a new address is still the
//! same choice.

use crate::mapping::{InputMapping, Protocol};
use std::net::{IpAddr, Ipv4Addr};

/// One IPv4 address of one network interface on this machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkInterface {
    /// The operating system's name for it, such as `en0` or `eth1`.
    pub name: String,
    pub address: Ipv4Addr,
    pub netmask: Ipv4Addr,
    pub index: Option<u32>,
    pub loopback: bool,
}

/// Every IPv4 interface on this machine, real networks first and loopback last.
pub fn network_interfaces() -> Result<Vec<NetworkInterface>, String> {
    let mut found: Vec<NetworkInterface> = if_addrs::get_if_addrs()
        .map_err(|error| format!("could not list this machine's network interfaces: {error}"))?
        .into_iter()
        .filter_map(|interface| match interface.addr {
            if_addrs::IfAddr::V4(address) => Some(NetworkInterface {
                loopback: address.ip.is_loopback(),
                name: interface.name,
                address: address.ip,
                netmask: address.netmask,
                index: interface.index,
            }),
            if_addrs::IfAddr::V6(_) => None,
        })
        .collect();
    found.sort_by(|left, right| {
        (left.loopback, &left.name, left.address).cmp(&(right.loopback, &right.name, right.address))
    });
    Ok(found)
}

/// The interface each protocol is received on. `None` receives it on every interface.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ListenInterfaces {
    pub art_net: Option<String>,
    pub sacn: Option<String>,
}

impl ListenInterfaces {
    pub fn for_protocol(&self, protocol: Protocol) -> Option<&str> {
        match protocol {
            Protocol::ArtNet => self.art_net.as_deref(),
            Protocol::Sacn => self.sacn.as_deref(),
        }
        .map(str::trim)
        .filter(|name| !name.is_empty())
    }

    /// Where input is received, as a status line says it.
    pub fn describe(&self) -> String {
        let art_net = self.for_protocol(Protocol::ArtNet);
        let sacn = self.for_protocol(Protocol::Sacn);
        if art_net.is_none() && sacn.is_none() {
            return "all interfaces".into();
        }
        format!(
            "Art-Net on {}, sACN on {}",
            art_net.unwrap_or("all interfaces"),
            sacn.unwrap_or("all interfaces")
        )
    }
}

/// Receive each protocol only on the interface the operator chose for it.
///
/// Only mappings that listen on every interface are narrowed; one that already names an address
/// keeps it. A chosen interface that is not on this machine now receives nothing rather than
/// quietly falling back to every interface — the operator asked for one network — and says so.
pub fn listen_on(
    mut mappings: Vec<InputMapping>,
    choice: &ListenInterfaces,
    available: &[NetworkInterface],
) -> (Vec<InputMapping>, Vec<String>) {
    let mut warnings = Vec::new();
    for protocol in [Protocol::ArtNet, Protocol::Sacn] {
        let Some(name) = choice.for_protocol(protocol) else {
            continue;
        };
        let address = available
            .iter()
            .find(|interface| interface.name == name)
            .map(|interface| interface.address);
        let mut narrowed = false;
        for mapping in mappings
            .iter_mut()
            .filter(|mapping| mapping.protocol == protocol && mapping.bind.ip().is_unspecified())
        {
            match address {
                Some(address) => mapping.bind.set_ip(IpAddr::V4(address)),
                None if mapping.enabled => {
                    mapping.enabled = false;
                    narrowed = true;
                }
                None => {}
            }
        }
        if narrowed {
            warnings.push(format!(
                "{label} input listens on {name}, which has no IPv4 address on this machine; \
                 no {label} is received until it returns.",
                label = protocol.label(),
            ));
        }
    }
    (mappings, warnings)
}

/// [`listen_on`] against the interfaces this machine has now. Choosing none changes nothing.
pub fn listen_on_this_machine(
    mappings: Vec<InputMapping>,
    choice: &ListenInterfaces,
) -> (Vec<InputMapping>, Vec<String>) {
    if choice == &ListenInterfaces::default() {
        return (mappings, Vec::new());
    }
    match network_interfaces() {
        Ok(available) => listen_on(mappings, choice, &available),
        Err(error) => {
            let (mappings, mut warnings) = listen_on(mappings, choice, &[]);
            warnings.insert(0, error);
            (mappings, warnings)
        }
    }
}

/// The interface index that owns `address`, for pinning a socket to it.
pub(crate) fn index_of(address: Ipv4Addr) -> Option<u32> {
    network_interfaces()
        .ok()?
        .into_iter()
        .find(|interface| interface.address == address)
        .and_then(|interface| interface.index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mapping::Delivery;
    use std::net::SocketAddr;

    fn interface(name: &str, address: [u8; 4]) -> NetworkInterface {
        NetworkInterface {
            name: name.into(),
            address: Ipv4Addr::from(address),
            netmask: Ipv4Addr::new(255, 255, 255, 0),
            index: Some(4),
            loopback: false,
        }
    }

    fn mapping(protocol: Protocol, bind: Ipv4Addr) -> InputMapping {
        InputMapping {
            id: format!("{}-{bind}", protocol.wire()),
            protocol,
            logical_universe: 1,
            destination_universe: 1,
            delivery: Delivery::Broadcast,
            bind: SocketAddr::from((bind, protocol.default_port())),
            priority: 100,
            enabled: true,
        }
    }

    #[test]
    fn each_protocol_listens_on_its_own_interface() {
        let (mappings, warnings) = listen_on(
            vec![
                mapping(Protocol::ArtNet, Ipv4Addr::UNSPECIFIED),
                mapping(Protocol::Sacn, Ipv4Addr::UNSPECIFIED),
            ],
            &ListenInterfaces {
                art_net: Some("en1".into()),
                sacn: Some("en0".into()),
            },
            &[
                interface("en0", [10, 0, 0, 5]),
                interface("en1", [2, 0, 0, 9]),
            ],
        );
        assert!(warnings.is_empty());
        assert_eq!(mappings[0].bind, "2.0.0.9:6454".parse().unwrap());
        assert_eq!(mappings[1].bind, "10.0.0.5:5568".parse().unwrap());
    }

    #[test]
    fn no_choice_keeps_every_interface() {
        let original = vec![mapping(Protocol::ArtNet, Ipv4Addr::UNSPECIFIED)];
        let (mappings, warnings) = listen_on(
            original.clone(),
            &ListenInterfaces {
                art_net: Some("  ".into()),
                sacn: None,
            },
            &[interface("en0", [10, 0, 0, 5])],
        );
        assert_eq!(mappings, original);
        assert!(warnings.is_empty());
    }

    #[test]
    fn a_mapping_that_names_an_address_keeps_it() {
        let (mappings, _) = listen_on(
            vec![mapping(Protocol::ArtNet, Ipv4Addr::LOCALHOST)],
            &ListenInterfaces {
                art_net: Some("en0".into()),
                sacn: None,
            },
            &[interface("en0", [10, 0, 0, 5])],
        );
        assert_eq!(mappings[0].bind.ip(), Ipv4Addr::LOCALHOST);
    }

    #[test]
    fn a_missing_interface_receives_nothing_and_says_so() {
        let (mappings, warnings) = listen_on(
            vec![
                mapping(Protocol::Sacn, Ipv4Addr::UNSPECIFIED),
                mapping(Protocol::ArtNet, Ipv4Addr::UNSPECIFIED),
            ],
            &ListenInterfaces {
                art_net: None,
                sacn: Some("en7".into()),
            },
            &[interface("en0", [10, 0, 0, 5])],
        );
        assert!(!mappings[0].enabled, "sACN is not received anywhere else");
        assert!(mappings[1].enabled, "Art-Net still listens everywhere");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("sACN input listens on en7"));
    }

    #[test]
    fn this_machine_lists_its_loopback_last() {
        let found = network_interfaces().expect("interfaces");
        if let Some(first_loopback) = found.iter().position(|interface| interface.loopback) {
            assert!(
                found[first_loopback..]
                    .iter()
                    .all(|interface| interface.loopback)
            );
        }
    }
}
