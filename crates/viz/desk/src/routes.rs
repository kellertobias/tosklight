//! Input mappings derived from the show's output routes.
//!
//! The desk configures where it sends DMX; the visualizer listens on exactly those destinations
//! rather than guessing from whatever is on the network.

use crate::wire::{ObjectRecord, OutputRouteBody};
use std::net::{Ipv4Addr, SocketAddr};
use viz_dmx::{Delivery, InputMapping, Protocol};

/// Convert stored output routes into receiver mappings.
///
/// `bind_interface` is the address the receiver binds to; `None` binds every interface.
pub fn mappings(routes: &[ObjectRecord], bind_interface: Option<Ipv4Addr>) -> Vec<InputMapping> {
    let mut mappings = Vec::new();
    for (index, record) in routes.iter().enumerate() {
        let Ok(route) = serde_json::from_value::<OutputRouteBody>(record.body.clone()) else {
            continue;
        };
        let Some(protocol) = protocol(&route.protocol) else {
            continue;
        };
        let delivery = delivery(route.delivery_mode.as_deref(), &route.destination, protocol);
        let port = destination_port(&route.destination).unwrap_or(protocol.default_port());
        mappings.push(InputMapping {
            id: record.id.clone(),
            protocol,
            logical_universe: route.logical_universe,
            destination_universe: route.destination_universe,
            delivery,
            bind: SocketAddr::from((bind_interface.unwrap_or(Ipv4Addr::UNSPECIFIED), port)),
            // Later routes are declared later by the operator; keep an explicit, stable order so
            // duplicate logical universes fail over deterministically.
            priority: 200_u16.saturating_sub(index as u16).min(255) as u8,
            enabled: route.enabled,
        });
    }
    mappings
}

fn protocol(value: &str) -> Option<Protocol> {
    match value {
        "art_net" | "artnet" => Some(Protocol::ArtNet),
        "sacn" | "s_acn" => Some(Protocol::Sacn),
        _ => None,
    }
}

fn delivery(mode: Option<&str>, destination: &Option<String>, protocol: Protocol) -> Delivery {
    match mode {
        Some("broadcast") => Delivery::Broadcast,
        Some("multicast") => Delivery::Multicast,
        Some("unicast") => Delivery::Unicast,
        _ if destination.is_some() => Delivery::Unicast,
        _ => match protocol {
            Protocol::ArtNet => Delivery::Broadcast,
            Protocol::Sacn => Delivery::Multicast,
        },
    }
}

fn destination_port(destination: &Option<String>) -> Option<u16> {
    destination
        .as_deref()?
        .parse::<SocketAddr>()
        .ok()
        .map(|address| address.port())
}

/// When the show configures no output routes, listen on the portable same-host defaults for both
/// protocols on the universes the show actually uses. This is what makes the first launch work
/// without any network configuration.
pub fn default_mappings(universes: &[u16], bind_interface: Option<Ipv4Addr>) -> Vec<InputMapping> {
    let mut mappings = Vec::with_capacity(universes.len() * 2);
    for universe in universes {
        for protocol in [Protocol::ArtNet, Protocol::Sacn] {
            mappings.push(InputMapping {
                id: format!("default-{}-u{universe}", protocol.label().to_lowercase()),
                protocol,
                logical_universe: *universe,
                destination_universe: *universe,
                delivery: match protocol {
                    Protocol::ArtNet => Delivery::Broadcast,
                    Protocol::Sacn => Delivery::Multicast,
                },
                bind: SocketAddr::from((
                    bind_interface.unwrap_or(Ipv4Addr::UNSPECIFIED),
                    protocol.default_port(),
                )),
                priority: 100,
                enabled: true,
            });
        }
    }
    mappings
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn record(id: &str, body: serde_json::Value) -> ObjectRecord {
        ObjectRecord {
            id: id.to_owned(),
            revision: 1,
            body,
        }
    }

    #[test]
    fn art_net_and_sacn_routes_become_mappings_on_their_own_ports() {
        let routes = vec![
            record(
                "a",
                json!({"protocol":"art_net","logical_universe":1,"destination_universe":1,
                       "delivery_mode":"unicast","destination":"127.0.0.1:6454"}),
            ),
            record(
                "b",
                json!({"protocol":"sacn","logical_universe":2,"destination_universe":2,
                       "delivery_mode":"unicast","destination":"127.0.0.1:5568"}),
            ),
        ];
        let mappings = mappings(&routes, None);
        assert_eq!(mappings.len(), 2);
        assert_eq!(mappings[0].protocol, Protocol::ArtNet);
        assert_eq!(mappings[0].bind.port(), 6454);
        assert_eq!(mappings[0].delivery, Delivery::Unicast);
        assert_eq!(mappings[1].protocol, Protocol::Sacn);
        assert_eq!(mappings[1].bind.port(), 5568);
    }

    #[test]
    fn duplicate_logical_universes_get_distinct_priorities() {
        let routes = vec![
            record(
                "first",
                json!({"protocol":"art_net","logical_universe":1,"destination_universe":1}),
            ),
            record(
                "second",
                json!({"protocol":"sacn","logical_universe":1,"destination_universe":1}),
            ),
        ];
        let mappings = mappings(&routes, None);
        assert_eq!(mappings[0].logical_universe, mappings[1].logical_universe);
        assert!(
            mappings[0].priority > mappings[1].priority,
            "the first declared route must win"
        );
    }

    #[test]
    fn a_disabled_route_is_carried_through_as_disabled_rather_than_dropped() {
        let routes = vec![record(
            "a",
            json!({"protocol":"art_net","logical_universe":1,"destination_universe":1,
                   "enabled":false}),
        )];
        let mappings = mappings(&routes, None);
        assert_eq!(mappings.len(), 1);
        assert!(!mappings[0].enabled);
    }

    #[test]
    fn an_unknown_protocol_is_skipped_without_failing_the_snapshot() {
        let routes = vec![record(
            "a",
            json!({"protocol":"future","logical_universe":1}),
        )];
        assert!(mappings(&routes, None).is_empty());
    }

    #[test]
    fn the_default_mappings_cover_both_protocols_for_every_used_universe() {
        let mappings = default_mappings(&[1, 2], None);
        assert_eq!(mappings.len(), 4);
        assert!(
            mappings
                .iter()
                .any(|mapping| mapping.protocol == Protocol::ArtNet)
        );
        assert!(
            mappings
                .iter()
                .any(|mapping| mapping.protocol == Protocol::Sacn)
        );
    }
}
