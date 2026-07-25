use super::{
    PlaybackTopologyCommand,
    stored::{conflict, invalid, not_found},
};
use crate::ActionError;
use light_playback::{MAX_PAGE_SLOTS, MAX_PLAYBACK_PAGES};
use light_show::PortableShowDocument;

pub(super) fn validate_show(
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

pub(super) fn validate_page(number: u8) -> Result<(), ActionError> {
    if (1..=MAX_PLAYBACK_PAGES).contains(&number) {
        return Ok(());
    }
    Err(invalid("page number must be within 1-127"))
}

pub(super) fn validate_page_slot((page, slot): (u8, u8)) -> Result<(), ActionError> {
    validate_page(page)?;
    if (1..=MAX_PAGE_SLOTS).contains(&slot) {
        return Ok(());
    }
    Err(invalid("page slot must be within 1-127"))
}
