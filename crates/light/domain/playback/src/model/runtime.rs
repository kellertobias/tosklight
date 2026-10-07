use crate::*;

fn default_true() -> bool {
    true
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum PlaybackKey {
    CueList(CueListId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TemporaryPlaybackKind {
    Flash,
    TempButton,
    TempFader,
    Swap,
}

/// Session-local state for one absolute physical control of a shared playback target.
///
/// This is deliberately separate from `ActivePlayback`: multiple assignments may expose one
/// target runtime while each non-motorized fader retains its own sensed position and pickup latch.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlaybackControlState {
    pub fader_position: f32,
    pub fader_pickup_required: bool,
    pub fader_pickup_target: Option<f32>,
    pub(crate) observed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackActivationSurface {
    Physical,
    Virtual,
    Osc,
    Matter,
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackExclusionScope {
    Show,
    OriginatingDesk,
    LegacyAllDesks,
    #[serde(other)]
    None,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlaybackActivationOrigin {
    pub at: DateTime<Utc>,
    pub desk_id: Option<Uuid>,
    pub surface: PlaybackActivationSurface,
    pub exclusion_scope: PlaybackExclusionScope,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PlaybackActivationProvenance {
    pub ordinal: u64,
    pub at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desk_id: Option<Uuid>,
    pub surface: PlaybackActivationSurface,
    pub exclusion_scope: PlaybackExclusionScope,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlaybackPreloadTimingState {
    pub enabled: bool,
    pub master: f32,
    pub activation: Option<PlaybackActivationProvenance>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActivePlayback {
    #[serde(default)]
    pub playback_number: Option<u16>,
    /// Stable address authority for runtimes which cannot be identified by the legacy scalar
    /// Playback number alone. Older persisted physical and direct-Cuelist rows omit this field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playback_identity: Option<PlaybackIdentity>,
    /// Internal restart authority. Public runtime payloads deliberately omit this field.
    #[serde(default, skip_serializing)]
    pub activation: Option<PlaybackActivationProvenance>,
    /// Stable engine-owned order of the last transition which changed this Playback's LTP
    /// contribution. This is persisted by the runtime adapter, but omitted from public status.
    #[serde(default, skip_serializing)]
    pub transition_ordinal: u64,
    pub cue_list_id: CueListId,
    pub cue_index: usize,
    pub previous_index: Option<usize>,
    pub paused: bool,
    pub activated_at: DateTime<Utc>,
    pub paused_at: Option<DateTime<Utc>>,
    /// Trigger-time row whose automatic interval most recently completed. It remains visible as
    /// complete until another Cue transition starts, including after a REST repair snapshot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_trigger_cue_id: Option<Uuid>,
    #[serde(default = "default_master")]
    pub master: f32,
    /// Last physical control position. On deliberately does not move this value.
    #[serde(default = "default_master")]
    pub fader_position: f32,
    /// Off at a non-zero physical position latches the fader until it reaches zero.
    #[serde(default)]
    pub fader_pickup_required: bool,
    /// Exact physical position that releases the non-motorized fader latch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fader_pickup_target: Option<f32>,
    #[serde(default)]
    pub flash: bool,
    #[serde(default)]
    pub master_transition: Option<PlaybackMasterTransition>,
    #[serde(default)]
    pub temporary: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// A later non-zero Master may resume this exact current Cue only when its own accepted
    /// zero-Master gesture caused the Off state. Every other Off path clears this marker.
    #[serde(default, skip_serializing)]
    pub fader_zero_auto_off_armed: bool,
    #[serde(default)]
    pub flash_restore_off: bool,
    /// Fast navigation bypasses Cue and per-attribute delay/fade for only this transition.
    #[serde(default)]
    pub transition_timing_bypassed: bool,
    /// Suppresses discrete Cue actions for state reconstruction without bypassing fade timing.
    #[serde(default)]
    pub discrete_cue_actions_suppressed: bool,
    /// A one-transition fallback supplied by an atomic Preload GO. Explicit Cue and
    /// per-attribute timings remain authoritative; this replaces only the Cue Fade master.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition_fade_fallback_millis: Option<u64>,
    /// Runtime-only completion contributed by actions started on the current Cue.
    #[serde(default, skip_serializing)]
    pub external_completion_millis: u64,
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
    pub current_cue_number: Option<CueNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted_cue_hold: Option<DeletedCueHold>,
    /// Resolved pre-master values captured at a navigation boundary. They keep an interrupted
    /// GO/GOTO/BACK or deleted-Cue recovery continuous instead of reconstructing from a stored Cue
    /// endpoint. This runtime snapshot is never written into Cue data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted_cue_transition_source: Option<Vec<PlaybackRetainedValue>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_history: Option<PlaybackSourceHistory>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loaded_cue_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loaded_cue_number: Option<CueNumber>,
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
    pub normal_next_cue_number: Option<CueNumber>,
    pub effective_next_cue_id: Option<Uuid>,
    pub effective_next_cue_number: Option<CueNumber>,
    pub effective_next_is_loaded: bool,
    pub temporary_active: bool,
    pub temporary_master: f32,
    pub swap_active: bool,
    pub cue_timing: Option<CueTimingRuntimeStatus>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CueTriggerTimingKind {
    Follow,
    Wait,
    Link,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CueTriggerTimingStatus {
    pub cue_id: Uuid,
    pub cue_number: CueNumber,
    pub kind: CueTriggerTimingKind,
    pub started_at: DateTime<Utc>,
    pub duration_millis: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CueTimingRuntimeStatus {
    pub cue_id: Uuid,
    pub in_delay_millis: u64,
    pub in_fade_millis: u64,
    pub out_delay_millis: u64,
    pub out_fade_millis: u64,
    pub completion_millis: u64,
    pub active_trigger: Option<CueTriggerTimingStatus>,
    pub completed_trigger_cue_id: Option<Uuid>,
}

/// Tracked Dynamic-layer value for one active Cuelist source.
///
/// The projection remains address-local. `instance_link` coordinates the lanes which
/// share one runtime clock, while FAT/static values continue to arbitrate independently.
#[derive(Clone, Debug, PartialEq)]
pub struct ActiveCueDynamicValue {
    /// The actual iterated normal or temporary Playback, including page-qualified virtual
    /// identity. A standalone Dynamic Playback is a different source and never uses this row.
    pub source: SequenceMasterSource,
    /// Stable controller scope; concurrent temporary kinds on one assignment have independent
    /// clocks even when their master ownership and exact activation timestamps are identical.
    pub source_key: CueDynamicSourceKey,
    /// Suppressed sources still reconcile and advance their controller clocks. Only emitted
    /// contributions are filtered, so releasing Swap can reveal the continuing Dynamic.
    pub output_enabled: bool,
    pub sequence_master: f32,
    pub snap_sequence_master: f32,
    pub playback_number: Option<u16>,
    pub cue_list_id: CueListId,
    /// Cue containing the surviving stored Dynamic row. Tracking through later sparse Cues
    /// preserves this identity; a replacement or generated restoration belongs to its own Cue.
    pub authored_cue_id: Uuid,
    /// Current activation context, independent from the earlier Cue supplying a tracked row.
    pub current_cue_id: Uuid,
    pub priority: i16,
    /// Preserve the full source instant and monotonic order; milliseconds alone cannot
    /// distinguish simultaneous actions or the temporary source's microsecond separation.
    /// Navigation intentionally updates this activation context even for a tracked row whose
    /// `authored_cue_id` has not changed.
    pub changed_at: DateTime<Utc>,
    pub transition_ordinal: u64,
    pub changed_at_millis: u64,
    pub fixture_id: FixtureId,
    pub attribute: AttributeKey,
    pub value: light_dynamics::DynamicSemanticValue,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActiveDynamicPlayback {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic_id: Option<Uuid>,
    pub playback_number: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playback_identity: Option<PlaybackIdentity>,
    pub enabled: bool,
    pub paused: bool,
    #[serde(default)]
    pub flash: bool,
    #[serde(default)]
    pub flash_restore_off: bool,
    pub activated_at: DateTime<Utc>,
    #[serde(default = "default_master")]
    pub fader_value: f32,
    #[serde(skip)]
    pub fader_pickup_required: bool,
    #[serde(skip)]
    pub fader_pickup_target: Option<f32>,
    #[serde(default = "default_master")]
    pub size: f32,
    #[serde(default = "default_master")]
    pub master: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub master_transition: Option<PlaybackMasterTransition>,
    #[serde(default = "default_dynamic_speed_multiplier")]
    pub local_speed_multiplier: light_dynamics::Rational,
    #[serde(default)]
    pub learned_duration_millis: Option<u64>,
    #[serde(default)]
    pub last_learn_tap_millis: Option<u64>,
    #[serde(default)]
    pub learn_intervals_millis: Vec<u64>,
}

const fn default_dynamic_speed_multiplier() -> light_dynamics::Rational {
    light_dynamics::Rational::ONE
}

/// A Position-family value which an active Cuelist can safely preposition while its fixture is
/// dark. The engine owns the resolved-dark clock and turns these look-ahead records into runtime
/// contributions; Cue data is never modified.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MoveInBlackTargetValue {
    pub attribute: AttributeKey,
    /// Missing semantic ownership must be filled from the engine underlay, never Normalized(0).
    pub current: Option<AttributeValue>,
    pub target: AttributeValue,
    pub fade_millis: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MoveInBlackCandidate {
    pub playback_number: Option<u16>,
    pub cue_list_id: CueListId,
    pub current_cue_id: Uuid,
    pub current_cue_number: CueNumber,
    pub target_cue_id: Uuid,
    pub target_cue_number: CueNumber,
    pub fixture_id: FixtureId,
    pub priority: i16,
    pub transition_ordinal: u64,
    pub values: Vec<MoveInBlackTargetValue>,
}

/// Stable identity of the playback whose sequence master applies to a contribution. Keeping this
/// separate from `TimedValue` lets the engine retain source-specific master semantics without
/// leaking playback concerns into programmer and show data.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SequenceMasterSource {
    pub playback_number: Option<u16>,
    pub playback_identity: Option<PlaybackIdentity>,
    pub cue_list_id: CueListId,
    pub temporary: bool,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CueDynamicSourceKey {
    Normal {
        source: SequenceMasterSource,
    },
    Temporary {
        source: SequenceMasterSource,
        kind: TemporaryPlaybackKind,
    },
}

impl CueDynamicSourceKey {
    pub fn source(self) -> SequenceMasterSource {
        match self {
            Self::Normal { source } | Self::Temporary { source, .. } => source,
        }
    }

    /// Stable source/instance identity, independent of Cue navigation and activation stamps.
    /// Fixed byte tags avoid Debug/serde formatting or randomized Hash implementations.
    pub fn controller_id(self, instance_link: Uuid) -> Uuid {
        const NAMESPACE: Uuid = Uuid::from_u128(0x4355455f_44594e41_4d494353_5f563100);
        let source = self.source();
        let mut key = [0_u8; 37];
        key[..16].copy_from_slice(source.cue_list_id.0.as_bytes());
        key[16..32].copy_from_slice(instance_link.as_bytes());
        let identity = source.playback_identity.or_else(|| {
            source
                .playback_number
                .and_then(|number| PlaybackIdentity::physical(number).ok())
        });
        match identity {
            Some(PlaybackIdentity::Physical(number)) => {
                key[32] = 1;
                key[34..36].copy_from_slice(&number.get().to_be_bytes());
            }
            Some(PlaybackIdentity::Virtual(address)) => {
                key[32] = 2;
                key[33] = address.page();
                key[34..36].copy_from_slice(&address.number().get().to_be_bytes());
            }
            None => {
                if let Some(number) = source.playback_number {
                    // An unqualified nonphysical legacy number is not a direct Cuelist source.
                    // Preserve its namespace without inventing a physical/virtual assignment.
                    key[32] = 3;
                    key[34..36].copy_from_slice(&number.to_be_bytes());
                }
            }
        }
        key[36] = match self {
            Self::Normal { .. } => 0,
            Self::Temporary { kind, .. } => match kind {
                TemporaryPlaybackKind::Flash => 1,
                TemporaryPlaybackKind::TempButton => 2,
                TemporaryPlaybackKind::TempFader => 3,
                TemporaryPlaybackKind::Swap => 4,
            },
        };
        Uuid::new_v5(&NAMESPACE, &key)
    }
}

impl ActivePlayback {
    pub(crate) fn begin_source_history(
        &mut self,
        at: DateTime<Utc>,
        ordinal: Option<u64>,
        compiled: &Arc<CompiledCueList>,
    ) {
        self.begin_source_history_from(at, ordinal, compiled, self.sequence_master_source());
    }
    pub(crate) fn begin_source_history_from(
        &mut self,
        at: DateTime<Utc>,
        ordinal: Option<u64>,
        compiled: &Arc<CompiledCueList>,
        source: SequenceMasterSource,
    ) {
        self.source_history = ordinal.map(|ordinal| {
            PlaybackSourceHistory::next(
                self.source_history.as_ref(),
                at,
                ordinal,
                compiled,
                self.cue_index,
                self.tracking_wrap,
                source,
                self.deleted_cue_transition_source.is_some(),
            )
        });
    }
    pub(crate) fn sequence_master_source(&self) -> SequenceMasterSource {
        SequenceMasterSource {
            playback_number: self.playback_number,
            playback_identity: self.playback_identity,
            cue_list_id: self.cue_list_id,
            temporary: self.temporary,
        }
    }

    pub(crate) fn sequence_masters(&self) -> (f32, f32) {
        if self.flash {
            return (1.0, 1.0);
        }
        let current = self.master.clamp(0.0, 1.0);
        let snapped = self
            .master_transition
            .as_ref()
            .map(|transition| transition.to)
            .unwrap_or(self.master)
            .clamp(0.0, 1.0);
        (current, snapped)
    }
}

#[derive(Clone, Debug)]
pub struct PlaybackContribution {
    pub value: TimedValue,
    pub family_evidence: Option<Arc<PlaybackFamilyEvidence>>,
    /// Output-only proof that this value is the complete authored target endpoint. Intermediate
    /// blends and retained/deleted Cue holds lack exact historical dependency evidence.
    pub authored_target: bool,
    pub transition_ordinal: u64,
    pub sequence_master: f32,
    pub source: SequenceMasterSource,
    /// Where the engine's frame keeps this pair, when the compiled cue list was told.
    pub address: Option<light_core::FrameAddress>,
    /// Runtime-only live Position crossing behind a held `value` (TL-544 G1). The physical
    /// Position adapter evaluates it per destination; it is never recorded or persisted.
    pub pending_transition: Option<Arc<light_core::programming::PendingFamilyTransition>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DeletedCueHold {
    pub deleted_number: CueNumber,
    pub previous_number: Option<CueNumber>,
    pub next_number: Option<CueNumber>,
    pub contributions: Vec<PlaybackRetainedValue>,
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
    playback.current_cue_number = Some(cue_list.cues[playback.cue_index].number.clone());
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
    playback.source_history = playback
        .source_history
        .take()
        .and_then(PlaybackSourceHistory::cancel_manual);
    playback.external_completion_millis = 0;
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
        playback_identity: None,
        activation: None,
        transition_ordinal: 0,
        cue_list_id: cue_list.id,
        cue_index: 0,
        previous_index: None,
        paused: false,
        activated_at: now,
        paused_at: None,
        completed_trigger_cue_id: None,
        master,
        fader_position: master,
        fader_pickup_required: false,
        fader_pickup_target: None,
        flash: false,
        master_transition: None,
        temporary: false,
        enabled,
        fader_zero_auto_off_armed: false,
        flash_restore_off: false,
        transition_timing_bypassed: false,
        discrete_cue_actions_suppressed: false,
        transition_fade_fallback_millis: None,
        external_completion_millis: 0,
        manual_xfade_position: 0.0,
        manual_xfade_direction: ManualXFadeDirection::TowardsHigh,
        manual_xfade_from_index: None,
        manual_xfade_to_index: None,
        manual_xfade_progress: 0.0,
        tracking_wrap: false,
        current_cue_id: cue_list.cues.first().map(|cue| cue.id),
        current_cue_number: cue_list.cues.first().map(|cue| cue.number.clone()),
        deleted_cue_hold: None,
        deleted_cue_transition_source: None,
        source_history: None,
        loaded_cue_id: None,
        loaded_cue_number: None,
    }
}
