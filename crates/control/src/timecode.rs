use crate::{FrameRate, ParseError, SmpteTimecode};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Duration};

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
