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
}

#[derive(Clone, Debug)]
pub struct EncodedPacket {
    pub protocol: Protocol,
    pub universe: Universe,
    pub destination: SocketAddr,
    pub bytes: Vec<u8>,
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

    pub async fn send_routes(
        &self,
        routes: &[OutputRoute],
        frames: &HashMap<Universe, DmxFrame>,
        sequences: &mut HashMap<(Protocol, Universe), u8>,
    ) -> io::Result<u64> {
        let packets = encode_routes(
            routes,
            frames,
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
            let sequence = next_sequence(sequences, (route.protocol, route.destination_universe));
            match route.protocol {
                Protocol::ArtNet => {
                    if let Some(destination) = route.destination {
                        self.artnet
                            .send_to(
                                &artdmx_packet(
                                    route.destination_universe,
                                    sequence,
                                    &[0; DMX_SLOTS],
                                ),
                                destination,
                            )
                            .await?;
                    }
                }
                Protocol::Sacn => {
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
    sequences: &mut HashMap<(Protocol, Universe), u8>,
    cid: [u8; 16],
    source_name: &str,
    sacn_priority: u8,
) -> io::Result<Vec<EncodedPacket>> {
    routes
        .iter()
        .filter(|route| route.enabled)
        .filter_map(|route| {
            frames
                .get(&route.logical_universe)
                .map(|frame| (route, frame))
        })
        .map(|(route, frame)| {
            let sequence = next_sequence(sequences, (route.protocol, route.destination_universe));
            let (destination, bytes) = match route.protocol {
                Protocol::ArtNet => (
                    route.destination.ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "Art-Net routes require a directed unicast/broadcast destination",
                        )
                    })?,
                    artdmx_packet(route.destination_universe, sequence, frame),
                ),
                Protocol::Sacn => (
                    route
                        .destination
                        .unwrap_or_else(|| sacn_multicast_destination(route.destination_universe)),
                    sacn_data_packet(
                        route.destination_universe,
                        sequence,
                        frame,
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

pub fn artdmx_packet(universe: Universe, sequence: u8, frame: &DmxFrame) -> Vec<u8> {
    let mut packet = Vec::with_capacity(18 + DMX_SLOTS);
    packet.extend_from_slice(b"Art-Net\0");
    packet.extend_from_slice(&0x5000_u16.to_le_bytes());
    packet.extend_from_slice(&14_u16.to_be_bytes());
    packet.push(sequence);
    packet.push(0);
    packet.extend_from_slice(&universe.to_le_bytes());
    packet.extend_from_slice(&(DMX_SLOTS as u16).to_be_bytes());
    packet.extend_from_slice(frame);
    packet
}

pub fn sacn_data_packet(
    universe: Universe,
    sequence: u8,
    frame: &DmxFrame,
    cid: [u8; 16],
    source_name: &str,
    priority: u8,
    stream_terminated: bool,
) -> Vec<u8> {
    const SIZE: usize = 126 + DMX_SLOTS;
    let mut packet = vec![0_u8; SIZE];
    packet[0..2].copy_from_slice(&0x0010_u16.to_be_bytes());
    packet[4..16].copy_from_slice(b"ASC-E1.17\0\0\0");
    set_flags_and_length(&mut packet[16..18], SIZE - 16);
    packet[18..22].copy_from_slice(&0x0000_0004_u32.to_be_bytes());
    packet[22..38].copy_from_slice(&cid);
    set_flags_and_length(&mut packet[38..40], SIZE - 38);
    packet[40..44].copy_from_slice(&0x0000_0002_u32.to_be_bytes());
    let source = source_name.as_bytes();
    packet[44..44 + source.len().min(63)].copy_from_slice(&source[..source.len().min(63)]);
    packet[108] = priority;
    packet[111] = sequence;
    packet[112] = if stream_terminated { 0x40 } else { 0 };
    packet[113..115].copy_from_slice(&universe.to_be_bytes());
    set_flags_and_length(&mut packet[115..117], SIZE - 115);
    packet[117] = 0x02;
    packet[118] = 0xa1;
    packet[121..123].copy_from_slice(&1_u16.to_be_bytes());
    packet[123..125].copy_from_slice(&((DMX_SLOTS + 1) as u16).to_be_bytes());
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
            },
            OutputRoute {
                protocol: Protocol::Sacn,
                logical_universe: 1,
                destination_universe: 20,
                destination: None,
                enabled: true,
            },
            OutputRoute {
                protocol: Protocol::Sacn,
                logical_universe: 2,
                destination_universe: 21,
                destination: None,
                enabled: false,
            },
        ];
        let frames = HashMap::from([(1, [0x33; DMX_SLOTS])]);
        let mut sequences = HashMap::new();
        let packets =
            encode_routes(&routes, &frames, &mut sequences, [1; 16], "Light", 100).unwrap();
        assert_eq!(packets.len(), 2);
        assert_eq!(packets[0].universe, 10);
        assert_eq!(packets[0].bytes[18], 0x33);
        assert_eq!(packets[1].destination, sacn_multicast_destination(20));
        assert_eq!(packets[1].bytes[126], 0x33);
    }

    #[test]
    fn artnet_route_requires_an_explicit_destination() {
        let routes = [OutputRoute {
            protocol: Protocol::ArtNet,
            logical_universe: 1,
            destination_universe: 1,
            destination: None,
            enabled: true,
        }];
        let frames = HashMap::from([(1, [0; DMX_SLOTS])]);
        assert!(
            encode_routes(&routes, &frames, &mut HashMap::new(), [0; 16], "Light", 100).is_err()
        );
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
