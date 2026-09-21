use std::collections::HashMap;
use std::io;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};

use light_dmx_wire::{
    ARTNET_PORT, DmxFrame, artdmx_packet, artnet_broadcast_destination, artpollreply_packets,
    is_artpoll, sacn_data_packet, sacn_multicast_destination,
};
use media_application::configuration::{DmxProtocol, PixelOutputRoute};
use media_domain::pixel_map::UniverseFrames;

/// How the Media Server names itself on the DMX network.
///
/// The desk's Nodes view marks an endpoint as ToskLight's own software by matching this name, so
/// it has to stay exactly the one the shared wire contract lists.
const SOFTWARE_NAME: &str = "ToskLight Media Server";
/// Polls answered per listener per frame, so a flood cannot stall the output.
const POLLS_PER_FRAME: usize = 16;

/// The sACN source name for an output the operator labelled `label`.
///
/// The identity goes first because the desk matches it as a prefix; the operator's own label stays
/// after it, so a network with more than one Media Server output still tells them apart.
pub fn sacn_source_name(label: &str) -> String {
    format!("{SOFTWARE_NAME} — {}", label.trim())
}

/// Why a mapped universe could not be sent.
#[derive(Debug, thiserror::Error)]
pub enum PixelSendError {
    #[error("the pixel output socket could not be opened: {0}")]
    Bind(std::io::Error),
    #[error("universe {universe} could not be sent to {destination}: {source}")]
    Send {
        universe: u16,
        destination: SocketAddr,
        #[source]
        source: std::io::Error,
    },
    #[error("route '{route}' names '{destination}', which is not an address this can send to")]
    Destination { route: String, destination: String },
}

/// Where one route's packets go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteDestination {
    pub address: SocketAddr,
    pub broadcast: bool,
}

/// The address a route sends to.
///
/// A route that names no destination takes its protocol's own convention: Art-Net goes out to the
/// broadcast address, sACN to the multicast group that belongs to its universe. Naming one turns
/// either into a unicast to that host, which is how a route reaches a node on a routed network.
pub fn route_destination(route: &PixelOutputRoute) -> Result<RouteDestination, PixelSendError> {
    let Some(destination) = route
        .destination
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return Ok(match route.protocol {
            DmxProtocol::ArtNet => RouteDestination {
                address: artnet_broadcast_destination(),
                broadcast: true,
            },
            DmxProtocol::Sacn => RouteDestination {
                address: sacn_multicast_destination(route.universe),
                broadcast: false,
            },
        });
    };
    let port = match route.protocol {
        DmxProtocol::ArtNet => light_dmx_wire::ARTNET_PORT,
        DmxProtocol::Sacn => light_dmx_wire::SACN_PORT,
    };
    let with_port = if destination.contains(':') {
        destination.to_owned()
    } else {
        format!("{destination}:{port}")
    };
    let address = with_port
        .parse::<SocketAddr>()
        .map_err(|_| PixelSendError::Destination {
            route: route.name.clone(),
            destination: destination.to_owned(),
        })?;
    Ok(RouteDestination {
        address,
        broadcast: false,
    })
}

/// Sends mapped universes on their routes.
pub struct PixelSender {
    socket: UdpSocket,
    cid: [u8; 16],
    /// The operator's own label for the output, as the Art-Net short name announces it.
    label: String,
    /// The announced sACN source name: the ToskLight identity, then the operator's label.
    source_name: String,
    /// Art-Net and sACN both number their packets per universe, and a receiver uses that to spot a
    /// frame that arrived out of order. The count belongs to the sender, not to the frame.
    sequences: HashMap<u16, u8>,
    /// Where controllers' ArtPolls arrive: one socket per lighting network, bound to its broadcast
    /// address so unicast Art-Net meant for another receiver on this computer never lands here.
    poll_listeners: Vec<PollListener>,
    /// Replies sent so far, for the counter an ArtPollReply's node report carries.
    replies: u16,
}

struct PollListener {
    socket: UdpSocket,
    /// The Media Server's own address on that network, as its replies name it.
    address: Ipv4Addr,
}

impl PixelSender {
    /// Opens the sockets one output sends and answers polls on.
    ///
    /// `label` is the operator's name for the output; what goes onto the network is the ToskLight
    /// identity carrying that label, because an endpoint that only ever sends data is otherwise
    /// nameless to everything else on the lighting network.
    pub fn bind(cid: [u8; 16], label: impl Into<String>) -> Result<Self, PixelSendError> {
        let socket = UdpSocket::bind("0.0.0.0:0").map_err(PixelSendError::Bind)?;
        socket.set_broadcast(true).map_err(PixelSendError::Bind)?;
        let label = label.into();
        Ok(Self {
            socket,
            cid,
            source_name: sacn_source_name(&label),
            label,
            sequences: HashMap::new(),
            poll_listeners: broadcast_poll_listeners(),
            replies: 0,
        })
    }

    /// Also answer ArtPolls arriving at `bind`, replying as `address`.
    ///
    /// Test-bench seam: loopback has no broadcast address to listen on.
    pub fn listen_for_art_polls(
        mut self,
        bind: SocketAddr,
        address: Ipv4Addr,
    ) -> Result<Self, PixelSendError> {
        self.poll_listeners
            .push(poll_listener(bind, address).map_err(PixelSendError::Bind)?);
        Ok(self)
    }

    /// The sACN source name this output announces.
    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    /// Sends every universe a route carries.
    ///
    /// A route with nothing mapped onto its universe is skipped rather than sent as darkness: the
    /// map says nothing about that universe, and a server that has not been told about a universe
    /// should not be the one blacking it out.
    pub fn send(
        &mut self,
        routes: &[PixelOutputRoute],
        frames: &UniverseFrames,
    ) -> Vec<PixelSendError> {
        let mut failures = Vec::new();
        for route in routes.iter().filter(|route| route.enabled) {
            let Some(frame) = frames.get(route.universe) else {
                continue;
            };
            if let Err(error) = self.send_one(route, frame) {
                failures.push(error);
            }
        }
        self.answer_art_polls(routes);
        failures
    }

    /// Answers the ArtPolls that arrived since the last frame.
    ///
    /// Answering never fails a frame and is never reported: a lost reply is asked for again by the
    /// next poll, while a failed frame is lost light.
    fn answer_art_polls(&mut self, routes: &[PixelOutputRoute]) {
        let universes = art_net_universes(routes);
        let mut replies_sent = self.replies;
        let mut buffer = [0_u8; 512];
        for listener in &self.poll_listeners {
            for _ in 0..POLLS_PER_FRAME {
                let Ok((length, from)) = listener.socket.recv_from(&mut buffer) else {
                    break;
                };
                if !is_artpoll(&buffer[..length]) {
                    continue;
                }
                replies_sent = replies_sent.wrapping_add(1) % 10_000;
                let report = format!("#0001 [{replies_sent:04}] Pixel output running");
                let replies = artpollreply_packets(
                    listener.address,
                    &self.label,
                    SOFTWARE_NAME,
                    &report,
                    &universes,
                );
                // Replies belong on port 6454; a poller asking from another port hears it there too.
                let mut destinations = vec![SocketAddr::new(from.ip(), ARTNET_PORT)];
                if from.port() != ARTNET_PORT {
                    destinations.push(from);
                }
                // Sent from the output socket: a socket bound to a broadcast address cannot name
                // itself as a unicast sender.
                for destination in destinations {
                    for reply in &replies {
                        let _ = self.socket.send_to(reply, destination);
                    }
                }
            }
        }
        self.replies = replies_sent;
    }

    fn send_one(
        &mut self,
        route: &PixelOutputRoute,
        frame: &DmxFrame,
    ) -> Result<(), PixelSendError> {
        let destination = route_destination(route)?;
        let sequence = self.next_sequence(route.universe);
        let packet = match route.protocol {
            DmxProtocol::ArtNet => artdmx_packet(route.universe, sequence, frame),
            DmxProtocol::Sacn => sacn_data_packet(
                route.universe,
                sequence,
                frame,
                self.cid,
                &self.source_name,
                100,
                false,
            ),
        };
        self.socket
            .send_to(&packet, destination.address)
            .map_err(|source| PixelSendError::Send {
                universe: route.universe,
                destination: destination.address,
                source,
            })?;
        Ok(())
    }

    fn next_sequence(&mut self, universe: u16) -> u8 {
        let sequence = self.sequences.entry(universe).or_insert(0);
        *sequence = sequence.wrapping_add(1);
        *sequence
    }
}

/// The universes this output puts onto the network as Art-Net, as its replies list their ports.
fn art_net_universes(routes: &[PixelOutputRoute]) -> Vec<u16> {
    routes
        .iter()
        .filter(|route| route.enabled && route.protocol == DmxProtocol::ArtNet)
        .map(|route| route.universe)
        .collect()
}

/// One poll listener per network this machine is on, as the desk's output builds them.
fn broadcast_poll_listeners() -> Vec<PollListener> {
    let Ok(interfaces) = if_addrs::get_if_addrs() else {
        return Vec::new();
    };
    interfaces
        .into_iter()
        .filter_map(|interface| match interface.addr {
            if_addrs::IfAddr::V4(address) if !address.ip.is_loopback() => poll_listener(
                SocketAddr::from((address.broadcast?, ARTNET_PORT)),
                address.ip,
            )
            .ok(),
            _ => None,
        })
        .collect()
}

/// A listener the Art-Net port is shared on, so the desk and a Visualizer on this machine keep
/// hearing polls beside it, and one that never blocks a frame waiting for a poll that never comes.
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

#[cfg(test)]
mod tests {
    use super::*;
    use media_domain::pixel_map::{
        CanvasImage, CanvasPoint, PixelLayout, PixelOrder, PixelZone, map_pixels,
    };

    fn route(protocol: DmxProtocol, universe: u16, destination: Option<&str>) -> PixelOutputRoute {
        PixelOutputRoute {
            id: "route".into(),
            name: "Universe".into(),
            protocol,
            universe,
            destination: destination.map(str::to_owned),
            enabled: true,
        }
    }

    #[test]
    fn art_net_broadcasts_when_no_destination_is_named() {
        let resolved = route_destination(&route(DmxProtocol::ArtNet, 3, None)).expect("a default");
        assert_eq!(resolved.address.port(), light_dmx_wire::ARTNET_PORT);
        assert!(resolved.broadcast);
    }

    #[test]
    fn sacn_uses_the_multicast_group_of_its_own_universe() {
        let resolved = route_destination(&route(DmxProtocol::Sacn, 5, None)).expect("a default");
        assert_eq!(resolved.address.ip().to_string(), "239.255.0.5");
        assert_eq!(resolved.address.port(), light_dmx_wire::SACN_PORT);
        assert!(!resolved.broadcast);
    }

    #[test]
    fn naming_a_host_turns_a_route_into_a_unicast() {
        let resolved = route_destination(&route(DmxProtocol::ArtNet, 1, Some("10.0.0.7")))
            .expect("the named host");
        assert_eq!(resolved.address.to_string(), "10.0.0.7:6454");
        assert!(!resolved.broadcast);
        let ported = route_destination(&route(DmxProtocol::Sacn, 1, Some("10.0.0.7:9999")))
            .expect("the named host and port");
        assert_eq!(ported.address.to_string(), "10.0.0.7:9999");
    }

    #[test]
    fn a_destination_that_is_not_an_address_is_refused_by_name() {
        let error = route_destination(&route(DmxProtocol::ArtNet, 1, Some("not a host")))
            .expect_err("a refusal");
        assert!(matches!(error, PixelSendError::Destination { .. }));
    }

    #[test]
    fn a_blank_destination_falls_back_to_the_protocol_default() {
        let resolved =
            route_destination(&route(DmxProtocol::ArtNet, 1, Some("   "))).expect("the default");
        assert!(resolved.broadcast);
    }

    #[test]
    fn a_sent_universe_carries_all_five_hundred_and_twelve_slots() {
        // One RGB pixel mapped onto a white canvas, sent as Art-Net.
        let zone = PixelZone {
            id: "one".into(),
            name: "One".into(),
            start: CanvasPoint::new(0.0, 0.0),
            end: CanvasPoint::new(1.0, 1.0),
            columns: 1,
            rows: 1,
            layout: PixelLayout::rgb(),
            order: PixelOrder::RowMajor,
            universe: 1,
            start_address: 1,
            enabled: true,
        };
        let rgba = vec![255, 255, 255, 255];
        let frames = map_pixels(
            &[zone],
            CanvasImage {
                width: 1,
                height: 1,
                rgba: &rgba,
            },
        );
        let frame = frames.get(1).expect("the universe");
        let packet = artdmx_packet(1, 1, frame);
        // Eighteen bytes of header, then the whole universe.
        assert_eq!(packet.len(), 18 + 512);
        assert_eq!(&packet[18..21], &[255, 255, 255]);
    }

    #[test]
    fn an_sacn_source_is_named_as_tosklight_software_carrying_the_operator_label() {
        let announced = sacn_source_name("Stage Left");
        assert_eq!(announced, "ToskLight Media Server — Stage Left");
        // The desk matches the identity as a prefix; the operator's label stays readable after it.
        assert_eq!(
            light_dmx_wire::tosklight_software(&announced),
            Some("Media Server")
        );
        let sender = PixelSender::bind([0; 16], "Stage Left").expect("a socket");
        assert_eq!(sender.source_name(), announced);
        let packet = sacn_data_packet(1, 1, &[0; 512], [0; 16], sender.source_name(), 100, false);
        assert_eq!(
            light_dmx_wire::decode_sacn_source_packet(&packet)
                .expect("an sACN packet")
                .source_name,
            announced
        );
    }

    #[test]
    fn an_artpoll_is_answered_with_the_media_server_identity() {
        let poller = UdpSocket::bind("127.0.0.1:0").expect("a poller");
        poller
            .set_read_timeout(Some(std::time::Duration::from_millis(50)))
            .expect("a timeout");
        let listen = UdpSocket::bind("127.0.0.1:0")
            .expect("a free port")
            .local_addr()
            .expect("its address");
        let mut sender = PixelSender::bind([0; 16], "Stage Left")
            .expect("a socket")
            .listen_for_art_polls(listen, Ipv4Addr::LOCALHOST)
            .expect("a poll listener");
        let mut poll = [0_u8; 14];
        poll[..8].copy_from_slice(b"Art-Net\0");
        poll[8..10].copy_from_slice(&0x2000_u16.to_le_bytes());
        poller.send_to(&poll, listen).expect("the poll arrives");
        // Polls are answered on the output frame, and the datagram may still be in flight when the
        // first frame looks, so a few frames are given to it.
        let mut buffer = [0_u8; 512];
        let mut received = None;
        for _ in 0..50 {
            sender.send(
                &[route(DmxProtocol::ArtNet, 3, None)],
                &UniverseFrames::default(),
            );
            if let Ok((length, _)) = poller.recv_from(&mut buffer) {
                received = Some(length);
                break;
            }
        }
        let length = received.expect("a reply");
        let (short_name, long_name) =
            light_dmx_wire::artpollreply_names(&buffer[..length]).expect("an ArtPollReply");
        assert_eq!(long_name, "ToskLight Media Server");
        assert_eq!(
            light_dmx_wire::tosklight_software(&long_name),
            Some("Media Server")
        );
        // The operator's own label is the short name, which is too short to hold both.
        assert_eq!(short_name, "Stage Left");
        // The route's universe is announced as a port the Media Server sends.
        assert_eq!(buffer[173], 1);
        assert_eq!(buffer[186], 3);
    }

    #[test]
    fn a_route_whose_universe_is_not_mapped_is_left_alone() {
        let mut sender = PixelSender::bind([0; 16], "Media").expect("a socket");
        let frames = UniverseFrames::default();
        // Nothing is mapped, so nothing is sent and nothing fails.
        assert!(
            sender
                .send(&[route(DmxProtocol::ArtNet, 1, None)], &frames)
                .is_empty()
        );
    }

    #[test]
    fn sequence_numbers_advance_per_universe_and_wrap() {
        let mut sender = PixelSender::bind([0; 16], "Media").expect("a socket");
        assert_eq!(sender.next_sequence(1), 1);
        assert_eq!(sender.next_sequence(1), 2);
        // A second universe counts on its own.
        assert_eq!(sender.next_sequence(2), 1);
        sender.sequences.insert(3, 255);
        assert_eq!(sender.next_sequence(3), 0);
    }
}
