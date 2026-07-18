use light_core::Universe;
use std::net::{Ipv4Addr, SocketAddr};

pub const ARTNET_PORT: u16 = 6454;

pub fn artnet_broadcast_destination() -> SocketAddr {
    SocketAddr::from((Ipv4Addr::BROADCAST, ARTNET_PORT))
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
