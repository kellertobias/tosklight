use crate::*;

fn default_true() -> bool {
    true
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum PlaybackKey {
    Number(u16),
    CueList(CueListId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum TemporaryPlaybackKind {
    Flash,
    TempButton,
    TempFader,
    Swap,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActivePlayback {
    #[serde(default)]
    pub playback_number: Option<u16>,
    pub cue_list_id: CueListId,
    pub cue_index: usize,
    pub previous_index: Option<usize>,
    pub paused: bool,
    pub activated_at: DateTime<Utc>,
    pub paused_at: Option<DateTime<Utc>>,
    #[serde(default = "default_master")]
    pub master: f32,
    /// Last physical control position. On deliberately does not move this value.
    #[serde(default = "default_master")]
    pub fader_position: f32,
    /// Off at a non-zero physical position latches the fader until it reaches zero.
    #[serde(default)]
    pub fader_pickup_required: bool,
    #[serde(default)]
    pub flash: bool,
    #[serde(default)]
    pub master_transition: Option<PlaybackMasterTransition>,
    #[serde(default)]
    pub temporary: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub flash_restore_off: bool,
    /// Fast navigation bypasses Cue and per-attribute delay/fade for only this transition.
    #[serde(default)]
    pub transition_timing_bypassed: bool,
    /// A one-transition fallback supplied by an atomic Preload GO. Explicit Cue and
    /// per-attribute timings remain authoritative; this replaces only the Cue Fade master.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition_fade_fallback_millis: Option<u64>,
    #[serde(default)]
    pub manual_xfade_position: f32,
    #[serde(default)]
    pub manual_xfade_direction: ManualXFadeDirection,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual_xfade_from_index: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manual_xfade_to_index: Option<usize>,
    #[serde(default)]
    pub manual_xfade_progress: f32,
    /// While set, forward navigation has wrapped in Tracking mode and the final
    /// tracked state remains the base until a Cue explicitly changes it.
    #[serde(default)]
    pub tracking_wrap: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_cue_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_cue_number: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted_cue_hold: Option<DeletedCueHold>,
    /// When navigation resolves a deleted-active Cue hold, this preserves the rendered held
    /// contribution as the source of the destination Cue's normal fade. It is cleared by the
    /// next navigation or activation operation and is never written into Cue data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted_cue_transition_source: Option<Vec<TimedValue>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loaded_cue_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loaded_cue_number: Option<f64>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManualXFadeDirection {
    #[default]
    TowardsHigh,
    TowardsLow,
}

#[derive(Clone, Debug, Serialize)]
pub struct PlaybackRuntimeStatus {
    #[serde(flatten)]
    pub playback: ActivePlayback,
    pub normal_next_cue_id: Option<Uuid>,
    pub normal_next_cue_number: Option<f64>,
    pub effective_next_cue_id: Option<Uuid>,
    pub effective_next_cue_number: Option<f64>,
    pub effective_next_is_loaded: bool,
    pub temporary_active: bool,
    pub temporary_master: f32,
    pub swap_active: bool,
}

/// A Position-family value which an active Cuelist can safely preposition while its fixture is
/// dark. The engine owns the resolved-dark clock and turns these look-ahead records into runtime
/// contributions; Cue data is never modified.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MoveInBlackTargetValue {
    pub attribute: AttributeKey,
    pub current: AttributeValue,
    pub target: AttributeValue,
    pub fade_millis: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MoveInBlackCandidate {
    pub playback_number: Option<u16>,
    pub cue_list_id: CueListId,
    pub current_cue_id: Uuid,
    pub current_cue_number: f64,
    pub target_cue_id: Uuid,
    pub target_cue_number: f64,
    pub fixture_id: FixtureId,
    pub priority: i16,
    pub values: Vec<MoveInBlackTargetValue>,
}

/// Stable identity of the playback whose sequence master applies to a contribution. Keeping this
/// separate from `TimedValue` lets the engine retain source-specific master semantics without
/// leaking playback concerns into programmer and show data.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SequenceMasterSource {
    pub playback_number: Option<u16>,
    pub cue_list_id: CueListId,
    pub temporary: bool,
}

#[derive(Clone, Debug)]
pub struct PlaybackContribution {
    pub value: TimedValue,
    pub sequence_master: f32,
    pub source: SequenceMasterSource,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeletedCueHold {
    pub deleted_number: f64,
    pub previous_number: Option<f64>,
    pub next_number: Option<f64>,
    pub contributions: Vec<TimedValue>,
}

pub(crate) fn advance_chaser_steps(
    playback: &mut ActivePlayback,
    cue_list: &CueList,
    steps: u64,
) -> u64 {
    if steps == 0 {
        return 0;
    }
    playback.deleted_cue_transition_source = None;
    let start = playback.cue_index as u128;
    let total = start + u128::from(steps);
    let last = cue_list.cues.len() - 1;
    if cue_list.effective_wrap_mode() == WrapMode::Off {
        playback.cue_index =
            usize::try_from(total.min(last as u128)).expect("clamped Cue index fits usize");
        playback.previous_index = Some(if total > last as u128 {
            last
        } else {
            playback.cue_index.saturating_sub(1)
        });
    } else {
        let cue_count = cue_list.cues.len() as u128;
        playback.cue_index =
            usize::try_from(total % cue_count).expect("modulo Cue index fits usize");
        playback.previous_index =
            Some(usize::try_from((total - 1) % cue_count).expect("modulo Cue index fits usize"));
        if cue_list.effective_wrap_mode() == WrapMode::Tracking && total >= cue_count {
            playback.tracking_wrap = true;
        } else if cue_list.effective_wrap_mode() == WrapMode::Reset {
            playback.tracking_wrap = false;
        }
    }
    playback.current_cue_number = Some(cue_list.cues[playback.cue_index].number);
    playback.current_cue_id = Some(cue_list.cues[playback.cue_index].id);
    if cue_list.effective_wrap_mode() == WrapMode::Off {
        steps.min(last.saturating_sub(start as usize) as u64)
    } else {
        steps
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlaybackMasterTransition {
    pub from: f32,
    pub to: f32,
    pub started_at: DateTime<Utc>,
    pub duration_millis: u64,
    pub release_after: bool,
}

fn default_master() -> f32 {
    1.0
}

pub(crate) fn reset_manual_transition(playback: &mut ActivePlayback) {
    playback.transition_timing_bypassed = false;
    playback.transition_fade_fallback_millis = None;
    playback.manual_xfade_from_index = None;
    playback.manual_xfade_to_index = None;
    playback.manual_xfade_progress = 0.0;
}

pub(crate) fn new_active_playback(
    playback_number: Option<u16>,
    cue_list: &CueList,
    now: DateTime<Utc>,
    master: f32,
    enabled: bool,
) -> ActivePlayback {
    ActivePlayback {
        playback_number,
        cue_list_id: cue_list.id,
        cue_index: 0,
        previous_index: None,
        paused: false,
        activated_at: now,
        paused_at: None,
        master,
        fader_position: master,
        fader_pickup_required: false,
        flash: false,
        master_transition: None,
        temporary: false,
        enabled,
        flash_restore_off: false,
        transition_timing_bypassed: false,
        transition_fade_fallback_millis: None,
        manual_xfade_position: 0.0,
        manual_xfade_direction: ManualXFadeDirection::TowardsHigh,
        manual_xfade_from_index: None,
        manual_xfade_to_index: None,
        manual_xfade_progress: 0.0,
        tracking_wrap: false,
        current_cue_id: cue_list.cues.first().map(|cue| cue.id),
        current_cue_number: cue_list.cues.first().map(|cue| cue.number),
        deleted_cue_hold: None,
        deleted_cue_transition_source: None,
        loaded_cue_id: None,
        loaded_cue_number: None,
    }
}
