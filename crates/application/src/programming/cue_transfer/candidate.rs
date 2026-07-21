use super::{
    CueTransferAuthority, ProgrammingCueTransferChoiceRequest, ProgrammingCueTransferMode,
    ProgrammingCueTransferObjectProjection, ProgrammingCueTransferSummary,
    resolution::{
        StoredCueList, ensure_destination_available, exact_cue_list, resolve_choice,
        source_position, validate_authority,
    },
};
use crate::active_show::PreparedActiveShowTransaction;
use crate::{
    ActionError, ActionErrorKind, ActiveShowObjectChange, ActiveShowObjectKind, CueNumber,
    lossless_json, prepare_show_candidate,
};
use light_core::ShowId;
use light_playback::{Cue, CueList};
use light_programmer::CueTransferOperation;
use light_show::{PortableShowDocument, PortableShowRevision};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

pub(super) struct PreparedCueTransfer {
    pub show_id: ShowId,
    pub show_revision: PortableShowRevision,
    pub summary: ProgrammingCueTransferSummary,
    pub projections: Arc<[ProgrammingCueTransferObjectProjection]>,
    pub changes: Vec<ActiveShowObjectChange>,
}

pub(super) fn prepare_transfer(
    document: &PortableShowDocument,
    authority: &CueTransferAuthority,
    mode: ProgrammingCueTransferMode,
) -> Result<PreparedActiveShowTransaction<PreparedCueTransfer>, ActionError> {
    validate_authority(document, authority)?;
    let source = exact_cue_list(document, &authority.source)?;
    let destination = exact_cue_list(document, &authority.destination)?;
    let source_index = source_position(&source.typed, &authority.source)?;
    ensure_destination_available(
        &destination.typed,
        authority.destination.requested.cue_number,
    )?;
    build_candidate(document, authority, mode, source, destination, source_index)
}

pub(super) fn prepare_current_transfer(
    document: &PortableShowDocument,
    request: &ProgrammingCueTransferChoiceRequest,
    mode: ProgrammingCueTransferMode,
) -> Result<PreparedActiveShowTransaction<PreparedCueTransfer>, ActionError> {
    let authority = resolve_choice(document, request)?;
    prepare_transfer(document, &authority, mode)
}

fn build_candidate(
    document: &PortableShowDocument,
    authority: &CueTransferAuthority,
    mode: ProgrammingCueTransferMode,
    source: StoredCueList,
    destination: StoredCueList,
    source_index: usize,
) -> Result<PreparedActiveShowTransaction<PreparedCueTransfer>, ActionError> {
    let source_cue = source.typed.cues[source_index].clone();
    let mut transferred = light_playback::transferred_cue(
        &source.typed,
        source_index,
        authority.destination.requested.cue_number.value(),
        playback_mode(mode),
    )
    .map_err(invalid)?;
    if authority.operation == CueTransferOperation::Copy {
        transferred.id = Uuid::new_v4();
    }
    let writes = transfer_writes(
        authority,
        &source,
        &destination,
        source_index,
        &source_cue,
        &transferred,
    )?;
    prepared_candidate(document, authority, mode, source_cue, transferred, writes)
}

fn transfer_writes(
    authority: &CueTransferAuthority,
    source: &StoredCueList,
    destination: &StoredCueList,
    source_index: usize,
    source_cue: &Cue,
    transferred: &Cue,
) -> Result<Vec<(StoredCueList, CueList, Option<TransferredRaw>)>, ActionError> {
    if source.object_id == destination.object_id {
        return same_list_write(authority, source, source_index, source_cue, transferred);
    }
    cross_list_writes(
        authority,
        source,
        destination,
        source_index,
        source_cue,
        transferred,
    )
}

fn same_list_write(
    authority: &CueTransferAuthority,
    source: &StoredCueList,
    source_index: usize,
    source_cue: &Cue,
    transferred: &Cue,
) -> Result<Vec<(StoredCueList, CueList, Option<TransferredRaw>)>, ActionError> {
    let mut desired = source.typed.clone();
    if authority.operation == CueTransferOperation::Move {
        desired.cues.remove(source_index);
    }
    desired.cues.push(transferred.clone());
    sort_cues(&mut desired);
    Ok(vec![(
        source.clone(),
        desired,
        Some(transferred_raw(
            source,
            source_index,
            source_cue,
            transferred,
        )?),
    )])
}

fn cross_list_writes(
    authority: &CueTransferAuthority,
    source: &StoredCueList,
    destination: &StoredCueList,
    source_index: usize,
    source_cue: &Cue,
    transferred: &Cue,
) -> Result<Vec<(StoredCueList, CueList, Option<TransferredRaw>)>, ActionError> {
    let mut writes = Vec::with_capacity(2);
    if authority.operation == CueTransferOperation::Move {
        if source.typed.cues.len() == 1 {
            return Err(invalid(
                "cannot move the only Cue out of a Cuelist; delete the Cuelist from its configuration instead",
            ));
        }
        let mut desired_source = source.typed.clone();
        desired_source.cues.remove(source_index);
        writes.push((source.clone(), desired_source, None));
    }
    let mut desired_destination = destination.typed.clone();
    desired_destination.cues.push(transferred.clone());
    sort_cues(&mut desired_destination);
    writes.push((
        destination.clone(),
        desired_destination,
        Some(transferred_raw(
            source,
            source_index,
            source_cue,
            transferred,
        )?),
    ));
    Ok(writes)
}

struct TransferredRaw {
    source_raw: Value,
    source_typed: Cue,
    destination_id: Uuid,
    destination_number: f64,
}

fn transferred_raw(
    source: &StoredCueList,
    source_index: usize,
    source_cue: &Cue,
    transferred: &Cue,
) -> Result<TransferredRaw, ActionError> {
    let source_raw = source
        .raw_body
        .get("cues")
        .and_then(Value::as_array)
        .and_then(|cues| cues.get(source_index))
        .cloned()
        .ok_or_else(|| invalid("stored source Cue is missing its ordered JSON entry"))?;
    Ok(TransferredRaw {
        source_raw,
        source_typed: source_cue.clone(),
        destination_id: transferred.id,
        destination_number: transferred.number,
    })
}

fn prepared_candidate(
    document: &PortableShowDocument,
    authority: &CueTransferAuthority,
    mode: ProgrammingCueTransferMode,
    source_cue: Cue,
    transferred: Cue,
    writes: Vec<(StoredCueList, CueList, Option<TransferredRaw>)>,
) -> Result<PreparedActiveShowTransaction<PreparedCueTransfer>, ActionError> {
    let mut transaction = document.transaction();
    for (stored, desired, transfer) in &writes {
        transaction.put(
            "cue_list",
            &stored.object_id,
            lossless_body(stored, desired, transfer.as_ref())?,
        );
    }
    let prepared = prepare_show_candidate(document, transaction)?;
    let candidate = document
        .candidate(prepared.transaction())
        .map_err(store_error)?;
    let projections = writes
        .iter()
        .map(|(stored, _, _)| candidate_projection(&candidate, stored))
        .collect::<Result<Vec<_>, _>>()?;
    let changes = projections.iter().map(object_change).collect();
    Ok(PreparedActiveShowTransaction::PreparedCommit {
        state: PreparedCueTransfer {
            show_id: authority.show_id,
            show_revision: candidate.revision(),
            summary: ProgrammingCueTransferSummary {
                operation: authority.operation,
                mode,
                source_cue_id: source_cue.id,
                source_cue_number: CueNumber::new(source_cue.number),
                destination_cue_id: transferred.id,
                destination_cue_number: CueNumber::new(transferred.number),
            },
            projections: projections.into(),
            changes,
        },
        prepared: Box::new(prepared),
    })
}

fn lossless_body(
    stored: &StoredCueList,
    desired: &CueList,
    transfer: Option<&TransferredRaw>,
) -> Result<Value, ActionError> {
    let mut body = lossless_json::merge_typed(&stored.raw_body, &stored.typed, desired)
        .map_err(|error| invalid(error.to_string()))?;
    let raw_cues = body
        .get_mut("cues")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| invalid("prepared Cuelist is missing its Cues"))?;
    for (index, cue) in desired.cues.iter().enumerate() {
        raw_cues[index] = lossless_cue(stored, cue, transfer)?;
    }
    Ok(body)
}

fn lossless_cue(
    stored: &StoredCueList,
    desired: &Cue,
    transfer: Option<&TransferredRaw>,
) -> Result<Value, ActionError> {
    if let Some(transfer) = transfer
        && desired.id == transfer.destination_id
        && desired.number == transfer.destination_number
    {
        return lossless_json::merge_typed(&transfer.source_raw, &transfer.source_typed, desired)
            .map_err(|error| invalid(error.to_string()));
    }
    let Some((index, before)) = stored
        .typed
        .cues
        .iter()
        .enumerate()
        .find(|(_, cue)| cue.id == desired.id && cue.number == desired.number)
    else {
        return serde_json::to_value(desired).map_err(|error| invalid(error.to_string()));
    };
    let raw = stored.raw_body["cues"]
        .as_array()
        .and_then(|cues| cues.get(index))
        .ok_or_else(|| invalid("stored Cue is missing its ordered JSON entry"))?;
    lossless_json::merge_typed(raw, before, desired).map_err(|error| invalid(error.to_string()))
}

fn candidate_projection(
    candidate: &light_show::PortableShowCandidate<'_>,
    stored: &StoredCueList,
) -> Result<ProgrammingCueTransferObjectProjection, ActionError> {
    let object = candidate
        .object("cue_list", &stored.object_id)
        .ok_or_else(|| invalid("prepared Cuelist projection is missing"))?;
    let cue_list = decode::<CueList>(object.body(), "prepared Cuelist")?;
    Ok(ProgrammingCueTransferObjectProjection {
        cue_list_id: cue_list.id,
        object_id: stored.object_id.clone(),
        object_revision: object.revision(),
        raw_body: Arc::new(object.body().clone()),
    })
}

fn object_change(projection: &ProgrammingCueTransferObjectProjection) -> ActiveShowObjectChange {
    ActiveShowObjectChange {
        kind: ActiveShowObjectKind::CueList,
        object_id: projection.object_id.clone(),
        object_revision: projection.object_revision,
        body: Some(projection.raw_body.as_ref().clone()),
        deleted: false,
    }
}

fn sort_cues(list: &mut CueList) {
    list.cues
        .sort_by(|left, right| left.number.total_cmp(&right.number));
}

const fn playback_mode(mode: ProgrammingCueTransferMode) -> light_playback::CueTransferMode {
    match mode {
        ProgrammingCueTransferMode::Plain => light_playback::CueTransferMode::Plain,
        ProgrammingCueTransferMode::Status => light_playback::CueTransferMode::Status,
    }
}

fn decode<T: DeserializeOwned>(value: &Value, label: &str) -> Result<T, ActionError> {
    serde_json::from_value(value.clone())
        .map_err(|error| invalid(format!("invalid {label}: {error}")))
}

fn store_error(error: light_show::StoreError) -> ActionError {
    invalid(error.to_string())
}

fn invalid(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, message)
}
