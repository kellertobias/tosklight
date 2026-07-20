use crate::{ActionError, ActionErrorKind, ActiveShowObjectKind, PlaybackTopologyObjectProjection};
use light_core::{CueListId, Revision};
use light_playback::{CueList, MAX_PLAYBACKS, PlaybackDefinition, PlaybackPage};
use light_show::{PortableShowCandidate, PortableShowDocument, PortableShowObject};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::{collections::HashSet, sync::Arc};

pub(super) struct Stored<T> {
    pub object_id: String,
    pub object_revision: Revision,
    pub raw_body: Value,
    pub typed: T,
}

pub(super) fn find_cue_list(
    document: &PortableShowDocument,
    id: CueListId,
) -> Result<Option<Stored<CueList>>, ActionError> {
    find_unique(document, "cue_list", |value: &CueList| value.id == id)
}

pub(super) fn find_playback(
    document: &PortableShowDocument,
    number: u16,
) -> Result<Option<Stored<PlaybackDefinition>>, ActionError> {
    find_unique(document, "playback", |value: &PlaybackDefinition| {
        value.number == number
    })
}

pub(super) fn find_page(
    document: &PortableShowDocument,
    number: u8,
) -> Result<Option<Stored<PlaybackPage>>, ActionError> {
    find_unique(document, "playback_page", |value: &PlaybackPage| {
        value.number == number
    })
}

pub(super) fn pages(
    document: &PortableShowDocument,
) -> Result<Vec<Stored<PlaybackPage>>, ActionError> {
    document
        .objects_of_kind("playback_page")
        .map(decode_stored)
        .collect()
}

pub(super) fn next_playback_number(document: &PortableShowDocument) -> Result<u16, ActionError> {
    let mut used = HashSet::new();
    for object in document.objects_of_kind("playback") {
        used.insert(decode::<PlaybackDefinition>(object)?.number);
    }
    for page in pages(document)? {
        used.extend(page.typed.slots.into_values());
    }
    (1..=MAX_PLAYBACKS)
        .find(|number| !used.contains(number))
        .ok_or_else(|| conflict("no free Playback number is available"))
}

pub(super) fn candidate_projection(
    candidate: &PortableShowCandidate<'_>,
    kind: ActiveShowObjectKind,
    object_id: &str,
) -> Result<PlaybackTopologyObjectProjection, ActionError> {
    let object = candidate
        .object(kind.as_str(), object_id)
        .ok_or_else(|| invalid("prepared topology object is absent"))?;
    Ok(PlaybackTopologyObjectProjection::Present {
        kind,
        object_id: object_id.to_owned(),
        object_revision: object.revision(),
        raw_body: Arc::new(object.body().clone()),
    })
}

pub(super) fn stored_projection<T>(
    kind: ActiveShowObjectKind,
    stored: &Stored<T>,
) -> PlaybackTopologyObjectProjection {
    PlaybackTopologyObjectProjection::Present {
        kind,
        object_id: stored.object_id.clone(),
        object_revision: stored.object_revision,
        raw_body: Arc::new(stored.raw_body.clone()),
    }
}

pub(super) fn validate_revision<T>(
    stored: Option<&Stored<T>>,
    expected: Revision,
    label: &str,
    show_revision: Revision,
) -> Result<(), ActionError> {
    let current = stored.map_or(0, |value| value.object_revision);
    if current == expected {
        return Ok(());
    }
    Err(conflict(format!("stale {label} revision"))
        .at_revision(show_revision)
        .at_related_revision(current))
}

pub(super) fn validate_identity<T>(
    stored: Option<&Stored<T>>,
    expected: Option<&str>,
    label: &str,
    show_revision: Revision,
) -> Result<(), ActionError> {
    let current = stored.map(|value| value.object_id.as_str());
    if current == expected {
        return Ok(());
    }
    Err(conflict(format!("stale {label} storage identity"))
        .at_revision(show_revision)
        .at_related_revision(stored.map_or(0, |value| value.object_revision)))
}

pub(super) fn next_revision(current: Revision) -> Result<Revision, ActionError> {
    current
        .checked_add(1)
        .ok_or_else(|| invalid("show object revision cannot be incremented").at_revision(current))
}

pub(super) fn same_typed<T: serde::Serialize>(left: &T, right: &T) -> Result<bool, ActionError> {
    Ok(serde_json::to_value(left).map_err(invalid)?
        == serde_json::to_value(right).map_err(invalid)?)
}

fn find_unique<T>(
    document: &PortableShowDocument,
    kind: &str,
    matches: impl Fn(&T) -> bool,
) -> Result<Option<Stored<T>>, ActionError>
where
    T: DeserializeOwned,
{
    let mut found = None;
    for object in document.objects_of_kind(kind) {
        let stored = decode_stored(object)?;
        if !matches(&stored.typed) {
            continue;
        }
        if found.is_some() {
            return Err(invalid(format!(
                "multiple {kind} objects share one identity"
            )));
        }
        found = Some(stored);
    }
    Ok(found)
}

fn decode_stored<T: DeserializeOwned>(
    object: &PortableShowObject,
) -> Result<Stored<T>, ActionError> {
    Ok(Stored {
        object_id: object.key().id().to_owned(),
        object_revision: object.revision(),
        raw_body: object.body().clone(),
        typed: decode(object)?,
    })
}

fn decode<T: DeserializeOwned>(object: &PortableShowObject) -> Result<T, ActionError> {
    serde_json::from_value(object.body().clone()).map_err(invalid)
}

pub(super) fn invalid(message: impl std::fmt::Display) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message.to_string())
}

pub(super) fn not_found(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::NotFound, message)
}

pub(super) fn conflict(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Conflict, message)
}
