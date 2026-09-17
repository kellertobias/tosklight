//! The ToskLight Speed Group OSC contract.
//!
//! No widely implemented lighting protocol carries several independent, identified tempo groups:
//! Art-Net and sACN carry level data, Art-Net TimeCode and MTC carry position rather than tempo,
//! MIDI Beat Clock and Ableton Link carry exactly one tempo, and CITP/MSEX has no tempo message.
//! Speed Groups therefore travel as OSC 1.0 over UDP, in the message this module decodes. The
//! operator-facing contract is `docs/help/90-Protocols/02-media-speed-groups.md`.
//!
//! ```text
//! /tosklight/speed-group  ,siiffi  source sequence group bpm beat-phase running
//! ```
//!
//! Floats may also be sent as OSC doubles (`d`), and `running` as OSC `T`/`F`. A bundle carries any
//! number of these messages. The Media Server only ever receives this message; it never sends it.

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::UdpSocket;

use crate::ingress::{IngressError, PortSharing, bind};
use crate::speed_group_reception::SpeedGroupUpdate;

/// The one OSC address this contract defines.
pub const SPEED_GROUP_OSC_ADDRESS: &str = "/tosklight/speed-group";

const BUNDLE_TAG: &[u8] = b"#bundle\0";
const RECEIVE_BUFFER: usize = 4096;

/// Why a datagram is not a valid Speed Group update.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SpeedGroupDecodeError {
    #[error("not an OSC packet: {0}")]
    Malformed(&'static str),
    #[error("unexpected OSC address {0}")]
    Address(String),
    #[error("type tags {0} do not match ,siiffi")]
    TypeTags(String),
    #[error("the sequence number {0} is negative")]
    Sequence(i32),
    #[error("the Speed Group number {0} is not positive")]
    Group(i32),
}

/// Decodes one datagram into the updates it carries, in order.
pub fn decode(packet: &[u8]) -> Result<Vec<SpeedGroupUpdate>, SpeedGroupDecodeError> {
    let mut updates = Vec::new();
    decode_into(packet, &mut updates, 0)?;
    Ok(updates)
}

fn decode_into(
    packet: &[u8],
    updates: &mut Vec<SpeedGroupUpdate>,
    depth: usize,
) -> Result<(), SpeedGroupDecodeError> {
    if !packet.starts_with(BUNDLE_TAG) {
        updates.push(decode_message(packet)?);
        return Ok(());
    }
    if depth > 4 {
        return Err(SpeedGroupDecodeError::Malformed("bundles nest too deeply"));
    }
    // The tag and the eight-byte time tag. Speed Group updates apply on arrival.
    let mut cursor = Cursor::new(packet.get(16..).ok_or(SpeedGroupDecodeError::Malformed(
        "the bundle header is truncated",
    ))?);
    while !cursor.is_empty() {
        let size = usize::try_from(cursor.int()?)
            .map_err(|_| SpeedGroupDecodeError::Malformed("a bundle element size is negative"))?;
        decode_into(cursor.take(size)?, updates, depth + 1)?;
    }
    Ok(())
}

fn decode_message(packet: &[u8]) -> Result<SpeedGroupUpdate, SpeedGroupDecodeError> {
    let mut cursor = Cursor::new(packet);
    let address = cursor.string()?;
    if address != SPEED_GROUP_OSC_ADDRESS {
        return Err(SpeedGroupDecodeError::Address(address));
    }
    let tags = cursor.string()?;
    let bytes = tags.as_bytes();
    let shape_matches = bytes.len() == 7
        && bytes[..4] == *b",sii"
        && matches!(bytes[4], b'f' | b'd')
        && matches!(bytes[5], b'f' | b'd')
        && matches!(bytes[6], b'i' | b'T' | b'F');
    if !shape_matches {
        return Err(SpeedGroupDecodeError::TypeTags(tags));
    }
    let source = cursor.string()?;
    let sequence = cursor.int()?;
    let group = cursor.int()?;
    let bpm = cursor.real(bytes[4])?;
    let beat_phase = cursor.real(bytes[5])?;
    let running = match bytes[6] {
        b'i' => cursor.int()? != 0,
        tag => tag == b'T',
    };
    if !cursor.is_empty() {
        return Err(SpeedGroupDecodeError::Malformed(
            "trailing bytes after the arguments",
        ));
    }
    Ok(SpeedGroupUpdate {
        source,
        sequence: u32::try_from(sequence).map_err(|_| SpeedGroupDecodeError::Sequence(sequence))?,
        group: u32::try_from(group)
            .ok()
            .filter(|group| *group > 0)
            .ok_or(SpeedGroupDecodeError::Group(group))?,
        bpm,
        beat_phase,
        running,
    })
}

struct Cursor<'a> {
    bytes: &'a [u8],
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    const fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], SpeedGroupDecodeError> {
        if length > self.bytes.len() {
            return Err(SpeedGroupDecodeError::Malformed("the packet is truncated"));
        }
        let (taken, rest) = self.bytes.split_at(length);
        self.bytes = rest;
        Ok(taken)
    }

    fn string(&mut self) -> Result<String, SpeedGroupDecodeError> {
        let end = self.bytes.iter().position(|byte| *byte == 0).ok_or(
            SpeedGroupDecodeError::Malformed("a string is not terminated"),
        )?;
        let raw = self.take((end + 4) & !3)?;
        String::from_utf8(raw[..end].to_vec())
            .map_err(|_| SpeedGroupDecodeError::Malformed("a string is not UTF-8"))
    }

    fn int(&mut self) -> Result<i32, SpeedGroupDecodeError> {
        let raw = self.take(4)?;
        Ok(i32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]))
    }

    fn real(&mut self, tag: u8) -> Result<f64, SpeedGroupDecodeError> {
        if tag == b'd' {
            let raw = self.take(8)?;
            let mut bytes = [0; 8];
            bytes.copy_from_slice(raw);
            return Ok(f64::from_be_bytes(bytes));
        }
        let raw = self.take(4)?;
        Ok(f64::from(f32::from_be_bytes([
            raw[0], raw[1], raw[2], raw[3],
        ])))
    }
}

/// Encodes one update exactly as the contract describes it.
///
/// The Media Server never sends Speed Groups; this exists so tests and tooling can produce the
/// datagrams a desk sends.
pub fn encode(update: &SpeedGroupUpdate) -> Vec<u8> {
    let mut packet = Vec::new();
    push_string(&mut packet, SPEED_GROUP_OSC_ADDRESS);
    push_string(&mut packet, ",siiffi");
    push_string(&mut packet, &update.source);
    packet.extend_from_slice(&(update.sequence as i32).to_be_bytes());
    packet.extend_from_slice(&(update.group as i32).to_be_bytes());
    packet.extend_from_slice(&(update.bpm as f32).to_be_bytes());
    packet.extend_from_slice(&(update.beat_phase as f32).to_be_bytes());
    packet.extend_from_slice(&i32::from(update.running).to_be_bytes());
    packet
}

fn push_string(packet: &mut Vec<u8>, value: &str) {
    packet.extend_from_slice(value.as_bytes());
    let padded = (value.len() + 4) & !3;
    packet.resize(packet.len() + padded - value.len(), 0);
}

/// Receives Speed Group datagrams.
#[derive(Debug)]
pub struct SpeedGroupListener {
    socket: Arc<UdpSocket>,
}

/// One received datagram: where it came from and what it decoded to.
pub type SpeedGroupDatagram = (
    SocketAddr,
    Result<Vec<SpeedGroupUpdate>, SpeedGroupDecodeError>,
);

impl SpeedGroupListener {
    pub fn bind(address: SocketAddr) -> Result<Self, IngressError> {
        let socket = bind("Speed Groups", address, PortSharing::Exclusive)?;
        let socket = UdpSocket::from_std(socket).map_err(|source| IngressError::BindConflict {
            protocol: "Speed Groups",
            address,
            source,
        })?;
        Ok(Self {
            socket: Arc::new(socket),
        })
    }

    pub fn local_address(&self) -> std::io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    /// Waits for the next datagram.
    pub async fn receive(&mut self) -> SpeedGroupDatagram {
        let mut buffer = vec![0u8; RECEIVE_BUFFER];
        loop {
            if let Ok((length, from)) = self.socket.recv_from(&mut buffer).await {
                return (from, decode(&buffer[..length]));
            }
        }
    }
}

#[cfg(test)]
#[path = "speed_group_osc_tests.rs"]
mod tests;
