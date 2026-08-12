//! Commands and control-source ownership.
//!
//! Every mutation enters through a typed command carrying its source and its timestamp. The
//! reducer decides acceptance; no adapter decides for itself whether it is allowed to write.
//!
//! The legacy web UI simply became read-only whenever DMX packets were active. That is now an
//! explicit policy rather than a side effect: active external DMX owns continuously controlled
//! values, the web UI can always inspect state and perform explicitly allowed administrative
//! actions, and DMX activity expires after a documented timeout so control returns.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::address::MediaAddress;
use crate::color::{FlipMirror, Tint};
use crate::layer::{EffectSlot, ScalingMode, SourceStatus};
use crate::output::OutputId;
use crate::personality::decode::DecodedFrame;
use crate::playback::PlayMode;
use crate::speed::SpeedMultiplier;

/// An intent-shaped web edit of the values the network personality controls on one layer.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LayerControls {
    pub address: Option<MediaAddress>,
    pub play_mode: Option<PlayMode>,
    pub scale_x: Option<f32>,
    pub scale_y: Option<f32>,
    pub scaling_mode: Option<ScalingMode>,
    pub position_x: Option<f32>,
    pub position_y: Option<f32>,
    pub rotation: Option<f32>,
    pub dimmer: Option<f32>,
    pub volume: Option<f32>,
    pub tint: Option<Tint>,
    pub grayscale: Option<f32>,
    pub mask_address: Option<MediaAddress>,
    pub mask_scale_x: Option<f32>,
    pub mask_scale_y: Option<f32>,
    pub mask_invert: Option<bool>,
    pub mask_opacity: Option<f32>,
    pub speed_multiplier: Option<SpeedMultiplier>,
    pub playback_bpm: Option<Option<u8>>,
    /// Replaces the complete ordered effect chain after the HTTP adapter has applied one
    /// intent-shaped slot edit to the current state.
    pub effects: Option<[EffectSlot; 4]>,
}

/// An intent-shaped edit of the master values at the end of the network personality.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MasterControls {
    pub dimmer: Option<f32>,
    pub volume: Option<f32>,
    pub tint: Option<Tint>,
    pub flip_mirror: Option<FlipMirror>,
    pub mask: Option<MediaAddress>,
}

/// How long external DMX keeps ownership after its last packet.
///
/// Long enough that a momentary gap in a desk's output does not hand control back mid-show,
/// short enough that an operator who unplugs the desk does not have to wait to take over.
pub const DMX_OWNERSHIP_TIMEOUT: Duration = Duration::from_millis(2_500);

/// Where a command came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CommandSource {
    ArtNet,
    Sacn,
    Web,
    /// The application itself — a playback session reporting status, a job completing.
    Internal,
    /// Restoring state after a failure. Never blocked by ownership.
    Recovery,
}

impl CommandSource {
    /// Whether this source is external DMX. Art-Net and sACN translate into identical domain
    /// commands and hold no separate copy of the mapping logic.
    pub const fn is_external_dmx(self) -> bool {
        matches!(self, Self::ArtNet | Self::Sacn)
    }
}

/// A monotonic instant, in milliseconds since the process's own reference point.
///
/// The domain never reads a clock. Adapters stamp commands; the reducer only compares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Timestamp(u64);

impl Timestamp {
    pub const ZERO: Self = Self(0);

    /// Microseconds, because a render clock reasons about frame intervals: at 60 Hz a whole
    /// millisecond is six percent of the budget, and rounding it away would make measured cadence
    /// meaningless.
    pub const fn from_micros(micros: u64) -> Self {
        Self(micros)
    }

    pub const fn from_millis(millis: u64) -> Self {
        Self(millis * 1_000)
    }

    pub const fn as_micros(self) -> u64 {
        self.0
    }

    pub const fn as_millis(self) -> u64 {
        self.0 / 1_000
    }

    /// This instant advanced by a duration. Saturates at the end of the range.
    pub const fn plus(self, duration: Duration) -> Self {
        Self(self.0.saturating_add(duration.as_micros() as u64))
    }

    /// How long after `earlier` this instant is. Saturates rather than wrapping, so a clock that
    /// goes backwards cannot manufacture an enormous age.
    pub const fn since(self, earlier: Self) -> Duration {
        Duration::from_micros(self.0.saturating_sub(earlier.0))
    }
}

/// What a command asks for.
#[derive(Debug, Clone, PartialEq)]
pub enum CommandKind {
    /// A whole DMX frame for one output, already decoded through the canonical personality.
    SetDmxFrame {
        output: OutputId,
        frame: Box<DecodedFrame>,
    },
    /// The web UI selecting media on one layer.
    SelectMedia {
        output: OutputId,
        layer: usize,
        address: MediaAddress,
    },
    /// The web UI setting one layer's dimmer.
    SetLayerDimmer {
        output: OutputId,
        layer: usize,
        dimmer: f32,
    },
    SetLayerControls {
        output: OutputId,
        layer: usize,
        controls: Box<LayerControls>,
    },
    SetMasterControls {
        output: OutputId,
        controls: Box<MasterControls>,
    },
    /// Restart the media on one layer without changing its address.
    ResetLayer { output: OutputId, layer: usize },
    /// Gives or releases the Media Server web operator priority over external DMX.
    TakeOverPlayback { output: OutputId, take_over: bool },
    /// A playback session reporting what happened to a source it was asked to load.
    ReportSourceStatus {
        output: OutputId,
        layer: usize,
        status: SourceStatus,
    },
}

impl CommandKind {
    /// Which output this command addresses.
    pub const fn output(&self) -> OutputId {
        match self {
            Self::SetDmxFrame { output, .. }
            | Self::SelectMedia { output, .. }
            | Self::SetLayerDimmer { output, .. }
            | Self::SetLayerControls { output, .. }
            | Self::SetMasterControls { output, .. }
            | Self::ResetLayer { output, .. }
            | Self::TakeOverPlayback { output, .. }
            | Self::ReportSourceStatus { output, .. } => *output,
        }
    }

    /// Whether this command writes a value external DMX continuously controls.
    ///
    /// Status reports and resets are not continuously controlled values: a source that has just
    /// failed must be able to say so even while a desk is driving the layer.
    pub const fn is_continuously_controlled(&self) -> bool {
        matches!(
            self,
            Self::SetDmxFrame { .. }
                | Self::SelectMedia { .. }
                | Self::SetLayerDimmer { .. }
                | Self::SetLayerControls { .. }
                | Self::SetMasterControls { .. }
        )
    }
}

/// A command with its provenance.
#[derive(Debug, Clone, PartialEq)]
pub struct Command {
    pub kind: CommandKind,
    pub source: CommandSource,
    pub at: Timestamp,
}

impl Command {
    pub const fn new(kind: CommandKind, source: CommandSource, at: Timestamp) -> Self {
        Self { kind, source, at }
    }
}

/// Who currently owns an output's continuously controlled values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlOwnership {
    /// The last external DMX source seen, and when.
    pub dmx: Option<DmxActivity>,
    /// Explicit local priority. While set, incoming network DMX is ignored.
    #[serde(default)]
    pub web_takeover: bool,
}

/// The most recent external DMX activity on an output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DmxActivity {
    pub source: CommandSource,
    pub last_seen: Timestamp,
}

impl ControlOwnership {
    /// Whether external DMX still owns this output at `now`.
    pub fn dmx_is_active(&self, now: Timestamp) -> bool {
        self.dmx
            .is_some_and(|activity| now.since(activity.last_seen) < DMX_OWNERSHIP_TIMEOUT)
    }

    /// Records that external DMX was seen. A later packet always wins, and a packet stamped
    /// earlier than the one already recorded never rewinds ownership.
    pub fn observe_dmx(&mut self, source: CommandSource, at: Timestamp) {
        debug_assert!(source.is_external_dmx());
        match self.dmx {
            Some(activity) if activity.last_seen >= at => {}
            _ => {
                self.dmx = Some(DmxActivity {
                    source,
                    last_seen: at,
                })
            }
        }
    }

    /// Whether a command is allowed to write continuously controlled values right now.
    pub fn accepts(&self, command: &Command) -> bool {
        match command.source {
            // Recovery restores state the process itself lost; ownership never blocks it.
            CommandSource::Recovery => true,
            // External DMX and the application always write.
            CommandSource::ArtNet | CommandSource::Sacn => !self.web_takeover,
            CommandSource::Internal => true,
            // The web UI may always inspect and may always perform administrative actions; it
            // yields only the values a live desk is driving.
            CommandSource::Web => {
                !command.kind.is_continuously_controlled()
                    || self.web_takeover
                    || !self.dmx_is_active(command.at)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn web(kind: CommandKind, millis: u64) -> Command {
        Command::new(kind, CommandSource::Web, Timestamp::from_millis(millis))
    }

    fn output() -> OutputId {
        OutputId::from_uuid(uuid::Uuid::nil())
    }

    fn select(millis: u64) -> Command {
        web(
            CommandKind::SelectMedia {
                output: output(),
                layer: 0,
                address: MediaAddress::new(1, 1),
            },
            millis,
        )
    }

    #[test]
    fn art_net_and_sacn_are_the_external_dmx_sources() {
        assert!(CommandSource::ArtNet.is_external_dmx());
        assert!(CommandSource::Sacn.is_external_dmx());
        for source in [
            CommandSource::Web,
            CommandSource::Internal,
            CommandSource::Recovery,
        ] {
            assert!(!source.is_external_dmx(), "{source:?}");
        }
    }

    #[test]
    fn the_web_ui_owns_the_output_until_a_desk_starts_sending() {
        let mut ownership = ControlOwnership::default();
        assert!(ownership.accepts(&select(0)));

        ownership.observe_dmx(CommandSource::ArtNet, Timestamp::from_millis(1_000));
        assert!(!ownership.accepts(&select(1_000)));
    }

    #[test]
    fn dmx_ownership_expires_after_the_documented_timeout() {
        let mut ownership = ControlOwnership::default();
        ownership.observe_dmx(CommandSource::Sacn, Timestamp::from_millis(1_000));

        let expiry = 1_000 + DMX_OWNERSHIP_TIMEOUT.as_millis() as u64;
        assert!(ownership.dmx_is_active(Timestamp::from_millis(expiry - 1)));
        assert!(!ownership.dmx_is_active(Timestamp::from_millis(expiry)));
        assert!(
            ownership.accepts(&select(expiry)),
            "control returns to the web UI"
        );
    }

    #[test]
    fn a_later_packet_refreshes_ownership_and_an_earlier_one_does_not_rewind_it() {
        let mut ownership = ControlOwnership::default();
        ownership.observe_dmx(CommandSource::ArtNet, Timestamp::from_millis(1_000));
        ownership.observe_dmx(CommandSource::ArtNet, Timestamp::from_millis(2_000));
        assert_eq!(
            ownership.dmx.unwrap().last_seen,
            Timestamp::from_millis(2_000)
        );

        ownership.observe_dmx(CommandSource::ArtNet, Timestamp::from_millis(1_500));
        assert_eq!(
            ownership.dmx.unwrap().last_seen,
            Timestamp::from_millis(2_000)
        );
    }

    #[test]
    fn a_live_desk_never_blocks_status_reports_resets_or_recovery() {
        let mut ownership = ControlOwnership::default();
        ownership.observe_dmx(CommandSource::ArtNet, Timestamp::from_millis(1_000));

        let status = Command::new(
            CommandKind::ReportSourceStatus {
                output: output(),
                layer: 0,
                status: SourceStatus::Ready,
            },
            CommandSource::Internal,
            Timestamp::from_millis(1_000),
        );
        assert!(ownership.accepts(&status));

        let reset = web(
            CommandKind::ResetLayer {
                output: output(),
                layer: 0,
            },
            1_000,
        );
        assert!(
            ownership.accepts(&reset),
            "an administrative action stays available"
        );

        let recovery = Command::new(
            CommandKind::SelectMedia {
                output: output(),
                layer: 0,
                address: MediaAddress::BLANK,
            },
            CommandSource::Recovery,
            Timestamp::from_millis(1_000),
        );
        assert!(ownership.accepts(&recovery));
    }

    #[test]
    fn a_clock_that_goes_backwards_cannot_manufacture_an_age() {
        let earlier = Timestamp::from_millis(5_000);
        assert_eq!(Timestamp::from_millis(1_000).since(earlier), Duration::ZERO);
    }

    #[test]
    fn explicit_web_takeover_reverses_normal_network_priority() {
        let mut ownership = ControlOwnership::default();
        ownership.observe_dmx(CommandSource::ArtNet, Timestamp::from_millis(1_000));
        assert!(!ownership.accepts(&select(1_000)));
        ownership.web_takeover = true;
        assert!(ownership.accepts(&select(1_000)));
        let network = Command::new(
            CommandKind::SelectMedia {
                output: output(),
                layer: 0,
                address: MediaAddress::new(1, 1),
            },
            CommandSource::ArtNet,
            Timestamp::from_millis(1_001),
        );
        assert!(!ownership.accepts(&network));
    }
}
