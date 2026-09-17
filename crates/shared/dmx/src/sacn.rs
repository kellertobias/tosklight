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

/// E1.31 reserves this universe's multicast group for universe discovery.
pub const SACN_DISCOVERY_UNIVERSE: Universe = 64214;
/// Universes one discovery page lists.
const UNIVERSES_PER_PAGE: usize = 512;

/// E1.31 universe-discovery packets announcing the `universes` a source sends, 512 to a page.
pub fn sacn_discovery_packets(
    cid: [u8; 16],
    source_name: &str,
    universes: &[Universe],
) -> Vec<Vec<u8>> {
    let mut sorted = universes.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    let pages: Vec<&[Universe]> = if sorted.is_empty() {
        vec![&[][..]]
    } else {
        sorted.chunks(UNIVERSES_PER_PAGE).take(256).collect()
    };
    let last_page = (pages.len() - 1) as u8;
    pages
        .iter()
        .enumerate()
        .map(|(page, universes)| {
            let size = 120 + universes.len() * 2;
            let mut packet = vec![0_u8; size];
            packet[0..2].copy_from_slice(&0x0010_u16.to_be_bytes());
            packet[4..16].copy_from_slice(b"ASC-E1.17\0\0\0");
            set_flags_and_length(&mut packet[16..18], size - 16);
            packet[18..22].copy_from_slice(&0x0000_0008_u32.to_be_bytes());
            packet[22..38].copy_from_slice(&cid);
            set_flags_and_length(&mut packet[38..40], size - 38);
            packet[40..44].copy_from_slice(&0x0000_0002_u32.to_be_bytes());
            let source = source_name.as_bytes();
            packet[44..44 + source.len().min(63)].copy_from_slice(&source[..source.len().min(63)]);
            set_flags_and_length(&mut packet[112..114], size - 112);
            packet[114..118].copy_from_slice(&0x0000_0001_u32.to_be_bytes());
            packet[118] = page as u8;
            packet[119] = last_page;
            for (index, universe) in universes.iter().enumerate() {
                packet[120 + index * 2..122 + index * 2].copy_from_slice(&universe.to_be_bytes());
            }
            packet
        })
        .collect()
}

/// What an sACN packet from another source says about that source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SacnSourcePacket {
    pub cid: [u8; 16],
    pub source_name: String,
    pub kind: SacnSourcePacketKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SacnSourcePacketKind {
    /// A data packet for one universe.
    Data { universe: Universe, priority: u8 },
    /// One page of a universe-discovery announcement.
    Discovery {
        page: u8,
        last_page: u8,
        universes: Vec<Universe>,
    },
}

/// Decodes an E1.31 data or universe-discovery packet; anything else is `None`.
pub fn decode_sacn_source_packet(bytes: &[u8]) -> Option<SacnSourcePacket> {
    if bytes.len() < 112 || &bytes[4..16] != b"ASC-E1.17\0\0\0" {
        return None;
    }
    let cid: [u8; 16] = bytes[22..38].try_into().ok()?;
    let name = &bytes[44..108];
    let end = name
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(name.len());
    let source_name = String::from_utf8_lossy(&name[..end]).into_owned();
    let root_vector = u32::from_be_bytes(bytes[18..22].try_into().ok()?);
    let kind = match root_vector {
        0x0000_0004 if bytes.len() >= 126 => SacnSourcePacketKind::Data {
            universe: u16::from_be_bytes([bytes[113], bytes[114]]),
            priority: bytes[108],
        },
        0x0000_0008 if bytes.len() >= 120 && bytes[114..118] == 1_u32.to_be_bytes() => {
            SacnSourcePacketKind::Discovery {
                page: bytes[118],
                last_page: bytes[119],
                universes: bytes[120..]
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| u16::from_be_bytes(*pair))
                    .collect(),
            }
        }
        _ => return None,
    };
    Some(SacnSourcePacket {
        cid,
        source_name,
        kind,
    })
}

fn set_flags_and_length(target: &mut [u8], length: usize) {
    target.copy_from_slice(&(0x7000_u16 | length as u16).to_be_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn universes(packet: &[u8]) -> Vec<Universe> {
        packet[120..]
            .chunks(2)
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
            .collect()
    }

    #[test]
    fn discovery_lists_each_universe_once_in_order() {
        let packets = sacn_discovery_packets([3; 16], "Light", &[20, 7, 20]);
        assert_eq!(packets.len(), 1);
        let packet = &packets[0];
        assert_eq!(packet.len(), 124);
        assert_eq!(&packet[18..22], &8_u32.to_be_bytes());
        assert_eq!(&packet[22..38], &[3; 16]);
        assert_eq!(&packet[40..44], &2_u32.to_be_bytes());
        assert_eq!(&packet[44..50], b"Light\0");
        assert_eq!(u16::from_be_bytes([packet[112], packet[113]]), 0x7000 | 12);
        assert_eq!(&packet[114..118], &1_u32.to_be_bytes());
        assert_eq!((packet[118], packet[119]), (0, 0));
        assert_eq!(universes(packet), [7, 20]);
        assert_eq!(
            sacn_multicast_destination(SACN_DISCOVERY_UNIVERSE).ip(),
            IpAddr::V4(Ipv4Addr::new(239, 255, 250, 214))
        );
    }

    #[test]
    fn data_and_discovery_packets_decode_their_source() {
        let data = decode_sacn_source_packet(&sacn_data_packet(
            9, 1, &[0; 4], [5; 16], "Other", 120, false,
        ))
        .unwrap();
        assert_eq!(data.cid, [5; 16]);
        assert_eq!(data.source_name, "Other");
        assert_eq!(
            data.kind,
            SacnSourcePacketKind::Data {
                universe: 9,
                priority: 120
            }
        );
        let discovery = &sacn_discovery_packets([6; 16], "Console", &[3, 1])[0];
        let decoded = decode_sacn_source_packet(discovery).unwrap();
        assert_eq!(decoded.source_name, "Console");
        assert_eq!(
            decoded.kind,
            SacnSourcePacketKind::Discovery {
                page: 0,
                last_page: 0,
                universes: vec![1, 3]
            }
        );
        assert_eq!(decode_sacn_source_packet(&discovery[..100]), None);
        assert_eq!(decode_sacn_source_packet(&[0; 130]), None);
    }

    #[test]
    fn more_than_512_universes_are_paged() {
        let all: Vec<Universe> = (1..=600).collect();
        let packets = sacn_discovery_packets([3; 16], "Light", &all);
        assert_eq!(packets.len(), 2);
        assert_eq!((packets[0][118], packets[0][119]), (0, 1));
        assert_eq!((packets[1][118], packets[1][119]), (1, 1));
        assert_eq!(universes(&packets[0]).len(), 512);
        assert_eq!(universes(&packets[1]), (513..=600).collect::<Vec<_>>());
    }
}
