use crate::{
    ActionError, ActionErrorKind, ProgrammingCueCommit, ProgrammingCuePageSlot,
    ProgrammingCueRecordTarget, ProgrammingCueResolvedTarget,
};
use light_core::CueListId;
use light_playback::{CueList, MAX_PLAYBACKS, PlaybackDefinition, PlaybackPage, PlaybackTarget};
use light_show::{PortableShowDocument, PortableShowObject};
use std::collections::HashSet;

#[derive(Clone)]
pub(super) struct Stored<T> {
    pub object_id: String,
    pub object_revision: u64,
    pub raw_body: serde_json::Value,
    pub typed: T,
}

pub(super) struct ResolvedCueTarget {
    pub cue_list: Option<Stored<CueList>>,
    pub playback: Option<Stored<PlaybackDefinition>>,
    pub page: Option<Stored<PlaybackPage>>,
    pub concrete_playback_number: Option<u16>,
    pub page_slot: Option<ProgrammingCuePageSlot>,
}

impl ResolvedCueTarget {
    pub fn creates_topology(&self) -> bool {
        self.cue_list.is_none()
    }
}

pub(super) fn resolve_target(
    document: &PortableShowDocument,
    commit: &ProgrammingCueCommit,
) -> Result<ResolvedCueTarget, ActionError> {
    match commit.environment().target {
        ProgrammingCueResolvedTarget::CueList { cue_list_id } => Ok(ResolvedCueTarget {
            cue_list: Some(required_cue_list(document, cue_list_id)?),
            playback: None,
            page: None,
            concrete_playback_number: None,
            page_slot: None,
        }),
        ProgrammingCueResolvedTarget::Playback {
            playback_number,
            page_slot,
        } => resolve_playback(document, commit, playback_number, page_slot),
        ProgrammingCueResolvedTarget::EmptyPageSlot(page_slot) => {
            resolve_empty_page_slot(document, page_slot)
        }
    }
}

fn resolve_playback(
    document: &PortableShowDocument,
    commit: &ProgrammingCueCommit,
    playback_number: u16,
    page_slot: Option<ProgrammingCuePageSlot>,
) -> Result<ResolvedCueTarget, ActionError> {
    let playback = find_playback(document, playback_number)?;
    if playback.is_none()
        && !matches!(
            commit.request().target,
            ProgrammingCueRecordTarget::Pool { .. } | ProgrammingCueRecordTarget::PageSlot { .. }
        )
    {
        return Err(not_found(format!(
            "Playback {playback_number} does not exist"
        )));
    }
    let cue_list = playback
        .as_ref()
        .map(|playback| cue_list_for_playback(document, playback))
        .transpose()?;
    let page = page_slot
        .map(|page_slot| verified_page(document, page_slot, playback_number))
        .transpose()?
        .flatten();
    Ok(ResolvedCueTarget {
        cue_list,
        playback,
        page,
        concrete_playback_number: Some(playback_number),
        page_slot,
    })
}

fn resolve_empty_page_slot(
    document: &PortableShowDocument,
    page_slot: ProgrammingCuePageSlot,
) -> Result<ResolvedCueTarget, ActionError> {
    let page = find_page(document, page_slot.page)?;
    if page
        .as_ref()
        .and_then(|page| page.typed.slots.get(&page_slot.slot))
        .is_some()
    {
        return Err(conflict(
            "Playback page slot changed while Cue recording was resolving",
        ));
    }
    let playback_number = allocate_playback(document)?;
    Ok(ResolvedCueTarget {
        cue_list: None,
        playback: None,
        page,
        concrete_playback_number: Some(playback_number),
        page_slot: Some(page_slot),
    })
}

fn required_cue_list(
    document: &PortableShowDocument,
    cue_list_id: CueListId,
) -> Result<Stored<CueList>, ActionError> {
    find_cue_list(document, cue_list_id)?
        .ok_or_else(|| not_found(format!("Cuelist {} does not exist", cue_list_id.0)))
}

fn cue_list_for_playback(
    document: &PortableShowDocument,
    playback: &Stored<PlaybackDefinition>,
) -> Result<Stored<CueList>, ActionError> {
    let PlaybackTarget::CueList { cue_list_id } = playback.typed.target else {
        return Err(invalid(format!(
            "Playback {} does not target a Cuelist",
            playback.typed.number
        )));
    };
    required_cue_list(document, cue_list_id)
}

fn verified_page(
    document: &PortableShowDocument,
    page_slot: ProgrammingCuePageSlot,
    playback_number: u16,
) -> Result<Option<Stored<PlaybackPage>>, ActionError> {
    let page = find_page(document, page_slot.page)?;
    let mapped = page
        .as_ref()
        .and_then(|page| page.typed.slots.get(&page_slot.slot))
        .copied();
    if mapped == Some(playback_number) {
        Ok(page)
    } else {
        Err(conflict(
            "Playback page slot changed while Cue recording was resolving",
        ))
    }
}

fn allocate_playback(document: &PortableShowDocument) -> Result<u16, ActionError> {
    let mut used = document
        .objects_of_kind("playback")
        .map(|object| decode_playback(object).map(|playback| playback.number))
        .collect::<Result<HashSet<_>, _>>()?;
    for object in document.objects_of_kind("playback_page") {
        used.extend(decode_page(object)?.slots.into_values());
    }
    (1..=MAX_PLAYBACKS)
        .find(|number| !used.contains(number))
        .ok_or_else(|| conflict("no free Playback number is available"))
}

fn find_cue_list(
    document: &PortableShowDocument,
    id: CueListId,
) -> Result<Option<Stored<CueList>>, ActionError> {
    find_unique(document, "cue_list", |object| {
        decode_cue_list(object).map(|value| (value.id == id).then_some(value))
    })
}

fn find_playback(
    document: &PortableShowDocument,
    number: u16,
) -> Result<Option<Stored<PlaybackDefinition>>, ActionError> {
    find_unique(document, "playback", |object| {
        decode_playback(object).map(|value| (value.number == number).then_some(value))
    })
}

fn find_page(
    document: &PortableShowDocument,
    number: u8,
) -> Result<Option<Stored<PlaybackPage>>, ActionError> {
    find_unique(document, "playback_page", |object| {
        decode_page(object).map(|value| (value.number == number).then_some(value))
    })
}

fn find_unique<T>(
    document: &PortableShowDocument,
    kind: &str,
    mut decode_match: impl FnMut(&PortableShowObject) -> Result<Option<T>, ActionError>,
) -> Result<Option<Stored<T>>, ActionError> {
    let mut found = None;
    for object in document.objects_of_kind(kind) {
        let Some(typed) = decode_match(object)? else {
            continue;
        };
        if found.is_some() {
            return Err(invalid(format!(
                "multiple {kind} objects share one identity"
            )));
        }
        found = Some(stored(object, typed));
    }
    Ok(found)
}

fn stored<T>(object: &PortableShowObject, typed: T) -> Stored<T> {
    Stored {
        object_id: object.key().id().to_owned(),
        object_revision: object.revision(),
        raw_body: object.body().clone(),
        typed,
    }
}

fn decode_cue_list(object: &PortableShowObject) -> Result<CueList, ActionError> {
    serde_json::from_value(object.body().clone()).map_err(invalid)
}

fn decode_playback(object: &PortableShowObject) -> Result<PlaybackDefinition, ActionError> {
    serde_json::from_value(object.body().clone()).map_err(invalid)
}

fn decode_page(object: &PortableShowObject) -> Result<PlaybackPage, ActionError> {
    serde_json::from_value(object.body().clone()).map_err(invalid)
}

fn invalid(message: impl std::fmt::Display) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message.to_string())
}

fn not_found(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::NotFound, message)
}

fn conflict(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Conflict, message)
}
