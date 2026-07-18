use embassy_futures::select::{Either, select};
use rs_matter::Matter;
use rs_matter::crypto::Crypto;
use rs_matter::error::{Error, ErrorCode};
use rs_matter::transport::network::mdns::builtin::{BuiltinMdns, Host};
use rs_matter::transport::network::mdns::{
    MDNS_IPV4_BROADCAST_ADDR, MDNS_IPV6_BROADCAST_ADDR, MDNS_PORT, MDNS_SOCKET_DEFAULT_BIND_ADDR,
};
use rs_matter::transport::network::{Address, Ipv4Addr, Ipv6Addr, NetworkReceive, NetworkSend};
use socket2::{Domain, Protocol, Socket, Type};
use std::cell::Cell;
use std::net::UdpSocket;

/// Builtin mDNS setup derived from rs-matter's official cross-platform example. Sockets are bound
/// before the transport reports `network_running`, so startup failures remain truthful.
pub(super) struct BuiltinMdnsRuntime {
    ipv4_socket: async_io::Async<UdpSocket>,
    ipv6_socket: async_io::Async<UdpSocket>,
    ipv4: Ipv4Addr,
    ipv6: Ipv6Addr,
    interface: u32,
    ready: Cell<MdnsReady>,
}

#[derive(Clone, Copy)]
enum MdnsReady {
    Ipv4,
    Ipv6,
}

impl BuiltinMdnsRuntime {
    pub(super) fn bind() -> Result<Self, String> {
        let (ipv4, ipv6, interface) = select_network_interface()
            .map_err(|error| format!("mDNS network interface selection failed: {error}"))?;
        let ipv6_socket = bind_ipv6(interface)?;
        let ipv4_socket = bind_ipv4(ipv4)?;
        Ok(Self {
            ipv4_socket,
            ipv6_socket,
            ipv4,
            ipv6,
            interface,
            ready: Cell::new(MdnsReady::Ipv4),
        })
    }

    pub(super) async fn run<C: Crypto>(
        &self,
        matter: &Matter<'_>,
        crypto: C,
        serial: &str,
    ) -> Result<(), Error> {
        BuiltinMdns::new()
            .run(
                self,
                self,
                &Host {
                    hostname: serial,
                    ip: self.ipv4,
                    ipv6: self.ipv6,
                },
                Some(self.ipv4),
                Some(self.interface),
                matter,
                crypto,
            )
            .await
    }
}

fn bind_ipv6(interface: u32) -> Result<async_io::Async<UdpSocket>, String> {
    let socket = Socket::new(Domain::IPV6, Type::DGRAM, Some(Protocol::UDP))
        .map_err(|error| format!("mDNS socket creation failed: {error}"))?;
    socket
        .set_reuse_address(true)
        .map_err(|error| format!("mDNS IPv6 SO_REUSEADDR failed: {error}"))?;
    #[cfg(unix)]
    socket
        .set_reuse_port(true)
        .map_err(|error| format!("mDNS IPv6 SO_REUSEPORT failed: {error}"))?;
    socket
        .set_only_v6(true)
        .map_err(|error| format!("mDNS IPv6-only socket setup failed: {error}"))?;
    socket
        .bind(&MDNS_SOCKET_DEFAULT_BIND_ADDR.into())
        .map_err(|error| format!("mDNS IPv6 UDP bind failed: {error}"))?;
    socket
        .set_multicast_if_v6(interface)
        .map_err(|error| format!("mDNS IPv6 multicast interface setup failed: {error}"))?;
    let socket = async_io::Async::<UdpSocket>::new_nonblocking(socket.into())
        .map_err(|error| format!("mDNS IPv6 async socket setup failed: {error}"))?;
    socket
        .get_ref()
        .join_multicast_v6(&MDNS_IPV6_BROADCAST_ADDR, interface)
        .map_err(|error| format!("mDNS IPv6 multicast join failed: {error}"))?;
    Ok(socket)
}

fn bind_ipv4(ipv4: Ipv4Addr) -> Result<async_io::Async<UdpSocket>, String> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))
        .map_err(|error| format!("mDNS IPv4 socket creation failed: {error}"))?;
    socket
        .set_reuse_address(true)
        .map_err(|error| format!("mDNS IPv4 SO_REUSEADDR failed: {error}"))?;
    #[cfg(unix)]
    socket
        .set_reuse_port(true)
        .map_err(|error| format!("mDNS IPv4 SO_REUSEPORT failed: {error}"))?;
    let bind = std::net::SocketAddr::V4(std::net::SocketAddrV4::new(
        std::net::Ipv4Addr::UNSPECIFIED,
        MDNS_PORT,
    ));
    socket
        .bind(&bind.into())
        .map_err(|error| format!("mDNS IPv4 UDP bind failed: {error}"))?;
    socket
        .set_multicast_if_v4(&ipv4)
        .map_err(|error| format!("mDNS IPv4 multicast interface setup failed: {error}"))?;
    let socket = async_io::Async::<UdpSocket>::new_nonblocking(socket.into())
        .map_err(|error| format!("mDNS IPv4 async socket setup failed: {error}"))?;
    socket
        .get_ref()
        .join_multicast_v4(&MDNS_IPV4_BROADCAST_ADDR, &ipv4)
        .map_err(|error| format!("mDNS IPv4 multicast join failed: {error}"))?;
    Ok(socket)
}

impl NetworkSend for &BuiltinMdnsRuntime {
    async fn send_to(&mut self, data: &[u8], address: Address) -> Result<(), Error> {
        let address = address.udp().ok_or(ErrorCode::NoNetworkInterface)?;
        match address {
            std::net::SocketAddr::V4(_) => self.ipv4_socket.send_to(data, address).await?,
            std::net::SocketAddr::V6(_) => self.ipv6_socket.send_to(data, address).await?,
        };
        Ok(())
    }
}

impl NetworkReceive for &BuiltinMdnsRuntime {
    async fn wait_available(&mut self) -> Result<(), Error> {
        match select(self.ipv4_socket.readable(), self.ipv6_socket.readable()).await {
            Either::First(result) => {
                result?;
                self.ready.set(MdnsReady::Ipv4);
            }
            Either::Second(result) => {
                result?;
                self.ready.set(MdnsReady::Ipv6);
            }
        }
        Ok(())
    }

    async fn recv_from(&mut self, buffer: &mut [u8]) -> Result<(usize, Address), Error> {
        let (length, address) = match self.ready.get() {
            MdnsReady::Ipv4 => self.ipv4_socket.recv_from(buffer).await?,
            MdnsReady::Ipv6 => self.ipv6_socket.recv_from(buffer).await?,
        };
        Ok((length, Address::Udp(address)))
    }
}

fn select_network_interface() -> Result<(Ipv4Addr, Ipv6Addr, u32), Error> {
    let all = if_addrs::get_if_addrs().map_err(|_| ErrorCode::StdIoError)?;
    let candidate = [true, false].into_iter().find_map(|link_local_only| {
        all.iter()
            .filter(|interface| !interface.is_loopback())
            .filter_map(|interface| match interface.addr {
                if_addrs::IfAddr::V6(ref ipv6)
                    if !link_local_only || ipv6.ip.is_unicast_link_local() =>
                {
                    Some((
                        interface.name.clone(),
                        ipv6.ip,
                        interface.index.unwrap_or(0),
                    ))
                }
                _ => None,
            })
            .find_map(|(name, ipv6, index)| {
                all.iter()
                    .filter(|other| other.name == name && !other.is_loopback())
                    .find_map(|other| match other.addr {
                        if_addrs::IfAddr::V4(ref ipv4) => Some((ipv4.ip, ipv6, index)),
                        _ => None,
                    })
            })
    });
    candidate
        .or_else(|| ipv4_only_interface(&all))
        .map(|(ipv4, ipv6, interface)| (ipv4.octets().into(), ipv6.octets().into(), interface))
        .ok_or_else(|| ErrorCode::StdIoError.into())
}

fn ipv4_only_interface(
    all: &[if_addrs::Interface],
) -> Option<(std::net::Ipv4Addr, std::net::Ipv6Addr, u32)> {
    all.iter()
        .filter(|interface| !interface.is_loopback())
        .find_map(|interface| match interface.addr {
            if_addrs::IfAddr::V4(ref ipv4) => Some((
                ipv4.ip,
                std::net::Ipv6Addr::UNSPECIFIED,
                interface.index.unwrap_or(0),
            )),
            _ => None,
        })
}
