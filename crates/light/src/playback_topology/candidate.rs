use super::{
    GroupMasterPlaybackAddress, PlaybackTopologyAction, PlaybackTopologyCommand,
    PlaybackTopologyResolution,
    change::{
        PreparedTopology, changed_configure, changed_cue_list_history, changed_present, no_change,
    },
    map_existing::map_existing_playback,
    page::configured_page,
    page_actions::{create_page, rename_page},
    stored::{
        Stored, cue_list_object_id, find_cue_list, find_group, find_macro, find_page,
        find_playback, find_timecode, invalid, next_playback_number, next_revision, not_found,
        page_object_id, pages, playback_object_id, same_typed, stored_projection,
        validate_identity, validate_revision,
    },
    validation::{validate_page_slot, validate_show},
};
use crate::active_show::PreparedActiveShowTransaction;
use crate::{ActionError, ActiveShowObjectKind, lossless_json};
use light_playback::{
    CueList, FlashReleaseMode, PlaybackDefinition, PlaybackFaderMode, PlaybackTarget,
};
use light_show::{PortableShowDocument, PortableShowObjectRedo, PortableShowObjectUndo};
use serde_json::Value;

pub(super) fn prepare(
    document: &PortableShowDocument,
    command: &PlaybackTopologyCommand,
    expected_show_revision: u64,
) -> Result<PreparedActiveShowTransaction<PreparedTopology>, ActionError> {
    validate_show(document, command, expected_show_revision)?;
    match &command.action {
        PlaybackTopologyAction::SaveCueList {
            cue_list_id,
            expected_revision,
            expected_object_id,
            cue_list,
            raw_body,
        } => save_cue_list(
            document,
            command,
            *cue_list_id,
            *expected_revision,
            expected_object_id.as_deref(),
            cue_list,
            raw_body,
        ),
        PlaybackTopologyAction::UndoCueList { .. } | PlaybackTopologyAction::RedoCueList { .. } => {
            Err(invalid(
                "Cuelist history actions require an active-show history boundary",
            ))
        }
        PlaybackTopologyAction::ConfigureSlot {
            page,
            slot,
            expected_page_revision,
            expected_page_object_id,
            expected_playback_revision,
            expected_playback_object_id,
            playback,
        } => configure_slot(
            document,
            command,
            (*page, *slot),
            (*expected_page_revision, *expected_playback_revision),
            (
                expected_page_object_id.as_deref(),
                expected_playback_object_id.as_deref(),
            ),
            playback,
        ),
        PlaybackTopologyAction::AssignGroupMaster {
            group_object_id,
            expected_group_revision,
            address,
        } => assign_group_master(
            document,
            command,
            group_object_id,
            *expected_group_revision,
            address,
        ),
        PlaybackTopologyAction::ConfigureVirtual {
            page,
            number,
            expected_page_revision,
            expected_page_object_id,
            playback,
        } => configure_virtual(
            document,
            command,
            *page,
            *number,
            *expected_page_revision,
            expected_page_object_id.as_deref(),
            playback,
        ),
        PlaybackTopologyAction::MapExistingPlayback {
            page,
            slot,
            playback_number,
            expected_page_revision,
            expected_page_object_id,
            expected_playback_revision,
            expected_playback_object_id,
        } => map_existing_playback(
            document,
            command,
            (*page, *slot),
            *playback_number,
            (*expected_page_revision, *expected_playback_revision),
            (
                expected_page_object_id.as_deref(),
                expected_playback_object_id.as_deref(),
            ),
        ),
        PlaybackTopologyAction::CreatePage {
            page,
            expected_page_revision,
            expected_page_object_id,
        } => create_page(
            document,
            command,
            *page,
            *expected_page_revision,
            expected_page_object_id.as_deref(),
        ),
        PlaybackTopologyAction::RenamePage {
            page,
            name,
            expected_page_revision,
            expected_page_object_id,
        } => rename_page(
            document,
            command,
            *page,
            name,
            *expected_page_revision,
            expected_page_object_id.as_deref(),
        ),
        PlaybackTopologyAction::ClearMappedPlayback {
            page,
            slot,
            expected_page_revision,
            expected_page_object_id,
            expected_playback_revision,
            expected_playback_object_id,
        } => clear_mapped_playback(
            document,
            command,
            (*page, *slot),
            (*expected_page_revision, *expected_playback_revision),
            (
                expected_page_object_id.as_deref(),
                expected_playback_object_id.as_deref(),
            ),
        ),
        PlaybackTopologyAction::ClearVirtual {
            page,
            number,
            expected_page_revision,
            expected_page_object_id,
        } => clear_virtual(
            document,
            command,
            *page,
            *number,
            *expected_page_revision,
            expected_page_object_id.as_deref(),
        ),
    }
}

pub(super) enum CueListHistory {
    Undo(PortableShowObjectUndo),
    Redo(PortableShowObjectRedo),
}

pub(super) fn prepare_cue_list_history(
    document: &PortableShowDocument,
    command: &PlaybackTopologyCommand,
    expected_show_revision: u64,
    history: CueListHistory,
) -> Result<PreparedActiveShowTransaction<PreparedTopology>, ActionError> {
    validate_show(document, command, expected_show_revision)?;
    let (cue_list_id, expected_revision, expected_object_id, is_undo) = match &command.action {
        PlaybackTopologyAction::UndoCueList {
            cue_list_id,
            expected_revision,
            expected_object_id,
        } => (
            *cue_list_id,
            *expected_revision,
            expected_object_id.as_str(),
            true,
        ),
        PlaybackTopologyAction::RedoCueList {
            cue_list_id,
            expected_revision,
            expected_object_id,
        } => (
            *cue_list_id,
            *expected_revision,
            expected_object_id.as_str(),
            false,
        ),
        _ => return Err(invalid("expected a Cuelist history action")),
    };
    let stored =
        find_cue_list(document, cue_list_id)?.ok_or_else(|| not_found("Cuelist was not found"))?;
    validate_identity(
        Some(&stored),
        Some(expected_object_id),
        "Cuelist",
        document.revision().value(),
    )?;
    validate_revision(
        Some(&stored),
        expected_revision,
        "Cuelist",
        document.revision().value(),
    )?;
    let (key, body) = match &history {
        CueListHistory::Undo(value) => (value.key(), value.body()),
        CueListHistory::Redo(value) => (value.key(), value.body()),
    };
    if key.kind() != ActiveShowObjectKind::CueList.as_str() || key.id() != expected_object_id {
        return Err(invalid("Cuelist history identity changed"));
    }
    let restored: CueList = serde_json::from_value(body.clone()).map_err(invalid)?;
    if restored.id != cue_list_id {
        return Err(invalid("Cuelist history body has a different identity"));
    }
    restored.validate().map_err(invalid)?;
    let mut transaction = document.transaction();
    match history {
        CueListHistory::Undo(value) if is_undo => {
            transaction.undo_object(value);
        }
        CueListHistory::Redo(value) if !is_undo => {
            transaction.redo_object(value);
        }
        _ => return Err(invalid("Cuelist history direction changed")),
    }
    changed_cue_list_history(
        document,
        command,
        PlaybackTopologyResolution::CueList { cue_list_id },
        transaction,
        expected_object_id,
    )
}

fn configure_virtual(
    document: &PortableShowDocument,
    command: &PlaybackTopologyCommand,
    page_number: u8,
    number: u16,
    expected_page_revision: u64,
    expected_page_object_id: Option<&str>,
    requested: &PlaybackDefinition,
) -> Result<PreparedActiveShowTransaction<PreparedTopology>, ActionError> {
    light_playback::VirtualPlaybackAddress::new(page_number, number).map_err(invalid)?;
    let stored = find_page(document, page_number)?;
    validate_identity(
        stored.as_ref(),
        expected_page_object_id,
        "Playback Page",
        document.revision().value(),
    )?;
    validate_revision(
        stored.as_ref(),
        expected_page_revision,
        "Playback Page",
        document.revision().value(),
    )?;
    let mut playback = requested.clone();
    playback.number = number;
    playback.has_fader = false;
    playback.button_count = 1;
    playback.buttons[1] = light_playback::PlaybackButtonAction::None;
    playback.buttons[2] = light_playback::PlaybackButtonAction::None;
    validate_macro_target(document, &playback)?;
    validate_timecode_target(document, &playback)?;
    playback.validate().map_err(invalid)?;
    let mut page = stored.as_ref().map_or_else(
        || light_playback::PlaybackPage {
            number: page_number,
            name: format!("Page {page_number}"),
            slots: std::collections::HashMap::new(),
            virtual_playbacks: std::collections::HashMap::new(),
        },
        |stored| stored.typed.clone(),
    );
    page.virtual_playbacks.insert(number, playback);
    page.validate().map_err(invalid)?;
    let resolution = PlaybackTopologyResolution::Virtual {
        page: page_number,
        playback_number: number,
    };
    if let Some(stored) = stored.as_ref()
        && same_typed(&stored.typed, &page)?
    {
        return Ok(no_change(
            document,
            command,
            resolution,
            vec![stored_projection(
                ActiveShowObjectKind::PlaybackPage,
                stored,
            )],
        ));
    }
    let object_id = page_object_id(document, stored.as_ref(), page_number)?;
    let body = stored.as_ref().map_or_else(
        || serde_json::to_value(&page).map_err(invalid),
        |stored| {
            lossless_json::merge_typed(&stored.raw_body, &stored.typed, &page).map_err(invalid)
        },
    )?;
    changed_present(
        document,
        command,
        resolution,
        vec![(ActiveShowObjectKind::PlaybackPage, object_id, body)],
        Vec::new(),
    )
}

fn clear_virtual(
    document: &PortableShowDocument,
    command: &PlaybackTopologyCommand,
    page_number: u8,
    number: u16,
    expected_page_revision: u64,
    expected_page_object_id: Option<&str>,
) -> Result<PreparedActiveShowTransaction<PreparedTopology>, ActionError> {
    light_playback::VirtualPlaybackAddress::new(page_number, number).map_err(invalid)?;
    let stored = find_page(document, page_number)?;
    validate_identity(
        stored.as_ref(),
        expected_page_object_id,
        "Playback Page",
        document.revision().value(),
    )?;
    validate_revision(
        stored.as_ref(),
        expected_page_revision,
        "Playback Page",
        document.revision().value(),
    )?;
    let resolution = PlaybackTopologyResolution::Virtual {
        page: page_number,
        playback_number: number,
    };
    let Some(stored) = stored else {
        return Ok(no_change(document, command, resolution, Vec::new()));
    };
    let mut page = stored.typed.clone();
    if page.virtual_playbacks.remove(&number).is_none() {
        return Ok(no_change(
            document,
            command,
            resolution,
            vec![stored_projection(
                ActiveShowObjectKind::PlaybackPage,
                &stored,
            )],
        ));
    }
    let body =
        lossless_json::merge_typed(&stored.raw_body, &stored.typed, &page).map_err(invalid)?;
    changed_present(
        document,
        command,
        resolution,
        vec![(ActiveShowObjectKind::PlaybackPage, stored.object_id, body)],
        Vec::new(),
    )
}

fn save_cue_list(
    document: &PortableShowDocument,
    command: &PlaybackTopologyCommand,
    cue_list_id: light_core::CueListId,
    expected_revision: u64,
    expected_object_id: Option<&str>,
    cue_list: &CueList,
    raw_body: &Value,
) -> Result<PreparedActiveShowTransaction<PreparedTopology>, ActionError> {
    if cue_list.id != cue_list_id {
        return Err(invalid(
            "Cuelist identity does not match the requested identity",
        ));
    }
    let raw_cue_list = serde_json::from_value::<CueList>(raw_body.clone()).map_err(invalid)?;
    if !same_typed(&raw_cue_list, cue_list)? {
        return Err(invalid("Cuelist body does not match its typed candidate"));
    }
    cue_list.validate().map_err(invalid)?;
    validate_timecode_graph_for_cue_list(document, cue_list)?;
    let stored = find_cue_list(document, cue_list_id)?;
    validate_identity(
        stored.as_ref(),
        expected_object_id,
        "Cuelist",
        document.revision().value(),
    )?;
    validate_revision(
        stored.as_ref(),
        expected_revision,
        "Cuelist",
        document.revision().value(),
    )?;
    let (cue_list, raw_body) = preserve_omitted_out_timing(stored.as_ref(), cue_list, raw_body)?;
    cue_list.validate().map_err(invalid)?;
    let resolution = PlaybackTopologyResolution::CueList { cue_list_id };
    if let Some(existing) = stored.as_ref()
        && same_typed(&existing.typed, &cue_list)?
    {
        return Ok(no_change(
            document,
            command,
            resolution,
            vec![stored_projection(ActiveShowObjectKind::CueList, existing)],
        ));
    }
    let object_id = cue_list_object_id(document, stored.as_ref(), cue_list_id)?;
    let body = cue_list_body(stored.as_ref(), &cue_list, &raw_body)?;
    changed_present(
        document,
        command,
        resolution,
        vec![(ActiveShowObjectKind::CueList, object_id, body)],
        Vec::new(),
    )
}

fn validate_timecode_graph_for_cue_list(
    document: &PortableShowDocument,
    candidate: &CueList,
) -> Result<(), ActionError> {
    let mut cue_lists = document
        .objects_of_kind("cue_list")
        .filter_map(|object| serde_json::from_value::<CueList>(object.body().clone()).ok())
        .filter(|cue_list| cue_list.id != candidate.id)
        .collect::<Vec<_>>();
    cue_lists.push(candidate.clone());
    let timecodes = document
        .objects_of_kind("timecode")
        .map(|object| {
            serde_json::from_value::<light_playback::TimecodeDefinition>(object.body().clone())
                .map_err(invalid)
        })
        .collect::<Result<Vec<_>, _>>()?;
    crate::validate_cue_timecode_graph(&cue_lists, &timecodes).map_err(invalid)
}

fn preserve_omitted_out_timing(
    stored: Option<&Stored<CueList>>,
    requested: &CueList,
    raw_body: &Value,
) -> Result<(CueList, Value), ActionError> {
    let Some(stored) = stored else {
        return Ok((requested.clone(), raw_body.clone()));
    };
    let mut normalized = requested.clone();
    let mut normalized_raw = raw_body.clone();
    let raw_cues = normalized_raw
        .get_mut("cues")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid("Cuelist body cues must be an array"))?;
    for (requested_cue, raw_cue) in normalized.cues.iter_mut().zip(raw_cues.iter_mut()) {
        let Some(stored_cue) = stored
            .typed
            .cues
            .iter()
            .find(|candidate| candidate.id == requested_cue.id)
        else {
            continue;
        };
        let raw_cue = raw_cue
            .as_object_mut()
            .ok_or_else(|| invalid("Cuelist cues must be objects"))?;
        preserve_optional_millis(
            raw_cue,
            "out_fade_millis",
            &mut requested_cue.out_fade_millis,
            stored_cue.out_fade_millis,
        );
        preserve_optional_millis(
            raw_cue,
            "out_delay_millis",
            &mut requested_cue.out_delay_millis,
            stored_cue.out_delay_millis,
        );
    }
    Ok((normalized, normalized_raw))
}

fn preserve_optional_millis(
    raw_cue: &mut serde_json::Map<String, Value>,
    field: &str,
    requested: &mut Option<u64>,
    stored: Option<u64>,
) {
    if raw_cue.contains_key(field) {
        return;
    }
    *requested = stored;
    if let Some(value) = stored {
        raw_cue.insert(field.to_owned(), Value::from(value));
    }
}

fn configure_slot(
    document: &PortableShowDocument,
    command: &PlaybackTopologyCommand,
    address: (u8, u8),
    expected: (u64, u64),
    expected_ids: (Option<&str>, Option<&str>),
    requested: &PlaybackDefinition,
) -> Result<PreparedActiveShowTransaction<PreparedTopology>, ActionError> {
    validate_page_slot(address)?;
    let (page_number, slot) = address;
    let page = find_page(document, page_number)?;
    validate_identity(
        page.as_ref(),
        expected_ids.0,
        "Playback Page",
        document.revision().value(),
    )?;
    validate_revision(
        page.as_ref(),
        expected.0,
        "Playback Page",
        document.revision().value(),
    )?;
    let number = page
        .as_ref()
        .and_then(|page| page.typed.slots.get(&slot).copied())
        .map_or_else(|| next_playback_number(document), Ok)?;
    let playback = find_playback(document, number)?;
    validate_identity(
        playback.as_ref(),
        expected_ids.1,
        "Playback",
        document.revision().value(),
    )?;
    validate_revision(
        playback.as_ref(),
        expected.1,
        "Playback",
        document.revision().value(),
    )?;
    let mut normalized = requested.clone();
    normalized.number = number;
    validate_macro_target(document, &normalized)?;
    validate_timecode_target(document, &normalized)?;
    normalized.validate().map_err(invalid)?;
    let desired_page = configured_page(page.as_ref(), page_number, slot, number)?;
    let playback_changed = match playback.as_ref() {
        Some(stored) => !same_typed(&stored.typed, &normalized)?,
        None => true,
    };
    let page_changed = match page.as_ref() {
        Some(stored) => !same_typed(&stored.typed, &desired_page)?,
        None => true,
    };
    let resolution = PlaybackTopologyResolution::PageSlot {
        page: page_number,
        slot,
        playback_number: Some(number),
    };
    if !playback_changed && !page_changed {
        return Ok(no_change(
            document,
            command,
            resolution,
            vec![
                stored_projection(
                    ActiveShowObjectKind::Playback,
                    playback.as_ref().expect("unchanged Playback exists"),
                ),
                stored_projection(
                    ActiveShowObjectKind::PlaybackPage,
                    page.as_ref().expect("unchanged Page exists"),
                ),
            ],
        ));
    }
    let mut writes = Vec::with_capacity(2);
    if playback_changed {
        writes.push((
            ActiveShowObjectKind::Playback,
            playback_object_id(document, playback.as_ref(), number)?,
            typed_body(playback.as_ref(), &normalized)?,
        ));
    }
    if page_changed {
        writes.push((
            ActiveShowObjectKind::PlaybackPage,
            page_object_id(document, page.as_ref(), page_number)?,
            typed_body(page.as_ref(), &desired_page)?,
        ));
    }
    changed_configure(document, command, resolution, writes, playback, page)
}

fn validate_macro_target(
    document: &PortableShowDocument,
    playback: &PlaybackDefinition,
) -> Result<(), ActionError> {
    let PlaybackTarget::Macro { macro_id } = &playback.target else {
        return Ok(());
    };
    if find_macro(document, *macro_id)?.is_none() {
        return Err(not_found(format!("Macro {macro_id} does not exist")));
    }
    Ok(())
}

fn validate_timecode_target(
    document: &PortableShowDocument,
    playback: &PlaybackDefinition,
) -> Result<(), ActionError> {
    let PlaybackTarget::Timecode { timecode_id } = &playback.target else {
        return Ok(());
    };
    if find_timecode(document, *timecode_id)?.is_none() {
        return Err(not_found(format!(
            "Timecode {} does not exist",
            timecode_id.0
        )));
    }
    Ok(())
}

fn assign_group_master(
    document: &PortableShowDocument,
    command: &PlaybackTopologyCommand,
    group_object_id: &str,
    expected_group_revision: u64,
    address: &GroupMasterPlaybackAddress,
) -> Result<PreparedActiveShowTransaction<PreparedTopology>, ActionError> {
    let group = find_group(document, group_object_id)?
        .ok_or_else(|| not_found(format!("Group {group_object_id} does not exist")))?;
    validate_revision(
        Some(&group),
        expected_group_revision,
        "Group",
        document.revision().value(),
    )?;
    match address {
        GroupMasterPlaybackAddress::Physical {
            page,
            slot,
            expected_page_revision,
            expected_page_object_id,
            expected_playback_revision,
            expected_playback_object_id,
        } => assign_physical_group_master(
            document,
            command,
            &group,
            (*page, *slot),
            (*expected_page_revision, *expected_playback_revision),
            (
                expected_page_object_id.as_deref(),
                expected_playback_object_id.as_deref(),
            ),
        ),
        GroupMasterPlaybackAddress::Virtual {
            page,
            playback_number,
            expected_page_revision,
            expected_page_object_id,
        } => assign_virtual_group_master(
            document,
            command,
            &group,
            *page,
            *playback_number,
            *expected_page_revision,
            expected_page_object_id.as_deref(),
        ),
    }
}

fn assign_physical_group_master(
    document: &PortableShowDocument,
    command: &PlaybackTopologyCommand,
    group: &Stored<light_programmer::GroupDefinition>,
    address: (u8, u8),
    expected: (u64, u64),
    expected_ids: (Option<&str>, Option<&str>),
) -> Result<PreparedActiveShowTransaction<PreparedTopology>, ActionError> {
    validate_page_slot(address)?;
    let (page_number, slot) = address;
    let page = find_page(document, page_number)?;
    validate_identity(
        page.as_ref(),
        expected_ids.0,
        "Playback Page",
        document.revision().value(),
    )?;
    validate_revision(
        page.as_ref(),
        expected.0,
        "Playback Page",
        document.revision().value(),
    )?;
    let number = page
        .as_ref()
        .and_then(|page| page.typed.slots.get(&slot).copied())
        .map_or_else(|| next_playback_number(document), Ok)?;
    let playback = find_playback(document, number)?;
    validate_identity(
        playback.as_ref(),
        expected_ids.1,
        "Playback",
        document.revision().value(),
    )?;
    validate_revision(
        playback.as_ref(),
        expected.1,
        "Playback",
        document.revision().value(),
    )?;
    let target = PlaybackTarget::Group {
        group_id: group.object_id.clone(),
        initial_master: preserved_initial_master(
            playback.as_ref().map(|stored| &stored.typed.target),
            &group.object_id,
        ),
    };
    let desired_playback = playback.as_ref().map_or_else(
        || PlaybackDefinition {
            number,
            name: group.typed.name.clone(),
            buttons: PlaybackDefinition::default_buttons(&target),
            button_count: 3,
            fader: PlaybackFaderMode::Master,
            has_fader: true,
            footprint: light_playback::PlaybackFootprint::Normal,
            go_activates: true,
            auto_off: true,
            xfade_millis: 0,
            color: group
                .typed
                .color
                .clone()
                .unwrap_or_else(|| "#20c997".into()),
            flash_release: FlashReleaseMode::default(),
            protect_from_swap: false,
            presentation_icon: group.typed.icon.clone(),
            presentation_image: None,
            target: target.clone(),
        },
        |stored| {
            let mut desired = stored.typed.clone();
            desired.target = target.clone();
            desired.reset_incompatible_layout();
            desired
        },
    );
    desired_playback.validate().map_err(invalid)?;
    let desired_page = configured_page(page.as_ref(), page_number, slot, number)?;
    let playback_changed = match playback.as_ref() {
        Some(stored) => !same_typed(&stored.typed, &desired_playback)?,
        None => true,
    };
    let page_changed = match page.as_ref() {
        Some(stored) => !same_typed(&stored.typed, &desired_page)?,
        None => true,
    };
    let resolution = PlaybackTopologyResolution::PageSlot {
        page: page_number,
        slot,
        playback_number: Some(number),
    };
    if !playback_changed && !page_changed {
        return Ok(no_change(
            document,
            command,
            resolution,
            vec![
                stored_projection(
                    ActiveShowObjectKind::Playback,
                    playback.as_ref().expect("unchanged Playback exists"),
                ),
                stored_projection(
                    ActiveShowObjectKind::PlaybackPage,
                    page.as_ref().expect("unchanged Page exists"),
                ),
            ],
        ));
    }
    let mut writes = Vec::with_capacity(2);
    if playback_changed {
        writes.push((
            ActiveShowObjectKind::Playback,
            playback_object_id(document, playback.as_ref(), number)?,
            typed_body(playback.as_ref(), &desired_playback)?,
        ));
    }
    if page_changed {
        writes.push((
            ActiveShowObjectKind::PlaybackPage,
            page_object_id(document, page.as_ref(), page_number)?,
            typed_body(page.as_ref(), &desired_page)?,
        ));
    }
    changed_configure(document, command, resolution, writes, playback, page)
}

fn assign_virtual_group_master(
    document: &PortableShowDocument,
    command: &PlaybackTopologyCommand,
    group: &Stored<light_programmer::GroupDefinition>,
    page_number: u8,
    playback_number: u16,
    expected_page_revision: u64,
    expected_page_object_id: Option<&str>,
) -> Result<PreparedActiveShowTransaction<PreparedTopology>, ActionError> {
    light_playback::VirtualPlaybackAddress::new(page_number, playback_number).map_err(invalid)?;
    let stored = find_page(document, page_number)?;
    validate_identity(
        stored.as_ref(),
        expected_page_object_id,
        "Playback Page",
        document.revision().value(),
    )?;
    validate_revision(
        stored.as_ref(),
        expected_page_revision,
        "Playback Page",
        document.revision().value(),
    )?;
    let mut page = stored.as_ref().map_or_else(
        || light_playback::PlaybackPage {
            number: page_number,
            name: format!("Page {page_number}"),
            slots: std::collections::HashMap::new(),
            virtual_playbacks: std::collections::HashMap::new(),
        },
        |stored| stored.typed.clone(),
    );
    let target = PlaybackTarget::Group {
        group_id: group.object_id.clone(),
        initial_master: preserved_initial_master(
            page.virtual_playbacks
                .get(&playback_number)
                .map(|playback| &playback.target),
            &group.object_id,
        ),
    };
    let desired = page.virtual_playbacks.get(&playback_number).map_or_else(
        || PlaybackDefinition {
            number: playback_number,
            name: group.typed.name.clone(),
            buttons: PlaybackDefinition::default_buttons(&target),
            button_count: 1,
            fader: PlaybackFaderMode::Master,
            has_fader: false,
            footprint: light_playback::PlaybackFootprint::Normal,
            go_activates: true,
            auto_off: true,
            xfade_millis: 0,
            color: group
                .typed
                .color
                .clone()
                .unwrap_or_else(|| "#20c997".into()),
            flash_release: FlashReleaseMode::default(),
            protect_from_swap: false,
            presentation_icon: group.typed.icon.clone(),
            presentation_image: None,
            target: target.clone(),
        },
        |existing| {
            let mut desired = existing.clone();
            desired.target = target.clone();
            desired.reset_incompatible_layout();
            desired.has_fader = false;
            desired.button_count = 1;
            desired.buttons[1] = light_playback::PlaybackButtonAction::None;
            desired.buttons[2] = light_playback::PlaybackButtonAction::None;
            desired
        },
    );
    desired.validate().map_err(invalid)?;
    page.virtual_playbacks.insert(playback_number, desired);
    page.validate().map_err(invalid)?;
    let resolution = PlaybackTopologyResolution::Virtual {
        page: page_number,
        playback_number,
    };
    if let Some(stored) = stored.as_ref()
        && same_typed(&stored.typed, &page)?
    {
        return Ok(no_change(
            document,
            command,
            resolution,
            vec![stored_projection(
                ActiveShowObjectKind::PlaybackPage,
                stored,
            )],
        ));
    }
    let object_id = page_object_id(document, stored.as_ref(), page_number)?;
    let body = stored.as_ref().map_or_else(
        || serde_json::to_value(&page).map_err(invalid),
        |stored| {
            lossless_json::merge_typed(&stored.raw_body, &stored.typed, &page).map_err(invalid)
        },
    )?;
    changed_present(
        document,
        command,
        resolution,
        vec![(ActiveShowObjectKind::PlaybackPage, object_id, body)],
        Vec::new(),
    )
}

fn preserved_initial_master(target: Option<&PlaybackTarget>, group_id: &str) -> Option<f32> {
    match target {
        Some(PlaybackTarget::Group {
            group_id: existing_group_id,
            initial_master,
        }) if existing_group_id == group_id => *initial_master,
        _ => None,
    }
}

fn clear_mapped_playback(
    document: &PortableShowDocument,
    command: &PlaybackTopologyCommand,
    address: (u8, u8),
    expected: (u64, u64),
    expected_ids: (Option<&str>, Option<&str>),
) -> Result<PreparedActiveShowTransaction<PreparedTopology>, ActionError> {
    validate_page_slot(address)?;
    let (page_number, slot) = address;
    let page = find_page(document, page_number)?;
    validate_identity(
        page.as_ref(),
        expected_ids.0,
        "Playback Page",
        document.revision().value(),
    )?;
    validate_revision(
        page.as_ref(),
        expected.0,
        "Playback Page",
        document.revision().value(),
    )?;
    let Some(number) = page
        .as_ref()
        .and_then(|value| value.typed.slots.get(&slot).copied())
    else {
        validate_identity::<PlaybackDefinition>(
            None,
            expected_ids.1,
            "Playback",
            document.revision().value(),
        )?;
        validate_revision::<PlaybackDefinition>(
            None,
            expected.1,
            "Playback",
            document.revision().value(),
        )?;
        let objects = page
            .as_ref()
            .map(|value| stored_projection(ActiveShowObjectKind::PlaybackPage, value))
            .into_iter()
            .collect();
        return Ok(no_change(
            document,
            command,
            PlaybackTopologyResolution::PageSlot {
                page: page_number,
                slot,
                playback_number: None,
            },
            objects,
        ));
    };
    let playback = find_playback(document, number)?;
    validate_identity(
        playback.as_ref(),
        expected_ids.1,
        "Playback",
        document.revision().value(),
    )?;
    validate_revision(
        playback.as_ref(),
        expected.1,
        "Playback",
        document.revision().value(),
    )?;
    let playback = playback.ok_or_else(|| not_found("mapped Playback does not exist"))?;
    let mut writes = Vec::new();
    for stored in pages(document)? {
        if stored.typed.slots.values().any(|value| *value == number) {
            let mut desired = stored.typed.clone();
            desired.slots.retain(|_, value| *value != number);
            writes.push((
                ActiveShowObjectKind::PlaybackPage,
                stored.object_id,
                lossless_json::merge_typed(&stored.raw_body, &stored.typed, &desired)
                    .map_err(invalid)?,
            ));
        }
    }
    changed_present(
        document,
        command,
        PlaybackTopologyResolution::PageSlot {
            page: page_number,
            slot,
            playback_number: Some(number),
        },
        writes,
        vec![(
            ActiveShowObjectKind::Playback,
            playback.object_id,
            next_revision(playback.object_revision)?,
        )],
    )
}

fn cue_list_body(
    stored: Option<&Stored<CueList>>,
    requested: &CueList,
    raw_body: &Value,
) -> Result<Value, ActionError> {
    let mut merged = lossless_json::merge_typed_request(
        stored.map(|value| &value.raw_body),
        stored.map(|value| &value.typed),
        raw_body,
        requested,
        requested,
    )
    .map_err(invalid)?;
    lossless_json::strip_zero_u64_echo(&mut merged, "chaser_xfade_millis");
    strip_inherited_out_timing_nulls(&mut merged);
    Ok(merged)
}

fn strip_inherited_out_timing_nulls(body: &mut Value) {
    let Some(cues) = body.get_mut("cues").and_then(Value::as_array_mut) else {
        return;
    };
    for cue in cues {
        let Some(cue) = cue.as_object_mut() else {
            continue;
        };
        for field in ["out_fade_millis", "out_delay_millis"] {
            if cue.get(field).is_some_and(Value::is_null) {
                cue.remove(field);
            }
        }
    }
}

fn typed_body<T: serde::Serialize>(
    stored: Option<&Stored<T>>,
    desired: &T,
) -> Result<Value, ActionError> {
    match stored {
        Some(stored) => {
            lossless_json::merge_typed(&stored.raw_body, &stored.typed, desired).map_err(invalid)
        }
        None => serde_json::to_value(desired).map_err(invalid),
    }
}
