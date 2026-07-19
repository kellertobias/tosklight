use light_application::{
    ActionError, CueListRuntimeProjection, GrandMasterRuntimeProjection, ManualXFadeDirection,
    PlaybackCueReference, PlaybackRuntimeIdentity, PlaybackRuntimeProjection, PlaybackShowScope,
    PlaybackTargetProjection, SoundLossReason, SoundStatus, SpeedGroupRuntimeProjection,
    SpeedSource,
};
use light_control::speed::{
    EffectiveSpeedSource, SoundLossReason as ControlSoundLossReason,
    SoundStatus as ControlSoundStatus, SpeedSnapshot,
};
use light_core::CueListId;
use light_engine::EngineSnapshot;
use light_playback::PlaybackRuntimeStatus;

use super::super::super::{application_millis, speed_group_index};
use super::super::{ServerPlaybackPorts, invalid};

pub(super) fn cue_list_projection(
    scope: PlaybackShowScope,
    requested: PlaybackRuntimeIdentity,
    playback_number: Option<u16>,
    cue_list_id: CueListId,
    status: Option<&PlaybackRuntimeStatus>,
) -> PlaybackRuntimeProjection {
    PlaybackRuntimeProjection {
        scope,
        requested,
        playback_number,
        target: PlaybackTargetProjection::CueList {
            cue_list_id,
            runtime: status.map(runtime_projection).map(Box::new),
        },
    }
}

fn runtime_projection(status: &PlaybackRuntimeStatus) -> CueListRuntimeProjection {
    let playback = &status.playback;
    CueListRuntimeProjection {
        cue_index: playback.cue_index,
        previous_index: playback.previous_index,
        current: cue(playback.current_cue_id, playback.current_cue_number),
        loaded: cue(playback.loaded_cue_id, playback.loaded_cue_number),
        normal_next: cue(status.normal_next_cue_id, status.normal_next_cue_number),
        effective_next: cue(
            status.effective_next_cue_id,
            status.effective_next_cue_number,
        ),
        effective_next_is_loaded: status.effective_next_is_loaded,
        paused: playback.paused,
        activated_at: playback.activated_at,
        master: playback.master,
        fader_position: playback.fader_position,
        fader_pickup_required: playback.fader_pickup_required,
        flash: playback.flash,
        temporary: playback.temporary,
        temporary_active: status.temporary_active,
        temporary_master: status.temporary_master,
        swap_active: status.swap_active,
        enabled: playback.enabled,
        transition_timing_bypassed: playback.transition_timing_bypassed,
        manual_xfade_position: playback.manual_xfade_position,
        manual_xfade_direction: match playback.manual_xfade_direction {
            light_playback::ManualXFadeDirection::TowardsHigh => ManualXFadeDirection::TowardsHigh,
            light_playback::ManualXFadeDirection::TowardsLow => ManualXFadeDirection::TowardsLow,
        },
        manual_xfade_progress: playback.manual_xfade_progress,
    }
}

pub(super) fn group_projection(
    ports: &ServerPlaybackPorts<'_>,
    snapshot: &EngineSnapshot,
    group_id: &str,
) -> Result<PlaybackTargetProjection, ActionError> {
    let group = snapshot
        .groups
        .iter()
        .find(|group| group.id == group_id)
        .ok_or_else(|| invalid("group does not exist"))?;
    Ok(PlaybackTargetProjection::Group {
        group_id: group_id.to_owned(),
        master: group.master,
        flash_level: ports.state.engine.group_master_flash(group_id),
    })
}

pub(super) fn speed_projection(
    ports: &ServerPlaybackPorts<'_>,
    group: &str,
) -> Result<PlaybackTargetProjection, ActionError> {
    let index = speed_group_index(group).map_err(|error| invalid(error.message))?;
    let snapshot = ports.state.speed_groups.lock()[index].snapshot(application_millis(ports.state));
    Ok(PlaybackTargetProjection::SpeedGroup {
        group: group.to_owned(),
        runtime: Box::new(speed_runtime(snapshot)),
    })
}

fn speed_runtime(snapshot: SpeedSnapshot) -> SpeedGroupRuntimeProjection {
    SpeedGroupRuntimeProjection {
        manual_bpm: snapshot.manual_bpm,
        sound_bpm: snapshot.sound_bpm,
        effective_bpm: snapshot.effective_bpm,
        source: match snapshot.source {
            EffectiveSpeedSource::Manual => SpeedSource::Manual,
            EffectiveSpeedSource::Sound => SpeedSource::Sound,
            EffectiveSpeedSource::HeldSound => SpeedSource::HeldSound,
            EffectiveSpeedSource::ManualFallback => SpeedSource::ManualFallback,
        },
        sound_status: sound_status(snapshot.sound_status),
        paused: snapshot.paused,
        phase_advancing: snapshot.phase_advancing,
        speed_master_scale: snapshot.speed_master_scale,
        sound_multiplier: snapshot.sound_multiplier,
        source_available: snapshot.source_available,
        usable_signal: snapshot.usable_signal,
        input_level: snapshot.input_level,
        selected_band_level: snapshot.selected_band_level,
        synchronized_with: snapshot.synchronized_with,
        phase_origin_millis: snapshot.phase_origin_millis,
        beat_phase: snapshot.beat_phase,
    }
}

fn sound_status(status: ControlSoundStatus) -> SoundStatus {
    match status {
        ControlSoundStatus::Disabled => SoundStatus::Disabled,
        ControlSoundStatus::Active {
            detected_bpm,
            confidence,
        } => SoundStatus::Active {
            detected_bpm,
            confidence,
        },
        ControlSoundStatus::Holding {
            reason,
            remaining_millis,
        } => SoundStatus::Holding {
            reason: sound_loss_reason(reason),
            remaining_millis,
        },
        ControlSoundStatus::ManualFallback { reason } => SoundStatus::ManualFallback {
            reason: sound_loss_reason(reason),
        },
    }
}

fn sound_loss_reason(reason: ControlSoundLossReason) -> SoundLossReason {
    match reason {
        ControlSoundLossReason::SourceUnavailable => SoundLossReason::SourceUnavailable,
        ControlSoundLossReason::NoUsableSignal => SoundLossReason::NoUsableSignal,
        ControlSoundLossReason::LowConfidence => SoundLossReason::LowConfidence,
        ControlSoundLossReason::TempoOutsideRange => SoundLossReason::TempoOutsideRange,
        ControlSoundLossReason::WaitingForAnalysis => SoundLossReason::WaitingForAnalysis,
    }
}

pub(super) fn grand_master_projection(ports: &ServerPlaybackPorts<'_>) -> PlaybackTargetProjection {
    let control = ports.state.output_control.lock();
    let level = control.options.grand_master;
    let flash_active = control.grand_master_flash;
    PlaybackTargetProjection::GrandMaster(GrandMasterRuntimeProjection {
        level,
        effective_level: if flash_active { 1.0 } else { level },
        blackout: control.options.blackout,
        flash_active,
        dynamics_paused: ports.state.engine.playback().read().dynamics_paused(),
    })
}

fn cue(id: Option<uuid::Uuid>, number: Option<f64>) -> Option<PlaybackCueReference> {
    id.zip(number)
        .map(|(id, number)| PlaybackCueReference { id, number })
}
