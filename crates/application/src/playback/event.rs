use uuid::Uuid;

use super::{PlaybackCueReference, PlaybackRuntimeProjection};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PlaybackTransitionCause {
    Go,
    Back,
    Jump,
    Chaser,
    Follow,
    Wait,
    Timecode,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlaybackCueTransition {
    pub playback_number: Option<u16>,
    pub cue_list_id: Uuid,
    pub previous: Option<PlaybackCueReference>,
    pub current: Option<PlaybackCueReference>,
    pub cause: PlaybackTransitionCause,
    pub advanced_steps: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlaybackRuntimeChange {
    pub projection: PlaybackRuntimeProjection,
    pub transition: Option<PlaybackCueTransition>,
}
