use super::{
    CueTransferAuthority, ProgrammingCueTransferAddress, ProgrammingCueTransferChoiceRequest,
    ProgrammingCueTransferEndpoint, ResolvedCueTransferEndpoint,
};
use crate::{ActionError, ActionErrorKind, CueNumber};
use light_core::{CueListId, ShowId};
use light_playback::{Cue, CueList, PlaybackDefinition, PlaybackPage, PlaybackTarget};
use light_programmer::CueTransferOperation;
use light_show::{PortableShowDocument, PortableShowObject};
use serde::de::DeserializeOwned;
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone)]
pub(super) struct StoredCueList {
    pub object_id: String,
    pub object_revision: u64,
    pub raw_body: Value,
    pub typed: CueList,
}

pub(super) fn resolve_choice(
    document: &PortableShowDocument,
    request: &ProgrammingCueTransferChoiceRequest,
) -> Result<CueTransferAuthority, ActionError> {
    validate_show(document, request.show_id)?;
    validate_endpoint(request.source)?;
    validate_endpoint(request.destination)?;
    let mut source = resolve_endpoint(document, request.source)?;
    let destination = resolve_endpoint(document, request.destination)?;
    let source_list = exact_cue_list(document, &source)?;
    let source_cue = cue_at_number(&source_list.typed, request.source.cue_number)?;
    source.cue_id = Some(source_cue.id);
    let destination_list = exact_cue_list(document, &destination)?;
    ensure_destination_available(&destination_list.typed, request.destination.cue_number)?;
    reject_sole_cue_cross_list_move(request.operation, &source, &destination, &source_list)?;
    Ok(CueTransferAuthority {
        choice_id: Uuid::new_v4(),
        show_id: request.show_id,
        show_revision: document.revision(),
        operation: request.operation,
        source,
        destination,
    })
}

fn reject_sole_cue_cross_list_move(
    operation: CueTransferOperation,
    source: &ResolvedCueTransferEndpoint,
    destination: &ResolvedCueTransferEndpoint,
    source_list: &StoredCueList,
) -> Result<(), ActionError> {
    if operation == CueTransferOperation::Move
        && source.object_id != destination.object_id
        && source_list.typed.cues.len() == 1
    {
        return Err(invalid(
            "cannot move the only Cue out of a Cuelist; delete the Cuelist from its configuration instead",
        ));
    }
    Ok(())
}

pub(super) fn validate_authority(
    document: &PortableShowDocument,
    authority: &CueTransferAuthority,
) -> Result<(), ActionError> {
    validate_show(document, authority.show_id)?;
    if document.revision() != authority.show_revision {
        return Err(conflict_revision(
            "the Show changed after the Cue transfer choice was prepared",
            document.revision().value(),
        ));
    }
    let source = resolve_endpoint(document, authority.source.requested)?;
    let destination = resolve_endpoint(document, authority.destination.requested)?;
    if !same_endpoint(&source, &authority.source)
        || !same_endpoint(&destination, &authority.destination)
    {
        return Err(conflict("Cue transfer source or destination changed"));
    }
    Ok(())
}

fn same_endpoint(
    current: &ResolvedCueTransferEndpoint,
    expected: &ResolvedCueTransferEndpoint,
) -> bool {
    current.playback_number == expected.playback_number
        && current.cue_list_id == expected.cue_list_id
        && current.object_id == expected.object_id
        && current.object_revision == expected.object_revision
}

fn resolve_endpoint(
    document: &PortableShowDocument,
    endpoint: ProgrammingCueTransferEndpoint,
) -> Result<ResolvedCueTransferEndpoint, ActionError> {
    let playback_number = resolve_playback_number(document, endpoint.address)?;
    let playback = find_playback(document, playback_number)?;
    let PlaybackTarget::CueList { cue_list_id } = playback.target else {
        return Err(invalid(format!(
            "Cuelist {playback_number} does not contain Cues"
        )));
    };
    let stored = find_cue_list(document, cue_list_id)?;
    Ok(ResolvedCueTransferEndpoint {
        requested: endpoint,
        playback_number,
        cue_list_id,
        object_id: stored.object_id,
        object_revision: stored.object_revision,
        cue_id: None,
    })
}

fn resolve_playback_number(
    document: &PortableShowDocument,
    address: ProgrammingCueTransferAddress,
) -> Result<u16, ActionError> {
    match address {
        ProgrammingCueTransferAddress::Pool { playback_number } => Ok(playback_number),
        ProgrammingCueTransferAddress::PageSlot { page, slot } => {
            let page = find_page(document, page)?;
            page.slots
                .get(&slot)
                .copied()
                .ok_or_else(|| missing(format!("page {} slot {slot} is not assigned", page.number)))
        }
    }
}

pub(super) fn exact_cue_list(
    document: &PortableShowDocument,
    endpoint: &ResolvedCueTransferEndpoint,
) -> Result<StoredCueList, ActionError> {
    let object = document
        .object("cue_list", &endpoint.object_id)
        .ok_or_else(|| conflict("the resolved Cuelist no longer exists"))?;
    if object.revision() != endpoint.object_revision {
        return Err(conflict_revision(
            "the resolved Cuelist changed",
            object.revision(),
        ));
    }
    let typed = decode::<CueList>(object.body(), "Cuelist")?;
    if typed.id != endpoint.cue_list_id {
        return Err(conflict("the resolved Cuelist identity changed"));
    }
    Ok(StoredCueList {
        object_id: endpoint.object_id.clone(),
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

fn find_playback(
    document: &PortableShowDocument,
    number: u16,
) -> Result<PlaybackDefinition, ActionError> {
    find_unique(document, "playback", "Playback", |object| {
        let value = decode::<PlaybackDefinition>(object.body(), "Playback")?;
        Ok((value.number == number).then_some(value))
    })?
    .ok_or_else(|| missing(format!("Cuelist {number} does not exist")))
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

pub(super) fn source_position(
    list: &CueList,
    endpoint: &ResolvedCueTransferEndpoint,
) -> Result<usize, ActionError> {
    let position = list
        .cues
        .iter()
        .position(|cue| cue.number == endpoint.requested.cue_number.value())
        .ok_or_else(|| missing("source Cue does not exist"))?;
    if endpoint
        .cue_id
        .is_some_and(|cue_id| list.cues[position].id != cue_id)
    {
        return Err(conflict("source Cue identity changed"));
    }
    Ok(position)
}

fn cue_at_number(list: &CueList, number: CueNumber) -> Result<&Cue, ActionError> {
    list.cues
        .iter()
        .find(|cue| cue.number == number.value())
        .ok_or_else(|| missing(format!("cue {} does not exist", number.value())))
}

pub(super) fn ensure_destination_available(
    list: &CueList,
    number: CueNumber,
) -> Result<(), ActionError> {
    if list.cues.iter().any(|cue| cue.number == number.value()) {
        Err(conflict("destination cue already exists"))
    } else {
        Ok(())
    }
}

fn validate_endpoint(endpoint: ProgrammingCueTransferEndpoint) -> Result<(), ActionError> {
    let number = endpoint.cue_number.value();
    if !number.is_finite() || number <= 0.0 {
        return Err(invalid("Cue number must be finite and greater than zero"));
    }
    validate_address(endpoint.address)
}

fn validate_address(address: ProgrammingCueTransferAddress) -> Result<(), ActionError> {
    match address {
        ProgrammingCueTransferAddress::Pool { playback_number }
            if !(1..=light_playback::MAX_PLAYBACKS).contains(&playback_number) =>
        {
            Err(invalid("playback number must be within 1-1000"))
        }
        ProgrammingCueTransferAddress::PageSlot { page, slot }
            if !(1..=light_playback::MAX_PLAYBACK_PAGES).contains(&page)
                || !(1..=light_playback::MAX_PAGE_SLOTS).contains(&slot) =>
        {
            Err(invalid("page and slot must be within 1-127"))
        }
        _ => Ok(()),
    }
}

fn validate_show(document: &PortableShowDocument, show_id: ShowId) -> Result<(), ActionError> {
    if document.id() == show_id {
        Ok(())
    } else {
        Err(conflict("Cue transfer Show authority changed"))
    }
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
    ActionError::new(ActionErrorKind::Conflict, message).at_revision(revision)
}
