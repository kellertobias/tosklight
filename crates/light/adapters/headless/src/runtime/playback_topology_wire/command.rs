use super::*;

pub(in crate::runtime) fn application_command(
    show_id: ShowId,
    request: wire::PlaybackTopologyActionRequest,
) -> Result<(String, application::PlaybackTopologyCommand), String> {
    let request_id = request.request_id;
    let action = match request.action {
        action @ wire::PlaybackTopologyAction::SaveCueList { .. } => {
            application_save_cue_list(action)?
        }
        wire::PlaybackTopologyAction::UndoCueList {
            cue_list_id,
            expected_revision,
            expected_object_id,
        } => application::PlaybackTopologyAction::UndoCueList {
            cue_list_id: CueListId(non_nil(cue_list_id, "cue_list_id")?),
            expected_revision: input_revision(expected_revision, "expected_revision")?,
            expected_object_id,
        },
        wire::PlaybackTopologyAction::RedoCueList {
            cue_list_id,
            expected_revision,
            expected_object_id,
        } => application::PlaybackTopologyAction::RedoCueList {
            cue_list_id: CueListId(non_nil(cue_list_id, "cue_list_id")?),
            expected_revision: input_revision(expected_revision, "expected_revision")?,
            expected_object_id,
        },
        wire::PlaybackTopologyAction::AssignPlayback {
            address,
            expected_page_revision,
            expected_page_object_id,
            definition,
        } => application::PlaybackTopologyAction::AssignPlayback {
            address: application_address(address)?,
            expected_page_revision: input_revision(
                expected_page_revision,
                "expected_page_revision",
            )?,
            expected_page_object_id: expected_page_object_id.into_option(),
            playback: application_playback(definition)?,
        },
        wire::PlaybackTopologyAction::UpdatePlayback {
            address,
            expected_page_revision,
            expected_page_object_id,
            definition,
        } => application::PlaybackTopologyAction::UpdatePlayback {
            address: application_address(address)?,
            expected_page_revision: input_revision(
                expected_page_revision,
                "expected_page_revision",
            )?,
            expected_page_object_id: expected_page_object_id.into_option(),
            playback: application_playback(definition)?,
        },
        wire::PlaybackTopologyAction::ClearPlayback {
            address,
            expected_page_revision,
            expected_page_object_id,
        } => application::PlaybackTopologyAction::ClearPlayback {
            address: application_address(address)?,
            expected_page_revision: input_revision(
                expected_page_revision,
                "expected_page_revision",
            )?,
            expected_page_object_id: expected_page_object_id.into_option(),
        },
        wire::PlaybackTopologyAction::AssignGroupMaster {
            group_object_id,
            expected_group_revision,
            address,
            expected_page_revision,
            expected_page_object_id,
        } => application::PlaybackTopologyAction::AssignGroupMaster {
            group_object_id,
            expected_group_revision: input_revision(
                expected_group_revision,
                "expected_group_revision",
            )?,
            address: application::GroupMasterPlaybackAddress {
                address: application_address(address)?,
                expected_page_revision: input_revision(
                    expected_page_revision,
                    "expected_page_revision",
                )?,
                expected_page_object_id: expected_page_object_id.into_option(),
            },
        },
        wire::PlaybackTopologyAction::CreatePlaybackPage {
            number,
            name,
            expected_page_revision,
            expected_page_object_id,
        } => application::PlaybackTopologyAction::CreatePage {
            page: number,
            name,
            expected_page_revision: input_revision(
                expected_page_revision,
                "expected_page_revision",
            )?,
            expected_page_object_id: expected_page_object_id.into_option(),
        },
        wire::PlaybackTopologyAction::RenamePlaybackPage {
            number,
            name,
            expected_page_revision,
            expected_page_object_id,
        } => application::PlaybackTopologyAction::RenamePage {
            page: number,
            name,
            expected_page_revision: input_revision(
                expected_page_revision,
                "expected_page_revision",
            )?,
            expected_page_object_id: expected_page_object_id.into_option(),
        },
        wire::PlaybackTopologyAction::DeletePlaybackPage {
            number,
            expected_page_revision,
            expected_page_object_id,
        } => application::PlaybackTopologyAction::DeletePage {
            page: number,
            expected_page_revision: input_revision(
                expected_page_revision,
                "expected_page_revision",
            )?,
            expected_page_object_id: expected_page_object_id.into_option(),
        },
    };
    Ok((
        request_id,
        application::PlaybackTopologyCommand { show_id, action },
    ))
}

fn application_address(
    value: light_wire::v2::playback::PlaybackAddressValue,
) -> Result<playback::PlaybackAddress, String> {
    playback::PlaybackAddress::new(value.page, value.position)
}

fn application_save_cue_list(
    action: wire::PlaybackTopologyAction,
) -> Result<application::PlaybackTopologyAction, String> {
    let wire::PlaybackTopologyAction::SaveCueList {
        cue_list_id,
        expected_revision,
        expected_object_id,
        body,
    } = action
    else {
        unreachable!("caller passes only SaveCueList")
    };
    Ok(application::PlaybackTopologyAction::SaveCueList {
        cue_list_id: CueListId(non_nil(cue_list_id, "cue_list_id")?),
        expected_revision: input_revision(expected_revision, "expected_revision")?,
        expected_object_id: expected_object_id.into_option(),
        cue_list: serde_json::from_value(body.clone())
            .map_err(|error| format!("Cuelist body is invalid: {error}"))?,
        raw_body: Arc::new(body),
    })
}
