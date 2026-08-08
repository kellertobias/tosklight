//! Art-Net packet parsing.
//!
//! The legacy receiver read only the low universe byte, so universes 256 apart were
//! indistinguishable. This reads the full 15-bit Port-Address — net, sub-net, and universe — as
//! the specification defines it.
//!
//! Parsing is total: every malformed packet produces a reason rather than a panic or a silently
//! wrong universe, because the input is a UDP port anyone on the network can write to.

/// Art-Net's fixed UDP port.
pub const PORT: u16 = 6454;

/// Every packet begins with this, NUL terminated.
pub const IDENTIFIER: &[u8; 8] = b"Art-Net\0";

/// The opcode for a DMX data packet, little-endian on the wire.
pub const OP_DMX: u16 = 0x5000;

/// The opcode for a poll, which a desk uses to discover nodes.
pub const OP_POLL: u16 = 0x2000;

/// The lowest protocol version this accepts. The specification has required 14 since 2001.
pub const MIN_PROTOCOL_VERSION: u16 = 14;

/// A parsed ArtDmx packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtDmx<'a> {
    /// The full 15-bit Port-Address: net, sub-net, and universe together.
    pub port_address: u16,
    /// Wraps at 255 and starts again at 1; zero disables sequencing.
    pub sequence: u8,
    /// Which physical input the sender used. Informational only.
    pub physical: u8,
    pub data: &'a [u8],
}

/// Why a datagram is not a usable Art-Net packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    #[error("the datagram is too short to be an Art-Net packet")]
    TooShort,
    #[error("the datagram does not begin with the Art-Net identifier")]
    NotArtNet,
    #[error("opcode {opcode:#06x} is not ArtDmx")]
    NotDmx { opcode: u16 },
    #[error("protocol version {found} is older than {MIN_PROTOCOL_VERSION}")]
    ProtocolTooOld { found: u16 },
    #[error("the packet claims {claimed} data bytes but carries {available}")]
    LengthMismatch { claimed: usize, available: usize },
    #[error("a slot count of {claimed} is outside 1..=512")]
    ImpossibleLength { claimed: usize },
}

/// The opcode a datagram carries, without requiring it to be ArtDmx.
///
/// Used to tell a poll apart from data, so a desk discovering nodes is not logged as a malformed
/// packet every time it looks for one.
pub fn opcode(datagram: &[u8]) -> Option<u16> {
    if datagram.len() < 10 || &datagram[0..8] != IDENTIFIER {
        return None;
    }
    Some(u16::from_le_bytes([datagram[8], datagram[9]]))
}

/// Parses an ArtDmx packet.
pub fn parse(datagram: &[u8]) -> Result<ArtDmx<'_>, ParseError> {
    // Identifier, opcode, version, sequence, physical, port address, and length: 18 bytes before
    // any slot data.
    if datagram.len() < 18 {
        return Err(ParseError::TooShort);
    }
    if &datagram[0..8] != IDENTIFIER {
        return Err(ParseError::NotArtNet);
    }

    let opcode = u16::from_le_bytes([datagram[8], datagram[9]]);
    if opcode != OP_DMX {
        return Err(ParseError::NotDmx { opcode });
    }

    // The version is big-endian, unlike the opcode. That asymmetry is in the specification.
    let version = u16::from_be_bytes([datagram[10], datagram[11]]);
    if version < MIN_PROTOCOL_VERSION {
        return Err(ParseError::ProtocolTooOld { found: version });
    }

    // Low byte first: universe and sub-net, then net in the high byte.
    let port_address = u16::from(datagram[14]) | (u16::from(datagram[15]) << 8);
    let claimed = usize::from(u16::from_be_bytes([datagram[16], datagram[17]]));

    if claimed == 0 || claimed > 512 {
        return Err(ParseError::ImpossibleLength { claimed });
    }
    let available = datagram.len() - 18;
    if available < claimed {
        return Err(ParseError::LengthMismatch { claimed, available });
    }

    Ok(ArtDmx {
        // The Port-Address is 15 bits; the top bit of the net byte is reserved.
        port_address: port_address & 0x7FFF,
        sequence: datagram[12],
        physical: datagram[13],
        data: &datagram[18..18 + claimed],
    })
}

/// Builds an ArtDmx packet. Used by the tests here and by anything that needs to speak Art-Net
/// back, such as an interoperability fixture.
pub fn encode(port_address: u16, sequence: u8, data: &[u8]) -> Vec<u8> {
    let mut packet = Vec::with_capacity(18 + data.len());
    packet.extend_from_slice(IDENTIFIER);
    packet.extend_from_slice(&OP_DMX.to_le_bytes());
    packet.extend_from_slice(&MIN_PROTOCOL_VERSION.to_be_bytes());
    packet.push(sequence);
    packet.push(0); // physical
    packet.push((port_address & 0x00FF) as u8);
    packet.push(((port_address >> 8) & 0x7F) as u8);
    packet.extend_from_slice(&(data.len() as u16).to_be_bytes());
    packet.extend_from_slice(data);
    packet
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_well_formed_packet_parses() {
        let packet = encode(1, 7, &[10, 20, 30]);
        let parsed = parse(&packet).unwrap();
        assert_eq!(parsed.port_address, 1);
        assert_eq!(parsed.sequence, 7);
        assert_eq!(parsed.data, &[10, 20, 30]);
    }

    #[test]
    fn the_whole_fifteen_bit_port_address_is_read() {
        // The legacy receiver read only the low byte, so these three were indistinguishable.
        for address in [0u16, 1, 255, 256, 257, 4096, 0x7FFF] {
            let packet = encode(address, 0, &[1]);
            assert_eq!(
                parse(&packet).unwrap().port_address,
                address,
                "address {address}"
            );
        }
    }

    #[test]
    fn universes_that_differ_only_above_the_low_byte_stay_distinct() {
        let (one, two_fifty_seven) = (encode(1, 0, &[1]), encode(257, 0, &[1]));
        let low = parse(&one).unwrap().port_address;
        let high = parse(&two_fifty_seven).unwrap().port_address;
        assert_ne!(low, high, "1 and 257 must not collapse onto each other");
    }

    #[test]
    fn a_full_universe_parses() {
        let packet = encode(0, 0, &[255u8; 512]);
        assert_eq!(parse(&packet).unwrap().data.len(), 512);
    }

    #[test]
    fn anything_that_is_not_art_net_is_refused() {
        assert_eq!(parse(&[]).unwrap_err(), ParseError::TooShort);
        assert_eq!(parse(&[0u8; 4]).unwrap_err(), ParseError::TooShort);
        assert_eq!(parse(&[0u8; 20]).unwrap_err(), ParseError::NotArtNet);

        let mut random = vec![0u8; 64];
        random[0..8].copy_from_slice(b"NotArt\0\0");
        assert_eq!(parse(&random).unwrap_err(), ParseError::NotArtNet);
    }

    #[test]
    fn a_poll_is_recognised_rather_than_reported_as_malformed() {
        // Built by hand: `encode` only makes ArtDmx.
        let mut poll = vec![0u8; 20];
        poll[0..8].copy_from_slice(IDENTIFIER);
        poll[8..10].copy_from_slice(&OP_POLL.to_le_bytes());

        assert_eq!(opcode(&poll), Some(OP_POLL));
        assert_eq!(
            parse(&poll).unwrap_err(),
            ParseError::NotDmx { opcode: OP_POLL }
        );
    }

    #[test]
    fn an_old_protocol_version_is_refused() {
        let mut packet = encode(0, 0, &[1]);
        packet[10..12].copy_from_slice(&13u16.to_be_bytes());
        assert_eq!(
            parse(&packet).unwrap_err(),
            ParseError::ProtocolTooOld { found: 13 }
        );
    }

    #[test]
    fn a_packet_that_lies_about_its_length_is_refused() {
        let mut packet = encode(0, 0, &[1, 2, 3]);
        packet[16..18].copy_from_slice(&500u16.to_be_bytes());
        assert_eq!(
            parse(&packet).unwrap_err(),
            ParseError::LengthMismatch {
                claimed: 500,
                available: 3
            }
        );
    }

    #[test]
    fn an_impossible_slot_count_is_refused() {
        let mut packet = encode(0, 0, &[1]);
        packet[16..18].copy_from_slice(&0u16.to_be_bytes());
        assert_eq!(
            parse(&packet).unwrap_err(),
            ParseError::ImpossibleLength { claimed: 0 }
        );

        let mut packet = encode(0, 0, &[1u8; 512]);
        packet[16..18].copy_from_slice(&513u16.to_be_bytes());
        assert_eq!(
            parse(&packet).unwrap_err(),
            ParseError::ImpossibleLength { claimed: 513 }
        );
    }

    #[test]
    fn extra_trailing_bytes_are_ignored_rather_than_rejected() {
        // Some senders pad. The claimed length is what counts.
        let mut packet = encode(0, 0, &[1, 2, 3]);
        packet.extend_from_slice(&[0u8; 32]);
        assert_eq!(parse(&packet).unwrap().data, &[1, 2, 3]);
    }

    #[test]
    fn the_reserved_top_bit_of_the_net_byte_is_masked_off() {
        let mut packet = encode(0x7FFF, 0, &[1]);
        packet[15] |= 0x80;
        assert_eq!(parse(&packet).unwrap().port_address, 0x7FFF);
    }

    #[test]
    fn parsing_never_panics_on_arbitrary_input() {
        // The input is a UDP port anyone can write to, so every shape has to be survivable.
        for length in 0..64usize {
            for fill in [0u8, 0x55, 0xAA, 0xFF] {
                let mut datagram = vec![fill; length];
                if length >= 8 {
                    datagram[0..8].copy_from_slice(IDENTIFIER);
                }
                let _ = parse(&datagram);
                let _ = opcode(&datagram);
            }
        }
    }
}
