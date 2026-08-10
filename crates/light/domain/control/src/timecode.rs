use crate::{FrameRate, ParseError, SmpteTimecode};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// The one Timecode authority selected for this desk.
///
/// External identities are the normalized identities emitted by an input adapter (for example
/// `artnet:10.0.0.1:2`). They are exact on purpose: selecting Art-Net must not make a second
/// sender or stream take over the desk silently.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TimecodeSourceSelection {
    Internal,
    External { source: String },
}

impl Default for TimecodeSourceSelection {
    fn default() -> Self {
        Self::Internal
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalTimecodeLossPolicy {
    #[default]
    ContinueInternal,
    Pause,
    Stop,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TimecodeRouterConfig {
    pub selected_source: TimecodeSourceSelection,
    pub desk_rate: FrameRate,
    pub external_loss_policy: ExternalTimecodeLossPolicy,
    pub loss_timeout_millis: u64,
}

impl Default for TimecodeRouterConfig {
    fn default() -> Self {
        Self {
            selected_source: TimecodeSourceSelection::Internal,
            desk_rate: FrameRate::Fps30,
            external_loss_policy: ExternalTimecodeLossPolicy::ContinueInternal,
            loss_timeout_millis: 500,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TimecodeSourceTransition {
    ExternalLocked { source: String },
    ExternalLost { policy: ExternalTimecodeLossPolicy },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimecodeRateWarning {
    pub source_rate: FrameRate,
    pub desk_rate: FrameRate,
}

#[derive(Clone, Debug)]
pub struct TimecodeRouter {
    config: TimecodeRouterConfig,
    current: Option<SmpteTimecode>,
    last_seen: Option<Instant>,
    external_locked: bool,
    transition: Option<TimecodeSourceTransition>,
    rate_warning: Option<TimecodeRateWarning>,
}

impl Default for TimecodeRouter {
    fn default() -> Self {
        Self {
            config: TimecodeRouterConfig::default(),
            current: None,
            last_seen: None,
            external_locked: false,
            transition: None,
            rate_warning: None,
        }
    }
}

impl TimecodeRouter {
    pub fn configure(&mut self, config: TimecodeRouterConfig) {
        self.config = config;
        self.current = None;
        self.last_seen = None;
        self.external_locked = false;
        self.transition = None;
        self.rate_warning = None;
    }

    pub fn config(&self) -> &TimecodeRouterConfig {
        &self.config
    }

    /// Accepts only the explicitly selected external identity. Known SMPTE rates are converted to
    /// the desk rate before they become authoritative; a warning remains available to operator
    /// surfaces until the source or configuration changes.
    pub fn ingest(&mut self, timecode: SmpteTimecode) -> Option<&SmpteTimecode> {
        self.ingest_at(timecode, Instant::now())
    }

    pub fn poll_loss(&mut self) -> Option<&SmpteTimecode> {
        self.poll_loss_at(Instant::now())
    }

    pub fn current(&self) -> Option<&SmpteTimecode> {
        self.current.as_ref()
    }

    pub fn active_source(&self) -> Option<&str> {
        match &self.config.selected_source {
            TimecodeSourceSelection::Internal => Some("internal"),
            TimecodeSourceSelection::External { source } if self.external_locked => Some(source),
            TimecodeSourceSelection::External { .. } => None,
        }
    }

    pub fn rate_warning(&self) -> Option<&TimecodeRateWarning> {
        self.rate_warning.as_ref()
    }

    /// Whether running Timecodes should advance from the desk's internal generator on this tick.
    /// An external source owns position while locked; Continue Internal takes over only for the
    /// loss interval and immediately yields again on the next selected-source frame.
    pub fn uses_internal_clock(&self) -> bool {
        match self.config.selected_source {
            TimecodeSourceSelection::Internal => true,
            TimecodeSourceSelection::External { .. } => {
                !self.external_locked
                    && self.config.external_loss_policy
                        == ExternalTimecodeLossPolicy::ContinueInternal
            }
        }
    }

    /// Returns each lock/loss edge once. Consumers use the loss edge to apply the configured
    /// continue, pause, or stop behavior to running Timecodes.
    pub fn take_transition(&mut self) -> Option<TimecodeSourceTransition> {
        self.transition.take()
    }

    pub(crate) fn ingest_at(
        &mut self,
        timecode: SmpteTimecode,
        now: Instant,
    ) -> Option<&SmpteTimecode> {
        let TimecodeSourceSelection::External { source } = &self.config.selected_source else {
            return None;
        };
        if &timecode.source != source || validate_timecode(&timecode).is_err() {
            return None;
        }

        let source_rate = timecode.rate;
        let converted = convert_rate(timecode, self.config.desk_rate);
        self.rate_warning = (source_rate != self.config.desk_rate).then_some(TimecodeRateWarning {
            source_rate,
            desk_rate: self.config.desk_rate,
        });
        self.current = Some(converted);
        self.last_seen = Some(now);
        if !self.external_locked {
            self.external_locked = true;
            self.transition = Some(TimecodeSourceTransition::ExternalLocked {
                source: source.clone(),
            });
        }
        self.current()
    }

    pub(crate) fn poll_loss_at(&mut self, now: Instant) -> Option<&SmpteTimecode> {
        if self.external_locked
            && self.last_seen.is_some_and(|last_seen| {
                now.saturating_duration_since(last_seen)
                    > Duration::from_millis(self.config.loss_timeout_millis)
            })
        {
            self.external_locked = false;
            self.current = None;
            self.transition = Some(TimecodeSourceTransition::ExternalLost {
                policy: self.config.external_loss_policy,
            });
        }
        self.current()
    }
}

fn convert_rate(mut timecode: SmpteTimecode, target: FrameRate) -> SmpteTimecode {
    if timecode.rate == target {
        return timecode;
    }
    let (source_numerator, source_denominator) = rate_ratio(timecode.rate);
    let (target_numerator, target_denominator) = rate_ratio(target);
    let numerator = u64::from(timecode.frames) * target_numerator * source_denominator;
    let denominator = source_numerator * target_denominator;
    timecode.frames = u8::try_from(numerator / denominator)
        .unwrap_or(u8::MAX)
        .min(target.nominal_frames().saturating_sub(1));
    timecode.rate = target;
    timecode
}

const fn rate_ratio(rate: FrameRate) -> (u64, u64) {
    match rate {
        FrameRate::Fps24 => (24, 1),
        FrameRate::Fps25 => (25, 1),
        FrameRate::Fps2997Drop => (30_000, 1_001),
        FrameRate::Fps30 => (30, 1),
        FrameRate::FpsCustom(rate) => (rate as u64, 1),
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
