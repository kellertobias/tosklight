//! Process-wide network configuration.
//!
//! Listen addresses, destinations, and advertised endpoints stay separate concepts. The Media
//! Server owns its local listen addresses; a Light output route owns its destination. `0.0.0.0`
//! means every local IPv4 interface — it is never a destination.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use serde::{Deserialize, Serialize};

/// Art-Net's fixed UDP port.
pub const ART_NET_PORT: u16 = 6454;
/// E1.31 (sACN)'s fixed UDP port.
pub const SACN_PORT: u16 = 5568;
/// CITP/MSEX 1.2 listens on TCP 4809. One configured port, published in discovery and status;
/// neither product may silently substitute a different one.
pub const CITP_PORT: u16 = 4809;
/// The administration HTTP service, matching the legacy application's port.
pub const HTTP_PORT: u16 = 8080;

/// Explicit IPv4 loopback. Persisted configuration uses the literal address rather than
/// `localhost`, which may resolve to IPv6 `::1` and make same-host behavior ambiguous.
pub const LOOPBACK: Ipv4Addr = Ipv4Addr::new(127, 0, 0, 1);

const UNSPECIFIED: Ipv4Addr = Ipv4Addr::UNSPECIFIED;

/// The operator's network settings, plus the same-computer preset that overlays them.
///
/// The preset never overwrites the stored values. Turning it off restores exactly the LAN
/// configuration the operator had before.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NetworkConfiguration {
    /// When set, every protocol listens on IPv4 loopback and the advertised endpoints report
    /// loopback, without touching the stored addresses below.
    #[serde(default)]
    pub same_computer_preset: bool,
    #[serde(default = "default_art_net_listen")]
    pub art_net_listen: SocketAddr,
    #[serde(default = "default_sacn_listen")]
    pub sacn_listen: SocketAddr,
    #[serde(default = "default_citp_listen")]
    pub citp_listen: SocketAddr,
    #[serde(default = "default_http_listen")]
    pub http_listen: SocketAddr,
    /// Where the Light desk publishes its Speed Group stream, when Media consumes one.
    #[serde(default)]
    pub speed_group_endpoint: Option<SocketAddr>,
}

fn default_art_net_listen() -> SocketAddr {
    SocketAddr::from((UNSPECIFIED, ART_NET_PORT))
}

fn default_sacn_listen() -> SocketAddr {
    SocketAddr::from((UNSPECIFIED, SACN_PORT))
}

fn default_citp_listen() -> SocketAddr {
    SocketAddr::from((UNSPECIFIED, CITP_PORT))
}

fn default_http_listen() -> SocketAddr {
    SocketAddr::from((LOOPBACK, HTTP_PORT))
}

impl Default for NetworkConfiguration {
    fn default() -> Self {
        Self {
            same_computer_preset: false,
            art_net_listen: default_art_net_listen(),
            sacn_listen: default_sacn_listen(),
            citp_listen: default_citp_listen(),
            http_listen: default_http_listen(),
            speed_group_endpoint: None,
        }
    }
}

impl NetworkConfiguration {
    /// The addresses the adapters actually bind, after the same-computer preset is applied.
    ///
    /// Settings and diagnostics show both this and the stored configuration, so an operator can
    /// always see the difference between what they typed and what the process is using.
    pub fn resolved(&self) -> ResolvedNetwork {
        if self.same_computer_preset {
            ResolvedNetwork {
                art_net_listen: loopback(self.art_net_listen),
                sacn_listen: loopback(self.sacn_listen),
                citp_listen: loopback(self.citp_listen),
                http_listen: loopback(self.http_listen),
                speed_group_endpoint: self.speed_group_endpoint.map(loopback),
                citp_advertised_port: self.citp_listen.port(),
            }
        } else {
            ResolvedNetwork {
                art_net_listen: self.art_net_listen,
                sacn_listen: self.sacn_listen,
                citp_listen: self.citp_listen,
                http_listen: self.http_listen,
                speed_group_endpoint: self.speed_group_endpoint,
                citp_advertised_port: self.citp_listen.port(),
            }
        }
    }
}

fn loopback(address: SocketAddr) -> SocketAddr {
    SocketAddr::new(IpAddr::V4(LOOPBACK), address.port())
}

/// What the adapters bind and advertise for this run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedNetwork {
    pub art_net_listen: SocketAddr,
    pub sacn_listen: SocketAddr,
    pub citp_listen: SocketAddr,
    pub http_listen: SocketAddr,
    pub speed_group_endpoint: Option<SocketAddr>,
    /// The port CITP discovery announces. Always the port CITP actually listens on.
    pub citp_advertised_port: u16,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_use_the_fixed_protocol_ports() {
        let resolved = NetworkConfiguration::default().resolved();
        assert_eq!(resolved.art_net_listen.port(), 6454);
        assert_eq!(resolved.sacn_listen.port(), 5568);
        assert_eq!(resolved.citp_listen.port(), 4809);
        assert_eq!(resolved.http_listen.port(), 8080);
        assert_eq!(resolved.citp_advertised_port, 4809);
    }

    #[test]
    fn the_preset_binds_loopback_without_destroying_lan_settings() {
        let lan: SocketAddr = "192.168.1.40:6454".parse().unwrap();
        let mut configuration = NetworkConfiguration {
            art_net_listen: lan,
            ..Default::default()
        };

        configuration.same_computer_preset = true;
        assert_eq!(
            configuration.resolved().art_net_listen,
            "127.0.0.1:6454".parse().unwrap()
        );
        assert_eq!(
            configuration.art_net_listen, lan,
            "stored LAN address must survive the preset"
        );

        configuration.same_computer_preset = false;
        assert_eq!(configuration.resolved().art_net_listen, lan);
    }

    #[test]
    fn the_preset_keeps_operator_chosen_ports() {
        let configuration = NetworkConfiguration {
            same_computer_preset: true,
            http_listen: "0.0.0.0:9090".parse().unwrap(),
            ..Default::default()
        };
        assert_eq!(
            configuration.resolved().http_listen,
            "127.0.0.1:9090".parse().unwrap()
        );
    }

    #[test]
    fn citp_advertises_exactly_the_port_it_listens_on() {
        let configuration = NetworkConfiguration {
            citp_listen: "0.0.0.0:4812".parse().unwrap(),
            ..Default::default()
        };
        assert_eq!(configuration.resolved().citp_advertised_port, 4812);
    }
}
