use super::{EncodedPacket, encode_routes, next_sequence};
use crate::{DMX_SLOTS, DeliveryMode, DmxFrame, OutputRoute, Protocol, sacn_data_packet};
use light_core::Universe;
use light_dmx_wire::{
    ARTNET_PORT, SACN_DISCOVERY_UNIVERSE, artpollreply_packets, is_artpoll, sacn_discovery_packets,
    sacn_multicast_destination,
};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    io,
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket as StdUdpSocket},
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::net::UdpSocket;

/// E1.31 announces a source's universes every ten seconds.
const SACN_DISCOVERY_INTERVAL: Duration = Duration::from_secs(10);
/// Polls answered per listener per output frame, so a flood cannot stall the frame.
const POLLS_PER_FRAME: usize = 16;
const LONG_NAME: &str = "ToskLight lighting desk";

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
    /// Where controllers' ArtPolls arrive: one socket per lighting network, bound to its broadcast
    /// address so unicast Art-Net meant for another receiver on this computer never lands here.
    poll_listeners: Vec<PollListener>,
    announcements: Mutex<Announcements>,
}

struct PollListener {
    socket: StdUdpSocket,
    /// The desk's own address on that network, as its replies name it.
    address: Ipv4Addr,
}

#[derive(Default)]
struct Announcements {
    sacn_universes: Vec<Universe>,
    sacn_announced_at: Option<Instant>,
    sacn_destination: Option<SocketAddr>,
    replies: u16,
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
            poll_listeners: broadcast_poll_listeners(bind_ip),
            announcements: Mutex::new(Announcements::default()),
        })
    }

    /// Also answer ArtPolls arriving at `bind`, replying as `address`.
    ///
    /// Test-bench seam: loopback has no broadcast address to listen on.
    pub fn listen_for_art_polls(mut self, bind: SocketAddr, address: Ipv4Addr) -> io::Result<Self> {
        self.poll_listeners.push(poll_listener(bind, address)?);
        Ok(self)
    }

    /// The addresses ArtPolls are answered on.
    pub fn art_poll_addresses(&self) -> Vec<SocketAddr> {
        self.poll_listeners
            .iter()
            .filter_map(|listener| listener.socket.local_addr().ok())
            .collect()
    }

    /// Send sACN universe discovery to `destination` instead of its multicast group.
    ///
    /// Test-bench seam, like [`Self::inject_failure`].
    pub fn redirect_sacn_discovery(&self, destination: SocketAddr) {
        self.announcements
            .lock()
            .expect("output announcement mutex poisoned")
            .sacn_destination = Some(destination);
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
        let sent = self.send_packets(&packets).await;
        self.announce(routes).await;
        sent
    }

    pub async fn terminate_routes(
        &self,
        routes: &[OutputRoute],
        sequences: &mut HashMap<(Protocol, Universe), u8>,
    ) -> io::Result<()> {
        for route in routes
            .iter()
            .filter(|route| route.enabled && route.target.is_network())
        {
            if route.protocol == Protocol::Sacn {
                self.terminate_sacn_route(route, sequences).await?;
            }
        }
        Ok(())
    }

    /// Make the desk findable on the network: answer ArtPolls, and announce its sACN universes.
    ///
    /// Announcing never fails the frame: a lost announcement is repeated by the next poll or the
    /// next interval, while a failed frame is lost light.
    async fn announce(&self, routes: &[OutputRoute]) {
        self.answer_art_polls(&sent_universes(routes, Protocol::ArtNet))
            .await;
        self.announce_sacn(sent_universes(routes, Protocol::Sacn))
            .await;
    }

    async fn answer_art_polls(&self, universes: &[Universe]) {
        let mut buffer = [0_u8; 512];
        for listener in &self.poll_listeners {
            for _ in 0..POLLS_PER_FRAME {
                let Ok((length, from)) = listener.socket.recv_from(&mut buffer) else {
                    break;
                };
                if !is_artpoll(&buffer[..length]) {
                    continue;
                }
                let report = {
                    let mut announcements = self
                        .announcements
                        .lock()
                        .expect("output announcement mutex poisoned");
                    announcements.replies = announcements.replies.wrapping_add(1) % 10_000;
                    format!("#0001 [{:04}] Output running", announcements.replies)
                };
                let replies = artpollreply_packets(
                    listener.address,
                    &self.source_name,
                    LONG_NAME,
                    &report,
                    universes,
                );
                // Replies belong on port 6454; a poller asking from another port hears it there too.
                let mut destinations = vec![SocketAddr::new(from.ip(), ARTNET_PORT)];
                if from.port() != ARTNET_PORT {
                    destinations.push(from);
                }
                // Sent from the Art-Net output socket: a socket bound to a broadcast address cannot
                // name itself as a unicast sender.
                for destination in destinations {
                    for reply in &replies {
                        let _ = self.artnet.send_to(reply, destination).await;
                    }
                }
            }
        }
    }

    async fn announce_sacn(&self, universes: Vec<Universe>) {
        let destination = {
            let mut announcements = self
                .announcements
                .lock()
                .expect("output announcement mutex poisoned");
            let changed = announcements.sacn_universes != universes;
            let due = announcements
                .sacn_announced_at
                .is_none_or(|at| at.elapsed() >= SACN_DISCOVERY_INTERVAL);
            if !changed && !due {
                return;
            }
            announcements.sacn_universes.clone_from(&universes);
            announcements.sacn_announced_at = Some(Instant::now());
            if universes.is_empty() {
                return;
            }
            announcements
                .sacn_destination
                .unwrap_or_else(|| sacn_multicast_destination(SACN_DISCOVERY_UNIVERSE))
        };
        for packet in sacn_discovery_packets(self.cid, &self.source_name, &universes) {
            let _ = self.sacn.send_to(&packet, destination).await;
        }
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

/// The destination universes enabled network routes send on `protocol`, each once, in order.
fn sent_universes(routes: &[OutputRoute], protocol: Protocol) -> Vec<Universe> {
    let mut universes: Vec<Universe> = routes
        .iter()
        .filter(|route| route.enabled && route.target.is_network() && route.protocol == protocol)
        .map(|route| route.destination_universe)
        .collect();
    universes.sort_unstable();
    universes.dedup();
    universes
}

/// A poll listener on the broadcast address of every network the output may send on.
///
/// A socket bound to the wildcard address would share port 6454 with a Visualizer on this
/// computer, and the system may hand it the unicast frames meant for that Visualizer. Bound to a
/// broadcast address it hears only broadcast, which is how ArtPolls are sent.
fn broadcast_poll_listeners(bind_ip: IpAddr) -> Vec<PollListener> {
    let Ok(interfaces) = if_addrs::get_if_addrs() else {
        return Vec::new();
    };
    interfaces
        .into_iter()
        .filter_map(|interface| match interface.addr {
            if_addrs::IfAddr::V4(address)
                if !address.ip.is_loopback()
                    && (bind_ip.is_unspecified() || bind_ip == IpAddr::V4(address.ip)) =>
            {
                let broadcast = address.broadcast?;
                poll_listener(SocketAddr::from((broadcast, ARTNET_PORT)), address.ip).ok()
            }
            _ => None,
        })
        .collect()
}

fn poll_listener(bind: SocketAddr, address: Ipv4Addr) -> io::Result<PollListener> {
    let socket = socket2::Socket::new(
        socket2::Domain::IPV4,
        socket2::Type::DGRAM,
        Some(socket2::Protocol::UDP),
    )?;
    socket.set_reuse_address(true)?;
    #[cfg(unix)]
    socket.set_reuse_port(true)?;
    socket.set_nonblocking(true)?;
    socket.bind(&bind.into())?;
    Ok(PollListener {
        socket: socket.into(),
        address,
    })
}

fn route_diagnostic(route: &OutputRoute) -> Option<RouteDiagnostic> {
    if !route.target.is_network() {
        return None;
    }
    Some(RouteDiagnostic {
        protocol: route.protocol,
        universe: route.destination_universe,
        delivery_mode: route.resolved_delivery_mode(),
        destination: route.resolved_destination().ok()?,
        enabled: route.enabled,
    })
}
