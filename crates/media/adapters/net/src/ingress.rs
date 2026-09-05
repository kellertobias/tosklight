//! The network ingress.
//!
//! One host-level listener per protocol, bound once. Art-Net uses a fixed UDP port, so one process
//! binds it, validates once, and routes universes to outputs — rather than each output binding for
//! itself and depending on platform-specific socket reuse.
//!
//! A bind conflict is reported with the exact address and port rather than being papered over.
//! `SO_REUSEPORT` behaviour differs by platform, so relying on it would make the same
//! configuration work on one operating system and fail on another.

use std::net::SocketAddr;
use std::sync::Arc;

use media_domain::{CommandSource, Timestamp};
use tokio::net::UdpSocket;

use crate::arbitration::{SourceArbiter, SourceKey, Winner};
use crate::{artnet, sacn};

/// The largest datagram either protocol produces, with room to spare.
const RECEIVE_BUFFER: usize = 2048;

/// A universe of slot values, and where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UniverseFrame {
    pub universe: u16,
    pub source: CommandSource,
    /// The winning sender an operator can identify: an IP for Art-Net, and the advertised source
    /// name plus IP for sACN.
    pub source_label: String,
    pub slots: Vec<u8>,
    pub received_at: Timestamp,
}

/// Why the ingress could not start.
#[derive(Debug, thiserror::Error)]
pub enum IngressError {
    #[error(
        "cannot bind {protocol} to {address}: {source}. Check the listen address, port availability, and network permissions."
    )]
    BindConflict {
        protocol: &'static str,
        address: SocketAddr,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot join the sACN multicast group for universe {universe}: {source}")]
    MulticastJoin {
        universe: u16,
        #[source]
        source: std::io::Error,
    },
}

/// The multicast group a universe's sACN traffic uses.
///
/// `239.255.<high>.<low>`, as E1.31 defines it.
pub const fn sacn_multicast_group(universe: u16) -> std::net::Ipv4Addr {
    std::net::Ipv4Addr::new(239, 255, (universe >> 8) as u8, (universe & 0xFF) as u8)
}

/// Whether a port may be shared with another process.
///
/// This is the difference between the two protocols, and it is deliberate rather than incidental.
/// A multicast group is designed to be shared — a media server and a monitoring tool both
/// listening to sACN on 5568 is ordinary — so that socket sets address reuse. Art-Net's unicast
/// port is not: two processes splitting a desk's packets between them is a fault, and the whole
/// point of the bind check is to surface it. Setting address reuse there would defeat it, because
/// BSD-derived systems then allow the second bind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortSharing {
    /// Fail if another process already holds the port.
    Exclusive,
    /// Allow sharing, as multicast reception requires.
    Shared,
}

/// Binds a UDP socket, reporting a conflict with enough detail to act on.
pub fn bind(
    protocol: &'static str,
    address: SocketAddr,
    sharing: PortSharing,
) -> Result<std::net::UdpSocket, IngressError> {
    let socket = socket2::Socket::new(
        socket2::Domain::for_address(address),
        socket2::Type::DGRAM,
        Some(socket2::Protocol::UDP),
    )
    .map_err(|source| IngressError::BindConflict {
        protocol,
        address,
        source,
    })?;

    if sharing == PortSharing::Shared {
        socket
            .set_reuse_address(true)
            .map_err(|source| IngressError::BindConflict {
                protocol,
                address,
                source,
            })?;
        // BSD-derived systems need port reuse as well before two sockets may share a unicast
        // port; address reuse alone only covers multicast there. Windows grants sharing from
        // address reuse and has no equivalent option.
        #[cfg(unix)]
        socket
            .set_reuse_port(true)
            .map_err(|source| IngressError::BindConflict {
                protocol,
                address,
                source,
            })?;
    }
    socket
        .bind(&address.into())
        .map_err(|source| IngressError::BindConflict {
            protocol,
            address,
            source,
        })?;
    socket
        .set_nonblocking(true)
        .map_err(|source| IngressError::BindConflict {
            protocol,
            address,
            source,
        })?;

    Ok(socket.into())
}

/// Receives Art-Net and hands validated universes on.
///
/// The listener owns parsing and arbitration; it never decides what a slot means, because that is
/// the canonical personality's job and belongs to the domain.
#[derive(Debug)]
pub struct ArtNetListener {
    socket: Arc<UdpSocket>,
    arbiter: SourceArbiter,
}

impl ArtNetListener {
    pub fn bind(address: SocketAddr) -> Result<Self, IngressError> {
        let socket = bind("Art-Net", address, PortSharing::Exclusive)?;
        Ok(Self {
            socket: Arc::new(UdpSocket::from_std(socket).map_err(|source| {
                IngressError::BindConflict {
                    protocol: "Art-Net",
                    address,
                    source,
                }
            })?),
            arbiter: SourceArbiter::new(),
        })
    }

    pub fn local_address(&self) -> std::io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    /// Waits for the next usable universe.
    ///
    /// Malformed packets, polls, and packets from an outranked sender are consumed and logged
    /// rather than returned, so a caller's loop only ever sees data it should apply.
    pub async fn receive(&mut self, now: impl Fn() -> Timestamp) -> UniverseFrame {
        let mut buffer = vec![0u8; RECEIVE_BUFFER];
        loop {
            let Ok((length, from)) = self.socket.recv_from(&mut buffer).await else {
                continue;
            };
            let received_at = now();

            let packet = match artnet::parse(&buffer[..length]) {
                Ok(packet) => packet,
                Err(artnet::ParseError::NotDmx { .. }) => continue, // a poll or similar
                Err(error) => {
                    tracing::debug!(%from, %error, "discarding an Art-Net datagram");
                    continue;
                }
            };

            let source = SourceKey::ArtNet { address: from.ip() };
            match self.arbiter.observe(
                packet.port_address,
                source,
                // Art-Net carries no priority, so every sender arbitrates at the same level and
                // only the timeout and sequence rules apply.
                sacn::DEFAULT_PRIORITY,
                packet.sequence,
                received_at,
            ) {
                Winner::Accepted => {
                    return UniverseFrame {
                        universe: packet.port_address,
                        source: CommandSource::ArtNet,
                        source_label: from.ip().to_string(),
                        slots: packet.data.to_vec(),
                        received_at,
                    };
                }
                outcome => {
                    tracing::trace!(%from, ?outcome, "not applying this Art-Net packet");
                }
            }
        }
    }
}

/// Receives sACN and hands validated universes on.
#[derive(Debug)]
pub struct SacnListener {
    socket: Arc<UdpSocket>,
    arbiter: SourceArbiter,
}

impl SacnListener {
    /// Binds and joins the multicast groups for the universes given.
    ///
    /// Unicast still arrives without any group being joined, which is the same-computer path.
    pub fn bind(address: SocketAddr, universes: &[u16]) -> Result<Self, IngressError> {
        // sACN is received over multicast, which several processes on one host legitimately share.
        let socket = bind("sACN", address, PortSharing::Shared)?;

        for universe in universes {
            let group = sacn_multicast_group(*universe);
            // Joining on the unspecified interface lets the operating system choose, which is
            // what a single-homed machine wants and what a multi-homed one can override at the
            // routing table.
            if let Err(source) = socket.join_multicast_v4(&group, &std::net::Ipv4Addr::UNSPECIFIED)
            {
                return Err(IngressError::MulticastJoin {
                    universe: *universe,
                    source,
                });
            }
        }

        Ok(Self {
            socket: Arc::new(UdpSocket::from_std(socket).map_err(|source| {
                IngressError::BindConflict {
                    protocol: "sACN",
                    address,
                    source,
                }
            })?),
            arbiter: SourceArbiter::new(),
        })
    }

    pub fn local_address(&self) -> std::io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    pub async fn receive(&mut self, now: impl Fn() -> Timestamp) -> UniverseFrame {
        let mut buffer = vec![0u8; RECEIVE_BUFFER];
        loop {
            let Ok((length, from)) = self.socket.recv_from(&mut buffer).await else {
                continue;
            };
            let received_at = now();

            let packet = match sacn::parse(&buffer[..length]) {
                Ok(packet) => packet,
                Err(error) => {
                    tracing::debug!(%from, %error, "discarding an sACN datagram");
                    continue;
                }
            };

            let source = SourceKey::Sacn { cid: packet.source };
            if packet.terminated {
                // The sender is finished, so its universe is released now rather than after the
                // timeout, and the terminating packet itself is not applied.
                self.arbiter.terminate(packet.universe, source);
                tracing::debug!(universe = packet.universe, "a sender terminated its stream");
                continue;
            }
            if packet.preview {
                continue; // Preview data is not for live output.
            }

            match self.arbiter.observe(
                packet.universe,
                source,
                packet.priority,
                packet.sequence,
                received_at,
            ) {
                Winner::Accepted => {
                    return UniverseFrame {
                        universe: packet.universe,
                        source: CommandSource::Sacn,
                        source_label: format!("{} ({})", packet.source_name, from.ip()),
                        slots: packet.data.to_vec(),
                        received_at,
                    };
                }
                outcome => {
                    tracing::trace!(%from, ?outcome, "not applying this sACN packet");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;

    fn loopback() -> SocketAddr {
        // Port zero: the operating system picks a free one, so tests never fight over a port.
        SocketAddr::from((Ipv4Addr::LOCALHOST, 0))
    }

    fn now() -> Timestamp {
        Timestamp::from_millis(0)
    }

    #[tokio::test]
    async fn several_processes_may_share_the_sacn_port_because_multicast_requires_it() {
        let first = SacnListener::bind(loopback(), &[]).unwrap();
        let shared = first.local_address().unwrap();
        assert!(
            SacnListener::bind(shared, &[]).is_ok(),
            "a monitoring tool listening alongside the server is ordinary, not a fault"
        );
    }

    #[test]
    fn multicast_groups_follow_the_universe() {
        assert_eq!(sacn_multicast_group(1), Ipv4Addr::new(239, 255, 0, 1));
        assert_eq!(sacn_multicast_group(255), Ipv4Addr::new(239, 255, 0, 255));
        assert_eq!(sacn_multicast_group(256), Ipv4Addr::new(239, 255, 1, 0));
        assert_eq!(
            sacn_multicast_group(63_999),
            Ipv4Addr::new(239, 255, 249, 255)
        );
    }

    #[tokio::test]
    async fn art_net_arrives_over_loopback_unicast() {
        // The same-computer path: a desk on this machine sends to 127.0.0.1 with no broadcast and
        // no multicast involved.
        let mut listener = ArtNetListener::bind(loopback()).unwrap();
        let address = listener.local_address().unwrap();

        let sender = tokio::net::UdpSocket::bind(loopback()).await.unwrap();
        sender
            .send_to(&artnet::encode(3, 1, &[1, 2, 3]), address)
            .await
            .unwrap();

        let frame = listener.receive(now).await;
        assert_eq!(frame.universe, 3);
        assert_eq!(frame.source, CommandSource::ArtNet);
        assert_eq!(frame.source_label, "127.0.0.1");
        assert_eq!(frame.slots, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn sacn_arrives_over_loopback_unicast_without_joining_a_group() {
        let mut listener = SacnListener::bind(loopback(), &[]).unwrap();
        let address = listener.local_address().unwrap();

        let sender = tokio::net::UdpSocket::bind(loopback()).await.unwrap();
        sender
            .send_to(&sacn::encode(7, 100, 1, [1u8; 16], &[9, 8, 7]), address)
            .await
            .unwrap();

        let frame = listener.receive(now).await;
        assert_eq!(frame.universe, 7);
        assert_eq!(frame.source, CommandSource::Sacn);
        assert!(frame.source_label.contains("ToskLight"));
        assert!(frame.source_label.contains("127.0.0.1"));
        assert_eq!(frame.slots, vec![9, 8, 7]);
    }

    #[tokio::test]
    async fn rubbish_on_the_port_is_discarded_and_the_next_real_packet_still_arrives() {
        let mut listener = ArtNetListener::bind(loopback()).unwrap();
        let address = listener.local_address().unwrap();
        let sender = tokio::net::UdpSocket::bind(loopback()).await.unwrap();

        // Anything can be sent to a UDP port. None of it may stop the listener.
        sender.send_to(b"hello", address).await.unwrap();
        sender.send_to(&[0xFFu8; 700], address).await.unwrap();
        sender
            .send_to(&artnet::encode(1, 1, &[42]), address)
            .await
            .unwrap();

        let frame = listener.receive(now).await;
        assert_eq!(frame.slots, vec![42]);
    }

    #[tokio::test]
    async fn a_terminated_sacn_stream_is_not_applied_and_releases_its_universe() {
        let mut listener = SacnListener::bind(loopback(), &[]).unwrap();
        let address = listener.local_address().unwrap();
        let sender = tokio::net::UdpSocket::bind(loopback()).await.unwrap();

        let mut terminating = sacn::encode(4, 100, 1, [2u8; 16], &[1]);
        terminating[112] = 0b0100_0000;
        sender.send_to(&terminating, address).await.unwrap();
        sender
            .send_to(&sacn::encode(4, 100, 2, [3u8; 16], &[5]), address)
            .await
            .unwrap();

        // The terminating packet is skipped; the next sender's data comes through.
        let frame = listener.receive(now).await;
        assert_eq!(frame.slots, vec![5]);
    }

    #[tokio::test]
    async fn preview_data_is_not_applied_to_live_output() {
        let mut listener = SacnListener::bind(loopback(), &[]).unwrap();
        let address = listener.local_address().unwrap();
        let sender = tokio::net::UdpSocket::bind(loopback()).await.unwrap();

        let mut preview = sacn::encode(4, 100, 1, [2u8; 16], &[99]);
        preview[112] = 0b1000_0000;
        sender.send_to(&preview, address).await.unwrap();
        sender
            .send_to(&sacn::encode(4, 100, 2, [2u8; 16], &[7]), address)
            .await
            .unwrap();

        let frame = listener.receive(now).await;
        assert_eq!(frame.slots, vec![7], "the preview packet was skipped");
    }

    #[tokio::test]
    async fn a_second_listener_on_one_port_fails_with_the_address_rather_than_silently_sharing_it()
    {
        // Platform SO_REUSEPORT behaviour differs, so this must be a clear failure everywhere
        // rather than two processes quietly splitting a desk's packets between them.
        let first = ArtNetListener::bind(loopback()).unwrap();
        let taken = first.local_address().unwrap();

        let error = ArtNetListener::bind(taken).unwrap_err();
        match error {
            IngressError::BindConflict {
                protocol, address, ..
            } => {
                assert_eq!(protocol, "Art-Net");
                assert_eq!(address, taken);
                assert!(error.to_string().contains(&taken.to_string()));
            }
            other => panic!("expected a bind conflict, got {other}"),
        }
    }
}
