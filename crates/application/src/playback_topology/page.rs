use super::stored::{Stored, invalid};
use crate::ActionError;
use light_playback::PlaybackPage;
use std::collections::HashMap;

pub(super) fn configured_page(
    stored: Option<&Stored<PlaybackPage>>,
    page: u8,
    slot: u8,
    playback: u16,
) -> Result<PlaybackPage, ActionError> {
    let mut desired = stored.map_or_else(
        || PlaybackPage {
            number: page,
            name: format!("Page {page}"),
            slots: HashMap::new(),
        },
        |stored| stored.typed.clone(),
    );
    desired.number = page;
    desired.slots.insert(slot, playback);
    desired.validate().map_err(invalid)?;
    Ok(desired)
}
