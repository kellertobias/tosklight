use crate::{ControlEvent, ControlInput, MidiTimecodeDecoder, ParseError};
use async_trait::async_trait;
use std::{
    collections::VecDeque,
    net::SocketAddr,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::net::UdpSocket;

pub struct RtpMidiInput {
    control: UdpSocket,
    data: UdpSocket,
    name: String,
    ssrc: u32,
    pending: VecDeque<ControlEvent>,
    mtc: MidiTimecodeDecoder,
}
impl RtpMidiInput {
    pub async fn bind(
        control_address: SocketAddr,
        name: impl Into<String>,
    ) -> std::io::Result<Self> {
        let data_address = SocketAddr::new(
            control_address.ip(),
            control_address.port().checked_add(1).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "RTP-MIDI control port cannot be 65535",
                )
            })?,
        );
        let ssrc = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        Ok(Self {
            control: UdpSocket::bind(control_address).await?,
            data: UdpSocket::bind(data_address).await?,
            name: name.into(),
            ssrc,
            pending: VecDeque::new(),
            mtc: MidiTimecodeDecoder::default(),
        })
    }
    async fn handle_session_packet(
        socket: &UdpSocket,
        packet: &[u8],
        source: SocketAddr,
        name: &str,
        ssrc: u32,
    ) -> bool {
        if packet.len() < 4 || packet[..2] != [0xff, 0xff] {
            return false;
        }
        match &packet[2..4] {
            b"IN" if packet.len() >= 16 => {
                let mut reply = Vec::with_capacity(16 + name.len() + 1);
                reply.extend_from_slice(&[0xff, 0xff]);
                reply.extend_from_slice(b"OK");
                reply.extend_from_slice(&packet[4..12]);
                reply.extend_from_slice(&ssrc.to_be_bytes());
                reply.extend_from_slice(name.as_bytes());
                reply.push(0);
                let _ = socket.send_to(&reply, source).await;
            }
            b"CK" if packet.len() >= 36 => {
                let count = packet[8];
                if count < 2 {
                    let mut reply = packet[..36].to_vec();
                    reply[8] = count + 1;
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_micros() as u64;
                    if count == 0 {
                        reply[20..28].copy_from_slice(&now.to_be_bytes());
                    } else {
                        reply[28..36].copy_from_slice(&now.to_be_bytes());
                    }
                    let _ = socket.send_to(&reply, source).await;
                }
            }
            _ => {}
        }
        true
    }
    fn enqueue_midi(&mut self, packet: &[u8], source: SocketAddr) {
        let Ok(messages) = parse_rtp_midi(packet) else {
            return;
        };
        for message in messages {
            if message.len() >= 2 && message[0] == 0xf1 {
                if let Ok(Some(timecode)) = self
                    .mtc
                    .push_quarter_frame(message[1], &format!("rtp:{source}"))
                {
                    self.pending.push_back(ControlEvent::Timecode(timecode));
                }
            } else if let Some(status) = message.first() {
                self.pending.push_back(ControlEvent::Midi {
                    status: *status,
                    data: message[1..].to_vec(),
                });
            }
        }
    }
}
#[async_trait]
impl ControlInput for RtpMidiInput {
    async fn next_event(&mut self) -> Option<ControlEvent> {
        loop {
            if let Some(event) = self.pending.pop_front() {
                return Some(event);
            }
            let mut control = [0_u8; 2_048];
            let mut data = [0_u8; 65_535];
            tokio::select! {received=self.control.recv_from(&mut control)=>{let(length,source)=received.ok()?;Self::handle_session_packet(&self.control,&control[..length],source,&self.name,self.ssrc).await;}received=self.data.recv_from(&mut data)=>{let(length,source)=received.ok()?;if !Self::handle_session_packet(&self.data,&data[..length],source,&self.name,self.ssrc).await{self.enqueue_midi(&data[..length],source);}}}
        }
    }
}

pub fn parse_rtp_midi(packet: &[u8]) -> Result<Vec<Vec<u8>>, ParseError> {
    if packet.len() < 13 || packet[0] >> 6 != 2 {
        return Err(ParseError("invalid RTP-MIDI packet"));
    }
    let mut cursor = 12;
    let header = packet[cursor];
    cursor += 1;
    let extended = header & 0x80 != 0;
    let first_delta = header & 0x20 != 0;
    let mut command_length = usize::from(header & 0x0f);
    if extended {
        let low = *packet
            .get(cursor)
            .ok_or(ParseError("truncated RTP-MIDI command length"))?;
        cursor += 1;
        command_length = (command_length << 8) | usize::from(low);
    }
    let end = cursor
        .checked_add(command_length)
        .filter(|end| *end <= packet.len())
        .ok_or(ParseError("truncated RTP-MIDI command section"))?;
    let mut messages = Vec::new();
    let mut running = None;
    let mut first = true;
    while cursor < end {
        if !first || first_delta {
            read_variable_length(packet, &mut cursor, end)?;
        }
        first = false;
        if cursor >= end {
            break;
        }
        let status = if packet[cursor] & 0x80 != 0 {
            let status = packet[cursor];
            cursor += 1;
            running = (status < 0xf0).then_some(status);
            status
        } else {
            running.ok_or(ParseError("RTP-MIDI running status is missing"))?
        };
        let data_length = midi_data_length(status, packet, cursor, end)?;
        let data_end = cursor + data_length;
        let mut message = vec![status];
        message.extend_from_slice(&packet[cursor..data_end]);
        cursor = data_end;
        messages.push(message);
    }
    Ok(messages)
}
fn read_variable_length(packet: &[u8], cursor: &mut usize, end: usize) -> Result<u32, ParseError> {
    let mut value = 0;
    for _ in 0..4 {
        let byte = *packet
            .get(*cursor)
            .filter(|_| *cursor < end)
            .ok_or(ParseError("truncated RTP-MIDI delta time"))?;
        *cursor += 1;
        value = (value << 7) | u32::from(byte & 0x7f);
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(ParseError("RTP-MIDI delta time is too long"))
}
fn midi_data_length(
    status: u8,
    packet: &[u8],
    cursor: usize,
    end: usize,
) -> Result<usize, ParseError> {
    let length = match status {
        0x80..=0xbf | 0xe0..=0xef => 2,
        0xc0..=0xdf => 1,
        0xf1 | 0xf3 => 1,
        0xf2 => 2,
        0xf0 => packet
            .get(cursor..end)
            .and_then(|tail| tail.iter().position(|byte| *byte == 0xf7))
            .map(|position| position + 1)
            .ok_or(ParseError("unterminated MIDI SysEx"))?,
        0xf4..=0xff => 0,
        _ => return Err(ParseError("invalid MIDI status")),
    };
    if cursor + length > end {
        return Err(ParseError("truncated MIDI command"));
    }
    Ok(length)
}
