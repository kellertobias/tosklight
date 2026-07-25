use super::{
    PlaybackTopologyCommand, PlaybackTopologyResolution,
    change::{PreparedTopology, changed_present, no_change},
    page::configured_page,
    stored::{
        find_page, find_playback, invalid, not_found, page_object_id, stored_projection,
        validate_identity, validate_revision,
    },
};
use crate::active_show::PreparedActiveShowTransaction;
use crate::{ActionError, ActiveShowObjectKind, lossless_json};
use light_playback::{
    MAX_PAGE_SLOTS, MAX_PLAYBACK_PAGES, MAX_PLAYBACKS, PlaybackPage, PlaybackTarget,
};
use light_show::PortableShowDocument;

pub(super) fn map_existing_playback(
    document: &PortableShowDocument,
    command: &PlaybackTopologyCommand,
    address: (u8, u8),
    playback_number: u16,
    expected: (u64, u64),
    expected_ids: (Option<&str>, Option<&str>),
) -> Result<PreparedActiveShowTransaction<PreparedTopology>, ActionError> {
    validate_address(address, playback_number)?;
    let page = validated_page(document, address.0, expected.0, expected_ids.0)?;
    validated_playback(document, playback_number, expected.1, expected_ids.1)?;
    let resolution = PlaybackTopologyResolution::PageSlot {
        page: address.0,
        slot: address.1,
        playback_number: Some(playback_number),
    };
    if page
        .as_ref()
        .is_some_and(|stored| stored.typed.slots.get(&address.1) == Some(&playback_number))
    {
        return Ok(no_change(
            document,
            command,
            resolution,
            vec![stored_projection(
                ActiveShowObjectKind::PlaybackPage,
                page.as_ref().expect("unchanged Page exists"),
            )],
        ));
    }
    let desired = configured_page(page.as_ref(), address.0, address.1, playback_number)?;
    let object_id = page_object_id(document, page.as_ref(), address.0)?;
    let body = page_body(page.as_ref(), &desired)?;
    changed_present(
        document,
        command,
        resolution,
        vec![(ActiveShowObjectKind::PlaybackPage, object_id, body)],
        Vec::new(),
    )
}

fn validated_page(
    document: &PortableShowDocument,
    number: u8,
    expected_revision: u64,
    expected_object_id: Option<&str>,
) -> Result<Option<super::stored::Stored<PlaybackPage>>, ActionError> {
    let page = find_page(document, number)?;
    validate_identity(
        page.as_ref(),
        expected_object_id,
        "Playback Page",
        document.revision().value(),
    )?;
    validate_revision(
        page.as_ref(),
        expected_revision,
        "Playback Page",
        document.revision().value(),
    )?;
    if let Some(stored) = page.as_ref() {
        stored.typed.validate().map_err(invalid)?;
    }
    Ok(page)
}

fn validated_playback(
    document: &PortableShowDocument,
    number: u16,
    expected_revision: u64,
    expected_object_id: Option<&str>,
) -> Result<(), ActionError> {
    let playback = find_playback(document, number)?;
    validate_identity(
        playback.as_ref(),
        expected_object_id,
        "Playback",
        document.revision().value(),
    )?;
    validate_revision(
        playback.as_ref(),
        expected_revision,
        "Playback",
        document.revision().value(),
    )?;
    let playback = playback.ok_or_else(|| not_found("Cuelist Playback does not exist"))?;
    playback.typed.validate().map_err(invalid)?;
    if !matches!(playback.typed.target, PlaybackTarget::CueList { .. }) {
        return Err(invalid("Playback target must be a Cuelist"));
    }
    Ok(())
}

fn page_body(
    stored: Option<&super::stored::Stored<PlaybackPage>>,
    desired: &PlaybackPage,
) -> Result<serde_json::Value, ActionError> {
    match stored {
        Some(stored) => {
            lossless_json::merge_typed(&stored.raw_body, &stored.typed, desired).map_err(invalid)
        }
        None => serde_json::to_value(desired).map_err(invalid),
    }
}

fn validate_address((page, slot): (u8, u8), playback: u16) -> Result<(), ActionError> {
    if !(1..=MAX_PLAYBACK_PAGES).contains(&page) {
        return Err(invalid("page number must be within 1-127"));
    }
    if !(1..=MAX_PAGE_SLOTS).contains(&slot) {
        return Err(invalid("page slot must be within 1-127"));
    }
    if !(1..=MAX_PLAYBACKS).contains(&playback) {
        return Err(invalid("playback number must be within 1-1000"));
    }
    Ok(())
}
