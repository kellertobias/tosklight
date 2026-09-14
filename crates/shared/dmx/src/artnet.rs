use light_core::Universe;
use std::net::{Ipv4Addr, SocketAddr};

pub const ARTNET_PORT: u16 = 6454;

const OP_POLL: u16 = 0x2000;
const OP_POLL_REPLY: u16 = 0x2100;
/// Ports one ArtPollReply describes. A device with more answers once per four, by BindIndex.
const PORTS_PER_REPLY: usize = 4;
const POLL_REPLY_LENGTH: usize = 239;

/// Whether `bytes` is an ArtPoll: a controller asking everything on the network to identify itself.
pub fn is_artpoll(bytes: &[u8]) -> bool {
    bytes.len() >= 12
        && &bytes[..8] == b"Art-Net\0"
        && u16::from_le_bytes([bytes[8], bytes[9]]) == OP_POLL
}

/// The ArtPollReply packets of a controller sending `universes` as Art-Net.
///
/// Each universe is a DMX input port — the controller puts it onto the network. One reply carries
/// four ports sharing a net and sub-net, so universes are grouped by those and split into fours,
/// numbered by BindIndex from 1. A controller sending nothing still answers once, with no ports.
pub fn artpollreply_packets(
    address: Ipv4Addr,
    short_name: &str,
    long_name: &str,
    report: &str,
    universes: &[Universe],
) -> Vec<Vec<u8>> {
    let mut sorted: Vec<Universe> = universes.iter().map(|universe| universe & 0x7fff).collect();
    sorted.sort_unstable();
    sorted.dedup();
    let mut groups: Vec<Vec<Universe>> = Vec::new();
    for universe in sorted {
        match groups.last_mut() {
            Some(group) if group.len() < PORTS_PER_REPLY && group[0] >> 4 == universe >> 4 => {
                group.push(universe)
            }
            _ => groups.push(vec![universe]),
        }
    }
    if groups.is_empty() {
        groups.push(Vec::new());
    }
    groups
        .iter()
        .enumerate()
        .map(|(index, ports)| {
            let bind_index = u8::try_from(index + 1).unwrap_or(u8::MAX);
            artpollreply(address, short_name, long_name, report, bind_index, ports)
        })
        .collect()
}

fn artpollreply(
    address: Ipv4Addr,
    short_name: &str,
    long_name: &str,
    report: &str,
    bind_index: u8,
    ports: &[Universe],
) -> Vec<u8> {
    let mut packet = vec![0_u8; POLL_REPLY_LENGTH];
    packet[..8].copy_from_slice(b"Art-Net\0");
    packet[8..10].copy_from_slice(&OP_POLL_REPLY.to_le_bytes());
    packet[10..14].copy_from_slice(&address.octets());
    packet[14..16].copy_from_slice(&ARTNET_PORT.to_le_bytes());
    let first = ports.first().copied().unwrap_or_default();
    packet[18] = (first >> 8) as u8;
    packet[19] = ((first >> 4) & 0x0f) as u8;
    // OEM code 0x00FF: no registered OEM.
    packet[21] = 0xff;
    write_text(&mut packet[26..44], short_name);
    write_text(&mut packet[44..108], long_name);
    write_text(&mut packet[108..172], report);
    packet[173] = ports.len() as u8;
    for (index, universe) in ports.iter().enumerate() {
        // A DMX512 input whose data is being received and sent.
        packet[174 + index] = 0x40;
        packet[178 + index] = 0x80;
        packet[186 + index] = (universe & 0x0f) as u8;
    }
    // StController: a console, not a node.
    packet[200] = 0x01;
    packet[207..211].copy_from_slice(&address.octets());
    packet[211] = bind_index;
    // Supports 15-bit Port-Addresses.
    packet[212] = 0x08;
    packet
}

/// A NUL-terminated text field, truncated to fit.
fn write_text(target: &mut [u8], value: &str) {
    let bytes = value.as_bytes();
    let length = bytes.len().min(target.len() - 1);
    target[..length].copy_from_slice(&bytes[..length]);
}

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

#[cfg(test)]
mod tests {
    use super::*;

    const DESK: Ipv4Addr = Ipv4Addr::new(10, 0, 0, 4);

    #[test]
    fn only_an_artpoll_is_a_poll() {
        let mut poll = [0_u8; 14];
        poll[..8].copy_from_slice(b"Art-Net\0");
        poll[8..10].copy_from_slice(&OP_POLL.to_le_bytes());
        assert!(is_artpoll(&poll));
        assert!(!is_artpoll(&artdmx_packet(1, 1, &[0; 2])));
        assert!(!is_artpoll(&poll[..10]));
    }

    #[test]
    fn a_reply_names_the_desk_and_sends_each_universe_as_an_input_port() {
        let replies = artpollreply_packets(
            DESK,
            "Light",
            "ToskLight desk",
            "#0001 [0001] OK",
            &[3, 1, 3],
        );
        assert_eq!(replies.len(), 1);
        let reply = &replies[0];
        assert_eq!(reply.len(), POLL_REPLY_LENGTH);
        assert_eq!(u16::from_le_bytes([reply[8], reply[9]]), OP_POLL_REPLY);
        assert_eq!(&reply[10..14], &DESK.octets());
        assert_eq!(&reply[26..32], b"Light\0");
        assert_eq!(&reply[44..58], b"ToskLight desk");
        assert_eq!(reply[173], 2);
        assert_eq!(&reply[174..178], &[0x40, 0x40, 0, 0]);
        assert_eq!(&reply[186..188], &[1, 3]);
        assert_eq!(reply[200], 0x01);
        assert_eq!(reply[211], 1);
    }

    #[test]
    fn universes_beyond_four_or_on_another_sub_net_get_their_own_reply() {
        let replies = artpollreply_packets(DESK, "Light", "", "", &[1, 2, 3, 4, 5, 0x0112]);
        let ports: Vec<_> = replies
            .iter()
            .map(|reply| (reply[18], reply[19], reply[173], reply[186], reply[211]))
            .collect();
        assert_eq!(ports, [(0, 0, 4, 1, 1), (0, 0, 1, 5, 2), (1, 1, 1, 2, 3)]);
    }

    #[test]
    fn a_desk_sending_no_art_net_still_answers() {
        let replies = artpollreply_packets(DESK, "Light", "", "", &[]);
        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0][173], 0);
    }

    #[test]
    fn a_long_name_is_truncated_and_stays_terminated() {
        let reply = &artpollreply_packets(DESK, &"x".repeat(40), "", "", &[])[0];
        assert_eq!(&reply[26..43], "x".repeat(17).as_bytes());
        assert_eq!(reply[43], 0);
    }
}
