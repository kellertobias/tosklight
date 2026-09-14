//! Every Art-Net node and sACN source on the network, for the DMX screen's Sources tab.
//!
//! Discovery polls and listens on the interface this machine chose for each protocol — the same
//! choice the Values tab and the Visualizer receive on — and only while the Sources tab asks.

use crate::dmx_input::{listening_mappings, renderer_interfaces};
use crate::session::Session;
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::BTreeSet;
use viz_dmx::{
    ArtNetPort, Delivery, DiscoveryPlan, ListenInterfaces, NetworkInterface, PortDirection,
    Protocol, SourceDiscovery, network_interfaces,
};

/// The discovery the Sources tab is reading, if it is open.
#[derive(Default)]
pub struct NetworkSourcesMonitor {
    discovery: Mutex<Option<SourceDiscovery>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkSources {
    pub nodes: Vec<SourceNode>,
    /// The broadcast addresses Art-Net polls go to.
    pub polling: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceNode {
    pub address: String,
    pub name: String,
    pub long_name: String,
    pub report: String,
    pub mac: Option<String>,
    pub protocols: Vec<&'static str>,
    pub inputs: Vec<NodePort>,
    pub outputs: Vec<NodePort>,
    pub sends: Vec<SentUniverse>,
    pub last_seen_millis: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodePort {
    pub label: String,
    pub universe: u16,
    pub kind: &'static str,
    pub active: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SentUniverse {
    pub protocol: &'static str,
    pub universe: u16,
    pub announced: bool,
    pub live: bool,
}

/// Look for nodes and report what was found. The first call starts polling.
#[tauri::command]
pub fn network_sources(
    session: tauri::State<'_, Session>,
    monitor: tauri::State<'_, NetworkSourcesMonitor>,
) -> NetworkSources {
    let (plan, warnings) = discovery_plan(&session);
    let mut current = monitor.discovery.lock();
    if current
        .as_ref()
        .is_none_or(|discovery| discovery.plan() != &plan)
    {
        if let Some(mut previous) = current.take() {
            // Close the old sockets before the new ones bind to the same ports.
            previous.shutdown();
        }
        *current = Some(SourceDiscovery::start(plan));
    }
    report(
        current.as_ref().expect("discovery was just started"),
        warnings,
    )
}

/// Stop polling. The Sources tab calls this when it closes.
#[tauri::command]
pub fn stop_network_sources(monitor: tauri::State<'_, NetworkSourcesMonitor>) {
    if let Some(mut discovery) = monitor.discovery.lock().take() {
        discovery.shutdown();
    }
}

fn discovery_plan(session: &Session) -> (DiscoveryPlan, Vec<String>) {
    let choice = renderer_interfaces(session);
    let mut warnings = Vec::new();
    let available = network_interfaces().unwrap_or_else(|error| {
        warnings.push(error);
        Vec::new()
    });
    let art_net = interfaces_for(&choice, Protocol::ArtNet, &available, &mut warnings);
    let sacn = interfaces_for(&choice, Protocol::Sacn, &available, &mut warnings);
    // Without an open show there are no show universes to watch; announcements still arrive.
    let sacn_universes: BTreeSet<u16> = session
        .with(|document| listening_mappings(document, &choice))
        .map(|(mappings, _)| mappings)
        .unwrap_or_default()
        .iter()
        .filter(|mapping| {
            mapping.enabled
                && mapping.protocol == Protocol::Sacn
                && mapping.delivery == Delivery::Multicast
        })
        .map(|mapping| mapping.destination_universe)
        .collect();
    (
        DiscoveryPlan::new(art_net, sacn, sacn_universes.into_iter().collect()),
        warnings,
    )
}

/// The interfaces a protocol is found on: the chosen one, or every one when none is chosen.
fn interfaces_for(
    choice: &ListenInterfaces,
    protocol: Protocol,
    available: &[NetworkInterface],
    warnings: &mut Vec<String>,
) -> Vec<NetworkInterface> {
    let Some(name) = choice.for_protocol(protocol) else {
        return available.to_vec();
    };
    let chosen: Vec<_> = available
        .iter()
        .filter(|interface| interface.name == name)
        .cloned()
        .collect();
    if chosen.is_empty() {
        warnings.push(format!(
            "{label} is received on {name}, which is not connected; no {label} nodes are found until it returns.",
            label = protocol.label(),
        ));
    }
    chosen
}

fn report(discovery: &SourceDiscovery, mut warnings: Vec<String>) -> NetworkSources {
    warnings.extend(discovery.warnings().iter().cloned());
    NetworkSources {
        nodes: discovery
            .nodes()
            .into_iter()
            .map(|node| SourceNode {
                address: node.address.to_string(),
                name: node.names.join(" · "),
                mac: node.mac.map(|mac| {
                    mac.iter()
                        .map(|byte| format!("{byte:02X}"))
                        .collect::<Vec<_>>()
                        .join(":")
                }),
                protocols: [(node.art_net, "artnet"), (node.sacn, "sacn")]
                    .into_iter()
                    .filter_map(|(found, protocol)| found.then_some(protocol))
                    .collect(),
                inputs: ports(&node.ports, PortDirection::Input),
                outputs: ports(&node.ports, PortDirection::Output),
                sends: node
                    .sends
                    .iter()
                    .map(|sent| SentUniverse {
                        protocol: sent.protocol.wire(),
                        universe: sent.universe,
                        announced: sent.announced,
                        live: sent.live,
                    })
                    .collect(),
                last_seen_millis: node.last_seen.as_millis() as u64,
                long_name: node.long_name,
                report: node.report,
            })
            .collect(),
        polling: discovery.polled().iter().map(ToString::to_string).collect(),
        warnings,
    }
}

fn ports(ports: &[ArtNetPort], direction: PortDirection) -> Vec<NodePort> {
    // A node answering with several replies numbers its ports per reply, so both numbers show.
    let bound = ports.iter().any(|port| port.bind_index > 1);
    ports
        .iter()
        .filter(|port| port.direction == direction)
        .map(|port| NodePort {
            label: if bound {
                format!("Port {}.{}", port.bind_index, port.port)
            } else {
                format!("Port {}", port.port)
            },
            universe: port.universe,
            kind: port.kind,
            active: port.active,
        })
        .collect()
}
