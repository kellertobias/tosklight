use super::{
    PlaybackTopologyAction, PlaybackTopologyCommand, PlaybackTopologyResolution,
    change::{PreparedTopology, changed_configure, changed_present, no_change},
    map_existing::map_existing_playback,
    page::configured_page,
    stored::{
        Stored, conflict, cue_list_object_id, find_cue_list, find_page, find_playback, invalid,
        next_playback_number, next_revision, not_found, page_object_id, pages, playback_object_id,
        same_typed, stored_projection, validate_identity, validate_revision,
    },
};
use crate::active_show::PreparedActiveShowTransaction;
use crate::{ActionError, ActiveShowObjectKind, lossless_json};
use light_playback::{CueList, MAX_PAGE_SLOTS, MAX_PLAYBACK_PAGES, PlaybackDefinition};
use light_show::PortableShowDocument;
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
    }
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
    let resolution = PlaybackTopologyResolution::CueList { cue_list_id };
    if let Some(existing) = stored.as_ref()
        && same_typed(&existing.typed, cue_list)?
    {
        return Ok(no_change(
            document,
            command,
            resolution,
            vec![stored_projection(ActiveShowObjectKind::CueList, existing)],
        ));
    }
    let object_id = cue_list_object_id(document, stored.as_ref(), cue_list_id)?;
    let body = cue_list_body(stored.as_ref(), cue_list, raw_body)?;
    changed_present(
        document,
        command,
        resolution,
        vec![(ActiveShowObjectKind::CueList, object_id, body)],
        Vec::new(),
    )
}

fn configure_slot(
    document: &PortableShowDocument,
    command: &PlaybackTopologyCommand,
    address: (u8, u8),
    expected: (u64, u64),
    expected_ids: (Option<&str>, Option<&str>),
    requested: &PlaybackDefinition,
) -> Result<PreparedActiveShowTransaction<PreparedTopology>, ActionError> {
    validate_address(address)?;
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

fn clear_mapped_playback(
    document: &PortableShowDocument,
    command: &PlaybackTopologyCommand,
    address: (u8, u8),
    expected: (u64, u64),
    expected_ids: (Option<&str>, Option<&str>),
) -> Result<PreparedActiveShowTransaction<PreparedTopology>, ActionError> {
    validate_address(address)?;
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
    lossless_json::merge_typed_request(
        stored.map(|value| &value.raw_body),
        stored.map(|value| &value.typed),
        raw_body,
        requested,
        requested,
    )
    .map_err(invalid)
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

fn validate_show(
    document: &PortableShowDocument,
    command: &PlaybackTopologyCommand,
    expected_revision: u64,
) -> Result<(), ActionError> {
    if document.id() != command.show_id {
        return Err(not_found("requested show is not active"));
    }
    if document.revision().value() == expected_revision {
        return Ok(());
    }
    Err(conflict("stale active-show revision").at_revision(document.revision().value()))
}

fn validate_address((page, slot): (u8, u8)) -> Result<(), ActionError> {
    if !(1..=MAX_PLAYBACK_PAGES).contains(&page) {
        return Err(invalid("page number must be within 1-127"));
    }
    if !(1..=MAX_PAGE_SLOTS).contains(&slot) {
        return Err(invalid("page slot must be within 1-127"));
    }
    Ok(())
}
