//! What reaches this machine over Art-Net and sACN, for the DMX screen's Values tab.
//!
//! The Architect outputs no DMX and has no desk behind it, so received values are the only DMX it
//! can show. It listens exactly where the Visualizer listens — the show's output routes, its Live
//! DMX Inputs over those, and the Art-Net and sACN defaults for every patched universe when
//! neither names any, each on the interface this machine chose for its protocol — and only while
//! the Values tab asks. The receiver shares its ports, so a Visualizer on the same machine keeps
//! receiving beside it.

use crate::session::Session;
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;
use viz_desk::wire::ObjectRecord;
use viz_dmx::{DMX_SLOTS, DmxReceiver, InputMapping, ListenInterfaces};
use viz_document::{LIVE_DMX_INPUT_KIND, PlanningDocument};
use viz_scene::{InputHealth, SourceProtocol};

/// The receiver the Values tab is reading, if it is open.
#[derive(Default)]
pub struct DmxInputMonitor {
    listening: Mutex<Option<Listening>>,
}

struct Listening {
    mappings: Vec<InputMapping>,
    receiver: DmxReceiver,
    /// The newest frame of every universe that has delivered one. A source that stops leaves its
    /// last frame here, exactly as the Visualizer holds the last look it received.
    frames: BTreeMap<u16, [u8; DMX_SLOTS]>,
}

/// One read of everything received, for the Values tab.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceivedDmx {
    pub universes: Vec<ReceivedUniverse>,
    pub inputs: Vec<ReceivingInput>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceivedUniverse {
    pub universe: u16,
    /// Absent until the first valid frame arrived.
    pub slots: Option<Vec<u8>>,
    /// Whether a source is delivering this universe now, rather than having stopped.
    pub live: bool,
    pub rate_hz: f32,
    pub protocol: Option<&'static str>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceivingInput {
    pub id: String,
    pub protocol: &'static str,
    pub logical_universe: u16,
    pub destination_universe: u16,
    pub delivery: String,
    pub bind: String,
    pub health: &'static str,
    pub source: Option<String>,
    pub accepted_packets: u64,
    pub detail: String,
}

impl DmxInputMonitor {
    /// Listen on `mappings` and report what arrived.
    ///
    /// The sockets stay open while the mappings stay the same, so a held universe keeps its frame
    /// between reads; a changed show rebinds and keeps the frames of universes it still reads.
    pub fn read(&self, mappings: Vec<InputMapping>, warnings: Vec<String>) -> ReceivedDmx {
        let mut current = self.listening.lock();
        if current
            .as_ref()
            .is_none_or(|listening| listening.mappings != mappings)
        {
            let mut frames = current
                .take()
                .map(|mut previous| {
                    // Close the old sockets before the new ones bind to the same ports.
                    previous.receiver.shutdown();
                    previous.frames
                })
                .unwrap_or_default();
            frames.retain(|universe, _| {
                mappings
                    .iter()
                    .any(|mapping| mapping.logical_universe == *universe)
            });
            *current = Some(Listening {
                receiver: DmxReceiver::start(mappings.clone(), Instant::now()),
                mappings,
                frames,
            });
        }
        let listening = current.as_mut().expect("the receiver was just started");
        for frame in listening.receiver.drain_changed() {
            if frame.received_micros > 0 {
                listening.frames.insert(frame.logical_universe, frame.slots);
            }
        }
        listening.report(warnings)
    }

    /// Close every socket. Nothing listens while the Values tab is not open.
    pub fn stop(&self) {
        if let Some(mut listening) = self.listening.lock().take() {
            listening.receiver.shutdown();
        }
    }
}

impl Listening {
    fn report(&self, warnings: Vec<String>) -> ReceivedDmx {
        let health: BTreeMap<u16, _> = self
            .receiver
            .universes()
            .into_iter()
            .map(|universe| (universe.universe, universe))
            .collect();
        let universes: BTreeSet<u16> = self
            .mappings
            .iter()
            .filter(|mapping| mapping.enabled)
            .map(|mapping| mapping.logical_universe)
            .chain(self.frames.keys().copied())
            .collect();
        ReceivedDmx {
            universes: universes
                .into_iter()
                .map(|universe| {
                    let health = health.get(&universe);
                    ReceivedUniverse {
                        universe,
                        slots: self.frames.get(&universe).map(|frame| frame.to_vec()),
                        live: health.is_some_and(|health| !health.stale),
                        rate_hz: health.map(|health| health.rate_hz).unwrap_or_default(),
                        protocol: health
                            .and_then(|health| health.protocol)
                            .map(source_protocol),
                    }
                })
                .collect(),
            inputs: self
                .receiver
                .status()
                .into_iter()
                .map(|status| ReceivingInput {
                    id: status.mapping_id,
                    protocol: source_protocol(status.protocol),
                    logical_universe: status.logical_universe,
                    destination_universe: status.destination_universe,
                    delivery: status.delivery,
                    bind: status.bind,
                    health: health_label(status.health),
                    source: source(status.source_name, status.source_address),
                    accepted_packets: status.accepted_packets,
                    detail: status.detail,
                })
                .collect(),
            warnings,
        }
    }
}

/// Where the Visualizer would listen for this document, on the interfaces this machine chose.
pub fn listening_mappings(
    document: &PlanningDocument,
    interfaces: &ListenInterfaces,
) -> Result<(Vec<InputMapping>, Vec<String>), String> {
    // Output routes are stored as show objects of kind `route`, as on the desk.
    let routes = records(document, "route")?;
    let inputs = records(document, LIVE_DMX_INPUT_KIND)?;
    let (mut mappings, mut warnings) =
        viz_desk::apply_document_inputs(viz_desk::mappings(&routes, None), &inputs, None);
    if mappings.is_empty() {
        let universes = patched_universes(document)?;
        if !universes.is_empty() {
            warnings.push(
                "The show configures no DMX inputs or output routes; listening on the Art-Net and sACN defaults."
                    .into(),
            );
        }
        mappings = viz_desk::default_mappings(&universes, None);
    }
    let (mappings, interface_warnings) = viz_dmx::listen_on_this_machine(mappings, interfaces);
    warnings.extend(interface_warnings);
    Ok((mappings, warnings))
}

fn records(document: &PlanningDocument, kind: &str) -> Result<Vec<ObjectRecord>, String> {
    Ok(document
        .objects(kind)
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|object| ObjectRecord {
            id: object.id,
            revision: object.revision,
            body: object.body,
        })
        .collect())
}

/// Every universe a fixture, one of its splits, or one of its multi-patches is patched to.
fn patched_universes(document: &PlanningDocument) -> Result<Vec<u16>, String> {
    let snapshot = document
        .patch_snapshot()
        .map_err(|error| error.to_string())?;
    let universes: BTreeSet<u16> = snapshot
        .fixtures
        .iter()
        .flat_map(|fixture| {
            let patch = &fixture.patch;
            let own = patch.universe.into_iter().chain(
                patch
                    .split_patches
                    .iter()
                    .filter_map(|split| split.universe),
            );
            let copies = patch.multipatch.iter().flat_map(|copy| {
                copy.universe
                    .into_iter()
                    .chain(copy.split_patches.iter().filter_map(|split| split.universe))
            });
            own.chain(copies).collect::<Vec<_>>()
        })
        .collect();
    Ok(universes.into_iter().collect())
}

/// The interfaces this machine receives Art-Net and sACN on, which the Visualizer shares.
fn renderer_interfaces(session: &Session) -> ListenInterfaces {
    session
        .scene_source()
        .renderer_settings()
        .map(|update| ListenInterfaces {
            art_net: update.settings.art_net_interface,
            sacn: update.settings.sacn_interface,
        })
        .unwrap_or_default()
}

fn source_protocol(protocol: SourceProtocol) -> &'static str {
    match protocol {
        SourceProtocol::ArtNet => "artnet",
        SourceProtocol::Sacn => "sacn",
    }
}

fn health_label(health: InputHealth) -> &'static str {
    health.label()
}

fn source(name: String, address: Option<String>) -> Option<String> {
    match (name.trim(), address) {
        ("", None) => None,
        ("", Some(address)) => Some(address),
        (name, None) => Some(name.to_owned()),
        (name, Some(address)) => Some(format!("{name} · {address}")),
    }
}

/// Listen for received DMX and report it. The first call opens the sockets.
#[tauri::command]
pub fn received_dmx(
    session: tauri::State<'_, Session>,
    monitor: tauri::State<'_, DmxInputMonitor>,
) -> Result<ReceivedDmx, String> {
    let interfaces = renderer_interfaces(&session);
    let (mappings, warnings) =
        session.with(|document| listening_mappings(document, &interfaces))?;
    Ok(monitor.read(mappings, warnings))
}

/// Stop listening. The Values tab calls this when it closes.
#[tauri::command]
pub fn stop_received_dmx(monitor: tauri::State<'_, DmxInputMonitor>) {
    monitor.stop();
}

/// One IPv4 address of one of this machine's network interfaces.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkInterface {
    pub name: String,
    pub address: String,
    pub netmask: String,
    pub loopback: bool,
}

/// This machine's interfaces, for choosing where Art-Net and sACN are received.
#[tauri::command]
pub fn network_interfaces() -> Result<Vec<NetworkInterface>, String> {
    Ok(viz_dmx::network_interfaces()?
        .into_iter()
        .map(|interface| NetworkInterface {
            name: interface.name,
            address: interface.address.to_string(),
            netmask: interface.netmask.to_string(),
            loopback: interface.loopback,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
    use std::time::Duration;
    use viz_dmx::{Delivery, Protocol};

    fn free_port() -> u16 {
        UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))
            .and_then(|socket| socket.local_addr())
            .map(|address| address.port())
            .expect("a free UDP port")
    }

    fn artnet_mapping(logical: u16, wire: u16, port: u16) -> InputMapping {
        InputMapping {
            id: format!("test-u{logical}"),
            protocol: Protocol::ArtNet,
            logical_universe: logical,
            destination_universe: wire,
            delivery: Delivery::Unicast,
            bind: SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
            priority: 100,
            enabled: true,
        }
    }

    /// An ArtDmx packet as a console sends it.
    fn artdmx(universe: u16, sequence: u8, slots: &[u8]) -> Vec<u8> {
        let mut packet = b"Art-Net\0".to_vec();
        packet.extend_from_slice(&0x5000_u16.to_le_bytes());
        packet.extend_from_slice(&[0, 14, sequence, 0]);
        packet.extend_from_slice(&universe.to_le_bytes());
        packet.extend_from_slice(&(slots.len() as u16).to_be_bytes());
        packet.extend_from_slice(slots);
        packet
    }

    fn read_until(
        monitor: &DmxInputMonitor,
        mappings: &[InputMapping],
        done: impl Fn(&ReceivedDmx) -> bool,
    ) -> ReceivedDmx {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let received = monitor.read(mappings.to_vec(), Vec::new());
            if done(&received) || Instant::now() > deadline {
                return received;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn a_mapped_universe_that_has_sent_nothing_is_reported_without_values() {
        let monitor = DmxInputMonitor::default();
        let received = monitor.read(vec![artnet_mapping(4, 3, free_port())], Vec::new());
        assert_eq!(received.universes.len(), 1);
        assert_eq!(received.universes[0].universe, 4);
        assert!(received.universes[0].slots.is_none());
        assert!(!received.universes[0].live);
        assert_eq!(received.inputs.len(), 1);
        assert_eq!(received.inputs[0].health, "Waiting for DMX");
        monitor.stop();
    }

    #[test]
    fn received_art_net_is_reported_on_its_show_universe() {
        let port = free_port();
        let mappings = vec![artnet_mapping(2, 7, port)];
        let monitor = DmxInputMonitor::default();
        monitor.read(mappings.clone(), Vec::new());
        let sender = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("sender socket");
        let mut slots = vec![0_u8; 512];
        slots[0] = 255;
        slots[9] = 128;
        let received = read_until(&monitor, &mappings, |received| {
            sender
                .send_to(&artdmx(7, 1, &slots), (Ipv4Addr::LOCALHOST, port))
                .expect("send");
            received.universes[0].slots.is_some()
        });
        let universe = &received.universes[0];
        assert_eq!(universe.universe, 2);
        let values = universe.slots.as_ref().expect("received values");
        assert_eq!((values[0], values[9], values[10]), (255, 128, 0));
        assert!(universe.live);
        assert_eq!(universe.protocol, Some("artnet"));
        monitor.stop();
    }

    #[test]
    fn the_last_frame_is_held_between_reads_while_the_show_is_unchanged() {
        let port = free_port();
        let mappings = vec![artnet_mapping(1, 1, port)];
        let monitor = DmxInputMonitor::default();
        monitor.read(mappings.clone(), Vec::new());
        let sender = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("sender socket");
        read_until(&monitor, &mappings, |received| {
            sender
                .send_to(&artdmx(1, 1, &[42; 512]), (Ipv4Addr::LOCALHOST, port))
                .expect("send");
            received.universes[0].slots.is_some()
        });
        // Nothing new arrives; the next read still has the frame.
        let again = monitor.read(mappings, Vec::new());
        assert_eq!(
            again.universes[0].slots.as_ref().map(|slots| slots[0]),
            Some(42)
        );
        monitor.stop();
    }

    #[test]
    fn the_interfaces_are_read_from_the_editor_settings() {
        let session = Session::default();
        assert_eq!(renderer_interfaces(&session), ListenInterfaces::default());
        let settings = viz_scene::RendererSettings {
            art_net_interface: Some("en5".into()),
            ..viz_scene::RendererSettings::default()
        };
        session
            .scene_source()
            .set_renderer_settings("test", settings)
            .expect("settings");
        assert_eq!(
            renderer_interfaces(&session),
            ListenInterfaces {
                art_net: Some("en5".into()),
                sacn: None,
            }
        );
    }

    #[test]
    fn this_machine_lists_its_interfaces_by_name_and_address() {
        let listed = network_interfaces().expect("interfaces");
        assert!(listed.iter().all(|interface| !interface.name.is_empty()));
        assert!(
            listed
                .iter()
                .all(|interface| interface.address.parse::<Ipv4Addr>().is_ok())
        );
    }

    #[test]
    fn source_names_the_sender_and_its_address() {
        assert_eq!(source(String::new(), None), None);
        assert_eq!(
            source("Desk".into(), Some("10.0.0.4:6454".into())),
            Some("Desk · 10.0.0.4:6454".into())
        );
        assert_eq!(
            source(" ".into(), Some("10.0.0.4:6454".into())),
            Some("10.0.0.4:6454".into())
        );
    }
}
