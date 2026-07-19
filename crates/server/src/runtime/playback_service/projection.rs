use std::sync::Arc;

use light_application::{
    ActionContext, ActionError, AutomaticPlaybackProjection, CueListRuntimeProjection,
    GrandMasterRuntimeProjection, ManualXFadeDirection, PlaybackCueReference,
    PlaybackDeskProjection, PlaybackRuntimeIdentity, PlaybackRuntimeProjection, PlaybackShowScope,
    PlaybackTargetProjection, SoundLossReason, SoundStatus, SpeedGroupRuntimeProjection,
    SpeedSource,
};
use light_control::speed::{
    EffectiveSpeedSource, SoundLossReason as ControlSoundLossReason,
    SoundStatus as ControlSoundStatus, SpeedSnapshot,
};
use light_core::CueListId;
use light_engine::EngineSnapshot;
use light_playback::{PlaybackRuntimeStatus, PlaybackTarget};

use super::super::{application_millis, speed_group_index};
use super::{ServerPlaybackPorts, invalid};

pub(in crate::runtime) fn automatic_changes(
    engine: &light_engine::Engine,
    scope: PlaybackShowScope,
    transitions: Vec<light_playback::AutomaticPlaybackTransition>,
) -> Vec<AutomaticPlaybackProjection> {
    if transitions.is_empty() {
        return Vec::new();
    }
    let runtime = engine.playback().read().runtime_status();
    transitions
        .into_iter()
        .map(|transition| {
            let requested = transition.playback_number.map_or(
                PlaybackRuntimeIdentity::CueList(transition.cue_list_id),
                PlaybackRuntimeIdentity::Playback,
            );
            let status = runtime.iter().find(|status| {
                transition.playback_number.map_or_else(
                    || {
                        status.playback.playback_number.is_none()
                            && status.playback.cue_list_id == transition.cue_list_id
                    },
                    |number| status.playback.playback_number == Some(number),
                )
            });
            AutomaticPlaybackProjection {
                projection: cue_list_projection(
                    scope,
                    requested,
                    transition.playback_number,
                    transition.cue_list_id,
                    status,
                ),
                transition,
            }
        })
        .collect()
}

pub(super) fn projection(
    ports: &ServerPlaybackPorts<'_>,
    _context: &ActionContext,
    identity: PlaybackRuntimeIdentity,
) -> Result<PlaybackRuntimeProjection, ActionError> {
    let snapshot = ports.state.engine.snapshot();
    let scope = show_scope(ports, &snapshot);
    let runtime = ports.state.engine.playback().read().runtime_status();
    match identity {
        PlaybackRuntimeIdentity::Playback(number) => {
            project_playback(ports, &snapshot, &runtime, scope, identity, number)
        }
        PlaybackRuntimeIdentity::CueList(cue_list_id) => Ok(cue_list_projection(
            scope,
            identity,
            None,
            cue_list_id,
            direct_cue_list_runtime(&runtime, cue_list_id),
        )),
    }
}

pub(super) fn projections(
    ports: &ServerPlaybackPorts<'_>,
    _context: &ActionContext,
    identities: &[PlaybackRuntimeIdentity],
) -> Result<Vec<PlaybackRuntimeProjection>, ActionError> {
    let snapshot = ports.state.engine.snapshot();
    let scope = show_scope(ports, &snapshot);
    let runtime = ports.state.engine.playback().read().runtime_status();
    let mut result = Vec::with_capacity(identities.len());
    for identity in identities {
        project_identity(ports, &snapshot, &runtime, scope, *identity, &mut result)?;
    }
    Ok(result)
}

pub(super) fn desk_projection(
    ports: &ServerPlaybackPorts<'_>,
    context: &ActionContext,
) -> Result<Option<PlaybackDeskProjection>, ActionError> {
    if context.desk_id.is_nil() {
        return Ok(None);
    }
    let snapshot = ports.state.engine.snapshot();
    let Some(show) = ports.state.active_show.read().clone() else {
        // Test-bench and startup compatibility paths may operate a prepared in-memory runtime
        // before its show index entry is installed. Nil makes that transient scope explicit.
        return Ok(Some(PlaybackDeskProjection {
            scope: PlaybackShowScope {
                show_id: uuid::Uuid::nil(),
                show_revision: snapshot.revision,
            },
            desk_id: context.desk_id,
            active_page: 1,
            selected_playback: None,
        }));
    };
    let store = ports.state.desk.lock();
    let active_page = store
        .desk_page(context.desk_id, show.id)
        .map_err(|error| invalid(error.to_string()))?;
    let selected_playback = store
        .selected_playback(context.desk_id, show.id)
        .map_err(|error| invalid(error.to_string()))?;
    Ok(Some(PlaybackDeskProjection {
        scope: PlaybackShowScope {
            show_id: show.id.0,
            show_revision: snapshot.revision,
        },
        desk_id: context.desk_id,
        active_page,
        selected_playback,
    }))
}

fn project_identity(
    ports: &ServerPlaybackPorts<'_>,
    snapshot: &Arc<EngineSnapshot>,
    runtime: &[PlaybackRuntimeStatus],
    scope: PlaybackShowScope,
    identity: PlaybackRuntimeIdentity,
    result: &mut Vec<PlaybackRuntimeProjection>,
) -> Result<(), ActionError> {
    match identity {
        PlaybackRuntimeIdentity::Playback(number) => {
            result.push(project_playback(
                ports, snapshot, runtime, scope, identity, number,
            )?);
        }
        PlaybackRuntimeIdentity::CueList(cue_list_id) => {
            project_cue_list(scope, snapshot, runtime, identity, cue_list_id, result);
        }
    }
    Ok(())
}

fn project_cue_list(
    scope: PlaybackShowScope,
    snapshot: &EngineSnapshot,
    runtime: &[PlaybackRuntimeStatus],
    requested: PlaybackRuntimeIdentity,
    cue_list_id: CueListId,
    result: &mut Vec<PlaybackRuntimeProjection>,
) {
    let start = result.len();
    for definition in snapshot.playbacks.iter().filter(|definition| {
        matches!(definition.target, PlaybackTarget::CueList { cue_list_id: id } if id == cue_list_id)
    }) {
        result.push(cue_list_projection(
            scope,
            requested,
            Some(definition.number),
            cue_list_id,
            runtime
                .iter()
                .find(|status| status.playback.playback_number == Some(definition.number)),
        ));
    }
    let direct = direct_cue_list_runtime(runtime, cue_list_id);
    if direct.is_some() || result.len() == start {
        result.push(cue_list_projection(
            scope,
            requested,
            None,
            cue_list_id,
            direct,
        ));
    }
}

fn direct_cue_list_runtime(
    runtime: &[PlaybackRuntimeStatus],
    cue_list_id: CueListId,
) -> Option<&PlaybackRuntimeStatus> {
    runtime.iter().find(|status| {
        status.playback.playback_number.is_none() && status.playback.cue_list_id == cue_list_id
    })
}

fn project_playback(
    ports: &ServerPlaybackPorts<'_>,
    snapshot: &EngineSnapshot,
    runtime: &[PlaybackRuntimeStatus],
    scope: PlaybackShowScope,
    requested: PlaybackRuntimeIdentity,
    number: u16,
) -> Result<PlaybackRuntimeProjection, ActionError> {
    let Some(definition) = snapshot
        .playbacks
        .iter()
        .find(|definition| definition.number == number)
    else {
        return Ok(PlaybackRuntimeProjection {
            scope,
            requested,
            playback_number: Some(number),
            target: PlaybackTargetProjection::Missing,
        });
    };
    let target = match &definition.target {
        PlaybackTarget::CueList { cue_list_id } => {
            return Ok(cue_list_projection(
                scope,
                requested,
                Some(number),
                *cue_list_id,
                runtime
                    .iter()
                    .find(|status| status.playback.playback_number == Some(number)),
            ));
        }
        PlaybackTarget::Group { group_id } => group_projection(ports, snapshot, group_id)?,
        PlaybackTarget::SpeedGroup { group } => speed_projection(ports, group)?,
        PlaybackTarget::GrandMaster => grand_master_projection(ports),
        PlaybackTarget::ProgrammerFade => PlaybackTargetProjection::ProgrammerFade {
            millis: ports.state.configuration.read().programmer_fade_millis,
        },
        PlaybackTarget::CueFade => PlaybackTargetProjection::CueFade {
            millis: ports.state.configuration.read().sequence_master_fade_millis,
        },
    };
    Ok(PlaybackRuntimeProjection {
        scope,
        requested,
        playback_number: Some(number),
        target,
    })
}

fn cue_list_projection(
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

fn show_scope(ports: &ServerPlaybackPorts<'_>, snapshot: &EngineSnapshot) -> PlaybackShowScope {
    let show_id = ports
        .state
        .active_show
        .read()
        .as_ref()
        .map(|show| show.id.0)
        .unwrap_or_else(uuid::Uuid::nil);
    PlaybackShowScope {
        show_id,
        show_revision: snapshot.revision,
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

fn group_projection(
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

fn speed_projection(
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

fn grand_master_projection(ports: &ServerPlaybackPorts<'_>) -> PlaybackTargetProjection {
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

#[cfg(test)]
#[path = "projection_tests.rs"]
mod tests;
