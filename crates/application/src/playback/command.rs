use super::{PlaybackDeskProjection, PlaybackRuntimeProjection};
use crate::{ActionContext, ApplicationCommand, CommandFamily};
use light_core::CueListId;
use light_playback::ActivePlayback;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PlaybackAddress {
    CueList(CueListId),
    Pool(u16),
    CurrentPage { slot: u8 },
    ExplicitPage { page: u8, slot: u8 },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResolvedPlaybackAddress {
    CueList(CueListId),
    Pool {
        number: u16,
        page: Option<u8>,
        slot: Option<u8>,
    },
}

impl ResolvedPlaybackAddress {
    pub const fn playback_number(self) -> Option<u16> {
        match self {
            Self::CueList(_) => None,
            Self::Pool { number, .. } => Some(number),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PlaybackLevel(u32);

impl PlaybackLevel {
    pub fn new(value: f32) -> Self {
        Self(value.to_bits())
    }

    pub fn value(self) -> f32 {
        f32::from_bits(self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CueNumber(u64);

impl CueNumber {
    pub fn new(value: f64) -> Self {
        Self(value.to_bits())
    }

    pub fn value(self) -> f64 {
        f64::from_bits(self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PlaybackAction {
    Go { pressed: bool },
    Back { pressed: bool },
    Pause { pressed: bool },
    Release,
    On { pressed: bool },
    Off { pressed: bool },
    Toggle { pressed: bool },
    FastForward { pressed: bool },
    FastRewind { pressed: bool },
    Flash { pressed: bool },
    Temp { pressed: bool },
    Swap { pressed: bool },
    Select { pressed: bool },
    SelectContents { pressed: bool },
    SelectDereferenced { pressed: bool },
    Learn { pressed: bool },
    Double { pressed: bool },
    Half { pressed: bool },
    Blackout { pressed: bool },
    PauseDynamics { pressed: bool },
    None { pressed: bool },
    Master(PlaybackLevel),
    GoTo(CueNumber),
    Load(CueNumber),
    Crossfade { enabled: bool },
    Temporary { enabled: bool, pressed: bool },
    ConfiguredButton { number: u8, pressed: bool },
}

impl PlaybackAction {
    pub const fn pressed(self) -> Option<bool> {
        match self {
            Self::Release | Self::Master(_) | Self::GoTo(_) | Self::Load(_) => None,
            Self::Crossfade { .. } => None,
            Self::Temporary { pressed, .. }
            | Self::ConfiguredButton { pressed, .. }
            | Self::Go { pressed }
            | Self::Back { pressed }
            | Self::Pause { pressed }
            | Self::On { pressed }
            | Self::Off { pressed }
            | Self::Toggle { pressed }
            | Self::FastForward { pressed }
            | Self::FastRewind { pressed }
            | Self::Flash { pressed }
            | Self::Temp { pressed }
            | Self::Swap { pressed }
            | Self::Select { pressed }
            | Self::SelectContents { pressed }
            | Self::SelectDereferenced { pressed }
            | Self::Learn { pressed }
            | Self::Double { pressed }
            | Self::Half { pressed }
            | Self::Blackout { pressed }
            | Self::PauseDynamics { pressed }
            | Self::None { pressed } => Some(pressed),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PlaybackSurface {
    Physical,
    Virtual,
    Osc,
    Matter,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PlaybackCommand {
    pub address: PlaybackAddress,
    pub action: PlaybackAction,
    pub surface: PlaybackSurface,
}

impl ApplicationCommand for PlaybackCommand {
    type Value = PlaybackResult;

    const FAMILY: CommandFamily = CommandFamily::Playback;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PendingPlaybackAction {
    Toggle,
    Go,
    Back,
    Off,
    On,
    TemporaryOn,
    TemporaryOff,
}

#[derive(Clone, Debug)]
pub enum PlaybackExecution {
    Active(Box<ActivePlayback>),
    ActiveList {
        active: Vec<ActivePlayback>,
        changed: bool,
    },
    Released(bool),
    Pool {
        changed: bool,
        pending: Option<PendingPlaybackAction>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaybackOutcome {
    Applied,
    NoChange,
    Captured(PendingPlaybackAction),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaybackDurability {
    Durable,
    PersistencePending,
}

#[derive(Clone, Debug)]
pub struct PlaybackRelatedResult {
    pub projection: PlaybackRuntimeProjection,
    pub event_sequence: u64,
}

#[derive(Clone, Debug)]
pub struct PlaybackResult {
    pub context: ActionContext,
    pub requested: PlaybackAddress,
    pub resolved: ResolvedPlaybackAddress,
    pub outcome: PlaybackOutcome,
    pub durability: PlaybackDurability,
    pub execution: PlaybackExecution,
    pub projection: PlaybackRuntimeProjection,
    /// Additional runtime identities changed atomically by the same application action.
    pub related: Vec<PlaybackRelatedResult>,
    pub desk: Option<PlaybackDeskProjection>,
    pub event_sequence: Option<u64>,
    pub desk_event_sequence: Option<u64>,
    pub replayed: bool,
}
