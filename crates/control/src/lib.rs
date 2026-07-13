#![forbid(unsafe_code)]
//! Normalized control and timecode parsing shared by local MIDI, RTP-MIDI, OSC, and Art-Net adapters.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use light_core::CueListId;
#[cfg(feature = "native-midi")]
use midir::{Ignore, MidiInput, MidiInputConnection};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    fmt,
    net::SocketAddr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::net::UdpSocket;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SmpteTimecode {
    pub hours: u8,
    pub minutes: u8,
    pub seconds: u8,
    pub frames: u8,
    pub rate: FrameRate,
    pub source: String,
    pub received_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameRate {
    Fps24,
    Fps25,
    Fps2997Drop,
    Fps30,
}

impl FrameRate {
    pub fn nominal_frames(self) -> u8 {
        match self {
            Self::Fps24 => 24,
            Self::Fps25 => 25,
            Self::Fps2997Drop | Self::Fps30 => 30,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OscArgument {
    Int(i32),
    Float(f32),
    String(String),
    Bool(bool),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlEvent {
    Midi {
        status: u8,
        data: Vec<u8>,
    },
    Osc {
        address: String,
        arguments: Vec<OscArgument>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<String>,
    },
    Timecode(SmpteTimecode),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlTrigger {
    Osc { address: String },
    Midi { status: u8, data1: Option<u8> },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ControlAction {
    CueGo { cue_list_id: CueListId },
    CueBack { cue_list_id: CueListId },
    CuePause { cue_list_id: CueListId },
    CueRelease { cue_list_id: CueListId },
    Blackout { enabled: bool },
    GrandMaster { level: f32 },
    /// Routes the desk's global SET key to connected operator surfaces.
    DeskSet,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ControlMapping {
    pub name: String,
    pub enabled: bool,
    pub trigger: ControlTrigger,
    pub action: ControlAction,
}

impl ControlMapping {
    pub fn matches(&self, event: &ControlEvent) -> bool {
        if !self.enabled {
            return false;
        }
        match (&self.trigger, event) {
            (ControlTrigger::Osc { address: expected }, ControlEvent::Osc { address, .. }) => {
                expected == address
            }
            (
                ControlTrigger::Midi {
                    status: expected,
                    data1,
                },
                ControlEvent::Midi { status, data },
            ) => expected == status && data1.is_none_or(|expected| data.first() == Some(&expected)),
            _ => false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseError(pub &'static str);
impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}
impl std::error::Error for ParseError {}

#[async_trait]
pub trait ControlInput: Send {
    async fn next_event(&mut self) -> Option<ControlEvent>;
}

#[cfg(feature = "native-midi")]
pub fn available_midi_inputs() -> Result<Vec<String>, String> {
    let input = MidiInput::new("Light discovery").map_err(|error| error.to_string())?;
    input
        .ports()
        .iter()
        .map(|port| input.port_name(port).map_err(|error| error.to_string()))
        .collect()
}

#[cfg(not(feature = "native-midi"))]
pub fn available_midi_inputs() -> Result<Vec<String>, String> {
    Ok(Vec::new())
}

#[cfg(feature = "native-midi")]
pub struct MidiControlInput {
    _connection: MidiInputConnection<()>,
    receiver: tokio::sync::mpsc::Receiver<ControlEvent>,
}

#[cfg(feature = "native-midi")]
impl MidiControlInput {
    pub fn open(port_name: &str) -> Result<Self, String> {
        let mut input = MidiInput::new("Light").map_err(|error| error.to_string())?;
        input.ignore(Ignore::None);
        let port = input
            .ports()
            .into_iter()
            .find(|port| input.port_name(port).is_ok_and(|name| name == port_name))
            .ok_or_else(|| format!("MIDI input '{port_name}' was not found"))?;
        let source = port_name.to_owned();
        let (sender, receiver) = tokio::sync::mpsc::channel(1_024);
        let mut mtc = MidiTimecodeDecoder::default();
        let connection = input
            .connect(
                &port,
                "light-input",
                move |_timestamp, message, _| {
                    let event = if message.len() >= 2 && message[0] == 0xf1 {
                        mtc.push_quarter_frame(message[1], &source)
                            .ok()
                            .flatten()
                            .map(ControlEvent::Timecode)
                    } else {
                        message.first().map(|status| ControlEvent::Midi {
                            status: *status,
                            data: message[1..].to_vec(),
                        })
                    };
                    if let Some(event) = event {
                        let _ = sender.try_send(event);
                    }
                },
                (),
            )
            .map_err(|error| error.to_string())?;
        Ok(Self {
            _connection: connection,
            receiver,
        })
    }
}

#[cfg(feature = "native-midi")]
#[async_trait]
impl ControlInput for MidiControlInput {
    async fn next_event(&mut self) -> Option<ControlEvent> {
        self.receiver.recv().await
    }
}

/// Placeholder used by portable builds that intentionally omit native USB-MIDI.
#[cfg(not(feature = "native-midi"))]
pub struct MidiControlInput;

#[cfg(not(feature = "native-midi"))]
impl MidiControlInput {
    pub fn open(_port_name: &str) -> Result<Self, String> {
        Err("native MIDI is unavailable in this portable build".to_owned())
    }
}

#[cfg(not(feature = "native-midi"))]
#[async_trait]
impl ControlInput for MidiControlInput {
    async fn next_event(&mut self) -> Option<ControlEvent> {
        None
    }
}

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UdpInputProtocol {
    Osc,
    ArtTimeCode,
}

pub struct UdpControlInput {
    socket: UdpSocket,
    protocol: UdpInputProtocol,
    buffer: Vec<u8>,
}

impl UdpControlInput {
    pub async fn bind(address: SocketAddr, protocol: UdpInputProtocol) -> std::io::Result<Self> {
        Ok(Self {
            socket: UdpSocket::bind(address).await?,
            protocol,
            buffer: vec![0; 65_535],
        })
    }
}

#[async_trait]
impl ControlInput for UdpControlInput {
    async fn next_event(&mut self) -> Option<ControlEvent> {
        loop {
            let (length, source) = self.socket.recv_from(&mut self.buffer).await.ok()?;
            let result = match self.protocol {
                UdpInputProtocol::Osc => parse_osc_message(&self.buffer[..length]).map(|event| match event {
                    ControlEvent::Osc { address, arguments, .. } => ControlEvent::Osc { address, arguments, source: Some(source.to_string()) },
                    event => event,
                }),
                UdpInputProtocol::ArtTimeCode => {
                    parse_art_timecode(&self.buffer[..length], &source.to_string())
                        .map(ControlEvent::Timecode)
                }
            };
            if let Ok(event) = result {
                return Some(event);
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimecodeSourceConfig {
    pub source_prefix: String,
    pub priority: i16,
    pub fallback: bool,
    pub loss_timeout_millis: u64,
}

#[derive(Clone, Debug)]
struct TimecodeSourceState {
    config: TimecodeSourceConfig,
    last: SmpteTimecode,
    last_seen: std::time::Instant,
}

#[derive(Clone, Debug, Default)]
pub struct TimecodeRouter {
    configured: Vec<TimecodeSourceConfig>,
    sources: HashMap<String, TimecodeSourceState>,
    active: Option<String>,
}

impl TimecodeRouter {
    pub fn configure(&mut self, configured: Vec<TimecodeSourceConfig>) {
        self.configured = configured;
        self.sources.clear();
        self.active = None;
    }

    pub fn ingest(&mut self, timecode: SmpteTimecode) -> Option<&SmpteTimecode> {
        let config = self
            .configured
            .iter()
            .filter(|config| timecode.source.starts_with(&config.source_prefix))
            .max_by_key(|config| config.priority)?
            .clone();
        let source = timecode.source.clone();
        self.sources.insert(
            source.clone(),
            TimecodeSourceState {
                config,
                last: timecode,
                last_seen: std::time::Instant::now(),
            },
        );
        match &self.active {
            None => self.active = Some(source),
            Some(active) if active == &source => {}
            Some(active) => {
                let active_priority = self
                    .sources
                    .get(active)
                    .map(|state| state.config.priority)
                    .unwrap_or(i16::MIN);
                let candidate = &self.sources[&source];
                if candidate.config.priority > active_priority {
                    self.active = Some(source);
                }
            }
        }
        self.current()
    }

    pub fn poll_loss(&mut self) -> Option<&SmpteTimecode> {
        let active = self.active.clone()?;
        let lost = self.sources.get(&active).is_none_or(|state| {
            state.last_seen.elapsed() > Duration::from_millis(state.config.loss_timeout_millis)
        });
        if lost {
            self.active = self
                .sources
                .iter()
                .filter(|(_, state)| {
                    state.config.fallback
                        && state.last_seen.elapsed()
                            <= Duration::from_millis(state.config.loss_timeout_millis)
                })
                .max_by_key(|(_, state)| state.config.priority)
                .map(|(source, _)| source.clone());
        }
        self.current()
    }

    pub fn current(&self) -> Option<&SmpteTimecode> {
        self.active
            .as_ref()
            .and_then(|active| self.sources.get(active))
            .map(|state| &state.last)
    }
    pub fn active_source(&self) -> Option<&str> {
        self.active.as_deref()
    }
}

/// Parses an ArtTimeCode datagram according to Art-Net 4. Stream ID is incorporated into the
/// normalized source identity so independent timecode streams never switch silently.
pub fn parse_art_timecode(packet: &[u8], source: &str) -> Result<SmpteTimecode, ParseError> {
    if packet.len() < 19 || &packet[..8] != b"Art-Net\0" {
        return Err(ParseError("invalid Art-Net packet"));
    }
    if u16::from_le_bytes([packet[8], packet[9]]) != 0x9700 {
        return Err(ParseError("packet is not ArtTimeCode"));
    }
    if u16::from_be_bytes([packet[10], packet[11]]) < 14 {
        return Err(ParseError("unsupported Art-Net protocol version"));
    }
    let rate = match packet[18] {
        0 => FrameRate::Fps24,
        1 => FrameRate::Fps25,
        2 => FrameRate::Fps2997Drop,
        3 => FrameRate::Fps30,
        _ => return Err(ParseError("invalid ArtTimeCode rate")),
    };
    let timecode = SmpteTimecode {
        frames: packet[14],
        seconds: packet[15],
        minutes: packet[16],
        hours: packet[17],
        rate,
        source: format!("artnet:{source}:{}", packet[13]),
        received_at: Utc::now(),
    };
    validate_timecode(&timecode)?;
    Ok(timecode)
}

#[derive(Clone, Debug, Default)]
pub struct MidiTimecodeDecoder {
    nibbles: [u8; 8],
    seen: u8,
}

impl MidiTimecodeDecoder {
    /// Pushes an MTC quarter-frame data byte (the payload following MIDI status 0xF1).
    /// A complete normalized value is returned after all eight message types have arrived.
    pub fn push_quarter_frame(
        &mut self,
        data: u8,
        source: &str,
    ) -> Result<Option<SmpteTimecode>, ParseError> {
        if data & 0x80 != 0 {
            return Err(ParseError("invalid MTC quarter-frame data"));
        }
        let piece = data >> 4;
        let nibble = data & 0x0f;
        if piece > 7 {
            return Err(ParseError("invalid MTC piece"));
        }
        self.nibbles[piece as usize] = nibble;
        self.seen |= 1 << piece;
        if self.seen != 0xff {
            return Ok(None);
        }
        let rate = match self.nibbles[7] >> 1 {
            0 => FrameRate::Fps24,
            1 => FrameRate::Fps25,
            2 => FrameRate::Fps2997Drop,
            3 => FrameRate::Fps30,
            _ => unreachable!(),
        };
        let timecode = SmpteTimecode {
            frames: self.nibbles[0] | ((self.nibbles[1] & 0x01) << 4),
            seconds: self.nibbles[2] | ((self.nibbles[3] & 0x03) << 4),
            minutes: self.nibbles[4] | ((self.nibbles[5] & 0x03) << 4),
            hours: self.nibbles[6] | ((self.nibbles[7] & 0x01) << 4),
            rate,
            source: format!("midi:{source}"),
            received_at: Utc::now(),
        };
        validate_timecode(&timecode)?;
        Ok(Some(timecode))
    }
}

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
        while !packet.len().is_multiple_of(4) { packet.push(0); }
    }
    let mut packet = Vec::new();
    push_string(&mut packet, address);
    let mut tags = String::from(",");
    for argument in arguments { tags.push(match argument { OscArgument::Int(_) => 'i', OscArgument::Float(_) => 'f', OscArgument::String(_) => 's', OscArgument::Bool(true) => 'T', OscArgument::Bool(false) => 'F' }); }
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

fn validate_timecode(timecode: &SmpteTimecode) -> Result<(), ParseError> {
    if timecode.hours >= 24
        || timecode.minutes >= 60
        || timecode.seconds >= 60
        || timecode.frames >= timecode.rate.nominal_frames()
    {
        return Err(ParseError("timecode value is out of range"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_art_timecode_with_stream_identity() {
        let packet = [
            b'A', b'r', b't', b'-', b'N', b'e', b't', 0, 0x00, 0x97, 0, 14, 0, 2, 12, 34, 56, 7, 1,
        ];
        let result = parse_art_timecode(&packet, "10.0.0.1").unwrap();
        assert_eq!(
            (result.hours, result.minutes, result.seconds, result.frames),
            (7, 56, 34, 12)
        );
        assert_eq!(result.rate, FrameRate::Fps25);
        assert_eq!(result.source, "artnet:10.0.0.1:2");
    }

    #[test]
    fn rejects_out_of_range_art_timecode() {
        let mut packet = [
            b'A', b'r', b't', b'-', b'N', b'e', b't', 0, 0x00, 0x97, 0, 14, 0, 0, 24, 0, 0, 0, 0,
        ];
        assert!(parse_art_timecode(&packet, "source").is_err());
        packet[14] = 23;
        assert!(parse_art_timecode(&packet, "source").is_ok());
    }

    #[test]
    fn decodes_complete_mtc_quarter_frame_cycle() {
        let mut decoder = MidiTimecodeDecoder::default();
        let messages = [0x02, 0x11, 0x23, 0x30, 0x44, 0x50, 0x66, 0x74];
        let mut result = None;
        for message in messages {
            result = decoder
                .push_quarter_frame(message, "port-1")
                .unwrap()
                .or(result);
        }
        let result = result.unwrap();
        assert_eq!(
            (result.hours, result.minutes, result.seconds, result.frames),
            (6, 4, 3, 18)
        );
        assert_eq!(result.rate, FrameRate::Fps2997Drop);
    }

    #[test]
    fn parses_typed_osc_message() {
        let packet = b"/light/go\0\0\0,ifsT\0\0\0\0\0\0*?\xc0\0\0main\0\0\0\0";
        let result = parse_osc_message(packet).unwrap();
        assert_eq!(
            result,
            ControlEvent::Osc {
                address: "/light/go".into(),
                arguments: vec![
                    OscArgument::Int(42),
                    OscArgument::Float(1.5),
                    OscArgument::String("main".into()),
                    OscArgument::Bool(true)
                ],
                source: None,
            }
        );
    }

    #[test]
    fn encoded_osc_message_round_trips_supported_arguments() {
        let arguments=vec![OscArgument::Int(7),OscArgument::Float(0.5),OscArgument::String("slow".into()),OscArgument::Bool(false)];
        let packet=encode_osc_message("/light/test",&arguments).unwrap();
        assert_eq!(parse_osc_message(&packet).unwrap(),ControlEvent::Osc{address:"/light/test".into(),arguments,source:None});
    }

    fn tc(source: &str) -> SmpteTimecode {
        SmpteTimecode {
            hours: 0,
            minutes: 0,
            seconds: 0,
            frames: 0,
            rate: FrameRate::Fps25,
            source: source.into(),
            received_at: Utc::now(),
        }
    }

    #[test]
    fn timecode_router_obeys_priority_and_explicit_fallback() {
        let mut router = TimecodeRouter::default();
        router.configure(vec![
            TimecodeSourceConfig {
                source_prefix: "osc:".into(),
                priority: 10,
                fallback: true,
                loss_timeout_millis: 1000,
            },
            TimecodeSourceConfig {
                source_prefix: "midi:".into(),
                priority: 20,
                fallback: false,
                loss_timeout_millis: 0,
            },
        ]);
        router.ingest(tc("osc:backup"));
        assert_eq!(router.active_source(), Some("osc:backup"));
        router.ingest(tc("midi:primary"));
        assert_eq!(router.active_source(), Some("midi:primary"));
        std::thread::sleep(Duration::from_millis(1));
        router.poll_loss();
        assert_eq!(router.active_source(), Some("osc:backup"));
    }

    #[test]
    fn parses_rtp_midi_commands_and_running_status() {
        let mut packet = vec![
            0x80, 0x61, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0x06, 0x90, 60, 127, 0, 61, 0,
        ];
        let messages = parse_rtp_midi(&packet).unwrap();
        assert_eq!(messages, vec![vec![0x90, 60, 127], vec![0x90, 61, 0]]);
        packet[12] = 0x20 | 0x07;
        packet.insert(13, 0);
        assert_eq!(parse_rtp_midi(&packet).unwrap().len(), 2);
    }

    #[test]
    fn rejects_truncated_rtp_midi_command_section() {
        let packet = [0x80, 0x61, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0x0f, 0x90, 60];
        assert!(parse_rtp_midi(&packet).is_err());
    }
}
