use super::{EncodedPacket, encode_routes, next_sequence};
use crate::{DMX_SLOTS, DeliveryMode, DmxFrame, OutputRoute, Protocol, sacn_data_packet};
use light_core::Universe;
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    io,
    net::{IpAddr, SocketAddr},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::net::UdpSocket;

/// Shared UDP transport for a dynamically reloadable set of show routes.
pub struct NetworkOutput {
    artnet: UdpSocket,
    sacn: UdpSocket,
    cid: [u8; 16],
    source_name: String,
    sacn_priority: u8,
    injected_failures: Mutex<HashSet<SocketAddr>>,
    send_errors: AtomicU64,
    route_send_errors: Mutex<HashMap<(Protocol, Universe, SocketAddr), u64>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RouteSendError {
    pub protocol: Protocol,
    pub universe: Universe,
    pub destination: SocketAddr,
    pub errors: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct RouteDiagnostic {
    pub protocol: Protocol,
    pub universe: Universe,
    pub delivery_mode: DeliveryMode,
    pub destination: SocketAddr,
    pub enabled: bool,
}

impl NetworkOutput {
    pub async fn bind(
        bind_ip: IpAddr,
        cid: [u8; 16],
        source_name: impl Into<String>,
    ) -> io::Result<Self> {
        let artnet = UdpSocket::bind(SocketAddr::new(bind_ip, 0)).await?;
        artnet.set_broadcast(true)?;
        Ok(Self {
            artnet,
            sacn: UdpSocket::bind(SocketAddr::new(bind_ip, 0)).await?,
            cid,
            source_name: source_name.into(),
            sacn_priority: 100,
            injected_failures: Mutex::new(HashSet::new()),
            send_errors: AtomicU64::new(0),
            route_send_errors: Mutex::new(HashMap::new()),
        })
    }

    /// Test-bench seam for deterministic route-scoped send failures.
    pub fn inject_failure(&self, destination: SocketAddr, enabled: bool) {
        let mut failures = self
            .injected_failures
            .lock()
            .expect("output failure mutex poisoned");
        if enabled {
            failures.insert(destination);
        } else {
            failures.remove(&destination);
        }
    }

    pub fn take_send_errors(&self) -> u64 {
        self.send_errors.swap(0, Ordering::Relaxed)
    }

    pub fn route_send_errors(&self) -> Vec<RouteSendError> {
        let mut errors = self.route_error_snapshot();
        errors.sort_by_key(|error| (error.protocol as u8, error.universe, error.destination));
        errors
    }

    pub fn route_diagnostics(routes: &[OutputRoute]) -> Vec<RouteDiagnostic> {
        routes.iter().filter_map(route_diagnostic).collect()
    }

    pub async fn send_routes(
        &self,
        routes: &[OutputRoute],
        frames: &HashMap<Universe, DmxFrame>,
        patched_slots: &HashMap<Universe, u16>,
        sequences: &mut HashMap<(Protocol, Universe), u8>,
    ) -> io::Result<u64> {
        let packets = encode_routes(
            routes,
            frames,
            patched_slots,
            sequences,
            self.cid,
            &self.source_name,
            self.sacn_priority,
        )?;
        self.send_packets(&packets).await
    }

    pub async fn terminate_routes(
        &self,
        routes: &[OutputRoute],
        sequences: &mut HashMap<(Protocol, Universe), u8>,
    ) -> io::Result<()> {
        for route in routes.iter().filter(|route| route.enabled) {
            if route.protocol == Protocol::Sacn {
                self.terminate_sacn_route(route, sequences).await?;
            }
        }
        Ok(())
    }

    async fn send_packets(&self, packets: &[EncodedPacket]) -> io::Result<u64> {
        let mut outcome = SendOutcome::default();
        for packet in packets {
            match self.send_packet(packet).await {
                Ok(()) => outcome.sent += 1,
                Err(error) => {
                    self.record_send_error(packet);
                    outcome.record_error(error);
                }
            }
        }
        outcome.finish()
    }

    async fn send_packet(&self, packet: &EncodedPacket) -> io::Result<()> {
        if self.failure_is_injected(packet.destination) {
            return Err(io::Error::other(format!(
                "injected output failure for {}",
                packet.destination
            )));
        }
        match packet.protocol {
            Protocol::ArtNet => {
                self.artnet
                    .send_to(&packet.bytes, packet.destination)
                    .await?
            }
            Protocol::Sacn => self.sacn.send_to(&packet.bytes, packet.destination).await?,
        };
        Ok(())
    }

    async fn terminate_sacn_route(
        &self,
        route: &OutputRoute,
        sequences: &mut HashMap<(Protocol, Universe), u8>,
    ) -> io::Result<()> {
        let key = (route.protocol, route.destination_universe);
        let sequence = next_sequence(sequences, key);
        let destination = route.resolved_destination().map_err(io::Error::other)?;
        let packet = self.termination_packet(route.destination_universe, sequence);
        for _ in 0..3 {
            self.sacn.send_to(&packet, destination).await?;
        }
        Ok(())
    }

    fn termination_packet(&self, universe: Universe, sequence: u8) -> Vec<u8> {
        sacn_data_packet(
            universe,
            sequence,
            &[0; DMX_SLOTS],
            self.cid,
            &self.source_name,
            self.sacn_priority,
            true,
        )
    }

    fn failure_is_injected(&self, destination: SocketAddr) -> bool {
        self.injected_failures
            .lock()
            .expect("output failure mutex poisoned")
            .contains(&destination)
    }

    fn record_send_error(&self, packet: &EncodedPacket) {
        self.send_errors.fetch_add(1, Ordering::Relaxed);
        *self
            .route_send_errors
            .lock()
            .expect("route output failure mutex poisoned")
            .entry((packet.protocol, packet.universe, packet.destination))
            .or_default() += 1;
    }

    fn route_error_snapshot(&self) -> Vec<RouteSendError> {
        self.route_send_errors
            .lock()
            .expect("route output failure mutex poisoned")
            .iter()
            .map(
                |(&(protocol, universe, destination), &errors)| RouteSendError {
                    protocol,
                    universe,
                    destination,
                    errors,
                },
            )
            .collect()
    }
}

#[derive(Default)]
struct SendOutcome {
    sent: u64,
    first_error: Option<io::Error>,
}

impl SendOutcome {
    fn record_error(&mut self, error: io::Error) {
        if self.first_error.is_none() {
            self.first_error = Some(error);
        }
    }

    fn finish(self) -> io::Result<u64> {
        match (self.sent, self.first_error) {
            (0, Some(error)) => Err(error),
            (sent, _) => Ok(sent),
        }
    }
}

fn route_diagnostic(route: &OutputRoute) -> Option<RouteDiagnostic> {
    Some(RouteDiagnostic {
        protocol: route.protocol,
        universe: route.destination_universe,
        delivery_mode: route.resolved_delivery_mode(),
        destination: route.resolved_destination().ok()?,
        enabled: route.enabled,
    })
}
