use light_core::Universe;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

pub const SACN_PORT: u16 = 5568;

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
    packet[125] = 0;
    packet[126..].copy_from_slice(frame);
    packet
}

fn set_flags_and_length(target: &mut [u8], length: usize) {
    target.copy_from_slice(&(0x7000_u16 | length as u16).to_be_bytes());
}
