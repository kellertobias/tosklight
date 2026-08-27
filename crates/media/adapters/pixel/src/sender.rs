use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};

use light_dmx_wire::{
    DmxFrame, artdmx_packet, artnet_broadcast_destination, sacn_data_packet,
    sacn_multicast_destination,
};
use media_application::configuration::{DmxProtocol, PixelOutputRoute};
use media_domain::pixel_map::UniverseFrames;

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
    source_name: String,
    /// Art-Net and sACN both number their packets per universe, and a receiver uses that to spot a
    /// frame that arrived out of order. The count belongs to the sender, not to the frame.
    sequences: HashMap<u16, u8>,
}

impl PixelSender {
    pub fn bind(cid: [u8; 16], source_name: impl Into<String>) -> Result<Self, PixelSendError> {
        let socket = UdpSocket::bind("0.0.0.0:0").map_err(PixelSendError::Bind)?;
        socket.set_broadcast(true).map_err(PixelSendError::Bind)?;
        Ok(Self {
            socket,
            cid,
            source_name: source_name.into(),
            sequences: HashMap::new(),
        })
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
        failures
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
