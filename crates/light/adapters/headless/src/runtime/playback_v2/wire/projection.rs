//! Playback runtime projection DTO mapping.

use light_application as application;
use light_wire::v2::playback as wire;

pub(super) fn target_projection(
    projection: &application::PlaybackTargetProjection,
) -> wire::PlaybackTargetProjection {
    use application::PlaybackTargetProjection as App;
    match projection {
        App::Missing => wire::PlaybackTargetProjection::Missing,
        App::CueList {
            cue_list_id,
            runtime,
        } => wire::PlaybackTargetProjection::CueList {
            cue_list_id: cue_list_id.0,
            runtime: runtime.as_deref().map(cue_list_runtime).map(Box::new),
        },
        App::Dynamic {
            dynamic_id,
            last_known_pool_number,
            embedded,
            runtime,
        } => wire::PlaybackTargetProjection::Dynamic {
            dynamic_id: *dynamic_id,
            last_known_pool_number: *last_known_pool_number,
            embedded: *embedded,
            runtime: runtime
                .as_ref()
                .map(|runtime| wire::DynamicPlaybackRuntimeProjection {
                    playback_number: runtime.active.playback_number,
                    enabled: runtime.active.enabled,
                    paused: runtime.active.paused,
                    flash: runtime.active.flash,
                    activated_at: runtime.active.activated_at.to_rfc3339(),
                    fader_value: runtime.active.fader_value,
                    fader_pickup_required: runtime.active.fader_pickup_required,
                    fader_pickup_target: runtime.active.fader_pickup_target,
                    size: runtime.active.size,
                    master: runtime.active.master,
                    local_speed_numerator: runtime.active.local_speed_multiplier.numerator,
                    local_speed_denominator: runtime.active.local_speed_multiplier.denominator,
                    learned_duration_millis: runtime.active.learned_duration_millis,
                    state: match runtime.state {
                        application::DynamicPlaybackRuntimeState::Off => {
                            wire::DynamicPlaybackRuntimeState::Off
                        }
                        application::DynamicPlaybackRuntimeState::Zero => {
                            wire::DynamicPlaybackRuntimeState::Zero
                        }
                        application::DynamicPlaybackRuntimeState::Pending => {
                            wire::DynamicPlaybackRuntimeState::Pending
                        }
                        application::DynamicPlaybackRuntimeState::Active => {
                            wire::DynamicPlaybackRuntimeState::Active
                        }
                        application::DynamicPlaybackRuntimeState::Paused => {
                            wire::DynamicPlaybackRuntimeState::Paused
                        }
                        application::DynamicPlaybackRuntimeState::Hidden => {
                            wire::DynamicPlaybackRuntimeState::Hidden
                        }
                        application::DynamicPlaybackRuntimeState::Failed => {
                            wire::DynamicPlaybackRuntimeState::Failed
                        }
                    },
                    instance_id: runtime.instance_id,
                    controller_id: runtime.controller_id,
                    winning_controller_id: runtime.winning_controller_id,
                    controller_status: match runtime.controller_status {
                        application::DynamicPlaybackControllerStatus::Winning => {
                            wire::DynamicPlaybackControllerStatus::Winning
                        }
                        application::DynamicPlaybackControllerStatus::Losing => {
                            wire::DynamicPlaybackControllerStatus::Losing
                        }
                        application::DynamicPlaybackControllerStatus::Missing => {
                            wire::DynamicPlaybackControllerStatus::Missing
                        }
                    },
                    target_count: runtime.target_count,
                    compatible_target_count: runtime.compatible_target_count,
                    missing_target_count: runtime.missing_target_count,
                    unpatched_target_count: runtime.unpatched_target_count,
                    lane_count: runtime.lane_count,
                    supported_address_count: runtime.supported_address_count,
                    skipped_address_count: runtime.skipped_address_count,
                    speed_source: match runtime.speed_source {
                        application::DynamicPlaybackSpeedSource::Fixed => {
                            wire::DynamicPlaybackSpeedSource::Fixed
                        }
                        application::DynamicPlaybackSpeedSource::SpeedGroup => {
                            wire::DynamicPlaybackSpeedSource::SpeedGroup
                        }
                    },
                    effective_speed_multiplier: runtime.effective_speed_multiplier,
                    effective_duration_millis: runtime.effective_duration_millis,
                    warning: runtime.warning.clone(),
                }),
        },
        App::Group {
            group_id,
            master,
            flash_level,
            fader_position,
            fader_pickup_required,
            fader_pickup_target,
        } => wire::PlaybackTargetProjection::Group {
            group_id: group_id.clone(),
            master: *master,
            flash_level: *flash_level,
            fader_position: *fader_position,
            fader_pickup_required: *fader_pickup_required,
            fader_pickup_target: *fader_pickup_target,
        },
        App::SpeedGroup { group, runtime } => wire::PlaybackTargetProjection::SpeedGroup {
            group: group.clone(),
            runtime: Box::new(speed_runtime(runtime)),
        },
        App::GrandMaster(runtime) => wire::PlaybackTargetProjection::GrandMaster {
            runtime: grand_master(*runtime),
        },
        App::ProgrammerFade { millis } => {
            wire::PlaybackTargetProjection::ProgrammerFade { millis: *millis }
        }
        App::CueFade { millis } => wire::PlaybackTargetProjection::CueFade { millis: *millis },
    }
}

fn cue_list_runtime(
    runtime: &application::CueListRuntimeProjection,
) -> wire::CueListRuntimeProjection {
    wire::CueListRuntimeProjection {
        cue_index: runtime.cue_index,
        previous_index: runtime.previous_index,
        current: runtime.current.as_ref().map(cue_reference),
        loaded: runtime.loaded.as_ref().map(cue_reference),
        normal_next: runtime.normal_next.as_ref().map(cue_reference),
        effective_next: runtime.effective_next.as_ref().map(cue_reference),
        effective_next_is_loaded: runtime.effective_next_is_loaded,
        paused: runtime.paused,
        activated_at: runtime.activated_at.to_rfc3339(),
        transition_ordinal: runtime.transition_ordinal,
        master: runtime.master,
        fader_position: runtime.fader_position,
        fader_pickup_required: runtime.fader_pickup_required,
        fader_pickup_target: runtime.fader_pickup_target,
        flash: runtime.flash,
        temporary: runtime.temporary,
        temporary_active: runtime.temporary_active,
        temporary_master: runtime.temporary_master,
        swap_active: runtime.swap_active,
        enabled: runtime.enabled,
        transition_timing_bypassed: runtime.transition_timing_bypassed,
        manual_xfade_position: runtime.manual_xfade_position,
        manual_xfade_direction: match runtime.manual_xfade_direction {
            application::ManualXFadeDirection::TowardsHigh => {
                wire::ManualXFadeDirection::TowardsHigh
            }
            application::ManualXFadeDirection::TowardsLow => wire::ManualXFadeDirection::TowardsLow,
        },
        manual_xfade_progress: runtime.manual_xfade_progress,
    }
}

fn speed_runtime(
    runtime: &application::SpeedGroupRuntimeProjection,
) -> wire::SpeedGroupRuntimeProjection {
    wire::SpeedGroupRuntimeProjection {
        manual_bpm: runtime.manual_bpm,
        sound_bpm: runtime.sound_bpm,
        effective_bpm: runtime.effective_bpm,
        source: match runtime.source {
            application::SpeedSource::Manual => wire::SpeedSource::Manual,
            application::SpeedSource::Sound => wire::SpeedSource::Sound,
            application::SpeedSource::HeldSound => wire::SpeedSource::HeldSound,
            application::SpeedSource::ManualFallback => wire::SpeedSource::ManualFallback,
        },
        sound_status: sound_status(runtime.sound_status),
        paused: runtime.paused,
        phase_advancing: runtime.phase_advancing,
        speed_master_scale: runtime.speed_master_scale,
        sound_multiplier: runtime.sound_multiplier,
        source_available: runtime.source_available,
        usable_signal: runtime.usable_signal,
        input_level: runtime.input_level,
        selected_band_level: runtime.selected_band_level,
        synchronized_with: runtime.synchronized_with,
        phase_origin_millis: runtime.phase_origin_millis,
        beat_phase: runtime.beat_phase,
    }
}

fn sound_status(status: application::SoundStatus) -> wire::SoundStatus {
    match status {
        application::SoundStatus::Disabled => wire::SoundStatus::Disabled,
        application::SoundStatus::Active {
            detected_bpm,
            confidence,
        } => wire::SoundStatus::Active {
            detected_bpm,
            confidence,
        },
        application::SoundStatus::Holding {
            reason,
            remaining_millis,
        } => wire::SoundStatus::Holding {
            reason: sound_loss_reason(reason),
            remaining_millis,
        },
        application::SoundStatus::ManualFallback { reason } => wire::SoundStatus::ManualFallback {
            reason: sound_loss_reason(reason),
        },
    }
}

fn sound_loss_reason(reason: application::SoundLossReason) -> wire::SoundLossReason {
    match reason {
        application::SoundLossReason::SourceUnavailable => wire::SoundLossReason::SourceUnavailable,
        application::SoundLossReason::NoUsableSignal => wire::SoundLossReason::NoUsableSignal,
        application::SoundLossReason::LowConfidence => wire::SoundLossReason::LowConfidence,
        application::SoundLossReason::TempoOutsideRange => wire::SoundLossReason::TempoOutsideRange,
        application::SoundLossReason::WaitingForAnalysis => {
            wire::SoundLossReason::WaitingForAnalysis
        }
    }
}

fn grand_master(
    runtime: application::GrandMasterRuntimeProjection,
) -> wire::GrandMasterRuntimeProjection {
    wire::GrandMasterRuntimeProjection {
        level: runtime.level,
        effective_level: runtime.effective_level,
        blackout: runtime.blackout,
        flash_active: runtime.flash_active,
        dynamics_paused: runtime.dynamics_paused,
    }
}

fn cue_reference(cue: &application::PlaybackCueReference) -> wire::PlaybackCueReference {
    wire::PlaybackCueReference {
        id: cue.id,
        number: cue.number,
    }
}
