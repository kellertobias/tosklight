use chrono::{DateTime, Utc};
use light_core::CueListId;
use std::collections::HashSet;
use uuid::Uuid;

use crate::{ActionError, ActionErrorKind};

pub const MAX_PLAYBACK_SNAPSHOT_IDENTITIES: usize = 256;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PlaybackRuntimeIdentity {
    Playback(u16),
    CueList(CueListId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlaybackRuntimeProjection {
    pub scope: PlaybackShowScope,
    /// The exact identity requested by the caller. A Cuelist identity may expand to multiple
    /// assigned Playbacks in a repair snapshot.
    pub requested: PlaybackRuntimeIdentity,
    pub playback_number: Option<u16>,
    pub target: PlaybackTargetProjection,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PlaybackTargetProjection {
    Missing,
    CueList {
        cue_list_id: CueListId,
        runtime: Option<Box<CueListRuntimeProjection>>,
    },
    Group {
        group_id: String,
        master: f32,
        flash_level: f32,
    },
    SpeedGroup {
        group: String,
        runtime: Box<SpeedGroupRuntimeProjection>,
    },
    GrandMaster(GrandMasterRuntimeProjection),
    ProgrammerFade {
        millis: u64,
    },
    CueFade {
        millis: u64,
    },
}

impl PlaybackRuntimeProjection {
    pub const fn cue_list_id(&self) -> Option<CueListId> {
        match self.target {
            PlaybackTargetProjection::CueList { cue_list_id, .. } => Some(cue_list_id),
            _ => None,
        }
    }

    pub fn current_cue(&self) -> Option<&PlaybackCueReference> {
        self.cue_list_runtime()?.current.as_ref()
    }

    pub fn cue_list_runtime(&self) -> Option<&CueListRuntimeProjection> {
        match &self.target {
            PlaybackTargetProjection::CueList {
                runtime: Some(runtime),
                ..
            } => Some(runtime),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlaybackCueReference {
    pub id: Uuid,
    pub number: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CueListRuntimeProjection {
    pub cue_index: usize,
    pub previous_index: Option<usize>,
    pub current: Option<PlaybackCueReference>,
    pub loaded: Option<PlaybackCueReference>,
    pub normal_next: Option<PlaybackCueReference>,
    pub effective_next: Option<PlaybackCueReference>,
    pub effective_next_is_loaded: bool,
    pub paused: bool,
    pub activated_at: DateTime<Utc>,
    pub master: f32,
    pub fader_position: f32,
    pub fader_pickup_required: bool,
    pub flash: bool,
    pub temporary: bool,
    pub temporary_active: bool,
    pub temporary_master: f32,
    pub swap_active: bool,
    pub enabled: bool,
    pub transition_timing_bypassed: bool,
    pub manual_xfade_position: f32,
    pub manual_xfade_direction: ManualXFadeDirection,
    pub manual_xfade_progress: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManualXFadeDirection {
    TowardsHigh,
    TowardsLow,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpeedGroupRuntimeProjection {
    pub manual_bpm: f64,
    pub sound_bpm: Option<f64>,
    pub effective_bpm: f64,
    pub source: SpeedSource,
    pub sound_status: SoundStatus,
    pub paused: bool,
    pub phase_advancing: bool,
    pub speed_master_scale: f64,
    pub sound_multiplier: f64,
    pub source_available: bool,
    pub usable_signal: bool,
    pub input_level: f32,
    pub selected_band_level: f32,
    pub synchronized_with: Option<u8>,
    pub phase_origin_millis: u64,
    pub beat_phase: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpeedSource {
    Manual,
    Sound,
    HeldSound,
    ManualFallback,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SoundStatus {
    Disabled,
    Active {
        detected_bpm: f64,
        confidence: f32,
    },
    Holding {
        reason: SoundLossReason,
        remaining_millis: u64,
    },
    ManualFallback {
        reason: SoundLossReason,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SoundLossReason {
    SourceUnavailable,
    NoUsableSignal,
    LowConfidence,
    TempoOutsideRange,
    WaitingForAnalysis,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GrandMasterRuntimeProjection {
    pub level: f32,
    pub effective_level: f32,
    pub blackout: bool,
    pub flash_active: bool,
    pub dynamics_paused: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlaybackDeskProjection {
    pub scope: PlaybackShowScope,
    pub desk_id: Uuid,
    pub active_page: u8,
    pub selected_playback: Option<u16>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlaybackShowScope {
    pub show_id: Uuid,
    pub show_revision: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlaybackRuntimeSnapshot {
    pub event_sequence: u64,
    pub desk: PlaybackDeskProjection,
    pub projections: Vec<PlaybackRuntimeProjection>,
}

pub(super) fn validate_snapshot_identities(
    identities: &[PlaybackRuntimeIdentity],
) -> Result<(), ActionError> {
    if identities.len() > MAX_PLAYBACK_SNAPSHOT_IDENTITIES {
        return Err(ActionError::new(
            ActionErrorKind::Invalid,
            format!(
                "at most {MAX_PLAYBACK_SNAPSHOT_IDENTITIES} playback runtime identities are allowed"
            ),
        ));
    }
    if identities.iter().copied().collect::<HashSet<_>>().len() != identities.len() {
        return Err(ActionError::new(
            ActionErrorKind::Invalid,
            "playback runtime identities must be unique",
        ));
    }
    Ok(())
}
