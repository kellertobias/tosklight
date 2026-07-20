use light_application as application;
use light_core::{CueListId, ShowId};
use light_playback as playback;
use light_wire::v2::playback_topology as wire;
use std::sync::Arc;

pub(super) fn application_command(
    show_id: ShowId,
    request: wire::PlaybackTopologyActionRequest,
) -> Result<(String, application::PlaybackTopologyCommand), String> {
    let request_id = request.request_id;
    let action = match request.action {
        wire::PlaybackTopologyAction::SaveCueList {
            cue_list_id,
            expected_revision,
            expected_object_id,
            body,
        } => application::PlaybackTopologyAction::SaveCueList {
            cue_list_id: CueListId(non_nil(cue_list_id, "cue_list_id")?),
            expected_revision,
            expected_object_id: expected_object_id.into_option(),
            cue_list: serde_json::from_value(body.clone())
                .map_err(|error| format!("Cuelist body is invalid: {error}"))?,
            raw_body: Arc::new(body),
        },
        wire::PlaybackTopologyAction::ConfigureSlot {
            page,
            slot,
            expected_page_revision,
            expected_page_object_id,
            expected_playback_revision,
            expected_playback_object_id,
            playback,
        } => application::PlaybackTopologyAction::ConfigureSlot {
            page,
            slot,
            expected_page_revision,
            expected_page_object_id: expected_page_object_id.into_option(),
            expected_playback_revision,
            expected_playback_object_id: expected_playback_object_id.into_option(),
            playback: application_playback(playback)?,
        },
        wire::PlaybackTopologyAction::ClearMappedPlayback {
            page,
            slot,
            expected_page_revision,
            expected_page_object_id,
            expected_playback_revision,
            expected_playback_object_id,
        } => application::PlaybackTopologyAction::ClearMappedPlayback {
            page,
            slot,
            expected_page_revision,
            expected_page_object_id: expected_page_object_id.into_option(),
            expected_playback_revision,
            expected_playback_object_id: expected_playback_object_id.into_option(),
        },
    };
    Ok((
        request_id,
        application::PlaybackTopologyCommand { show_id, action },
    ))
}

pub(super) fn outcome(
    result: application::PlaybackTopologyResult,
) -> Result<wire::PlaybackTopologyActionOutcome, application::ActionError> {
    let (show_revision, resolution, state) = match result.outcome {
        application::PlaybackTopologyOutcome::Changed {
            show_revision,
            resolution,
            objects,
            event_sequence,
        } => (
            show_revision.value(),
            wire_resolution(resolution),
            wire::PlaybackTopologyActionState::Changed {
                objects: wire_objects(&objects)?,
                event_sequence,
            },
        ),
        application::PlaybackTopologyOutcome::NoChange {
            show_revision,
            resolution,
            objects,
        } => (
            show_revision.value(),
            wire_resolution(resolution),
            wire::PlaybackTopologyActionState::NoChange {
                objects: wire_objects(&objects)?,
            },
        ),
    };
    Ok(wire::PlaybackTopologyActionOutcome {
        request_id: result.request_id,
        correlation_id: result.correlation_id,
        show_revision,
        resolution,
        outcome: state,
        replayed: result.replayed,
    })
}

fn application_playback(
    value: wire::PlaybackTopologyPlaybackDefinition,
) -> Result<playback::PlaybackDefinition, String> {
    let target = application_target(value.target)?;
    let fader = normalize_legacy_fader(&target, application_fader(value.fader));
    Ok(playback::PlaybackDefinition {
        number: value.number,
        name: value.name,
        target,
        buttons: value.buttons.map(application_button),
        button_count: value.button_count,
        fader,
        has_fader: value.has_fader,
        go_activates: value.go_activates,
        auto_off: value.auto_off,
        xfade_millis: value.xfade_millis,
        color: value.color,
        flash_release: match value.flash_release {
            wire::PlaybackTopologyFlashReleaseMode::ReleaseAll => {
                playback::FlashReleaseMode::ReleaseAll
            }
            wire::PlaybackTopologyFlashReleaseMode::ReleaseIntensityOnly => {
                playback::FlashReleaseMode::ReleaseIntensityOnly
            }
        },
        protect_from_swap: value.protect_from_swap,
        presentation_icon: value.presentation_icon,
        presentation_image: value.presentation_image,
    })
}

fn normalize_legacy_fader(
    target: &playback::PlaybackTarget,
    fader: playback::PlaybackFaderMode,
) -> playback::PlaybackFaderMode {
    if matches!(target, playback::PlaybackTarget::SpeedGroup { .. })
        && fader == playback::PlaybackFaderMode::Speed
    {
        return playback::PlaybackFaderMode::LearnedPercentage;
    }
    fader
}

fn application_target(
    value: wire::PlaybackTopologyTarget,
) -> Result<playback::PlaybackTarget, String> {
    Ok(match value {
        wire::PlaybackTopologyTarget::CueList { cue_list_id } => {
            playback::PlaybackTarget::CueList {
                cue_list_id: CueListId(non_nil(cue_list_id, "playback.target.cue_list_id")?),
            }
        }
        wire::PlaybackTopologyTarget::Group { group_id } => {
            playback::PlaybackTarget::Group { group_id }
        }
        wire::PlaybackTopologyTarget::SpeedGroup { group } => {
            playback::PlaybackTarget::SpeedGroup { group }
        }
        wire::PlaybackTopologyTarget::ProgrammerFade {} => playback::PlaybackTarget::ProgrammerFade,
        wire::PlaybackTopologyTarget::CueFade {} => playback::PlaybackTarget::CueFade,
        wire::PlaybackTopologyTarget::GrandMaster {} => playback::PlaybackTarget::GrandMaster,
    })
}

fn application_button(value: wire::PlaybackTopologyButtonAction) -> playback::PlaybackButtonAction {
    use playback::PlaybackButtonAction as Output;
    use wire::PlaybackTopologyButtonAction as Input;
    match value {
        Input::On => Output::On,
        Input::Off => Output::Off,
        Input::Toggle => Output::Toggle,
        Input::Go => Output::Go,
        Input::GoMinus => Output::GoMinus,
        Input::FastForward => Output::FastForward,
        Input::FastRewind => Output::FastRewind,
        Input::Flash => Output::Flash,
        Input::Temp => Output::Temp,
        Input::Swap => Output::Swap,
        Input::Select => Output::Select,
        Input::SelectContents => Output::SelectContents,
        Input::SelectDereferenced => Output::SelectDereferenced,
        Input::Learn => Output::Learn,
        Input::Double => Output::Double,
        Input::Half => Output::Half,
        Input::Pause => Output::Pause,
        Input::Blackout => Output::Blackout,
        Input::PauseDynamics => Output::PauseDynamics,
        Input::None => Output::None,
    }
}

fn application_fader(value: wire::PlaybackTopologyFaderMode) -> playback::PlaybackFaderMode {
    use playback::PlaybackFaderMode as Output;
    use wire::PlaybackTopologyFaderMode as Input;
    match value {
        Input::Master => Output::Master,
        Input::Temp => Output::Temp,
        Input::Speed => Output::Speed,
        Input::XFade => Output::XFade,
        Input::DirectBpm => Output::DirectBpm,
        Input::CenteredRelative => Output::CenteredRelative,
        Input::LearnedPercentage => Output::LearnedPercentage,
    }
}

fn wire_resolution(
    value: application::PlaybackTopologyResolution,
) -> wire::PlaybackTopologyResolution {
    match value {
        application::PlaybackTopologyResolution::CueList { cue_list_id } => {
            wire::PlaybackTopologyResolution::CueList {
                cue_list_id: cue_list_id.0,
            }
        }
        application::PlaybackTopologyResolution::PageSlot {
            page,
            slot,
            playback_number,
        } => wire::PlaybackTopologyResolution::PageSlot {
            page,
            slot,
            playback_number,
        },
    }
}

fn wire_objects(
    values: &[application::PlaybackTopologyObjectProjection],
) -> Result<Vec<wire::PlaybackTopologyObjectProjection>, application::ActionError> {
    values.iter().map(wire_object).collect()
}

fn wire_object(
    value: &application::PlaybackTopologyObjectProjection,
) -> Result<wire::PlaybackTopologyObjectProjection, application::ActionError> {
    Ok(match value {
        application::PlaybackTopologyObjectProjection::Present {
            kind,
            object_id,
            object_revision,
            raw_body,
        } => wire::PlaybackTopologyObjectProjection::Present {
            kind: wire_kind(*kind)?,
            object_id: object_id.clone(),
            object_revision: *object_revision,
            body: raw_body.as_ref().clone(),
        },
        application::PlaybackTopologyObjectProjection::Deleted {
            kind,
            object_id,
            object_revision,
        } => wire::PlaybackTopologyObjectProjection::Deleted {
            kind: wire_kind(*kind)?,
            object_id: object_id.clone(),
            object_revision: *object_revision,
        },
    })
}

fn wire_kind(
    kind: application::ActiveShowObjectKind,
) -> Result<light_wire::v2::events::ShowObjectKind, application::ActionError> {
    use application::ActiveShowObjectKind as Input;
    use light_wire::v2::events::ShowObjectKind as Output;
    match kind {
        Input::CueList => Ok(Output::CueList),
        Input::Playback => Ok(Output::Playback),
        Input::PlaybackPage => Ok(Output::PlaybackPage),
        Input::Group | Input::Preset => Err(application::ActionError::new(
            application::ActionErrorKind::Internal,
            "Playback topology returned an unrelated object kind",
        )),
    }
}

fn non_nil(value: uuid::Uuid, name: &str) -> Result<uuid::Uuid, String> {
    if value.is_nil() {
        return Err(format!("{name} must not be nil"));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_speed_group_fader_uses_the_domain_migration() {
        let target = playback::PlaybackTarget::SpeedGroup { group: "A".into() };

        assert_eq!(
            normalize_legacy_fader(&target, playback::PlaybackFaderMode::Speed),
            playback::PlaybackFaderMode::LearnedPercentage
        );
        assert_eq!(
            normalize_legacy_fader(&target, playback::PlaybackFaderMode::DirectBpm),
            playback::PlaybackFaderMode::DirectBpm
        );
    }
}
