//! E1.31 (sACN) packet parsing.
//!
//! sACN carries what Art-Net does not: a per-packet priority, a source identity, and an explicit
//! stream termination. Those are what make multiple senders on one universe resolvable, so they
//! are parsed rather than ignored.

/// E1.31's fixed UDP port.
pub const PORT: u16 = 5568;

/// Every packet begins with this ACN packet identifier.
pub const IDENTIFIER: &[u8; 12] = b"ASC-E1.17\0\0\0";

/// The root vector for an E1.31 data packet.
const ROOT_VECTOR_DATA: u32 = 0x0000_0004;
/// The framing vector for a DMP data packet.
const FRAMING_VECTOR_DATA: u32 = 0x0000_0002;
/// The DMP vector for "set property".
const DMP_VECTOR_SET: u8 = 0x02;

/// The default priority a sender uses when it expresses no preference.
pub const DEFAULT_PRIORITY: u8 = 100;

/// A parsed E1.31 data packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataPacket<'a> {
    pub universe: u16,
    /// `0..=200`. The highest priority on a universe wins.
    pub priority: u8,
    /// The sender's stable identity, used to tell two senders apart when they share a priority.
    pub source: [u8; 16],
    /// The operator-facing name the sender publishes.
    pub source_name: String,
    pub sequence: u8,
    /// The sender is finished with this universe. A receiver releases it rather than holding the
    /// last values for ever.
    pub terminated: bool,
    /// Preview data is not for live output.
    pub preview: bool,
    /// Slot values, without the start code.
    pub data: &'a [u8],
}

/// Why a datagram is not a usable E1.31 packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    #[error("the datagram is too short to be an E1.31 packet")]
    TooShort,
    #[error("the datagram does not begin with the ACN packet identifier")]
    NotAcn,
    #[error("root vector {vector:#010x} is not an E1.31 data packet")]
    NotData { vector: u32 },
    #[error("framing vector {vector:#010x} is not DMP data")]
    NotDmp { vector: u32 },
    #[error("DMP vector {vector:#04x} is not a property set")]
    NotPropertySet { vector: u8 },
    #[error("start code {code:#04x} is not null; this is not dimmer data")]
    NotNullStartCode { code: u8 },
    #[error("universe 0 and universes above 63999 are reserved")]
    ReservedUniverse,
    #[error("the packet claims {claimed} property values but carries {available}")]
    LengthMismatch { claimed: usize, available: usize },
}

/// Parses an E1.31 data packet.
pub fn parse(datagram: &[u8]) -> Result<DataPacket<'_>, ParseError> {
    // Root layer 38, framing layer 77, DMP layer 11 up to and including the start code.
    if datagram.len() < 126 {
        return Err(ParseError::TooShort);
    }
    if &datagram[4..16] != IDENTIFIER {
        return Err(ParseError::NotAcn);
    }

    let root_vector = u32::from_be_bytes(datagram[18..22].try_into().expect("checked length"));
    if root_vector != ROOT_VECTOR_DATA {
        return Err(ParseError::NotData {
            vector: root_vector,
        });
    }
    let framing_vector = u32::from_be_bytes(datagram[40..44].try_into().expect("checked length"));
    if framing_vector != FRAMING_VECTOR_DATA {
        return Err(ParseError::NotDmp {
            vector: framing_vector,
        });
    }

    let source_name = String::from_utf8_lossy(&datagram[44..108])
        .trim_end_matches('\0')
        .trim()
        .to_owned();
    let priority = datagram[108];
    let sequence = datagram[111];
    let options = datagram[112];
    let universe = u16::from_be_bytes([datagram[113], datagram[114]]);

    let dmp_vector = datagram[117];
    if dmp_vector != DMP_VECTOR_SET {
        return Err(ParseError::NotPropertySet { vector: dmp_vector });
    }

    // The property value count includes the start code byte.
    let claimed = usize::from(u16::from_be_bytes([datagram[123], datagram[124]]));
    let start_code = datagram[125];
    if start_code != 0x00 {
        return Err(ParseError::NotNullStartCode { code: start_code });
    }
    if !(1..=63_999).contains(&universe) {
        return Err(ParseError::ReservedUniverse);
    }

    let slots = claimed.saturating_sub(1);
    let available = datagram.len() - 126;
    if available < slots {
        return Err(ParseError::LengthMismatch {
            claimed: slots,
            available,
        });
    }

    let mut source = [0u8; 16];
    source.copy_from_slice(&datagram[22..38]);

    Ok(DataPacket {
        universe,
        priority,
        source,
        source_name,
        sequence,
        // Bit 6 is stream termination, bit 7 is preview data.
        terminated: options & 0b0100_0000 != 0,
        preview: options & 0b1000_0000 != 0,
        data: &datagram[126..126 + slots],
    })
}

/// Builds an E1.31 data packet, for tests and interoperability fixtures.
pub fn encode(universe: u16, priority: u8, sequence: u8, source: [u8; 16], data: &[u8]) -> Vec<u8> {
    let mut packet = vec![0u8; 126 + data.len()];
    packet[0..2].copy_from_slice(&0x0010u16.to_be_bytes()); // preamble size
    packet[4..16].copy_from_slice(IDENTIFIER);
    packet[18..22].copy_from_slice(&ROOT_VECTOR_DATA.to_be_bytes());
    packet[22..38].copy_from_slice(&source);
    packet[40..44].copy_from_slice(&FRAMING_VECTOR_DATA.to_be_bytes());
    packet[44..53].copy_from_slice(b"ToskLight");
    packet[108] = priority;
    packet[111] = sequence;
    packet[113..115].copy_from_slice(&universe.to_be_bytes());
    packet[117] = DMP_VECTOR_SET;
    packet[123..125].copy_from_slice(&((data.len() + 1) as u16).to_be_bytes());
    packet[125] = 0x00; // null start code
    packet[126..].copy_from_slice(data);
    packet
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: [u8; 16] = [7u8; 16];

    #[test]
    fn a_well_formed_packet_parses() {
        let packet = encode(1, 100, 5, SOURCE, &[10, 20, 30]);
        let parsed = parse(&packet).unwrap();
        assert_eq!(parsed.universe, 1);
        assert_eq!(parsed.priority, 100);
        assert_eq!(parsed.sequence, 5);
        assert_eq!(parsed.source, SOURCE);
        assert_eq!(parsed.data, &[10, 20, 30]);
        assert!(!parsed.terminated);
        assert!(!parsed.preview);
    }

    #[test]
    fn the_source_name_is_readable() {
        let packet = encode(1, 100, 0, SOURCE, &[1]);
        assert_eq!(parse(&packet).unwrap().source_name, "ToskLight");
    }

    #[test]
    fn a_full_universe_parses() {
        let packet = encode(1, 100, 0, SOURCE, &[255u8; 512]);
        assert_eq!(parse(&packet).unwrap().data.len(), 512);
    }

    #[test]
    fn priority_is_carried_so_two_senders_can_be_resolved() {
        for priority in [0u8, 100, 200] {
            let packet = encode(1, priority, 0, SOURCE, &[1]);
            assert_eq!(parse(&packet).unwrap().priority, priority);
        }
    }

    #[test]
    fn termination_and_preview_are_read_from_the_options() {
        let mut packet = encode(1, 100, 0, SOURCE, &[1]);
        packet[112] = 0b0100_0000;
        assert!(parse(&packet).unwrap().terminated);

        packet[112] = 0b1000_0000;
        let parsed = parse(&packet).unwrap();
        assert!(parsed.preview && !parsed.terminated);
    }

    #[test]
    fn reserved_universes_are_refused() {
        for universe in [0u16, 64_000] {
            let packet = encode(universe, 100, 0, SOURCE, &[1]);
            assert_eq!(
                parse(&packet).unwrap_err(),
                ParseError::ReservedUniverse,
                "{universe}"
            );
        }
        let highest = encode(63_999, 100, 0, SOURCE, &[1]);
        assert!(parse(&highest).is_ok());
    }

    #[test]
    fn a_non_null_start_code_is_not_dimmer_data() {
        let mut packet = encode(1, 100, 0, SOURCE, &[1]);
        packet[125] = 0xDD;
        assert_eq!(
            parse(&packet).unwrap_err(),
            ParseError::NotNullStartCode { code: 0xDD }
        );
    }

    #[test]
    fn anything_that_is_not_acn_is_refused() {
        assert_eq!(parse(&[]).unwrap_err(), ParseError::TooShort);
        assert_eq!(parse(&[0u8; 64]).unwrap_err(), ParseError::TooShort);
        assert_eq!(parse(&[0u8; 200]).unwrap_err(), ParseError::NotAcn);
    }

    #[test]
    fn a_packet_that_lies_about_its_length_is_refused() {
        let mut packet = encode(1, 100, 0, SOURCE, &[1, 2, 3]);
        packet[123..125].copy_from_slice(&500u16.to_be_bytes());
        assert_eq!(
            parse(&packet).unwrap_err(),
            ParseError::LengthMismatch {
                claimed: 499,
                available: 3
            }
        );
    }

    #[test]
    fn parsing_never_panics_on_arbitrary_input() {
        for length in [0usize, 1, 50, 125, 126, 127, 200, 700] {
            for fill in [0u8, 0x55, 0xAA, 0xFF] {
                let mut datagram = vec![fill; length];
                if length >= 16 {
                    datagram[4..16].copy_from_slice(IDENTIFIER);
                }
                let _ = parse(&datagram);
            }
        }
    }
}
