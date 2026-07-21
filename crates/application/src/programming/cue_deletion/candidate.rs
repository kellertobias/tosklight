use super::{
    ProgrammingCueDeletionAuthority, ProgrammingCueDeletionExpectation,
    ProgrammingCueDeletionObjectProjection, ProgrammingDeletedCue, ResolvedCueDeletionRequest,
};
use crate::active_show::PreparedActiveShowTransaction;
use crate::programming::cue_list_resolution::{
    CueListAddress, ResolvedCueList, StoredCueList, exact_cue_list, resolve_cue_list,
};
use crate::{
    ActionError, ActionErrorKind, ActiveShowObjectChange, ActiveShowObjectKind, lossless_json,
    prepare_show_candidate,
};
use light_playback::{Cue, CueList};
use light_show::{PortableShowDocument, PortableShowRevision};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::sync::Arc;

pub(super) struct PreparedCueDeletion {
    pub show_id: light_core::ShowId,
    pub show_revision: PortableShowRevision,
    pub projection: ProgrammingCueDeletionObjectProjection,
    pub deleted_cue: ProgrammingDeletedCue,
    pub changes: Vec<ActiveShowObjectChange>,
}

pub(super) fn prepare_deletion(
    document: &PortableShowDocument,
    request: &ResolvedCueDeletionRequest,
    expected_show_revision: Option<u64>,
) -> Result<PreparedActiveShowTransaction<PreparedCueDeletion>, ActionError> {
    validate_request(request)?;
    validate_show(document, request)?;
    validate_show_revision(document, request, expected_show_revision)?;
    let resolved = resolve_cue_list(document, request.address)?;
    let stored = exact_resolved_cue_list(document, resolved, &request.expectation)?;
    let index = cue_index(&stored.typed, request.cue_number.value())?;
    let deleted = stored.typed.cues[index].clone();
    validate_deleted_cue(&deleted, &request.expectation)?;
    if stored.typed.cues.len() == 1 {
        return Err(invalid(
            "cannot delete the only Cue; delete the Cuelist from its configuration instead",
        ));
    }
    build_candidate(document, stored, index, deleted)
}

fn validate_request(request: &ResolvedCueDeletionRequest) -> Result<(), ActionError> {
    let cue = request.cue_number.value();
    if !cue.is_finite() || cue <= 0.0 {
        return Err(invalid("Cue number must be finite and greater than zero"));
    }
    match request.address {
        CueListAddress::Pool { playback_number }
            if !(1..=light_playback::MAX_PLAYBACKS).contains(&playback_number) =>
        {
            Err(invalid("playback number must be within 1-1000"))
        }
        CueListAddress::PageSlot { page, slot }
            if !(1..=light_playback::MAX_PLAYBACK_PAGES).contains(&page)
                || !(1..=light_playback::MAX_PAGE_SLOTS).contains(&slot) =>
        {
            Err(invalid("page and slot must be within 1-127"))
        }
        _ => validate_exact_authority(&request.expectation),
    }
}

fn validate_exact_authority(
    expectation: &ProgrammingCueDeletionExpectation,
) -> Result<(), ActionError> {
    let ProgrammingCueDeletionExpectation::Exact(authority) = expectation else {
        return Ok(());
    };
    if authority.cue_list_id.0.is_nil() || authority.cue_id.is_nil() {
        return Err(invalid("Cue deletion authority IDs must not be nil"));
    }
    if authority.object_id.trim().is_empty() || authority.object_id.len() > 256 {
        return Err(invalid("Cue deletion object_id must contain 1-256 bytes"));
    }
    Ok(())
}

fn build_candidate(
    document: &PortableShowDocument,
    stored: StoredCueList,
    index: usize,
    deleted: Cue,
) -> Result<PreparedActiveShowTransaction<PreparedCueDeletion>, ActionError> {
    let mut desired = stored.typed.clone();
    desired.cues.remove(index);
    let body = lossless_json::merge_typed(&stored.raw_body, &stored.typed, &desired)
        .map_err(|error| invalid(error.to_string()))?;
    let mut transaction = document.transaction();
    transaction.put("cue_list", &stored.object_id, body);
    let prepared = prepare_show_candidate(document, transaction)?;
    let (show_revision, projection) = {
        let candidate = document
            .candidate(prepared.transaction())
            .map_err(|error| invalid(error.to_string()))?;
        let object = candidate
            .object("cue_list", &stored.object_id)
            .ok_or_else(|| invalid("prepared Cuelist projection is missing"))?;
        let typed = decode::<CueList>(object.body(), "prepared Cuelist")?;
        (
            candidate.revision(),
            ProgrammingCueDeletionObjectProjection {
                cue_list_id: typed.id,
                object_id: stored.object_id,
                object_revision: object.revision(),
                raw_body: Arc::new(object.body().clone()),
            },
        )
    };
    let change = object_change(&projection);
    Ok(PreparedActiveShowTransaction::PreparedCommit {
        prepared: Box::new(prepared),
        state: PreparedCueDeletion {
            show_id: document.id(),
            show_revision,
            projection,
            deleted_cue: ProgrammingDeletedCue {
                id: deleted.id,
                number: crate::CueNumber::new(deleted.number),
            },
            changes: vec![change],
        },
    })
}

fn validate_show(
    document: &PortableShowDocument,
    request: &ResolvedCueDeletionRequest,
) -> Result<(), ActionError> {
    if document.id() == request.show_id {
        Ok(())
    } else {
        Err(conflict("Cue deletion Show authority changed"))
    }
}

fn validate_show_revision(
    document: &PortableShowDocument,
    request: &ResolvedCueDeletionRequest,
    expected: Option<u64>,
) -> Result<(), ActionError> {
    match request.expectation {
        ProgrammingCueDeletionExpectation::Current => Ok(()),
        ProgrammingCueDeletionExpectation::Exact(_) => match expected {
            Some(revision) if revision == document.revision().value() => Ok(()),
            Some(_) => Err(conflict_revision(
                "stale active-show revision",
                document.revision().value(),
            )),
            None => Err(invalid("Cue deletion requires an expected Show revision")),
        },
    }
}

fn exact_resolved_cue_list(
    document: &PortableShowDocument,
    resolved: ResolvedCueList,
    expectation: &ProgrammingCueDeletionExpectation,
) -> Result<StoredCueList, ActionError> {
    let ProgrammingCueDeletionExpectation::Exact(expected) = expectation else {
        return Ok(resolved.stored);
    };
    if resolved.playback_number != expected.playback_number
        || resolved.stored.typed.id != expected.cue_list_id
        || resolved.stored.object_id != expected.object_id
    {
        return Err(conflict("Cue deletion address authority changed"));
    }
    if resolved.stored.object_revision != expected.object_revision {
        return Err(conflict_related_revision(
            "the addressed Cuelist changed",
            resolved.stored.object_revision,
        ));
    }
    exact_cue_list(
        document,
        expected.cue_list_id,
        &expected.object_id,
        expected.object_revision,
    )
}

fn validate_deleted_cue(
    cue: &Cue,
    expectation: &ProgrammingCueDeletionExpectation,
) -> Result<(), ActionError> {
    if let ProgrammingCueDeletionExpectation::Exact(ProgrammingCueDeletionAuthority {
        cue_id, ..
    }) = expectation
        && cue.id != *cue_id
    {
        return Err(conflict("Cue deletion target identity changed"));
    }
    Ok(())
}

fn cue_index(list: &CueList, number: f64) -> Result<usize, ActionError> {
    list.cues
        .iter()
        .position(|cue| cue.number == number)
        .ok_or_else(|| missing(format!("cue {number} does not exist")))
}

fn object_change(projection: &ProgrammingCueDeletionObjectProjection) -> ActiveShowObjectChange {
    ActiveShowObjectChange {
        kind: ActiveShowObjectKind::CueList,
        object_id: projection.object_id.clone(),
        object_revision: projection.object_revision,
        body: Some(projection.raw_body.as_ref().clone()),
        deleted: false,
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
    conflict(message).at_revision(revision)
}

fn conflict_related_revision(message: impl Into<String>, revision: u64) -> ActionError {
    conflict(message).at_related_revision(revision)
}
