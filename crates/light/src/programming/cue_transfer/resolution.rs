use super::{
    CueTransferAuthority, ProgrammingCueTransferAddress, ProgrammingCueTransferChoiceRequest,
    ProgrammingCueTransferEndpoint, ResolvedCueTransferEndpoint,
};
use crate::programming::cue_list_resolution::{
    CueListAddress, StoredCueList, exact_cue_list as exact_stored_cue_list, resolve_cue_list,
};
use crate::{ActionError, ActionErrorKind, CueNumber};
use light_core::ShowId;
use light_playback::{Cue, CueList};
use light_programmer::CueTransferOperation;
use light_show::PortableShowDocument;
use uuid::Uuid;

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

pub(super) fn resolve_endpoint(
    document: &PortableShowDocument,
    endpoint: ProgrammingCueTransferEndpoint,
) -> Result<ResolvedCueTransferEndpoint, ActionError> {
    let resolved = resolve_cue_list(document, cue_list_address(endpoint.address))?;
    let playback_number = resolved.playback_number;
    let stored = resolved.stored;
    let cue_list_id = stored.typed.id;
    Ok(ResolvedCueTransferEndpoint {
        requested: endpoint,
        playback_number,
        cue_list_id,
        object_id: stored.object_id,
        object_revision: stored.object_revision,
        cue_id: None,
    })
}

fn cue_list_address(address: ProgrammingCueTransferAddress) -> CueListAddress {
    match address {
        ProgrammingCueTransferAddress::Pool { playback_number } => {
            CueListAddress::Pool { playback_number }
        }
        ProgrammingCueTransferAddress::PageSlot { page, slot } => {
            CueListAddress::PageSlot { page, slot }
        }
    }
}

pub(super) fn exact_cue_list(
    document: &PortableShowDocument,
    endpoint: &ResolvedCueTransferEndpoint,
) -> Result<StoredCueList, ActionError> {
    exact_stored_cue_list(
        document,
        endpoint.cue_list_id,
        &endpoint.object_id,
        endpoint.object_revision,
    )
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
