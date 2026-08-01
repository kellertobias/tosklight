//! Bounded Art-Net and sACN receivers.
//!
//! Mappings that share a protocol port share one socket, because that is how the protocols
//! actually work: one Art-Net or sACN listener sees every universe on the wire and sorts them out
//! by destination universe. A packet for a universe this show does not use belongs to another
//! consumer on the network and is ignored, never counted as malformed.
//!
//! Frames are coalesced to the newest state per logical universe, so a burst never queues work for
//! the renderer and a renderer stall never backpressures the desk that sent the packets.

use crate::mapping::{Delivery, InputMapping, Protocol};
use crate::packet::{
    DMX_SLOTS, DecodedFrame, PacketReject, decode_artdmx, decode_sacn, sacn_multicast_group,
    sequence_is_stale,
};
use crate::statistics::UniverseStatistics;
use socket2::{Domain, Protocol as SocketProtocol, Socket, Type};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use viz_scene::{InputHealth, InputMappingStatus, SourceProtocol, UniverseHealth};

/// Protocol source-loss timeout. E1.31 specifies 2.5 s; Art-Net has no mandated value, so the
/// same bound is applied for one consistent operator-visible rule.
const SOURCE_LOSS: Duration = Duration::from_millis(2_500);
/// How long a higher-priority mapping must stay healthy before failover returns to it.
const FAILBACK_DWELL: Duration = Duration::from_millis(2_000);

/// Latest state of one logical universe.
#[derive(Clone, Debug)]
pub struct UniverseFrame {
    pub logical_universe: u16,
    pub slots: [u8; DMX_SLOTS],
    pub received_micros: u64,
    pub stale: bool,
}

#[derive(Default)]
struct Counters {
    accepted: AtomicU64,
    duplicate: AtomicU64,
    malformed: AtomicU64,
    out_of_order: AtomicU64,
    last_packet_micros: AtomicU64,
}

struct MappingState {
    mapping: InputMapping,
    counters: Counters,
    /// Malformed packets seen on the listener this mapping reads. A packet that fails to decode
    /// cannot be attributed to one mapping when several share a listener, so the listener's count
    /// is reported alongside this mapping's own refusals.
    listener: Arc<Counters>,
    detail: Mutex<String>,
    source: Mutex<(String, Option<String>)>,
    health: Mutex<InputHealth>,
}

struct UniverseState {
    frame: [u8; DMX_SLOTS],
    received_micros: u64,
    changed: bool,
    owner: Option<usize>,
    owner_since: Instant,
    stale: bool,
    /// The sender currently feeding this universe. A second sender on the same universe is a
    /// duplicate copy, not a second frame, and is ignored while this one is alive.
    source: Option<SocketAddr>,
    source_micros: u64,
    statistics: UniverseStatistics,
}

impl Default for UniverseState {
    fn default() -> Self {
        Self {
            frame: [0; DMX_SLOTS],
            received_micros: 0,
            changed: false,
            owner: None,
            owner_since: Instant::now(),
            stale: true,
            source: None,
            source_micros: 0,
            statistics: UniverseStatistics::default(),
        }
    }
}

struct Shared {
    epoch: Instant,
    universes: Mutex<HashMap<u16, UniverseState>>,
    mappings: Vec<Arc<MappingState>>,
    running: AtomicBool,
    /// Receive time of the newest accepted packet on any mapping, regardless of whether its
    /// content changed. A held look still arrives at full rate, so this is what
    /// packet-to-visible latency and the input-rate readout must be measured against.
    newest_accepted_micros: AtomicU64,
    accepted_packets: AtomicU64,
}

impl Shared {
    fn micros(&self) -> u64 {
        self.epoch.elapsed().as_micros() as u64
    }
}

/// One bound listener plus the mappings it serves, indexed by destination universe.
struct Listener {
    socket: UdpSocket,
    protocol: Protocol,
    counters: Arc<Counters>,
    by_universe: HashMap<u16, Vec<usize>>,
}

pub struct DmxReceiver {
    shared: Arc<Shared>,
    threads: Vec<std::thread::JoinHandle<()>>,
    sockets: Vec<UdpSocket>,
}

impl DmxReceiver {
    /// Bind every enabled mapping. A mapping that cannot bind is reported rather than dropped.
    pub fn start(mappings: Vec<InputMapping>, epoch: Instant) -> Self {
        let groups = group_by_listener(&mappings);
        let listener_counters: HashMap<ListenerKey, Arc<Counters>> = groups
            .keys()
            .map(|key| (key.clone(), Arc::new(Counters::default())))
            .collect();
        let states: Vec<Arc<MappingState>> = mappings
            .iter()
            .map(|mapping| {
                let key = listener_key(mapping);
                Arc::new(MappingState {
                    mapping: mapping.clone(),
                    counters: Counters::default(),
                    listener: listener_counters.get(&key).cloned().unwrap_or_default(),
                    detail: Mutex::new(String::new()),
                    source: Mutex::new((String::new(), None)),
                    health: Mutex::new(InputHealth::Waiting),
                })
            })
            .collect();
        let shared = Arc::new(Shared {
            epoch,
            universes: Mutex::new(HashMap::new()),
            mappings: states,
            running: AtomicBool::new(true),
            newest_accepted_micros: AtomicU64::new(0),
            accepted_packets: AtomicU64::new(0),
        });

        for state in &shared.mappings {
            if !state.mapping.enabled {
                *state.health.lock().expect("mapping health") = InputHealth::Failed;
                *state.detail.lock().expect("mapping detail") = "Disabled".into();
            }
        }

        let mut threads = Vec::new();
        let mut sockets = Vec::new();
        for (key, members) in groups {
            let enabled: Vec<usize> = members
                .into_iter()
                .filter(|index| shared.mappings[*index].mapping.enabled)
                .collect();
            let Some(first) = enabled.first().copied() else {
                continue;
            };
            let representative = shared.mappings[first].mapping.clone();
            let multicast_universes: Vec<u16> = enabled
                .iter()
                .map(|index| shared.mappings[*index].mapping.destination_universe)
                .collect();
            match bind(&representative, &multicast_universes) {
                Ok(socket) => {
                    let cloned = socket.try_clone().expect("clone receiver socket");
                    sockets.push(socket);
                    let mut by_universe: HashMap<u16, Vec<usize>> = HashMap::new();
                    for index in &enabled {
                        by_universe
                            .entry(shared.mappings[*index].mapping.destination_universe)
                            .or_default()
                            .push(*index);
                    }
                    let listener = Listener {
                        socket: cloned,
                        protocol: key.protocol,
                        counters: shared.mappings[first].listener.clone(),
                        by_universe,
                    };
                    let shared = shared.clone();
                    threads.push(
                        std::thread::Builder::new()
                            .name(format!("viz-dmx {} {}", key.protocol.label(), key.bind))
                            .spawn(move || receive_loop(shared, listener))
                            .expect("spawn receiver thread"),
                    );
                }
                Err(error) => {
                    for index in enabled {
                        let state = &shared.mappings[index];
                        *state.health.lock().expect("mapping health") = InputHealth::Failed;
                        *state.detail.lock().expect("mapping detail") = error.clone();
                    }
                }
            }
        }
        Self {
            shared,
            threads,
            sockets,
        }
    }

    /// Take every logical universe whose content changed since the last call.
    pub fn drain_changed(&self) -> Vec<UniverseFrame> {
        self.expire_sources();
        let mut universes = self.shared.universes.lock().expect("universe frames");
        universes
            .iter_mut()
            .filter(|(_, state)| state.changed)
            .map(|(logical, state)| {
                state.changed = false;
                UniverseFrame {
                    logical_universe: *logical,
                    slots: state.frame,
                    received_micros: state.received_micros,
                    stale: state.stale,
                }
            })
            .collect()
    }

    /// Mark every universe already held as changed, so the next drain re-decodes all of them.
    ///
    /// A structural change hands the decoder new bindings, and a desk holding a look sends the
    /// same slots for minutes on end. Without this, a fixture patched — or moved to another
    /// address — during a held state would sit at its defaults until something on its universe
    /// happened to move.
    pub fn refresh_all(&self) {
        let mut universes = self.shared.universes.lock().expect("universe frames");
        for state in universes.values_mut() {
            if state.received_micros > 0 {
                state.changed = true;
            }
        }
    }

    /// Mark universes whose owning mapping timed out.
    fn expire_sources(&self) {
        let now = self.shared.micros();
        let mut universes = self.shared.universes.lock().expect("universe frames");
        for (logical, state) in universes.iter_mut() {
            let expired =
                now.saturating_sub(state.received_micros) > SOURCE_LOSS.as_micros() as u64;
            if expired && !state.stale {
                state.stale = true;
                state.changed = true;
                state.source = None;
                if let Some(owner) = state.owner
                    && let Some(mapping) = self.shared.mappings.get(owner)
                {
                    *mapping.health.lock().expect("mapping health") = InputHealth::Stale;
                    *mapping.detail.lock().expect("mapping detail") =
                        format!("No valid frame for universe {logical} within 2.5 s");
                }
                state.owner = None;
            }
        }
    }

    /// Receive time of the newest accepted packet, in microseconds since the shared epoch.
    pub fn newest_accepted_micros(&self) -> u64 {
        self.shared.newest_accepted_micros.load(Ordering::Relaxed)
    }

    /// Total accepted packets across every mapping.
    pub fn accepted_packets(&self) -> u64 {
        self.shared.accepted_packets.load(Ordering::Relaxed)
    }

    /// Health of every logical universe this show reads, ordered by universe number.
    pub fn universes(&self) -> Vec<UniverseHealth> {
        let now = self.shared.micros();
        let universes = self.shared.universes.lock().expect("universe frames");
        let mut rows: Vec<UniverseHealth> = universes
            .iter()
            .map(|(logical, state)| UniverseHealth {
                universe: *logical,
                rate_hz: state.statistics.rate_hz(now),
                grade: state.statistics.grade(now, state.stale),
                accepted: state.statistics.total_accepted(),
                broken: state.statistics.total_broken(),
                stale: state.stale,
                protocol: state
                    .owner
                    .and_then(|owner| self.shared.mappings.get(owner))
                    .map(|mapping| match mapping.mapping.protocol {
                        Protocol::ArtNet => SourceProtocol::ArtNet,
                        Protocol::Sacn => SourceProtocol::Sacn,
                    }),
            })
            .collect();
        rows.sort_by_key(|row| row.universe);
        rows
    }

    /// Observable per-mapping status for the connection surface.
    pub fn status(&self) -> Vec<InputMappingStatus> {
        self.shared
            .mappings
            .iter()
            .map(|state| {
                let (name, address) = state.source.lock().expect("mapping source").clone();
                let last = state.counters.last_packet_micros.load(Ordering::Relaxed);
                InputMappingStatus {
                    mapping_id: state.mapping.id.clone(),
                    protocol: match state.mapping.protocol {
                        Protocol::ArtNet => SourceProtocol::ArtNet,
                        Protocol::Sacn => SourceProtocol::Sacn,
                    },
                    logical_universe: state.mapping.logical_universe,
                    destination_universe: state.mapping.destination_universe,
                    delivery: state.mapping.delivery.label().to_owned(),
                    bind: state.mapping.bind.to_string(),
                    health: *state.health.lock().expect("mapping health"),
                    last_packet_micros: (last > 0).then_some(last),
                    accepted_packets: state.counters.accepted.load(Ordering::Relaxed),
                    duplicate_packets: state.counters.duplicate.load(Ordering::Relaxed),
                    malformed_packets: state.listener.malformed.load(Ordering::Relaxed)
                        + state.counters.malformed.load(Ordering::Relaxed),
                    out_of_order_packets: state.counters.out_of_order.load(Ordering::Relaxed),
                    source_name: name,
                    source_address: address,
                    detail: state.detail.lock().expect("mapping detail").clone(),
                }
            })
            .collect()
    }

    pub fn shutdown(&mut self) {
        self.shared.running.store(false, Ordering::Relaxed);
        self.sockets.clear();
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

impl Drop for DmxReceiver {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Listeners are shared by protocol, bind address, and delivery mode.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ListenerKey {
    protocol: Protocol,
    bind: SocketAddr,
    delivery: Delivery,
}

fn listener_key(mapping: &InputMapping) -> ListenerKey {
    ListenerKey {
        protocol: mapping.protocol,
        bind: match mapping.delivery {
            // Multicast always binds the wildcard address; membership decides delivery.
            Delivery::Multicast => SocketAddr::from((Ipv4Addr::UNSPECIFIED, mapping.bind.port())),
            _ => mapping.bind,
        },
        delivery: mapping.delivery,
    }
}

fn group_by_listener(mappings: &[InputMapping]) -> HashMap<ListenerKey, Vec<usize>> {
    let mut groups: HashMap<ListenerKey, Vec<usize>> = HashMap::new();
    for (index, mapping) in mappings.iter().enumerate() {
        groups.entry(listener_key(mapping)).or_default().push(index);
    }
    groups
}

fn bind(mapping: &InputMapping, multicast_universes: &[u16]) -> Result<UdpSocket, String> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(SocketProtocol::UDP))
        .map_err(|error| format!("socket: {error}"))?;
    socket
        .set_reuse_address(true)
        .map_err(|error| format!("reuse address: {error}"))?;
    #[cfg(unix)]
    socket
        .set_reuse_port(true)
        .map_err(|error| format!("reuse port: {error}"))?;
    socket
        .set_read_timeout(Some(Duration::from_millis(200)))
        .map_err(|error| format!("read timeout: {error}"))?;
    // A generous receive buffer keeps a burst from being dropped while the render thread is busy.
    // It is a fixed kernel allocation, so it cannot grow without bound.
    let _ = socket.set_recv_buffer_size(1 << 21);
    let bind_address = listener_key(mapping).bind;
    socket
        .bind(&bind_address.into())
        .map_err(|error| format!("bind {bind_address}: {error}"))?;
    if mapping.delivery == Delivery::Multicast {
        let interface = match mapping.bind.ip() {
            IpAddr::V4(address) => address,
            IpAddr::V6(_) => Ipv4Addr::UNSPECIFIED,
        };
        for universe in multicast_universes {
            let group = sacn_multicast_group(*universe);
            socket
                .join_multicast_v4(&group, &interface)
                .map_err(|error| format!("join {group} on {interface}: {error}"))?;
        }
        let _ = socket.set_multicast_loop_v4(true);
    }
    Ok(socket.into())
}

/// Per-destination-universe source state, tracked inside one listener.
#[derive(Default)]
struct SourceState {
    sequence: Option<u8>,
    priority: u8,
    cid: Option<[u8; 16]>,
}

fn receive_loop(shared: Arc<Shared>, listener: Listener) {
    let mut buffer = [0_u8; 2048];
    // Keyed by destination universe and source address: two senders on one universe each
    // keep their own sequence, which is what stops them being read as each other's replays.
    let mut sources: HashMap<(u16, SocketAddr), SourceState> = HashMap::new();
    while shared.running.load(Ordering::Relaxed) {
        let (length, from) = match listener.socket.recv_from(&mut buffer) {
            Ok(value) => value,
            Err(error) if is_timeout(&error) => continue,
            Err(_) => continue,
        };
        let decoded = match listener.protocol {
            Protocol::ArtNet => decode_artdmx(&buffer[..length]),
            Protocol::Sacn => decode_sacn(&buffer[..length]),
        };
        let frame = match decoded {
            Ok(Some(frame)) => frame,
            Ok(None) => continue,
            Err(reject) => {
                listener.counters.malformed.fetch_add(1, Ordering::Relaxed);
                let now = shared.micros();
                for indices in listener.by_universe.values() {
                    for index in indices {
                        note(&shared.mappings[*index], reject);
                        record_broken(
                            &shared,
                            shared.mappings[*index].mapping.logical_universe,
                            now,
                        );
                    }
                }
                continue;
            }
        };
        // A universe nobody mapped belongs to another consumer on this network. Ignoring it is
        // correct; counting it as malformed would be a false alarm.
        let Some(targets) = listener.by_universe.get(&frame.destination_universe) else {
            continue;
        };
        let state = sources
            .entry((frame.destination_universe, from))
            .or_default();
        if frame.terminated {
            *state = SourceState::default();
            for index in targets {
                mark_stale(
                    &shared,
                    &shared.mappings[*index],
                    "Stream terminated by the source",
                );
            }
            continue;
        }
        if listener.protocol == Protocol::Sacn {
            let same_source = state.cid == Some(frame.cid);
            if !same_source && frame.priority < state.priority {
                for index in targets {
                    reject(&shared.mappings[*index], PacketReject::LowerPriority);
                }
                continue;
            }
            if !same_source {
                state.sequence = None;
            }
            state.cid = Some(frame.cid);
            state.priority = frame.priority;
        }
        if sequence_is_stale(state.sequence, frame.sequence) {
            let now = shared.micros();
            for index in targets {
                shared.mappings[*index]
                    .counters
                    .out_of_order
                    .fetch_add(1, Ordering::Relaxed);
                note(&shared.mappings[*index], PacketReject::OutOfOrder);
                record_broken(
                    &shared,
                    shared.mappings[*index].mapping.logical_universe,
                    now,
                );
            }
            continue;
        }
        state.sequence = Some(frame.sequence);
        let now = shared.micros();
        shared
            .newest_accepted_micros
            .fetch_max(now, Ordering::Relaxed);
        shared.accepted_packets.fetch_add(1, Ordering::Relaxed);
        for index in targets {
            let mapping = &shared.mappings[*index];
            mapping
                .counters
                .last_packet_micros
                .store(now, Ordering::Relaxed);
            *mapping.source.lock().expect("mapping source") =
                (frame.source_name.clone(), Some(from.to_string()));
            apply(&shared, *index, mapping, &frame, from, now);
        }
    }
}

/// Count one unusable frame against the logical universe it was meant for.
fn record_broken(shared: &Arc<Shared>, logical: u16, now: u64) {
    let mut universes = shared.universes.lock().expect("universe frames");
    universes.entry(logical).or_default().statistics.broken(now);
}

/// Record why a packet was discarded without attributing a malformed count to this mapping.
fn note(state: &MappingState, reason: PacketReject) {
    *state.detail.lock().expect("mapping detail") = format!("Discarded {reason:?}");
}

/// Discard a packet that was addressed to this mapping and count it against it.
fn reject(state: &MappingState, reason: PacketReject) {
    state.counters.malformed.fetch_add(1, Ordering::Relaxed);
    note(state, reason);
}

fn mark_stale(shared: &Arc<Shared>, state: &MappingState, detail: &str) {
    *state.health.lock().expect("mapping health") = InputHealth::Stale;
    *state.detail.lock().expect("mapping detail") = detail.to_owned();
    let mut universes = shared.universes.lock().expect("universe frames");
    if let Some(universe) = universes.get_mut(&state.mapping.logical_universe) {
        universe.stale = true;
        universe.changed = true;
        universe.owner = None;
    }
}

/// Apply one accepted frame, honouring duplicate-mapping priority and the anti-flap rule.
#[allow(clippy::too_many_arguments)]
/// A byte-identical frame arriving inside this window is a duplicate copy of the frame just
/// applied, not a new one. It is shorter than one interval of the slowest rate worth showing.
const DUPLICATE_WINDOW: Duration = Duration::from_millis(6);

fn apply(
    shared: &Arc<Shared>,
    index: usize,
    state: &MappingState,
    frame: &DecodedFrame,
    from: SocketAddr,
    now: u64,
) {
    let logical = state.mapping.logical_universe;
    let mut universes = shared.universes.lock().expect("universe frames");
    let universe = universes.entry(logical).or_default();
    let owner_priority = universe
        .owner
        .and_then(|owner| shared.mappings.get(owner))
        .map(|owner| owner.mapping.priority);
    let owns = match (universe.owner, owner_priority) {
        (Some(owner), _) if owner == index => true,
        (None, _) => true,
        (Some(_), Some(priority)) if state.mapping.priority > priority => {
            // Returning to a higher-priority source waits out the documented dwell so a flapping
            // source cannot switch back and forth every frame.
            universe.owner_since.elapsed() >= FAILBACK_DWELL
        }
        _ => false,
    };
    if !owns {
        *state.health.lock().expect("mapping health") = InputHealth::Superseded;
        *state.detail.lock().expect("mapping detail") =
            format!("Universe {logical} is carried by a higher-priority mapping");
        return;
    }
    if universe.owner != Some(index) {
        universe.owner = Some(index);
        universe.owner_since = Instant::now();
        for (other, mapping) in shared.mappings.iter().enumerate() {
            if other != index && mapping.mapping.logical_universe == logical {
                *mapping.health.lock().expect("mapping health") = InputHealth::Superseded;
            }
        }
    }
    // Two senders carrying the same universe are two copies of one frame. Applying both would
    // double the reported rate and, worse, interleave two versions of the same look.
    let source_alive = now.saturating_sub(universe.source_micros) < SOURCE_LOSS.as_micros() as u64;
    match universe.source {
        Some(existing) if existing != from && source_alive => {
            state.counters.duplicate.fetch_add(1, Ordering::Relaxed);
            *state.detail.lock().expect("mapping detail") =
                format!("Universe {logical} is already carried by {existing}");
            return;
        }
        _ => universe.source = Some(from),
    }
    universe.source_micros = now;

    // A second route configured onto the same universe sends from the same socket, so the copy
    // cannot be told apart by its address. What gives it away is that it carries byte-identical
    // slots and lands within a fraction of one frame interval. A real refresh of an unchanged
    // look arrives a whole frame later and still counts.
    let slots = frame.slot_count as usize;
    let copied = now.saturating_sub(universe.received_micros) < DUPLICATE_WINDOW.as_micros() as u64
        && universe.frame[..slots.min(DMX_SLOTS)] == frame.slots[..slots.min(DMX_SLOTS)];
    if copied {
        state.counters.duplicate.fetch_add(1, Ordering::Relaxed);
        *state.detail.lock().expect("mapping detail") =
            format!("Universe {logical} is also carried by a second route");
        return;
    }

    // Counted here, past every arbitration, so the mapping's accepted total means the same thing
    // as the universe's frame rate rather than counting deliveries the universe never took.
    state.counters.accepted.fetch_add(1, Ordering::Relaxed);
    universe.statistics.accept(now);
    let changed = universe.frame[..slots] != frame.slots[..slots] || universe.stale;
    universe.frame[..slots].copy_from_slice(&frame.slots[..slots]);
    universe.received_micros = now;
    universe.stale = false;
    universe.changed |= changed;
    *state.health.lock().expect("mapping health") = InputHealth::Healthy;
    *state.detail.lock().expect("mapping detail") = String::new();
}

fn is_timeout(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Send one real ArtDMX packet through the operating-system network stack and read it back.
    ///
    /// This is deliberately not an in-process shortcut: the packet is encoded, sent over UDP to
    /// the loopback interface, received by a real socket, and decoded.
    #[test]
    fn a_real_art_net_packet_reaches_the_receiver_over_loopback() {
        let port = free_port();
        let receiver = start(&[(Protocol::ArtNet, 5, port)]);
        let sender = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("sender socket");

        let frame = deliver(&receiver, &sender, port, &artdmx(5, 1, &[200, 21, 0, 0]));
        assert_eq!(frame.logical_universe, 5);
        assert_eq!(frame.slots[0], 200);
        assert_eq!(frame.slots[1], 21);
        assert!(!frame.stale);

        let status = receiver.status();
        assert_eq!(status[0].health, InputHealth::Healthy);
        assert!(status[0].accepted_packets >= 1);
        assert_eq!(status[0].malformed_packets, 0);
    }

    #[test]
    fn a_universe_this_show_does_not_map_is_ignored_rather_than_counted_as_malformed() {
        let port = free_port();
        let receiver = start(&[(Protocol::ArtNet, 6, port)]);
        let sender = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("sender socket");

        deliver(&receiver, &sender, port, &artdmx(6, 1, &[77, 0, 0, 0]));
        for _ in 0..10 {
            sender
                .send_to(&artdmx(99, 2, &[1, 2, 3, 4]), (Ipv4Addr::LOCALHOST, port))
                .expect("send");
        }
        std::thread::sleep(Duration::from_millis(250));
        assert_eq!(
            receiver.status()[0].malformed_packets,
            0,
            "another consumer's universe is not an error"
        );
        assert!(receiver.drain_changed().is_empty());
    }

    #[test]
    fn a_second_route_onto_one_universe_is_one_frame_not_two() {
        let port = free_port();
        let receiver = start(&[(Protocol::ArtNet, 11, port)]);
        let sender = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("sender socket");

        // The desk carries the same universe on two routes: every frame leaves twice, back to
        // back, from one socket. The visualizer must see one frame at the real rate.
        for sequence in 1..=10u8 {
            let packet = artdmx(11, sequence, &[40, 90, 0, 0]);
            sender
                .send_to(&packet, (Ipv4Addr::LOCALHOST, port))
                .expect("send");
            sender
                .send_to(&packet, (Ipv4Addr::LOCALHOST, port))
                .expect("send copy");
            std::thread::sleep(Duration::from_millis(24));
        }
        std::thread::sleep(Duration::from_millis(120));

        let accepted = receiver.status()[0].accepted_packets;
        assert!(
            (8..=12).contains(&accepted),
            "ten sent frames, each delivered twice, must count about ten times, not twenty: \
             {accepted}"
        );
        assert_eq!(receiver.status()[0].malformed_packets, 0);
    }

    #[test]
    fn a_malformed_packet_is_counted_and_never_changes_the_last_valid_frame() {
        let port = free_port();
        let receiver = start(&[(Protocol::ArtNet, 7, port)]);
        let sender = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("sender socket");

        let good = deliver(&receiver, &sender, port, &artdmx(7, 1, &[77, 0, 0, 0]));
        assert_eq!(good.slots[0], 77);
        sender
            .send_to(b"not an art-net packet at all", (Ipv4Addr::LOCALHOST, port))
            .expect("send");
        wait_for(|| receiver.status()[0].malformed_packets >= 1);
        assert!(
            receiver.drain_changed().is_empty(),
            "an invalid packet must not produce a new frame"
        );
    }

    #[test]
    fn out_of_order_packets_are_counted_and_discarded() {
        let port = free_port();
        let receiver = start(&[(Protocol::ArtNet, 8, port)]);
        let sender = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("sender socket");

        deliver(&receiver, &sender, port, &artdmx(8, 10, &[10, 0, 0, 0]));
        sender
            .send_to(&artdmx(8, 5, &[99, 0, 0, 0]), (Ipv4Addr::LOCALHOST, port))
            .expect("send");
        wait_for(|| receiver.status()[0].out_of_order_packets >= 1);
        assert!(receiver.drain_changed().is_empty());
    }

    #[test]
    fn several_universes_share_one_listener_and_are_sorted_by_destination() {
        let port = free_port();
        let receiver = start(&[
            (Protocol::ArtNet, 1, port),
            (Protocol::ArtNet, 2, port),
            (Protocol::ArtNet, 3, port),
        ]);
        let sender = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("sender socket");

        let mut seen = std::collections::HashSet::new();
        for _ in 0..100 {
            for universe in 1_u16..=3 {
                sender
                    .send_to(
                        &artdmx(universe, 1, &[universe as u8 * 10, 0, 0, 0]),
                        (Ipv4Addr::LOCALHOST, port),
                    )
                    .expect("send");
            }
            std::thread::sleep(Duration::from_millis(15));
            for frame in receiver.drain_changed() {
                assert_eq!(frame.slots[0], frame.logical_universe as u8 * 10);
                seen.insert(frame.logical_universe);
            }
            if seen.len() == 3 {
                break;
            }
        }
        assert_eq!(seen.len(), 3, "every mapped universe must be delivered");
        assert!(
            receiver
                .status()
                .iter()
                .all(|status| status.malformed_packets == 0),
            "sharing a listener must not create false malformed counts"
        );
    }

    fn start(mappings: &[(Protocol, u16, u16)]) -> DmxReceiver {
        let mappings = mappings
            .iter()
            .map(|(protocol, universe, port)| {
                let mut mapping = InputMapping::loopback(*protocol, *universe, *universe);
                mapping.bind = SocketAddr::from((Ipv4Addr::LOCALHOST, *port));
                mapping
            })
            .collect();
        DmxReceiver::start(mappings, Instant::now())
    }

    fn deliver(
        receiver: &DmxReceiver,
        sender: &UdpSocket,
        port: u16,
        packet: &[u8],
    ) -> UniverseFrame {
        for _ in 0..100 {
            sender
                .send_to(packet, (Ipv4Addr::LOCALHOST, port))
                .expect("send");
            std::thread::sleep(Duration::from_millis(15));
            if let Some(frame) = receiver.drain_changed().into_iter().next() {
                return frame;
            }
        }
        panic!("no frame arrived over loopback");
    }

    fn wait_for(mut condition: impl FnMut() -> bool) {
        for _ in 0..120 {
            if condition() {
                return;
            }
            std::thread::sleep(Duration::from_millis(15));
        }
        panic!("condition never held");
    }

    fn free_port() -> u16 {
        UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))
            .expect("probe socket")
            .local_addr()
            .expect("probe address")
            .port()
    }

    fn artdmx(universe: u16, sequence: u8, slots: &[u8]) -> Vec<u8> {
        let mut packet = Vec::new();
        packet.extend_from_slice(b"Art-Net\0");
        packet.extend_from_slice(&0x5000_u16.to_le_bytes());
        packet.extend_from_slice(&14_u16.to_be_bytes());
        packet.push(sequence);
        packet.push(0);
        packet.extend_from_slice(&universe.to_le_bytes());
        packet.extend_from_slice(&(slots.len() as u16).to_be_bytes());
        packet.extend_from_slice(slots);
        packet
    }
}
