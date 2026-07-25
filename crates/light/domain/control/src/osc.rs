use crate::{ControlEvent, OscArgument, ParseError};

pub fn parse_osc_message(packet: &[u8]) -> Result<ControlEvent, ParseError> {
    let (address, offset) = osc_string(packet, 0)?;
    if !address.starts_with('/') {
        return Err(ParseError("invalid OSC address"));
    }
    let (types, mut offset) = osc_string(packet, offset)?;
    let types = types
        .strip_prefix(',')
        .ok_or(ParseError("OSC type tag is missing"))?;
    let mut arguments = Vec::with_capacity(types.len());
    for kind in types.bytes() {
        match kind {
            b'i' => {
                let bytes = packet
                    .get(offset..offset + 4)
                    .ok_or(ParseError("truncated OSC int"))?;
                arguments.push(OscArgument::Int(i32::from_be_bytes(
                    bytes.try_into().expect("four bytes"),
                )));
                offset += 4;
            }
            b'f' => {
                let bytes = packet
                    .get(offset..offset + 4)
                    .ok_or(ParseError("truncated OSC float"))?;
                arguments.push(OscArgument::Float(f32::from_bits(u32::from_be_bytes(
                    bytes.try_into().expect("four bytes"),
                ))));
                offset += 4;
            }
            b's' => {
                let (value, next) = osc_string(packet, offset)?;
                arguments.push(OscArgument::String(value.to_owned()));
                offset = next;
            }
            b'T' => arguments.push(OscArgument::Bool(true)),
            b'F' => arguments.push(OscArgument::Bool(false)),
            _ => return Err(ParseError("unsupported OSC type")),
        }
    }
    Ok(ControlEvent::Osc {
        address: address.to_owned(),
        arguments,
        source: None,
    })
}

/// Encodes one OSC 1.0 message using the argument types supported by the parser.
pub fn encode_osc_message(address: &str, arguments: &[OscArgument]) -> Result<Vec<u8>, ParseError> {
    if !address.starts_with('/') || address.as_bytes().contains(&0) {
        return Err(ParseError("invalid OSC address"));
    }
    fn push_string(packet: &mut Vec<u8>, value: &str) {
        packet.extend_from_slice(value.as_bytes());
        packet.push(0);
        while !packet.len().is_multiple_of(4) {
            packet.push(0);
        }
    }
    let mut packet = Vec::new();
    push_string(&mut packet, address);
    let mut tags = String::from(",");
    for argument in arguments {
        tags.push(match argument {
            OscArgument::Int(_) => 'i',
            OscArgument::Float(_) => 'f',
            OscArgument::String(_) => 's',
            OscArgument::Bool(true) => 'T',
            OscArgument::Bool(false) => 'F',
        });
    }
    push_string(&mut packet, &tags);
    for argument in arguments {
        match argument {
            OscArgument::Int(value) => packet.extend_from_slice(&value.to_be_bytes()),
            OscArgument::Float(value) => packet.extend_from_slice(&value.to_bits().to_be_bytes()),
            OscArgument::String(value) => push_string(&mut packet, value),
            OscArgument::Bool(_) => {}
        }
    }
    Ok(packet)
}

fn osc_string(packet: &[u8], offset: usize) -> Result<(&str, usize), ParseError> {
    let tail = packet
        .get(offset..)
        .ok_or(ParseError("truncated OSC string"))?;
    let length = tail
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(ParseError("unterminated OSC string"))?;
    let value =
        std::str::from_utf8(&tail[..length]).map_err(|_| ParseError("OSC string is not UTF-8"))?;
    let next = offset + (length + 1).next_multiple_of(4);
    if next > packet.len() {
        return Err(ParseError("truncated OSC padding"));
    }
    Ok((value, next))
}
