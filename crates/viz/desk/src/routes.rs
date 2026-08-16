//! Input mappings derived from the show's output routes.
//!
//! The desk configures where it sends DMX; the visualizer listens on exactly those destinations
//! rather than guessing from whatever is on the network.

use crate::wire::{ObjectRecord, OutputRouteBody};
use std::net::{Ipv4Addr, SocketAddr};
use viz_dmx::{Delivery, InputMapping, Protocol};
use viz_document::LiveDmxInputs;

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

/// Apply the planning document's explicit receiver configuration over routes derived from output.
///
/// One configured logical universe replaces every derived route for that universe. Local renderer
/// overrides are applied later and therefore remain the final authority.
pub fn apply_document_inputs(
    mut derived: Vec<InputMapping>,
    records: &[ObjectRecord],
    bind_interface: Option<Ipv4Addr>,
) -> (Vec<InputMapping>, Vec<String>) {
    let Some(record) = records.iter().find(|record| record.id == "main") else {
        return (derived, Vec::new());
    };
    let inputs = match serde_json::from_value::<LiveDmxInputs>(record.body.clone()) {
        Ok(inputs) => inputs,
        Err(error) => {
            return (
                derived,
                vec![format!(
                    "The show's Live DMX Inputs are unreadable: {error}"
                )],
            );
        }
    };
    if let Err(error) = inputs.validate() {
        return (
            derived,
            vec![format!("The show's Live DMX Inputs are invalid: {error}")],
        );
    }
    let configured: std::collections::HashSet<u16> = inputs
        .mappings
        .iter()
        .map(|mapping| mapping.logical_universe)
        .collect();
    derived.retain(|mapping| !configured.contains(&mapping.logical_universe));
    derived.extend(inputs.mappings.into_iter().filter_map(|mapping| {
        let protocol = Protocol::from_wire(&mapping.protocol)?;
        let delivery = match mapping.delivery.as_str() {
            "broadcast" => Delivery::Broadcast,
            "multicast" => Delivery::Multicast,
            "unicast" => Delivery::Unicast,
            _ => return None,
        };
        Some(InputMapping {
            id: format!("document-{}", mapping.id),
            protocol,
            logical_universe: mapping.logical_universe,
            destination_universe: mapping.destination_universe,
            delivery,
            bind: SocketAddr::from((
                bind_interface.unwrap_or(Ipv4Addr::UNSPECIFIED),
                mapping.port,
            )),
            priority: 240,
            enabled: mapping.enabled,
        })
    }));
    (derived, Vec::new())
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

    #[test]
    fn document_inputs_replace_only_their_logical_universe() {
        let routes = mappings(
            &[
                record(
                    "u1",
                    json!({"protocol":"art_net","logical_universe":1,"destination_universe":1}),
                ),
                record(
                    "u2",
                    json!({"protocol":"art_net","logical_universe":2,"destination_universe":2}),
                ),
            ],
            None,
        );
        let input = record(
            "main",
            json!({"schemaVersion":1,"mappings":[{
                "id":"input-u2","logicalUniverse":2,"protocol":"sacn",
                "destinationUniverse":22,"port":5568,"enabled":true,"delivery":"multicast"
            }]}),
        );
        let (resolved, warnings) = apply_document_inputs(routes, &[input], None);
        assert!(warnings.is_empty());
        assert!(
            resolved.iter().any(
                |mapping| mapping.logical_universe == 1 && mapping.protocol == Protocol::ArtNet
            )
        );
        assert!(resolved.iter().any(|mapping| mapping.logical_universe == 2
            && mapping.protocol == Protocol::Sacn
            && mapping.destination_universe == 22));
        assert!(
            !resolved.iter().any(
                |mapping| mapping.logical_universe == 2 && mapping.protocol == Protocol::ArtNet
            )
        );
    }
}
