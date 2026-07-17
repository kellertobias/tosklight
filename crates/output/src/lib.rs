#![forbid(unsafe_code)]
//! DMX frame scheduling and production Art-Net 4 / ANSI E1.31 output.

use async_trait::async_trait;
use light_core::Universe;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    io,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::atomic::{AtomicU16, AtomicU64, Ordering},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::net::UdpSocket;
use tokio_util::sync::CancellationToken;

pub const DMX_SLOTS: usize = 512;
pub const ARTNET_PORT: u16 = 6454;
pub const SACN_PORT: u16 = 5568;
pub type DmxFrame = [u8; DMX_SLOTS];

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    ArtNet,
    Sacn,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OutputRoute {
    pub protocol: Protocol,
    pub logical_universe: Universe,
    pub destination_universe: Universe,
    pub destination: Option<SocketAddr>,
    pub enabled: bool,
    /// Smallest DMX payload emitted for this route. Historical routes omitted this field and
    /// continue to emit full universes for wire compatibility.
    #[serde(default = "legacy_route_minimum_slots")]
    pub minimum_slots: u16,
}

const fn legacy_route_minimum_slots() -> u16 {
    DMX_SLOTS as u16
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct OutputHealth {
    pub frames_sent: u64,
    pub packets_sent: u64,
    pub send_errors: u64,
    pub deadline_misses: u64,
    pub maximum_lateness_micros: u64,
    pub frame_hz: f32,
    pub last_tick_micros: u64,
    pub maximum_tick_micros: u64,
    pub scheduler_utilization: f32,
}

#[async_trait]
pub trait OutputDriver: Send + Sync {
    async fn send(&self, universe: Universe, sequence: u8, frame: &DmxFrame) -> io::Result<()>;

    async fn terminate(&self, universe: Universe, sequence: u8) -> io::Result<()> {
        self.send(universe, sequence, &[0; DMX_SLOTS]).await
    }
}

pub struct ArtNetDriver {
    socket: UdpSocket,
    destination: SocketAddr,
}

impl ArtNetDriver {
    pub async fn bind(bind: SocketAddr, destination: SocketAddr) -> io::Result<Self> {
        let socket = UdpSocket::bind(bind).await?;
        if matches!(destination.ip(), IpAddr::V4(address) if address.is_broadcast()) {
            socket.set_broadcast(true)?;
        }
        Ok(Self {
            socket,
            destination,
        })
    }
}

#[async_trait]
impl OutputDriver for ArtNetDriver {
    async fn send(&self, universe: Universe, sequence: u8, frame: &DmxFrame) -> io::Result<()> {
        let packet = artdmx_packet(universe, sequence, frame);
        self.socket.send_to(&packet, self.destination).await?;
        Ok(())
    }
}

pub struct SacnDriver {
    socket: UdpSocket,
    cid: [u8; 16],
    source_name: String,
    priority: u8,
    destination: Option<SocketAddr>,
}

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

#[derive(Clone, Debug)]
pub struct EncodedPacket {
    pub protocol: Protocol,
    pub universe: Universe,
    pub destination: SocketAddr,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RouteSendError {
    pub protocol: Protocol,
    pub universe: Universe,
    pub destination: SocketAddr,
    pub errors: u64,
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
        let mut errors = self
            .route_send_errors
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
            .collect::<Vec<_>>();
        errors.sort_by_key(|error| (error.protocol as u8, error.universe, error.destination));
        errors
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
        let mut sent = 0_u64;
        let mut first_error = None;
        for packet in &packets {
            let injected = self
                .injected_failures
                .lock()
                .expect("output failure mutex poisoned")
                .contains(&packet.destination);
            let result = if injected {
                Err(io::Error::other(format!(
                    "injected output failure for {}",
                    packet.destination
                )))
            } else {
                match packet.protocol {
                    Protocol::ArtNet => {
                        self.artnet.send_to(&packet.bytes, packet.destination).await
                    }
                    Protocol::Sacn => self.sacn.send_to(&packet.bytes, packet.destination).await,
                }
            };
            match result {
                Ok(_) => sent += 1,
                Err(error) => {
                    self.send_errors.fetch_add(1, Ordering::Relaxed);
                    *self
                        .route_send_errors
                        .lock()
                        .expect("route output failure mutex poisoned")
                        .entry((packet.protocol, packet.universe, packet.destination))
                        .or_default() += 1;
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
            };
        }
        if sent == 0
            && let Some(error) = first_error
        {
            return Err(error);
        }
        Ok(sent)
    }

    pub async fn terminate_routes(
        &self,
        routes: &[OutputRoute],
        sequences: &mut HashMap<(Protocol, Universe), u8>,
    ) -> io::Result<()> {
        for route in routes.iter().filter(|route| route.enabled) {
            match route.protocol {
                // Art-Net has no stream-termination packet. Disabling must relinquish the
                // universe immediately so another desk can take over without a final black frame.
                Protocol::ArtNet => {}
                Protocol::Sacn => {
                    let sequence =
                        next_sequence(sequences, (route.protocol, route.destination_universe));
                    let destination = route
                        .destination
                        .unwrap_or_else(|| sacn_multicast_destination(route.destination_universe));
                    let packet = sacn_data_packet(
                        route.destination_universe,
                        sequence,
                        &[0; DMX_SLOTS],
                        self.cid,
                        &self.source_name,
                        self.sacn_priority,
                        true,
                    );
                    for _ in 0..3 {
                        self.sacn.send_to(&packet, destination).await?;
                    }
                }
            }
        }
        Ok(())
    }
}

pub fn encode_routes(
    routes: &[OutputRoute],
    frames: &HashMap<Universe, DmxFrame>,
    patched_slots: &HashMap<Universe, u16>,
    sequences: &mut HashMap<(Protocol, Universe), u8>,
    cid: [u8; 16],
    source_name: &str,
    sacn_priority: u8,
) -> io::Result<Vec<EncodedPacket>> {
    routes
        .iter()
        .filter(|route| route.enabled)
        .map(|route| {
            let empty = [0; DMX_SLOTS];
            let frame = frames.get(&route.logical_universe).unwrap_or(&empty);
            let minimum_slots = usize::from(route.minimum_slots.clamp(1, DMX_SLOTS as u16));
            let patched_slots = usize::from(
                patched_slots
                    .get(&route.logical_universe)
                    .copied()
                    .unwrap_or_default(),
            );
            let mut slot_count = minimum_slots.max(patched_slots).min(DMX_SLOTS);
            if route.protocol == Protocol::ArtNet && slot_count % 2 != 0 {
                slot_count = (slot_count + 1).min(DMX_SLOTS);
            }
            let payload = &frame[..slot_count];
            let sequence = next_sequence(sequences, (route.protocol, route.destination_universe));
            let (destination, bytes) = match route.protocol {
                Protocol::ArtNet => (
                    route
                        .destination
                        .unwrap_or_else(artnet_broadcast_destination),
                    artdmx_packet(route.destination_universe, sequence, payload),
                ),
                Protocol::Sacn => (
                    route
                        .destination
                        .unwrap_or_else(|| sacn_multicast_destination(route.destination_universe)),
                    sacn_data_packet(
                        route.destination_universe,
                        sequence,
                        payload,
                        cid,
                        source_name,
                        sacn_priority,
                        false,
                    ),
                ),
            };
            Ok(EncodedPacket {
                protocol: route.protocol,
                universe: route.destination_universe,
                destination,
                bytes,
            })
        })
        .collect()
}

impl SacnDriver {
    pub async fn bind(
        bind: SocketAddr,
        cid: [u8; 16],
        source_name: impl Into<String>,
        priority: u8,
        destination: Option<SocketAddr>,
    ) -> io::Result<Self> {
        Ok(Self {
            socket: UdpSocket::bind(bind).await?,
            cid,
            source_name: source_name.into(),
            priority,
            destination,
        })
    }

    fn destination_for(&self, universe: Universe) -> SocketAddr {
        self.destination
            .unwrap_or_else(|| sacn_multicast_destination(universe))
    }
}

pub fn sacn_multicast_destination(universe: Universe) -> SocketAddr {
    SocketAddr::new(
        IpAddr::V4(Ipv4Addr::new(
            239,
            255,
            (universe >> 8) as u8,
            universe as u8,
        )),
        SACN_PORT,
    )
}

pub fn artnet_broadcast_destination() -> SocketAddr {
    SocketAddr::from((Ipv4Addr::BROADCAST, ARTNET_PORT))
}

#[async_trait]
impl OutputDriver for SacnDriver {
    async fn send(&self, universe: Universe, sequence: u8, frame: &DmxFrame) -> io::Result<()> {
        let packet = sacn_data_packet(
            universe,
            sequence,
            frame,
            self.cid,
            &self.source_name,
            self.priority,
            false,
        );
        self.socket
            .send_to(&packet, self.destination_for(universe))
            .await?;
        Ok(())
    }

    async fn terminate(&self, universe: Universe, sequence: u8) -> io::Result<()> {
        // E1.31 requires three stream-terminated packets for reliable receivers.
        let packet = sacn_data_packet(
            universe,
            sequence,
            &[0; DMX_SLOTS],
            self.cid,
            &self.source_name,
            self.priority,
            true,
        );
        for _ in 0..3 {
            self.socket
                .send_to(&packet, self.destination_for(universe))
                .await?;
        }
        Ok(())
    }
}

/// A scheduler tick is isolated from persistence and API work. Frames are supplied as immutable
/// snapshots and each output protocol gets its own monotonically wrapping sequence number.
pub async fn run_scheduler<F, Fut>(
    rate_hz: u16,
    cancel: CancellationToken,
    health: Arc<Mutex<OutputHealth>>,
    tick: F,
) where
    F: FnMut() -> Fut,
    Fut: Future<Output = io::Result<u64>>,
{
    run_scheduler_dynamic(Arc::new(AtomicU16::new(rate_hz)), cancel, health, tick).await
}

pub async fn run_scheduler_dynamic<F, Fut>(
    rate_hz: Arc<AtomicU16>,
    cancel: CancellationToken,
    health: Arc<Mutex<OutputHealth>>,
    mut tick: F,
) where
    F: FnMut() -> Fut,
    Fut: Future<Output = io::Result<u64>>,
{
    let mut deadline = Instant::now();
    while !cancel.is_cancelled() {
        let current_rate = rate_hz.load(Ordering::Relaxed).clamp(40, 44);
        let interval = Duration::from_secs_f64(1.0 / f64::from(current_rate));
        let tick_started = Instant::now();
        match tick().await {
            Ok(packets) => {
                let mut current = health.lock().expect("output health mutex poisoned");
                current.frames_sent += 1;
                current.packets_sent += packets;
                current.frame_hz = f32::from(current_rate);
            }
            Err(_) => {
                health
                    .lock()
                    .expect("output health mutex poisoned")
                    .send_errors += 1
            }
        }
        let tick_micros = tick_started.elapsed().as_micros() as u64;
        {
            let mut current = health.lock().expect("output health mutex poisoned");
            current.last_tick_micros = tick_micros;
            current.maximum_tick_micros = current.maximum_tick_micros.max(tick_micros);
            current.scheduler_utilization =
                (tick_started.elapsed().as_secs_f64() / interval.as_secs_f64()) as f32;
        }
        deadline += interval;
        let now = Instant::now();
        if now > deadline {
            let lateness = now.duration_since(deadline).as_micros() as u64;
            let mut current = health.lock().expect("output health mutex poisoned");
            current.deadline_misses += 1;
            current.maximum_lateness_micros = current.maximum_lateness_micros.max(lateness);
            deadline = now;
        } else {
            tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)).await;
        }
    }
}

pub fn next_sequence(
    sequences: &mut HashMap<(Protocol, Universe), u8>,
    key: (Protocol, Universe),
) -> u8 {
    let sequence = sequences.entry(key).or_insert(0);
    *sequence = sequence.wrapping_add(1);
    if *sequence == 0 {
        *sequence = 1;
    }
    *sequence
}

pub fn artdmx_packet(universe: Universe, sequence: u8, frame: &[u8]) -> Vec<u8> {
    let mut packet = Vec::with_capacity(18 + frame.len());
    packet.extend_from_slice(b"Art-Net\0");
    packet.extend_from_slice(&0x5000_u16.to_le_bytes());
    packet.extend_from_slice(&14_u16.to_be_bytes());
    packet.push(sequence);
    packet.push(0);
    packet.extend_from_slice(&universe.to_le_bytes());
    packet.extend_from_slice(&(frame.len() as u16).to_be_bytes());
    packet.extend_from_slice(frame);
    packet
}

pub fn sacn_data_packet(
    universe: Universe,
    sequence: u8,
    frame: &[u8],
    cid: [u8; 16],
    source_name: &str,
    priority: u8,
    stream_terminated: bool,
) -> Vec<u8> {
    let size = 126 + frame.len();
    let mut packet = vec![0_u8; size];
    packet[0..2].copy_from_slice(&0x0010_u16.to_be_bytes());
    packet[4..16].copy_from_slice(b"ASC-E1.17\0\0\0");
    set_flags_and_length(&mut packet[16..18], size - 16);
    packet[18..22].copy_from_slice(&0x0000_0004_u32.to_be_bytes());
    packet[22..38].copy_from_slice(&cid);
    set_flags_and_length(&mut packet[38..40], size - 38);
    packet[40..44].copy_from_slice(&0x0000_0002_u32.to_be_bytes());
    let source = source_name.as_bytes();
    packet[44..44 + source.len().min(63)].copy_from_slice(&source[..source.len().min(63)]);
    packet[108] = priority;
    packet[111] = sequence;
    packet[112] = if stream_terminated { 0x40 } else { 0 };
    packet[113..115].copy_from_slice(&universe.to_be_bytes());
    set_flags_and_length(&mut packet[115..117], size - 115);
    packet[117] = 0x02;
    packet[118] = 0xa1;
    packet[121..123].copy_from_slice(&1_u16.to_be_bytes());
    packet[123..125].copy_from_slice(&((frame.len() + 1) as u16).to_be_bytes());
    packet[125] = 0; // DMX start code
    packet[126..].copy_from_slice(frame);
    packet
}

fn set_flags_and_length(target: &mut [u8], length: usize) {
    target.copy_from_slice(&(0x7000_u16 | length as u16).to_be_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artdmx_packet_matches_protocol_fields() {
        let mut frame = [0; DMX_SLOTS];
        frame[0] = 0xaa;
        let packet = artdmx_packet(0x1234, 7, &frame);
        assert_eq!(packet.len(), 530);
        assert_eq!(&packet[..8], b"Art-Net\0");
        assert_eq!(&packet[8..10], &[0x00, 0x50]);
        assert_eq!(&packet[10..12], &[0x00, 0x0e]);
        assert_eq!(packet[12], 7);
        assert_eq!(&packet[14..16], &[0x34, 0x12]);
        assert_eq!(&packet[16..18], &[0x02, 0x00]);
        assert_eq!(packet[18], 0xaa);
    }

    #[test]
    fn sacn_packet_matches_e131_layers() {
        let packet = sacn_data_packet(0x1234, 9, &[0x55; DMX_SLOTS], [1; 16], "Light", 100, false);
        assert_eq!(packet.len(), 638);
        assert_eq!(&packet[4..16], b"ASC-E1.17\0\0\0");
        assert_eq!(&packet[18..22], &[0, 0, 0, 4]);
        assert_eq!(&packet[40..44], &[0, 0, 0, 2]);
        assert_eq!(packet[108], 100);
        assert_eq!(packet[111], 9);
        assert_eq!(&packet[113..115], &[0x12, 0x34]);
        assert_eq!(packet[117], 0x02);
        assert_eq!(packet[118], 0xa1);
        assert_eq!(&packet[123..125], &[0x02, 0x01]);
        assert_eq!(packet[125], 0);
        assert_eq!(packet[126], 0x55);
    }

    #[test]
    fn sacn_termination_sets_option() {
        let packet = sacn_data_packet(1, 1, &[0; DMX_SLOTS], [0; 16], "Light", 100, true);
        assert_eq!(packet[112], 0x40);
    }

    #[test]
    fn sequence_never_emits_zero_after_wrap() {
        let mut values = HashMap::from([((Protocol::ArtNet, 1), 255)]);
        assert_eq!(next_sequence(&mut values, (Protocol::ArtNet, 1)), 1);
    }

    #[test]
    fn route_encoding_maps_logical_to_protocol_universes() {
        let routes = vec![
            OutputRoute {
                protocol: Protocol::ArtNet,
                logical_universe: 1,
                destination_universe: 10,
                destination: Some("127.0.0.1:6454".parse().unwrap()),
                enabled: true,
                minimum_slots: 512,
            },
            OutputRoute {
                protocol: Protocol::Sacn,
                logical_universe: 1,
                destination_universe: 20,
                destination: None,
                enabled: true,
                minimum_slots: 512,
            },
            OutputRoute {
                protocol: Protocol::Sacn,
                logical_universe: 2,
                destination_universe: 21,
                destination: None,
                enabled: false,
                minimum_slots: 512,
            },
        ];
        let frames = HashMap::from([(1, [0x33; DMX_SLOTS])]);
        let mut sequences = HashMap::new();
        let packets = encode_routes(
            &routes,
            &frames,
            &HashMap::from([(1, 512)]),
            &mut sequences,
            [1; 16],
            "Light",
            100,
        )
        .unwrap();
        assert_eq!(packets.len(), 2);
        assert_eq!(packets[0].universe, 10);
        assert_eq!(packets[0].bytes[18], 0x33);
        assert_eq!(packets[1].destination, sacn_multicast_destination(20));
        assert_eq!(packets[1].bytes[126], 0x33);
    }

    #[test]
    fn artnet_route_without_an_explicit_destination_uses_standard_broadcast() {
        let routes = [OutputRoute {
            protocol: Protocol::ArtNet,
            logical_universe: 1,
            destination_universe: 1,
            destination: None,
            enabled: true,
            minimum_slots: 512,
        }];
        let frames = HashMap::from([(1, [0; DMX_SLOTS])]);
        let packets = encode_routes(
            &routes,
            &frames,
            &HashMap::new(),
            &mut HashMap::new(),
            [0; 16],
            "Light",
            100,
        )
        .unwrap();
        assert_eq!(packets[0].destination, artnet_broadcast_destination());
    }

    #[test]
    fn enabled_empty_routes_emit_their_minimum_and_patched_zero_slots_extend_payloads() {
        let routes = [
            OutputRoute {
                protocol: Protocol::ArtNet,
                logical_universe: 32,
                destination_universe: 10,
                destination: Some("127.0.0.1:6454".parse().unwrap()),
                enabled: true,
                minimum_slots: 128,
            },
            OutputRoute {
                protocol: Protocol::Sacn,
                logical_universe: 33,
                destination_universe: 101,
                destination: Some("127.0.0.1:5568".parse().unwrap()),
                enabled: true,
                minimum_slots: 128,
            },
        ];
        let packets = encode_routes(
            &routes,
            &HashMap::from([(33, [0; DMX_SLOTS])]),
            &HashMap::from([(33, 201)]),
            &mut HashMap::new(),
            [0; 16],
            "Light",
            100,
        )
        .unwrap();

        assert_eq!(packets[0].bytes.len(), 18 + 128);
        assert!(packets[0].bytes[18..].iter().all(|slot| *slot == 0));
        assert_eq!(packets[1].bytes.len(), 126 + 201);
        assert_eq!(&packets[1].bytes[123..125], &202_u16.to_be_bytes());
        assert!(packets[1].bytes[126..].iter().all(|slot| *slot == 0));
    }

    #[test]
    fn legacy_route_without_minimum_slots_keeps_full_universe_payloads() {
        let route: OutputRoute = serde_json::from_value(serde_json::json!({
            "protocol": "art_net",
            "logical_universe": 1,
            "destination_universe": 1,
            "destination": null,
            "enabled": true
        }))
        .unwrap();
        assert_eq!(route.minimum_slots, 512);
    }

    #[tokio::test]
    async fn route_scoped_failure_is_observable_without_stopping_healthy_output() {
        let output = NetworkOutput::bind(IpAddr::V4(Ipv4Addr::LOCALHOST), [7; 16], "Light")
            .await
            .unwrap();
        let healthy = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let failing = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let healthy_destination = healthy.local_addr().unwrap();
        let failing_destination = failing.local_addr().unwrap();
        let routes = [
            OutputRoute {
                protocol: Protocol::ArtNet,
                logical_universe: 1,
                destination_universe: 10,
                destination: Some(healthy_destination),
                enabled: true,
                minimum_slots: 512,
            },
            OutputRoute {
                protocol: Protocol::ArtNet,
                logical_universe: 1,
                destination_universe: 11,
                destination: Some(failing_destination),
                enabled: true,
                minimum_slots: 512,
            },
        ];
        let frames = HashMap::from([(1, [0x44; DMX_SLOTS])]);
        let mut sequences = HashMap::new();

        output.inject_failure(failing_destination, true);
        assert_eq!(
            output
                .send_routes(&routes, &frames, &HashMap::from([(1, 512)]), &mut sequences)
                .await
                .unwrap(),
            1
        );
        let mut healthy_packet = [0_u8; 18 + DMX_SLOTS];
        tokio::time::timeout(
            Duration::from_secs(1),
            healthy.recv_from(&mut healthy_packet),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(healthy_packet[18], 0x44);
        let errors = output.route_send_errors();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].protocol, Protocol::ArtNet);
        assert_eq!(errors[0].universe, 11);
        assert_eq!(errors[0].destination, failing_destination);
        assert_eq!(errors[0].errors, 1);
        assert_eq!(output.take_send_errors(), 1);

        output.inject_failure(failing_destination, false);
        assert_eq!(
            output
                .send_routes(&routes, &frames, &HashMap::from([(1, 512)]), &mut sequences)
                .await
                .unwrap(),
            2
        );
        let mut recovered_packet = [0_u8; 18 + DMX_SLOTS];
        tokio::time::timeout(
            Duration::from_secs(1),
            failing.recv_from(&mut recovered_packet),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(recovered_packet[18], 0x44);
        assert_eq!(output.route_send_errors()[0].errors, 1);
    }

    #[tokio::test]
    async fn scheduler_updates_health_and_stops_on_cancellation() {
        let cancel = CancellationToken::new();
        let stop = cancel.clone();
        let health = Arc::new(Mutex::new(OutputHealth::default()));
        let health_result = Arc::clone(&health);
        let ticks = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let tick_count = Arc::clone(&ticks);
        run_scheduler(44, cancel, health, move || {
            let ticks = Arc::clone(&tick_count);
            let stop = stop.clone();
            async move {
                let count = ticks.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                if count >= 3 {
                    stop.cancel();
                }
                Ok(2)
            }
        })
        .await;
        let result = health_result.lock().unwrap().clone();
        assert_eq!(result.frames_sent, 3);
        assert_eq!(result.packets_sent, 6);
        assert_eq!(result.frame_hz, 44.0);
    }
}
