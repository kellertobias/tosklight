//! Art-Net and sACN packet validation and decoding.
//!
//! Decoding is pure and allocation-free so it can be unit tested against captured bytes without
//! opening a socket.

pub const DMX_SLOTS: usize = 512;
pub const ARTNET_PORT: u16 = 6454;
pub const SACN_PORT: u16 = 5568;

/// Why a packet was discarded. Counted and shown per mapping; never mutates the last valid frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PacketReject {
    TooShort,
    WrongIdentifier,
    UnrelatedOpcode,
    UnsupportedVersion,
    WrongUniverse,
    BadLength,
    WrongVector,
    WrongStartCode,
    Terminated,
    OutOfOrder,
    LowerPriority,
}

/// One accepted DMX frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedFrame {
    pub destination_universe: u16,
    pub sequence: u8,
    pub priority: u8,
    pub slot_count: u16,
    pub slots: [u8; DMX_SLOTS],
    pub source_name: String,
    /// sACN component identifier; all zeroes for Art-Net.
    pub cid: [u8; 16],
    pub terminated: bool,
}

impl DecodedFrame {
    fn empty(destination_universe: u16) -> Self {
        Self {
            destination_universe,
            sequence: 0,
            priority: 100,
            slot_count: 0,
            slots: [0; DMX_SLOTS],
            source_name: String::new(),
            cid: [0; 16],
            terminated: false,
        }
    }
}

/// Decode an ArtDMX packet.
///
/// Returns `Ok(None)` for a well-formed Art-Net packet that is not ArtDMX, which must be ignored
/// rather than counted as malformed.
pub fn decode_artdmx(bytes: &[u8]) -> Result<Option<DecodedFrame>, PacketReject> {
    if bytes.len() < 18 {
        return Err(PacketReject::TooShort);
    }
    if &bytes[0..8] != b"Art-Net\0" {
        return Err(PacketReject::WrongIdentifier);
    }
    let opcode = u16::from_le_bytes([bytes[8], bytes[9]]);
    if opcode != 0x5000 {
        return Ok(None);
    }
    let version = u16::from_be_bytes([bytes[10], bytes[11]]);
    if version < 14 {
        return Err(PacketReject::UnsupportedVersion);
    }
    let length = u16::from_be_bytes([bytes[16], bytes[17]]) as usize;
    if length == 0 || length > DMX_SLOTS || bytes.len() < 18 + length {
        return Err(PacketReject::BadLength);
    }
    let mut frame = DecodedFrame::empty(u16::from_le_bytes([bytes[14], bytes[15]]));
    frame.sequence = bytes[12];
    frame.slot_count = length as u16;
    frame.slots[..length].copy_from_slice(&bytes[18..18 + length]);
    frame.source_name = "Art-Net".into();
    Ok(Some(frame))
}

const SACN_ACN_IDENTIFIER: &[u8; 12] = b"ASC-E1.17\0\0\0";

/// Decode an E1.31 data packet.
///
/// Returns `Ok(None)` for a valid E1.31 packet that is not a DMP data packet, for example a
/// synchronisation or discovery packet.
pub fn decode_sacn(bytes: &[u8]) -> Result<Option<DecodedFrame>, PacketReject> {
    if bytes.len() < 126 {
        return Err(PacketReject::TooShort);
    }
    if u16::from_be_bytes([bytes[0], bytes[1]]) != 0x0010
        || u16::from_be_bytes([bytes[2], bytes[3]]) != 0x0000
    {
        return Err(PacketReject::WrongIdentifier);
    }
    if &bytes[4..16] != SACN_ACN_IDENTIFIER {
        return Err(PacketReject::WrongIdentifier);
    }
    if u32::from_be_bytes([bytes[18], bytes[19], bytes[20], bytes[21]]) != 0x0000_0004 {
        return Err(PacketReject::WrongVector);
    }
    let framing_vector = u32::from_be_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]);
    if framing_vector != 0x0000_0002 {
        // Synchronisation and universe-discovery packets are valid but not data.
        return Ok(None);
    }
    if bytes[117] != 0x02 {
        return Err(PacketReject::WrongVector);
    }
    if bytes[118] != 0xa1 {
        return Err(PacketReject::WrongVector);
    }
    if u16::from_be_bytes([bytes[119], bytes[120]]) != 0x0000
        || u16::from_be_bytes([bytes[121], bytes[122]]) != 0x0001
    {
        return Err(PacketReject::BadLength);
    }
    let property_count = u16::from_be_bytes([bytes[123], bytes[124]]) as usize;
    if property_count == 0 || property_count > DMX_SLOTS + 1 || bytes.len() < 125 + property_count {
        return Err(PacketReject::BadLength);
    }
    if bytes[125] != 0x00 {
        return Err(PacketReject::WrongStartCode);
    }
    let slot_count = property_count - 1;
    let mut frame = DecodedFrame::empty(u16::from_be_bytes([bytes[113], bytes[114]]));
    frame.sequence = bytes[111];
    frame.priority = bytes[108];
    frame.terminated = bytes[112] & 0x40 != 0;
    frame.slot_count = slot_count as u16;
    frame.slots[..slot_count].copy_from_slice(&bytes[126..126 + slot_count]);
    frame.cid.copy_from_slice(&bytes[22..38]);
    frame.source_name = String::from_utf8_lossy(&bytes[44..108])
        .trim_end_matches('\0')
        .trim()
        .to_owned();
    Ok(Some(frame))
}

/// E1.31 §6.7.2 sequence rule, also applied to Art-Net so both protocols reject provably stale
/// frames while tolerating wrap.
pub fn sequence_is_stale(previous: Option<u8>, current: u8) -> bool {
    let Some(previous) = previous else {
        return false;
    };
    let difference = current.wrapping_sub(previous) as i16;
    let signed = if difference > 127 {
        difference - 256
    } else {
        difference
    };
    signed <= 0 && signed > -20
}

/// Standard multicast group for one sACN universe.
pub fn sacn_multicast_group(universe: u16) -> std::net::Ipv4Addr {
    std::net::Ipv4Addr::new(239, 255, (universe >> 8) as u8, universe as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artdmx(universe: u16, sequence: u8, slots: &[u8]) -> Vec<u8> {
        let mut packet = Vec::new();
        packet.extend_from_slice(b"Art-Net\0");
        packet.extend_from_slice(&0x5000_u16.to_le_bytes());
        packet.extend_from_slice(&14_u16.to_be_bytes());
        packet.push(sequence);
        packet.push(0);
        packet.extend_from_slice(&universe.to_le_bytes());
        packet.extend_from_slice(&(slots.len() as u16).to_be_bytes());
        packet.extend_from_slice(slots);
        packet
    }

    fn sacn(universe: u16, sequence: u8, priority: u8, slots: &[u8], terminated: bool) -> Vec<u8> {
        let size = 126 + slots.len();
        let mut packet = vec![0_u8; size];
        packet[0..2].copy_from_slice(&0x0010_u16.to_be_bytes());
        packet[4..16].copy_from_slice(SACN_ACN_IDENTIFIER);
        packet[16..18].copy_from_slice(&(0x7000_u16 | (size - 16) as u16).to_be_bytes());
        packet[18..22].copy_from_slice(&0x0000_0004_u32.to_be_bytes());
        packet[38..40].copy_from_slice(&(0x7000_u16 | (size - 38) as u16).to_be_bytes());
        packet[40..44].copy_from_slice(&0x0000_0002_u32.to_be_bytes());
        packet[44..49].copy_from_slice(b"Tosk\0");
        packet[108] = priority;
        packet[111] = sequence;
        packet[112] = if terminated { 0x40 } else { 0 };
        packet[113..115].copy_from_slice(&universe.to_be_bytes());
        packet[115..117].copy_from_slice(&(0x7000_u16 | (size - 115) as u16).to_be_bytes());
        packet[117] = 0x02;
        packet[118] = 0xa1;
        packet[121..123].copy_from_slice(&1_u16.to_be_bytes());
        packet[123..125].copy_from_slice(&((slots.len() + 1) as u16).to_be_bytes());
        packet[125] = 0;
        packet[126..].copy_from_slice(slots);
        packet
    }

    #[test]
    fn artdmx_round_trips_the_desk_encoder() {
        let packet = artdmx(3, 9, &[1, 2, 3, 255]);
        let frame = decode_artdmx(&packet).unwrap().unwrap();
        assert_eq!(frame.destination_universe, 3);
        assert_eq!(frame.sequence, 9);
        assert_eq!(frame.slot_count, 4);
        assert_eq!(&frame.slots[..4], &[1, 2, 3, 255]);
    }

    #[test]
    fn unrelated_artnet_opcodes_are_ignored_rather_than_counted_as_malformed() {
        let mut packet = artdmx(1, 0, &[0]);
        packet[8..10].copy_from_slice(&0x2000_u16.to_le_bytes());
        assert_eq!(decode_artdmx(&packet), Ok(None));
    }

    #[test]
    fn truncated_and_mislabelled_packets_are_rejected() {
        assert_eq!(decode_artdmx(&[0; 4]), Err(PacketReject::TooShort));
        let mut packet = artdmx(1, 0, &[0, 0]);
        packet[0] = b'X';
        assert_eq!(decode_artdmx(&packet), Err(PacketReject::WrongIdentifier));
        let mut packet = artdmx(1, 0, &[0, 0]);
        packet[16..18].copy_from_slice(&600_u16.to_be_bytes());
        assert_eq!(decode_artdmx(&packet), Err(PacketReject::BadLength));
    }

    #[test]
    fn sacn_round_trips_the_desk_encoder() {
        let packet = sacn(7, 12, 120, &[9, 8, 7], false);
        let frame = decode_sacn(&packet).unwrap().unwrap();
        assert_eq!(frame.destination_universe, 7);
        assert_eq!(frame.sequence, 12);
        assert_eq!(frame.priority, 120);
        assert_eq!(frame.slot_count, 3);
        assert_eq!(&frame.slots[..3], &[9, 8, 7]);
        assert_eq!(frame.source_name, "Tosk");
        assert!(!frame.terminated);
    }

    #[test]
    fn sacn_termination_is_reported() {
        let packet = sacn(7, 13, 100, &[0], true);
        assert!(decode_sacn(&packet).unwrap().unwrap().terminated);
    }

    #[test]
    fn sacn_rejects_a_non_zero_start_code() {
        let mut packet = sacn(7, 13, 100, &[0], false);
        packet[125] = 0xdd;
        assert_eq!(decode_sacn(&packet), Err(PacketReject::WrongStartCode));
    }

    #[test]
    fn the_sequence_rule_rejects_replays_while_tolerating_wrap() {
        assert!(!sequence_is_stale(None, 5));
        assert!(sequence_is_stale(Some(10), 10));
        assert!(sequence_is_stale(Some(10), 5));
        assert!(!sequence_is_stale(Some(10), 11));
        assert!(!sequence_is_stale(Some(250), 3), "wrap must be accepted");
        assert!(
            sequence_is_stale(Some(3), 250),
            "a large step back is stale"
        );
    }

    #[test]
    fn multicast_groups_follow_the_standard_mapping() {
        assert_eq!(
            sacn_multicast_group(1),
            std::net::Ipv4Addr::new(239, 255, 0, 1)
        );
        assert_eq!(
            sacn_multicast_group(300),
            std::net::Ipv4Addr::new(239, 255, 1, 44)
        );
    }
}
