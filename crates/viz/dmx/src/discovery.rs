//! Who is on the Art-Net and sACN network, for the Architect's DMX Sources tab.
//!
//! Art-Net nodes answer an ArtPoll with ArtPollReply packets that name their ports and the
//! universe each port carries. sACN has no poll: a source announces the universes it sends in
//! universe-discovery packets every ten seconds, and a receiver announces nothing. Both protocols'
//! data is watched as well, so a sender that neither answers polls nor announces — a desk
//! broadcasting ArtDmx, a source that skips discovery — still appears with what it was seen sending.

use crate::interfaces::NetworkInterface;
use crate::mapping::Protocol;
use crate::packet::{
    ARTNET_PORT, SACN_ACN_IDENTIFIER, SACN_PORT, decode_artdmx, decode_sacn, sacn_multicast_group,
};
use light_dmx_wire::{artpollreply_packets, is_artpoll};
use socket2::{Domain, Protocol as SocketProtocol, Socket, Type};
use std::collections::{BTreeMap, HashMap};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const ARTPOLL: u16 = 0x2000;
const ARTPOLLREPLY: u16 = 0x2100;
/// E1.31 reserves this universe's multicast group for universe discovery.
pub const SACN_DISCOVERY_UNIVERSE: u16 = 64214;

/// How often each network is polled for Art-Net nodes.
const POLL_INTERVAL: Duration = Duration::from_secs(3);
/// A node that missed three polls is gone.
const ART_NET_NODE_TIMEOUT: Duration = Duration::from_secs(10);
/// E1.31 announces every 10 s; two missed announcements mean the source is gone.
const SACN_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(25);
/// Data not seen for this long is no longer being sent, as the receiver's source loss rules.
const DATA_TIMEOUT: Duration = Duration::from_millis(2_500);

/// How the Visualizer names itself when something else polls the network.
///
/// The desk's Nodes view marks an endpoint as ToskLight's own software by matching the long name,
/// so it has to stay exactly the one the shared wire contract lists. Without a reply the
/// Visualizer is only ever seen asking, never answering, and appears as an anonymous poller.
const LONG_NAME: &str = "ToskLight Visualizer";
/// The 17 characters an ArtPollReply's short name holds.
const SHORT_NAME: &str = "Visualizer";
const REPORT: &str = "#0001 [0000] Visualizer receiving";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PortDirection {
    /// A DMX input: the node sends what arrives on it as this universe.
    Input,
    /// A DMX output: the node plays this universe out of it.
    Output,
}

/// One physical port an Art-Net node reports.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtNetPort {
    /// Which reply of a multi-reply node the port belongs to, from 1.
    pub bind_index: u8,
    /// The port within that reply, from 1.
    pub port: u8,
    pub direction: PortDirection,
    /// The 15-bit Art-Net port-address, numbered as the show numbers Art-Net universes.
    pub universe: u16,
    /// What the port carries: `DMX512` for nearly every node.
    pub kind: &'static str,
    /// The node reports data passing through the port now.
    pub active: bool,
}

/// One decoded ArtPollReply.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtPollReply {
    pub address: Ipv4Addr,
    pub bind_index: u8,
    pub short_name: String,
    pub long_name: String,
    pub report: String,
    pub mac: Option<[u8; 6]>,
    pub ports: Vec<ArtNetPort>,
}

/// One page of an sACN source's universe-discovery announcement.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SacnDiscovery {
    pub cid: [u8; 16],
    pub source_name: String,
    pub page: u8,
    pub last_page: u8,
    pub universes: Vec<u16>,
}

/// One universe a node sends.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SentUniverse {
    pub protocol: Protocol,
    pub universe: u16,
    /// The node itself says it sends it: an Art-Net input port or an sACN announcement.
    pub announced: bool,
    /// Data for it from this node is arriving now.
    pub live: bool,
}

/// Everything found at one IP address.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkNode {
    pub address: Ipv4Addr,
    /// Its Art-Net short name and every sACN source name, without repeats.
    pub names: Vec<String>,
    pub long_name: String,
    pub report: String,
    pub mac: Option<[u8; 6]>,
    pub art_net: bool,
    pub sacn: bool,
    pub ports: Vec<ArtNetPort>,
    pub sends: Vec<SentUniverse>,
    /// How long ago anything was last heard from it.
    pub last_seen: Duration,
}

/// The ArtPoll this machine sends to find nodes.
pub fn artpoll() -> [u8; 14] {
    let mut packet = [0_u8; 14];
    packet[..8].copy_from_slice(b"Art-Net\0");
    packet[8..10].copy_from_slice(&ARTPOLL.to_le_bytes());
    packet[10..12].copy_from_slice(&14_u16.to_be_bytes());
    // Ask for a reply whenever a node's conditions change, not only when polled.
    packet[12] = 0x02;
    packet
}

/// Decode an ArtPollReply. Anything else, including a truncated reply, is `None`.
pub fn decode_artpollreply(bytes: &[u8]) -> Option<ArtPollReply> {
    // Through SwOut; the MAC address and bind index after it are later additions.
    if bytes.len() < 194
        || &bytes[..8] != b"Art-Net\0"
        || u16::from_le_bytes([bytes[8], bytes[9]]) != ARTPOLLREPLY
    {
        return None;
    }
    let net = u16::from(bytes[18] & 0x7f) << 8;
    let sub = u16::from(bytes[19] & 0x0f) << 4;
    let bind_index = bytes
        .get(211)
        .copied()
        .filter(|index| *index > 0)
        .unwrap_or(1);
    let mac = bytes
        .get(201..207)
        .and_then(|mac| <[u8; 6]>::try_from(mac).ok())
        .filter(|mac| mac != &[0; 6]);
    let mut ports = Vec::new();
    // NumPorts is unreliable in the field; a port type without direction bits is simply absent.
    for index in 0..4 {
        let types = bytes[174 + index];
        let port = index as u8 + 1;
        let kind = port_kind(types & 0x3f);
        if types & 0x40 != 0 {
            ports.push(ArtNetPort {
                bind_index,
                port,
                direction: PortDirection::Input,
                universe: net | sub | u16::from(bytes[186 + index] & 0x0f),
                kind,
                active: bytes[178 + index] & 0x80 != 0,
            });
        }
        if types & 0x80 != 0 {
            ports.push(ArtNetPort {
                bind_index,
                port,
                direction: PortDirection::Output,
                universe: net | sub | u16::from(bytes[190 + index] & 0x0f),
                kind,
                active: bytes[182 + index] & 0x80 != 0,
            });
        }
    }
    Some(ArtPollReply {
        address: Ipv4Addr::new(bytes[10], bytes[11], bytes[12], bytes[13]),
        bind_index,
        short_name: text(&bytes[26..44]),
        long_name: text(&bytes[44..108]),
        report: text(&bytes[108..172]),
        mac,
        ports,
    })
}

/// Decode an E1.31 universe-discovery packet. Anything else is `None`.
pub fn decode_sacn_discovery(bytes: &[u8]) -> Option<SacnDiscovery> {
    if bytes.len() < 120
        || bytes[0..2] != [0x00, 0x10]
        || bytes[2..4] != [0x00, 0x00]
        || &bytes[4..16] != SACN_ACN_IDENTIFIER
    {
        return None;
    }
    let vector =
        |at: usize| u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    if vector(18) != 0x0000_0008 || vector(40) != 0x0000_0002 || vector(114) != 0x0000_0001 {
        return None;
    }
    let layer = usize::from(u16::from_be_bytes([bytes[112], bytes[113]]) & 0x0fff);
    let end = (112 + layer).min(bytes.len());
    let universes = bytes
        .get(120..end)
        .unwrap_or_default()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_be_bytes(*pair))
        .filter(|universe| (1..=63_999).contains(universe))
        .collect();
    let mut cid = [0_u8; 16];
    cid.copy_from_slice(&bytes[22..38]);
    Some(SacnDiscovery {
        cid,
        source_name: text(&bytes[44..108]),
        page: bytes[118],
        last_page: bytes[119],
        universes,
    })
}

fn port_kind(code: u8) -> &'static str {
    match code {
        0 => "DMX512",
        1 => "MIDI",
        2 => "Avab",
        3 => "Colortran CMX",
        4 => "ADB 62.5",
        5 => "Art-Net",
        6 => "DALI",
        _ => "Other",
    }
}

/// A NUL-padded text field.
fn text(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).trim().to_owned()
}

/// Where discovery polls and listens.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveryPlan {
    /// Networks Art-Net is polled and heard on.
    pub art_net: Vec<NetworkInterface>,
    /// Networks sACN is heard on.
    pub sacn: Vec<NetworkInterface>,
    /// sACN universes whose data is watched besides the discovery announcements, so a source
    /// that never announces — the desk among them — still appears for the show's own universes.
    pub sacn_universes: Vec<u16>,
    pub art_net_port: u16,
    pub sacn_port: u16,
}

impl DiscoveryPlan {
    pub fn new(
        art_net: Vec<NetworkInterface>,
        sacn: Vec<NetworkInterface>,
        sacn_universes: Vec<u16>,
    ) -> Self {
        Self {
            art_net,
            sacn,
            sacn_universes,
            art_net_port: ARTNET_PORT,
            sacn_port: SACN_PORT,
        }
    }
}

/// Polls and listens until shut down. Its ports are shared, so a Visualizer on the same machine
/// keeps receiving beside it.
pub struct SourceDiscovery {
    plan: DiscoveryPlan,
    seen: Arc<Mutex<Seen>>,
    running: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
    polled: Vec<Ipv4Addr>,
    warnings: Vec<String>,
}

impl SourceDiscovery {
    pub fn start(plan: DiscoveryPlan) -> Self {
        let seen = Arc::new(Mutex::new(Seen::default()));
        let running = Arc::new(AtomicBool::new(true));
        let mut warnings = Vec::new();
        let mut threads = Vec::new();
        // Loopback has no broadcast to poll; a node on this machine still answers another poll.
        let mut polled: Vec<Ipv4Addr> = plan
            .art_net
            .iter()
            .filter(|interface| !interface.loopback)
            .map(directed_broadcast)
            .collect();
        polled.sort();
        polled.dedup();

        match listen(plan.art_net_port) {
            Ok(socket) => {
                if let Err(error) = socket.set_broadcast(true) {
                    warnings.push(format!("Art-Net nodes cannot be polled: {error}"));
                }
                let task = Task {
                    socket,
                    seen: seen.clone(),
                    running: running.clone(),
                    networks: plan.art_net.clone(),
                };
                let (targets, port) = (polled.clone(), plan.art_net_port);
                threads.push(spawn("viz-dmx art-net discovery", move || {
                    task.art_net(&targets, port)
                }));
            }
            Err(error) => warnings.push(format!(
                "Art-Net nodes cannot be found: port {} could not be opened: {error}",
                plan.art_net_port
            )),
        }

        match listen(plan.sacn_port) {
            Ok(socket) => {
                let mut failed = 0;
                let mut first_error = None;
                for interface in &plan.sacn {
                    let groups = std::iter::once(SACN_DISCOVERY_UNIVERSE)
                        .chain(plan.sacn_universes.iter().copied())
                        .map(sacn_multicast_group);
                    for group in groups {
                        // Loopback often refuses multicast; unicast to it still arrives.
                        if let Err(error) = socket.join_multicast_v4(&group, &interface.address)
                            && !interface.loopback
                        {
                            failed += 1;
                            first_error.get_or_insert_with(|| {
                                format!("{group} on {}: {error}", interface.name)
                            });
                        }
                    }
                }
                if let Some(error) = first_error {
                    warnings.push(format!(
                        "{failed} sACN multicast group{} could not be joined, so some sources may be missing ({error}).",
                        if failed == 1 { "" } else { "s" }
                    ));
                }
                let _ = socket.set_multicast_loop_v4(true);
                let task = Task {
                    socket,
                    seen: seen.clone(),
                    running: running.clone(),
                    networks: plan.sacn.clone(),
                };
                threads.push(spawn("viz-dmx sacn discovery", move || task.sacn()));
            }
            Err(error) => warnings.push(format!(
                "sACN sources cannot be found: port {} could not be opened: {error}",
                plan.sacn_port
            )),
        }

        Self {
            plan,
            seen,
            running,
            threads,
            polled,
            warnings,
        }
    }

    pub fn plan(&self) -> &DiscoveryPlan {
        &self.plan
    }

    /// The broadcast addresses Art-Net polls go to.
    pub fn polled(&self) -> &[Ipv4Addr] {
        &self.polled
    }

    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Every node heard recently, one per IP address, in address order.
    pub fn nodes(&self) -> Vec<NetworkNode> {
        self.seen
            .lock()
            .expect("discovered nodes")
            .snapshot(Instant::now())
    }

    pub fn shutdown(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

impl Drop for SourceDiscovery {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn spawn(name: &str, task: impl FnOnce() + Send + 'static) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name(name.to_owned())
        .spawn(task)
        .expect("spawn discovery thread")
}

fn listen(port: u16) -> std::io::Result<UdpSocket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(SocketProtocol::UDP))?;
    socket.set_reuse_address(true)?;
    #[cfg(unix)]
    socket.set_reuse_port(true)?;
    socket.set_read_timeout(Some(Duration::from_millis(200)))?;
    socket.bind(&SocketAddr::from((Ipv4Addr::UNSPECIFIED, port)).into())?;
    Ok(socket.into())
}

fn directed_broadcast(interface: &NetworkInterface) -> Ipv4Addr {
    Ipv4Addr::from(u32::from(interface.address) | !u32::from(interface.netmask))
}

/// Whether `address` is on one of `networks`: a choice of interface narrows what is listed.
fn on_networks(networks: &[NetworkInterface], address: Ipv4Addr) -> bool {
    own_address(networks, address).is_some()
}

/// This machine's own address on the network `address` is on, which is how a reply to it names
/// this Visualizer.
fn own_address(networks: &[NetworkInterface], address: Ipv4Addr) -> Option<Ipv4Addr> {
    networks
        .iter()
        .find(|network| {
            let mask = u32::from(network.netmask);
            u32::from(network.address) & mask == u32::from(address) & mask
        })
        .map(|network| network.address)
}

/// The ArtPollReply packets answering a poll from `from`, or `None` when it needs no answer.
///
/// The reply names no ports: the Visualizer receives universes, it never puts one onto the
/// network. A poll from one of this machine's own addresses is the Visualizer's own broadcast
/// coming back, and answering it would list the Visualizer as a node in its own Sources tab.
fn poll_answer(networks: &[NetworkInterface], from: Ipv4Addr) -> Option<Vec<Vec<u8>>> {
    if networks.iter().any(|network| network.address == from) {
        return None;
    }
    let address = own_address(networks, from)?;
    Some(artpollreply_packets(
        address,
        SHORT_NAME,
        LONG_NAME,
        REPORT,
        &[],
    ))
}

struct Task {
    socket: UdpSocket,
    seen: Arc<Mutex<Seen>>,
    running: Arc<AtomicBool>,
    networks: Vec<NetworkInterface>,
}

impl Task {
    fn next(&self, buffer: &mut [u8]) -> Option<(usize, SocketAddrV4)> {
        match self.socket.recv_from(buffer) {
            Ok((length, SocketAddr::V4(from))) if on_networks(&self.networks, *from.ip()) => {
                Some((length, from))
            }
            _ => None,
        }
    }

    /// Answers a controller's ArtPoll, so the Visualizer is a named endpoint instead of an
    /// anonymous poller. A lost reply is asked for again by the next poll, so nothing is reported.
    fn answer_art_poll(&self, from: SocketAddrV4, port: u16) {
        let Some(replies) = poll_answer(&self.networks, *from.ip()) else {
            return;
        };
        // Replies belong on the Art-Net port; a poller asking from another port hears it there too.
        let mut destinations = vec![SocketAddrV4::new(*from.ip(), port)];
        if from.port() != port {
            destinations.push(from);
        }
        for destination in destinations {
            for reply in &replies {
                let _ = self.socket.send_to(reply, destination);
            }
        }
    }

    fn art_net(self, targets: &[Ipv4Addr], port: u16) {
        let poll = artpoll();
        let mut polled_at: Option<Instant> = None;
        let mut buffer = [0_u8; 2048];
        while self.running.load(Ordering::Relaxed) {
            if polled_at.is_none_or(|at| at.elapsed() >= POLL_INTERVAL) {
                for target in targets {
                    let _ = self.socket.send_to(&poll, (*target, port));
                }
                polled_at = Some(Instant::now());
            }
            let Some((length, from)) = self.next(&mut buffer) else {
                continue;
            };
            let bytes = &buffer[..length];
            let now = Instant::now();
            if let Some(reply) = decode_artpollreply(bytes) {
                self.seen
                    .lock()
                    .expect("discovered nodes")
                    .art_net_reply(reply, *from.ip(), now);
            } else if is_artpoll(bytes) {
                self.answer_art_poll(from, port);
            } else if let Ok(Some(frame)) = decode_artdmx(bytes) {
                self.seen.lock().expect("discovered nodes").art_net_data(
                    *from.ip(),
                    frame.destination_universe,
                    now,
                );
            }
        }
    }

    fn sacn(self) {
        let mut buffer = [0_u8; 2048];
        while self.running.load(Ordering::Relaxed) {
            let Some((length, from)) = self.next(&mut buffer) else {
                continue;
            };
            let bytes = &buffer[..length];
            let now = Instant::now();
            if let Some(discovery) = decode_sacn_discovery(bytes) {
                self.seen.lock().expect("discovered nodes").sacn_discovery(
                    discovery,
                    *from.ip(),
                    now,
                );
            } else if let Ok(Some(frame)) = decode_sacn(bytes) {
                self.seen.lock().expect("discovered nodes").sacn_data(
                    frame.cid,
                    &frame.source_name,
                    *from.ip(),
                    frame.destination_universe,
                    frame.terminated,
                    now,
                );
            }
        }
    }
}

/// Everything heard, before it is merged per address.
#[derive(Default)]
struct Seen {
    art_net_nodes: HashMap<(Ipv4Addr, u8), (ArtPollReply, Instant)>,
    art_net_data: HashMap<(Ipv4Addr, u16), Instant>,
    sacn_sources: HashMap<[u8; 16], SacnSource>,
}

struct SacnSource {
    address: Ipv4Addr,
    name: String,
    /// Universes announced on each discovery page, and when that page arrived.
    pages: BTreeMap<u8, (Vec<u16>, Instant)>,
    data: HashMap<u16, Instant>,
}

impl Seen {
    fn art_net_reply(&mut self, mut reply: ArtPollReply, from: Ipv4Addr, now: Instant) {
        if reply.address.is_unspecified() {
            reply.address = from;
        }
        self.art_net_nodes
            .insert((reply.address, reply.bind_index), (reply, now));
    }

    fn art_net_data(&mut self, from: Ipv4Addr, universe: u16, now: Instant) {
        self.art_net_data.insert((from, universe), now);
    }

    fn sacn_source(&mut self, cid: [u8; 16], name: &str, from: Ipv4Addr) -> &mut SacnSource {
        let source = self.sacn_sources.entry(cid).or_insert_with(|| SacnSource {
            address: from,
            name: String::new(),
            pages: BTreeMap::new(),
            data: HashMap::new(),
        });
        source.address = from;
        if !name.is_empty() {
            name.clone_into(&mut source.name);
        }
        source
    }

    fn sacn_discovery(&mut self, discovery: SacnDiscovery, from: Ipv4Addr, now: Instant) {
        let source = self.sacn_source(discovery.cid, &discovery.source_name, from);
        // A shorter announcement drops the pages it no longer has.
        source.pages.retain(|page, _| *page <= discovery.last_page);
        source
            .pages
            .insert(discovery.page, (discovery.universes, now));
    }

    fn sacn_data(
        &mut self,
        cid: [u8; 16],
        name: &str,
        from: Ipv4Addr,
        universe: u16,
        terminated: bool,
        now: Instant,
    ) {
        let source = self.sacn_source(cid, name, from);
        if terminated {
            source.data.remove(&universe);
        } else {
            source.data.insert(universe, now);
        }
    }

    /// Forget what went quiet, then merge the rest into one node per address.
    fn snapshot(&mut self, now: Instant) -> Vec<NetworkNode> {
        let age = |at: Instant| now.saturating_duration_since(at);
        self.art_net_nodes
            .retain(|_, (_, at)| age(*at) <= ART_NET_NODE_TIMEOUT);
        self.art_net_data.retain(|_, at| age(*at) <= DATA_TIMEOUT);
        for source in self.sacn_sources.values_mut() {
            source
                .pages
                .retain(|_, (_, at)| age(*at) <= SACN_DISCOVERY_TIMEOUT);
            source.data.retain(|_, at| age(*at) <= DATA_TIMEOUT);
        }
        self.sacn_sources
            .retain(|_, source| !source.pages.is_empty() || !source.data.is_empty());

        let mut drafts: BTreeMap<Ipv4Addr, Draft> = BTreeMap::new();
        let mut replies: Vec<_> = self.art_net_nodes.values().collect();
        replies.sort_by_key(|(reply, _)| (reply.address, reply.bind_index));
        for (reply, at) in replies {
            let draft = draft(&mut drafts, reply.address, age(*at));
            draft.node.art_net = true;
            draft.name(&reply.short_name);
            if draft.node.long_name.is_empty() {
                draft.node.long_name.clone_from(&reply.long_name);
            }
            if draft.node.report.is_empty() {
                draft.node.report.clone_from(&reply.report);
            }
            draft.node.mac = draft.node.mac.or(reply.mac);
            for port in &reply.ports {
                draft.node.ports.push(port.clone());
                if port.direction == PortDirection::Input {
                    draft.send(Protocol::ArtNet, port.universe, true, port.active);
                }
            }
        }
        for ((address, universe), at) in &self.art_net_data {
            let draft = draft(&mut drafts, *address, age(*at));
            draft.node.art_net = true;
            draft.send(Protocol::ArtNet, *universe, false, true);
        }
        let mut sources: Vec<_> = self.sacn_sources.iter().collect();
        sources.sort_by_key(|(cid, _)| **cid);
        for (_, source) in sources {
            let newest = source
                .pages
                .values()
                .map(|(_, at)| *at)
                .chain(source.data.values().copied())
                .max()
                .unwrap_or(now);
            let draft = draft(&mut drafts, source.address, age(newest));
            draft.node.sacn = true;
            draft.name(&source.name);
            for (universes, _) in source.pages.values() {
                for universe in universes {
                    draft.send(Protocol::Sacn, *universe, true, false);
                }
            }
            for universe in source.data.keys() {
                draft.send(Protocol::Sacn, *universe, false, true);
            }
        }
        drafts.into_values().map(Draft::finish).collect()
    }
}

/// One node while it is being merged.
struct Draft {
    node: NetworkNode,
    /// Keyed by whether it is sACN, then universe: Art-Net first. Valued announced, live.
    sends: BTreeMap<(bool, u16), (bool, bool)>,
}

fn draft(drafts: &mut BTreeMap<Ipv4Addr, Draft>, address: Ipv4Addr, age: Duration) -> &mut Draft {
    let draft = drafts.entry(address).or_insert_with(|| Draft {
        node: NetworkNode {
            address,
            names: Vec::new(),
            long_name: String::new(),
            report: String::new(),
            mac: None,
            art_net: false,
            sacn: false,
            ports: Vec::new(),
            sends: Vec::new(),
            last_seen: Duration::MAX,
        },
        sends: BTreeMap::new(),
    });
    draft.node.last_seen = draft.node.last_seen.min(age);
    draft
}

impl Draft {
    fn name(&mut self, name: &str) {
        if !name.is_empty() && !self.node.names.iter().any(|known| known == name) {
            self.node.names.push(name.to_owned());
        }
    }

    fn send(&mut self, protocol: Protocol, universe: u16, announced: bool, live: bool) {
        let entry = self
            .sends
            .entry((protocol == Protocol::Sacn, universe))
            .or_default();
        entry.0 |= announced;
        entry.1 |= live;
    }

    fn finish(mut self) -> NetworkNode {
        self.node
            .ports
            .sort_by_key(|port| (port.bind_index, port.port, port.direction));
        self.node.sends = self
            .sends
            .into_iter()
            .map(|((sacn, universe), (announced, live))| SentUniverse {
                protocol: if sacn {
                    Protocol::Sacn
                } else {
                    Protocol::ArtNet
                },
                universe,
                announced,
                live,
            })
            .collect();
        self.node
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NODE: Ipv4Addr = Ipv4Addr::new(10, 0, 0, 20);

    /// A two-port node on net 0, sub-net 1: port 1 is a DMX input and output, port 2 an output.
    fn artpollreply(name: &str) -> Vec<u8> {
        let mut packet = vec![0_u8; 239];
        packet[..8].copy_from_slice(b"Art-Net\0");
        packet[8..10].copy_from_slice(&ARTPOLLREPLY.to_le_bytes());
        packet[10..14].copy_from_slice(&NODE.octets());
        packet[14..16].copy_from_slice(&ARTNET_PORT.to_le_bytes());
        packet[19] = 1;
        packet[26..26 + name.len()].copy_from_slice(name.as_bytes());
        packet[44..53].copy_from_slice(b"Long name");
        packet[173] = 2;
        packet[174] = 0xc0;
        packet[175] = 0x80;
        packet[178] = 0x80;
        packet[183] = 0x80;
        packet[186] = 2;
        packet[190] = 3;
        packet[191] = 4;
        packet[201..207].copy_from_slice(&[0x00, 0x11, 0x22, 0x33, 0x44, 0x55]);
        packet[211] = 1;
        packet
    }

    fn sacn_discovery(name: &str, universes: &[u16]) -> Vec<u8> {
        let mut packet = vec![0_u8; 120 + universes.len() * 2];
        packet[0..2].copy_from_slice(&0x0010_u16.to_be_bytes());
        packet[4..16].copy_from_slice(SACN_ACN_IDENTIFIER);
        packet[18..22].copy_from_slice(&8_u32.to_be_bytes());
        packet[22..38].copy_from_slice(&[7; 16]);
        packet[40..44].copy_from_slice(&2_u32.to_be_bytes());
        packet[44..44 + name.len()].copy_from_slice(name.as_bytes());
        let layer = (8 + universes.len() * 2) as u16;
        packet[112..114].copy_from_slice(&(0x7000 | layer).to_be_bytes());
        packet[114..118].copy_from_slice(&1_u32.to_be_bytes());
        for (index, universe) in universes.iter().enumerate() {
            packet[120 + index * 2..122 + index * 2].copy_from_slice(&universe.to_be_bytes());
        }
        packet
    }

    #[test]
    fn the_poll_is_an_art_net_4_artpoll() {
        let poll = artpoll();
        assert_eq!(&poll[..8], b"Art-Net\0");
        assert_eq!(u16::from_le_bytes([poll[8], poll[9]]), ARTPOLL);
        assert_eq!(u16::from_be_bytes([poll[10], poll[11]]), 14);
    }

    #[test]
    fn a_reply_names_the_node_and_every_port_with_its_universe() {
        let reply = decode_artpollreply(&artpollreply("Node 4")).expect("a reply");
        assert_eq!(reply.address, NODE);
        assert_eq!(reply.short_name, "Node 4");
        assert_eq!(reply.long_name, "Long name");
        assert_eq!(reply.mac, Some([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]));
        let ports: Vec<_> = reply
            .ports
            .iter()
            .map(|port| (port.port, port.direction, port.universe, port.active))
            .collect();
        assert_eq!(
            ports,
            [
                (1, PortDirection::Input, 0x12, true),
                (1, PortDirection::Output, 0x13, false),
                (2, PortDirection::Output, 0x14, true),
            ]
        );
    }

    #[test]
    fn other_art_net_packets_and_truncated_replies_are_not_replies() {
        let mut poll = artpollreply("Node");
        poll[8..10].copy_from_slice(&ARTPOLL.to_le_bytes());
        assert_eq!(decode_artpollreply(&poll), None);
        assert_eq!(decode_artpollreply(&artpollreply("Node")[..190]), None);
    }

    #[test]
    fn a_discovery_page_lists_the_universes_a_source_sends() {
        let page = decode_sacn_discovery(&sacn_discovery("Console", &[1, 7])).expect("a page");
        assert_eq!(page.source_name, "Console");
        assert_eq!(page.cid, [7; 16]);
        assert_eq!(page.universes, [1, 7]);
        assert_eq!(decode_sacn_discovery(&artpollreply("Node")), None);
    }

    #[test]
    fn one_address_is_one_node_with_everything_it_sends_until_it_goes_quiet() {
        let start = Instant::now();
        let mut seen = Seen::default();
        seen.art_net_reply(
            decode_artpollreply(&artpollreply("Node 4")).expect("a reply"),
            NODE,
            start,
        );
        seen.art_net_data(NODE, 5, start);
        seen.sacn_discovery(
            decode_sacn_discovery(&sacn_discovery("Console", &[1, 7])).expect("a page"),
            NODE,
            start,
        );
        seen.sacn_data([7; 16], "Console", NODE, 1, false, start);

        let nodes = seen.snapshot(start + Duration::from_secs(1));
        assert_eq!(nodes.len(), 1);
        let node = &nodes[0];
        assert_eq!(node.names, ["Node 4", "Console"]);
        assert!(node.art_net && node.sacn);
        assert_eq!(node.ports.len(), 3);
        let sends: Vec<_> = node
            .sends
            .iter()
            .map(|sent| (sent.protocol, sent.universe, sent.announced, sent.live))
            .collect();
        assert_eq!(
            sends,
            [
                (Protocol::ArtNet, 5, false, true),
                (Protocol::ArtNet, 0x12, true, true),
                (Protocol::Sacn, 1, true, true),
                (Protocol::Sacn, 7, true, false),
            ]
        );
        assert_eq!(node.last_seen, Duration::from_secs(1));

        // Data stops first; the announcements outlive it, then everything is forgotten.
        let later = seen.snapshot(start + Duration::from_secs(5));
        assert!(
            later[0]
                .sends
                .iter()
                .all(|sent| !sent.live || sent.announced)
        );
        assert!(seen.snapshot(start + Duration::from_secs(30)).is_empty());
    }

    #[test]
    fn a_node_on_this_computer_is_found_on_the_ports_it_listens_on() {
        let free_port = || {
            UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))
                .and_then(|socket| socket.local_addr())
                .map(|address| address.port())
                .expect("a free UDP port")
        };
        let loopback = NetworkInterface {
            name: "lo0".into(),
            address: Ipv4Addr::LOCALHOST,
            netmask: Ipv4Addr::new(255, 0, 0, 0),
            index: None,
            loopback: true,
        };
        let mut plan = DiscoveryPlan::new(vec![loopback.clone()], vec![loopback], Vec::new());
        plan.art_net_port = free_port();
        plan.sacn_port = free_port();
        let mut discovery = SourceDiscovery::start(plan.clone());
        assert!(discovery.polled().is_empty());

        let sender = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("sender socket");
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut nodes = Vec::new();
        while nodes.is_empty() && Instant::now() < deadline {
            let _ = sender.send_to(
                &artpollreply("Loopback node"),
                (Ipv4Addr::LOCALHOST, plan.art_net_port),
            );
            let _ = sender.send_to(
                &sacn_discovery("Loopback source", &[3]),
                (Ipv4Addr::LOCALHOST, plan.sacn_port),
            );
            std::thread::sleep(Duration::from_millis(50));
            nodes = discovery.nodes();
        }
        discovery.shutdown();
        let names: Vec<_> = nodes.iter().flat_map(|node| node.names.clone()).collect();
        assert!(
            names.contains(&"Loopback node".to_owned()),
            "found {names:?}"
        );
    }

    #[test]
    fn a_poll_is_answered_as_the_visualizer_and_never_by_this_machine_to_itself() {
        let networks = [NetworkInterface {
            name: "en0".into(),
            address: Ipv4Addr::new(10, 0, 0, 9),
            netmask: Ipv4Addr::new(255, 255, 255, 0),
            index: None,
            loopback: false,
        }];
        let replies = poll_answer(&networks, NODE).expect("an answer");
        let reply = decode_artpollreply(&replies[0]).expect("a reply");
        assert_eq!(reply.long_name, "ToskLight Visualizer");
        assert_eq!(reply.short_name, "Visualizer");
        assert_eq!(reply.address, Ipv4Addr::new(10, 0, 0, 9));
        // The Visualizer only receives, so it announces no port it sends.
        assert!(reply.ports.is_empty());
        // Its own poll comes back from the broadcast; answering would list it in its own tab.
        assert_eq!(poll_answer(&networks, Ipv4Addr::new(10, 0, 0, 9)), None);
        // A poll from another network is not this Visualizer's to answer.
        assert_eq!(poll_answer(&networks, Ipv4Addr::new(10, 0, 1, 4)), None);
    }

    #[test]
    fn a_polling_controller_hears_the_visualizer_answer() {
        // A second loopback address, so the poll does not appear to come from this Visualizer.
        let loopback = NetworkInterface {
            name: "lo0".into(),
            address: Ipv4Addr::new(127, 0, 0, 2),
            netmask: Ipv4Addr::new(255, 0, 0, 0),
            index: None,
            loopback: true,
        };
        let mut plan = DiscoveryPlan::new(vec![loopback.clone()], vec![loopback], Vec::new());
        plan.art_net_port = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))
            .and_then(|socket| socket.local_addr())
            .map(|address| address.port())
            .expect("a free UDP port");
        let mut discovery = SourceDiscovery::start(plan.clone());

        let poller = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("poller socket");
        poller
            .set_read_timeout(Some(Duration::from_millis(100)))
            .expect("a timeout");
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut buffer = [0_u8; 2048];
        let mut heard = None;
        while heard.is_none() && Instant::now() < deadline {
            let _ = poller.send_to(&artpoll(), (Ipv4Addr::LOCALHOST, plan.art_net_port));
            if let Ok((length, _)) = poller.recv_from(&mut buffer) {
                heard = decode_artpollreply(&buffer[..length]);
            }
        }
        discovery.shutdown();
        assert_eq!(
            heard.expect("a reply to the poll").long_name,
            "ToskLight Visualizer"
        );
    }
}
