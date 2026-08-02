use light_application as application;
use light_core::{CueListId, ShowId};
use light_playback as playback;
use light_wire::v2::playback_topology as wire;
use std::sync::Arc;

const JAVASCRIPT_MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

mod command;

pub(super) use command::application_command;

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
            output_revision(show_revision.value(), "show_revision")?,
            wire_resolution(resolution),
            wire::PlaybackTopologyActionState::Changed {
                objects: wire_objects(&objects)?,
                event_sequence: output_revision(event_sequence, "event_sequence")?,
            },
        ),
        application::PlaybackTopologyOutcome::NoChange {
            show_revision,
            resolution,
            objects,
        } => (
            output_revision(show_revision.value(), "show_revision")?,
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
        wire::PlaybackTopologyTarget::Dynamic { assignment } => {
            let definition = serde_json::from_value::<light_dynamics::DynamicDefinition>(
                serde_json::to_value(assignment.embedded_fallback)
                    .map_err(|error| format!("Dynamic fallback is invalid: {error}"))?,
            )
            .map_err(|error| format!("Dynamic fallback is invalid: {error}"))?;
            playback::PlaybackTarget::Dynamic {
                assignment: playback::DynamicPlaybackAssignment {
                    dynamic: light_dynamics::DynamicReference {
                        dynamic_id: assignment.dynamic_id,
                        last_known_pool_number: assignment.last_known_pool_number,
                        embedded_fallback: light_dynamics::DynamicDefinitionSnapshot {
                            definition: Arc::new(definition),
                        },
                    },
                    revision: assignment.revision,
                    target_scope: assignment.target_scope.map(|scope| match scope {
                        wire::PlaybackTopologyDynamicTargetScope::LiveGroup { group_id } => {
                            playback::DynamicPlaybackTargetScope::LiveGroup { group_id }
                        }
                        wire::PlaybackTopologyDynamicTargetScope::FrozenTargets { targets } => {
                            playback::DynamicPlaybackTargetScope::FrozenTargets {
                                targets: targets.into_iter().map(light_core::FixtureId).collect(),
                            }
                        }
                    }),
                    fader_mode: match assignment.fader_mode {
                        wire::PlaybackTopologyDynamicFaderMode::None => {
                            playback::DynamicPlaybackFaderMode::None
                        }
                        wire::PlaybackTopologyDynamicFaderMode::Master => {
                            playback::DynamicPlaybackFaderMode::Master
                        }
                        wire::PlaybackTopologyDynamicFaderMode::Size => {
                            playback::DynamicPlaybackFaderMode::Size
                        }
                        wire::PlaybackTopologyDynamicFaderMode::SizeAndMaster => {
                            playback::DynamicPlaybackFaderMode::SizeAndMaster
                        }
                    },
                    priority: assignment.priority,
                    activation_override: assignment.activation_override.map(|policy| match policy {
                        light_wire::v2::dynamics::DynamicActivationPolicyProjection::StartNow => {
                            light_dynamics::ActivationPolicy::StartNow
                        }
                        light_wire::v2::dynamics::DynamicActivationPolicyProjection::JoinSyncNow => {
                            light_dynamics::ActivationPolicy::JoinSyncNow
                        }
                        light_wire::v2::dynamics::DynamicActivationPolicyProjection::NextBoundary => {
                            light_dynamics::ActivationPolicy::NextBoundary
                        }
                    }),
                    resume_policy: match assignment.resume_policy {
                        wire::PlaybackTopologyDynamicResumePolicy::FollowDynamic => {
                            playback::DynamicPlaybackResumePolicy::FollowDynamic
                        }
                        wire::PlaybackTopologyDynamicResumePolicy::ResumeFrozenPhase => {
                            playback::DynamicPlaybackResumePolicy::ResumeFrozenPhase
                        }
                        wire::PlaybackTopologyDynamicResumePolicy::RejoinSynchronizedPosition => {
                            playback::DynamicPlaybackResumePolicy::RejoinSynchronizedPosition
                        }
                        wire::PlaybackTopologyDynamicResumePolicy::ResumeOnNextBoundary => {
                            playback::DynamicPlaybackResumePolicy::ResumeOnNextBoundary
                        }
                    },
                    local_speed_multiplier: light_dynamics::Rational {
                        numerator: assignment.local_speed_multiplier.numerator,
                        denominator: assignment.local_speed_multiplier.denominator,
                    },
                    learned_duration_millis: assignment.learned_duration_millis,
                    crossfade_non_intensity: assignment.crossfade_non_intensity,
                    auto_off_at_zero: assignment.auto_off_at_zero,
                    auto_off_flash_release: assignment.auto_off_flash_release,
                    auto_off_full_control: assignment.auto_off_full_control,
                },
            }
        }
        wire::PlaybackTopologyTarget::Group {
            group_id,
            initial_master,
        } => playback::PlaybackTarget::Group {
            group_id,
            initial_master,
        },
        wire::PlaybackTopologyTarget::SpeedGroup { group } => {
            playback::PlaybackTarget::SpeedGroup { group }
        }
        wire::PlaybackTopologyTarget::ProgrammerFade {} => playback::PlaybackTarget::ProgrammerFade,
        wire::PlaybackTopologyTarget::CueFade {} => playback::PlaybackTarget::CueFade,
        wire::PlaybackTopologyTarget::GrandMaster {} => playback::PlaybackTarget::GrandMaster,
    })
}

fn application_group_master_address(
    value: wire::PlaybackTopologyGroupMasterAddress,
) -> Result<application::GroupMasterPlaybackAddress, String> {
    Ok(match value {
        wire::PlaybackTopologyGroupMasterAddress::Physical {
            page,
            slot,
            expected_page_revision,
            expected_page_object_id,
            expected_playback_revision,
            expected_playback_object_id,
        } => application::GroupMasterPlaybackAddress::Physical {
            page,
            slot,
            expected_page_revision: input_revision(
                expected_page_revision,
                "expected_page_revision",
            )?,
            expected_page_object_id: expected_page_object_id.into_option(),
            expected_playback_revision: input_revision(
                expected_playback_revision,
                "expected_playback_revision",
            )?,
            expected_playback_object_id: expected_playback_object_id.into_option(),
        },
        wire::PlaybackTopologyGroupMasterAddress::Virtual {
            page,
            playback_number,
            expected_page_revision,
            expected_page_object_id,
        } => application::GroupMasterPlaybackAddress::Virtual {
            page,
            playback_number,
            expected_page_revision: input_revision(
                expected_page_revision,
                "expected_page_revision",
            )?,
            expected_page_object_id: expected_page_object_id.into_option(),
        },
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
        Input::DynamicRestart => Output::DynamicRestart,
        Input::DynamicDoubleSpeed => Output::DynamicDoubleSpeed,
        Input::DynamicHalfSpeed => Output::DynamicHalfSpeed,
        Input::DynamicLearnSpeed => Output::DynamicLearnSpeed,
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
        application::PlaybackTopologyResolution::Page { page } => {
            wire::PlaybackTopologyResolution::Page { page }
        }
        application::PlaybackTopologyResolution::Virtual {
            page,
            playback_number,
        } => wire::PlaybackTopologyResolution::Virtual {
            page,
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
            object_revision: output_revision(*object_revision, "object_revision")?,
            body: raw_body.as_ref().clone(),
        },
        application::PlaybackTopologyObjectProjection::Deleted {
            kind,
            object_id,
            object_revision,
        } => wire::PlaybackTopologyObjectProjection::Deleted {
            kind: wire_kind(*kind)?,
            object_id: object_id.clone(),
            object_revision: output_revision(*object_revision, "object_revision")?,
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
        Input::AttributeConfiguration
        | Input::Group
        | Input::Dynamic
        | Input::PatchLayer
        | Input::Preset
        | Input::Schedule
        | Input::StageLayout
        | Input::UserLayout => Err(application::ActionError::new(
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

fn input_revision(value: u64, name: &str) -> Result<u64, String> {
    if value > JAVASCRIPT_MAX_SAFE_INTEGER {
        return Err(format!(
            "{name} must not exceed the JavaScript maximum safe integer"
        ));
    }
    Ok(value)
}

fn output_revision(value: u64, name: &str) -> Result<u64, application::ActionError> {
    input_revision(value, name).map_err(|message| {
        application::ActionError::new(application::ActionErrorKind::Internal, message)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assign_group_request(revision: u64) -> wire::PlaybackTopologyActionRequest {
        wire::PlaybackTopologyActionRequest {
            request_id: "assign-front".into(),
            action: wire::PlaybackTopologyAction::AssignGroupMaster {
                group_object_id: "front".into(),
                expected_group_revision: revision,
                address: wire::PlaybackTopologyGroupMasterAddress::Physical {
                    page: 2,
                    slot: 4,
                    expected_page_revision: 5,
                    expected_page_object_id: wire::PlaybackTopologyObjectIdentity::Present(
                        "page-two".into(),
                    ),
                    expected_playback_revision: 6,
                    expected_playback_object_id: wire::PlaybackTopologyObjectIdentity::Present(
                        "playback-six".into(),
                    ),
                },
            },
        }
    }

    #[test]
    fn group_master_assignment_maps_every_authority_field() {
        let show_id = ShowId(uuid::Uuid::from_u128(1));
        let (request_id, command) = application_command(show_id, assign_group_request(3)).unwrap();

        assert_eq!(request_id, "assign-front");
        assert_eq!(command.show_id, show_id);
        let application::PlaybackTopologyAction::AssignGroupMaster {
            group_object_id,
            expected_group_revision,
            address,
        } = command.action
        else {
            panic!("expected AssignGroupMaster action");
        };
        assert_eq!(group_object_id, "front");
        assert_eq!(expected_group_revision, 3);
        let application::GroupMasterPlaybackAddress::Physical {
            page,
            slot,
            expected_page_revision,
            expected_page_object_id,
            expected_playback_revision,
            expected_playback_object_id,
        } = address
        else {
            panic!("expected physical Group Master address");
        };
        assert_eq!((page, slot), (2, 4));
        assert_eq!(expected_page_revision, 5);
        assert_eq!(expected_page_object_id.as_deref(), Some("page-two"));
        assert_eq!(expected_playback_revision, 6);
        assert_eq!(expected_playback_object_id.as_deref(), Some("playback-six"));
    }

    #[test]
    fn group_master_assignment_rejects_unsafe_javascript_revisions() {
        let error = application_command(
            ShowId(uuid::Uuid::from_u128(1)),
            assign_group_request(JAVASCRIPT_MAX_SAFE_INTEGER + 1),
        )
        .unwrap_err();

        assert!(error.contains("expected_group_revision"));
        assert!(error.contains("maximum safe integer"));
    }

    #[test]
    fn virtual_group_master_assignment_maps_exact_page_contained_authority() {
        let request = wire::PlaybackTopologyActionRequest {
            request_id: "assign-virtual-front".into(),
            action: wire::PlaybackTopologyAction::AssignGroupMaster {
                group_object_id: "front".into(),
                expected_group_revision: 3,
                address: wire::PlaybackTopologyGroupMasterAddress::Virtual {
                    page: 2,
                    playback_number: 1301,
                    expected_page_revision: 5,
                    expected_page_object_id: wire::PlaybackTopologyObjectIdentity::Present(
                        "page-two".into(),
                    ),
                },
            },
        };

        let (_, command) = application_command(ShowId(uuid::Uuid::from_u128(1)), request).unwrap();
        let application::PlaybackTopologyAction::AssignGroupMaster { address, .. } = command.action
        else {
            panic!("expected AssignGroupMaster action");
        };
        assert!(matches!(
            address,
            application::GroupMasterPlaybackAddress::Virtual {
                page: 2,
                playback_number: 1301,
                expected_page_revision: 5,
                expected_page_object_id: Some(ref id),
            } if id == "page-two"
        ));
    }

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
