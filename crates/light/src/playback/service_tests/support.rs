use super::*;

pub(super) fn cue_projection(number: u16, cue_number: f64) -> PlaybackRuntimeProjection {
    PlaybackRuntimeProjection {
        scope: test_scope(),
        requested: PlaybackRuntimeIdentity::Playback(number),
        playback_number: Some(number),
        target: PlaybackTargetProjection::CueList {
            cue_list_id: light_core::CueListId(Uuid::from_u128(80)),
            runtime: Some(Box::new(CueListRuntimeProjection {
                cue_index: cue_number as usize - 1,
                previous_index: None,
                current: Some(PlaybackCueReference {
                    id: Uuid::from_u128(100 + cue_number as u128),
                    number: cue_number,
                }),
                loaded: None,
                normal_next: None,
                effective_next: None,
                effective_next_is_loaded: false,
                paused: false,
                activated_at: chrono::Utc::now(),
                transition_ordinal: 0,
                master: 1.0,
                fader_position: 1.0,
                fader_pickup_required: false,
                fader_pickup_target: None,
                flash: false,
                temporary: false,
                temporary_active: false,
                temporary_master: 0.0,
                swap_active: false,
                enabled: true,
                transition_timing_bypassed: false,
                manual_xfade_position: 0.0,
                manual_xfade_direction: ManualXFadeDirection::TowardsHigh,
                manual_xfade_progress: 0.0,
            })),
        },
    }
}

pub(super) fn find_projection_mut(
    projections: &mut [PlaybackRuntimeProjection],
    number: u16,
) -> &mut PlaybackRuntimeProjection {
    projections
        .iter_mut()
        .find(|projection| projection.requested == PlaybackRuntimeIdentity::Playback(number))
        .expect("test projection should exist")
}

pub(super) fn set_enabled(projection: &mut PlaybackRuntimeProjection, enabled: bool) {
    let PlaybackTargetProjection::CueList {
        runtime: Some(runtime),
        ..
    } = &mut projection.target
    else {
        panic!("test projection should target a Cuelist runtime");
    };
    runtime.enabled = enabled;
}

pub(super) fn runtime_projection(event: &crate::EventEnvelope) -> &PlaybackRuntimeProjection {
    let crate::ApplicationEvent::Playback(crate::PlaybackEvent::RuntimeChanged(change)) =
        &event.payload
    else {
        panic!("expected Playback runtime event");
    };
    &change.projection
}

pub(super) fn test_desk(desk_id: Uuid) -> PlaybackDeskProjection {
    PlaybackDeskProjection {
        scope: test_scope(),
        desk_id,
        active_page: 1,
        selected_playback: None,
    }
}

pub(super) fn test_scope() -> PlaybackShowScope {
    PlaybackShowScope {
        show_id: Uuid::from_u128(70),
        show_revision: 3,
    }
}

pub(super) fn runtime_event(number: u16) -> crate::EventDraft {
    crate::EventDraft::playback_runtime_changed(
        None,
        PlaybackRuntimeChange {
            projection: missing_projection(PlaybackRuntimeIdentity::Playback(number)),
            transition: None,
        },
        crate::EventSource::Runtime,
        None,
    )
}
