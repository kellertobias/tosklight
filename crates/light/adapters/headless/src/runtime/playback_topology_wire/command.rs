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
        action @ wire::PlaybackTopologyAction::ConfigureSlot { .. } => {
            application_configure_slot(action)?
        }
        wire::PlaybackTopologyAction::AssignGroupMaster {
            group_object_id,
            expected_group_revision,
            address,
        } => application::PlaybackTopologyAction::AssignGroupMaster {
            group_object_id,
            expected_group_revision: input_revision(
                expected_group_revision,
                "expected_group_revision",
            )?,
            address: application_group_master_address(address)?,
        },
        wire::PlaybackTopologyAction::ConfigureVirtual {
            page,
            playback_number,
            expected_page_revision,
            expected_page_object_id,
            playback,
        } => application::PlaybackTopologyAction::ConfigureVirtual {
            page,
            number: playback_number,
            expected_page_revision: input_revision(
                expected_page_revision,
                "expected_page_revision",
            )?,
            expected_page_object_id: expected_page_object_id.into_option(),
            playback: application_playback(playback)?,
        },
        action @ wire::PlaybackTopologyAction::MapExistingPlayback { .. } => {
            application_map_existing_playback(action)?
        }
        wire::PlaybackTopologyAction::CreatePage {
            page,
            expected_page_revision,
            expected_page_object_id,
        } => application::PlaybackTopologyAction::CreatePage {
            page,
            expected_page_revision: input_revision(
                expected_page_revision,
                "expected_page_revision",
            )?,
            expected_page_object_id: expected_page_object_id.into_option(),
        },
        wire::PlaybackTopologyAction::RenamePage {
            page,
            name,
            expected_page_revision,
            expected_page_object_id,
        } => application::PlaybackTopologyAction::RenamePage {
            page,
            name,
            expected_page_revision: input_revision(
                expected_page_revision,
                "expected_page_revision",
            )?,
            expected_page_object_id: expected_page_object_id.into_option(),
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
        wire::PlaybackTopologyAction::ClearVirtual {
            page,
            playback_number,
            expected_page_revision,
            expected_page_object_id,
        } => application::PlaybackTopologyAction::ClearVirtual {
            page,
            number: playback_number,
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

fn application_configure_slot(
    action: wire::PlaybackTopologyAction,
) -> Result<application::PlaybackTopologyAction, String> {
    let wire::PlaybackTopologyAction::ConfigureSlot {
        page,
        slot,
        expected_page_revision,
        expected_page_object_id,
        expected_playback_revision,
        expected_playback_object_id,
        playback,
    } = action
    else {
        unreachable!("caller passes only ConfigureSlot")
    };
    Ok(application::PlaybackTopologyAction::ConfigureSlot {
        page,
        slot,
        expected_page_revision: input_revision(expected_page_revision, "expected_page_revision")?,
        expected_page_object_id: expected_page_object_id.into_option(),
        expected_playback_revision: input_revision(
            expected_playback_revision,
            "expected_playback_revision",
        )?,
        expected_playback_object_id: expected_playback_object_id.into_option(),
        playback: application_playback(playback)?,
    })
}

fn application_map_existing_playback(
    action: wire::PlaybackTopologyAction,
) -> Result<application::PlaybackTopologyAction, String> {
    let wire::PlaybackTopologyAction::MapExistingPlayback {
        page,
        slot,
        playback_number,
        expected_page_revision,
        expected_page_object_id,
        expected_playback_revision,
        expected_playback_object_id,
    } = action
    else {
        unreachable!("caller passes only MapExistingPlayback")
    };
    Ok(application::PlaybackTopologyAction::MapExistingPlayback {
        page,
        slot,
        playback_number,
        expected_page_revision: input_revision(expected_page_revision, "expected_page_revision")?,
        expected_page_object_id: expected_page_object_id.into_option(),
        expected_playback_revision: input_revision(
            expected_playback_revision,
            "expected_playback_revision",
        )?,
        expected_playback_object_id: expected_playback_object_id.into_option(),
    })
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
