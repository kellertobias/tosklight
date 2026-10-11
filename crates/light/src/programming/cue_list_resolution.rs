//! Shared portable-Show resolution for Cue actions that address a Cuelist through Playback.

use crate::{ActionError, ActionErrorKind};
use light_core::CueListId;
use light_playback::{CueList, PlaybackAddress, PlaybackPage, PlaybackTarget};
use light_show::{PortableShowDocument, PortableShowObject};
use serde::de::DeserializeOwned;
use serde_json::Value;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::programming) enum CueListAddress {
    Playback { address: PlaybackAddress },
    CuelistPool { number: u16 },
    CueList { id: CueListId },
}

#[derive(Clone)]
pub(in crate::programming) struct StoredCueList {
    pub object_id: String,
    pub object_revision: u64,
    pub raw_body: Value,
    pub typed: CueList,
}

pub(in crate::programming) struct ResolvedCueList {
    pub playback_address: Option<PlaybackAddress>,
    pub stored: StoredCueList,
}

pub(in crate::programming) fn resolve_cue_list(
    document: &PortableShowDocument,
    address: CueListAddress,
) -> Result<ResolvedCueList, ActionError> {
    match address {
        CueListAddress::CuelistPool { number } => {
            let id = crate::cuelist_pool_catalog(document)?
                .resolve(number)
                .ok_or_else(|| missing(format!("Cuelist {number} does not exist")))?;
            Ok(ResolvedCueList {
                playback_address: None,
                stored: find_cue_list(document, id)?,
            })
        }
        CueListAddress::CueList { id } => Ok(ResolvedCueList {
            playback_address: None,
            stored: find_cue_list(document, id)?,
        }),
        CueListAddress::Playback { address } => {
            let page = find_page(document, address.page())?;
            let playback = page.assignments.get(&address.position()).ok_or_else(|| {
                missing(format!(
                    "Playback {}.{} is not recorded",
                    address.page(),
                    address.position()
                ))
            })?;
            let PlaybackTarget::CueList { cue_list_id } = playback.target else {
                return Err(invalid(format!(
                    "Playback {}.{} does not contain Cues",
                    address.page(),
                    address.position()
                )));
            };
            Ok(ResolvedCueList {
                playback_address: Some(address),
                stored: find_cue_list(document, cue_list_id)?,
            })
        }
    }
}

pub(in crate::programming) fn exact_cue_list(
    document: &PortableShowDocument,
    cue_list_id: CueListId,
    object_id: &str,
    object_revision: u64,
) -> Result<StoredCueList, ActionError> {
    let object = document
        .object("cue_list", object_id)
        .ok_or_else(|| conflict("the resolved Cuelist no longer exists"))?;
    if object.revision() != object_revision {
        return Err(conflict_revision(
            "the resolved Cuelist changed",
            object.revision(),
        ));
    }
    let typed = decode::<CueList>(object.body(), "Cuelist")?;
    if typed.id != cue_list_id {
        return Err(conflict("the resolved Cuelist identity changed"));
    }
    Ok(StoredCueList {
        object_id: object_id.to_owned(),
        object_revision: object.revision(),
        raw_body: object.body().clone(),
        typed,
    })
}

fn find_cue_list(
    document: &PortableShowDocument,
    cue_list_id: CueListId,
) -> Result<StoredCueList, ActionError> {
    let canonical = cue_list_id.0.to_string();
    if let Some(object) = document.object("cue_list", &canonical) {
        let typed = decode::<CueList>(object.body(), "Cuelist")?;
        if typed.id != cue_list_id {
            return Err(invalid(
                "stored Cuelist identity does not match its object key",
            ));
        }
        return Ok(stored_cue_list(object, typed));
    }
    find_cue_list_by_semantic_id(document, cue_list_id)
}

fn find_cue_list_by_semantic_id(
    document: &PortableShowDocument,
    cue_list_id: CueListId,
) -> Result<StoredCueList, ActionError> {
    let mut found = None;
    for object in document.objects_of_kind("cue_list") {
        let typed = decode::<CueList>(object.body(), "Cuelist")?;
        if typed.id == cue_list_id && found.replace(stored_cue_list(object, typed)).is_some() {
            return Err(invalid(
                "multiple stored Cuelists share the requested semantic identity",
            ));
        }
    }
    found.ok_or_else(|| missing(format!("Cuelist {} does not exist", cue_list_id.0)))
}

fn stored_cue_list(object: &PortableShowObject, typed: CueList) -> StoredCueList {
    StoredCueList {
        object_id: object.key().id().to_owned(),
        object_revision: object.revision(),
        raw_body: object.body().clone(),
        typed,
    }
}

fn find_page(document: &PortableShowDocument, number: u8) -> Result<PlaybackPage, ActionError> {
    find_unique(document, "playback_page", "Playback page", |object| {
        let value = decode::<PlaybackPage>(object.body(), "Playback page")?;
        Ok((value.number == number).then_some(value))
    })?
    .ok_or_else(|| missing(format!("page {number} does not exist")))
}

fn find_unique<T>(
    document: &PortableShowDocument,
    kind: &str,
    label: &str,
    mut candidate: impl FnMut(&PortableShowObject) -> Result<Option<T>, ActionError>,
) -> Result<Option<T>, ActionError> {
    let mut found = None;
    for object in document.objects_of_kind(kind) {
        if let Some(value) = candidate(object)?
            && found.replace(value).is_some()
        {
            return Err(invalid(format!(
                "multiple {label} objects share one address"
            )));
        }
    }
    Ok(found)
}

fn decode<T: DeserializeOwned>(value: &Value, label: &str) -> Result<T, ActionError> {
    serde_json::from_value(value.clone())
        .map_err(|error| invalid(format!("invalid {label}: {error}")))
}

fn invalid(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}

fn missing(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::NotFound, message)
}

fn conflict(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Conflict, message)
}

fn conflict_revision(message: impl Into<String>, revision: u64) -> ActionError {
    conflict(message).at_revision(revision)
}
