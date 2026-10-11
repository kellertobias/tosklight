use super::{
    PlaybackTopologyCommand, PlaybackTopologyObjectProjection, PlaybackTopologyResolution,
    stored::{candidate_projection, invalid},
};
use crate::active_show::PreparedActiveShowTransaction;
use crate::show_compiler::prepare_show_candidate_exact_transaction;
use crate::{ActionError, ActiveShowObjectChange, ActiveShowObjectKind};
use light_show::{PortableShowDocument, PortableShowRevision, PortableShowTransaction};
use serde_json::Value;
use std::sync::Arc;

pub(super) struct PreparedTopology {
    pub show_id: light_core::ShowId,
    pub show_revision: PortableShowRevision,
    pub resolution: PlaybackTopologyResolution,
    pub objects: Arc<[PlaybackTopologyObjectProjection]>,
    pub changes: Vec<ActiveShowObjectChange>,
}

pub(super) fn changed_present(
    document: &PortableShowDocument,
    command: &PlaybackTopologyCommand,
    resolution: PlaybackTopologyResolution,
    writes: Vec<(ActiveShowObjectKind, String, Value)>,
    deletes: Vec<(ActiveShowObjectKind, String, u64)>,
) -> Result<PreparedActiveShowTransaction<PreparedTopology>, ActionError> {
    let mut transaction = document.transaction();
    for (kind, id, body) in &writes {
        transaction.put(kind.as_str(), id, body.clone());
    }
    for (kind, id, _) in &deletes {
        transaction.delete(kind.as_str(), id);
    }
    let prepared = prepare_show_candidate_exact_transaction(document, transaction)?;
    let candidate = document
        .candidate(prepared.transaction())
        .map_err(invalid)?;
    let mut objects = Vec::with_capacity(writes.len() + deletes.len());
    for (kind, id, _) in &writes {
        objects.push(candidate_projection(&candidate, *kind, id)?);
    }
    objects.extend(deletes.iter().map(|(kind, id, revision)| {
        PlaybackTopologyObjectProjection::Deleted {
            kind: *kind,
            object_id: id.clone(),
            object_revision: *revision,
        }
    }));
    let changes = objects.iter().map(object_change).collect();
    Ok(PreparedActiveShowTransaction::PreparedCommit {
        state: PreparedTopology {
            show_id: command.show_id,
            show_revision: candidate.revision(),
            resolution,
            objects: objects.into(),
            changes,
        },
        prepared: Box::new(prepared),
    })
}

pub(super) fn changed_cue_list_history(
    document: &PortableShowDocument,
    command: &PlaybackTopologyCommand,
    resolution: PlaybackTopologyResolution,
    transaction: PortableShowTransaction,
    object_id: &str,
) -> Result<PreparedActiveShowTransaction<PreparedTopology>, ActionError> {
    let prepared = prepare_show_candidate_exact_transaction(document, transaction)?;
    let candidate = document
        .candidate(prepared.transaction())
        .map_err(invalid)?;
    let projection = candidate_projection(&candidate, ActiveShowObjectKind::CueList, object_id)?;
    let changes = vec![object_change(&projection)];
    Ok(PreparedActiveShowTransaction::PreparedCommit {
        state: PreparedTopology {
            show_id: command.show_id,
            show_revision: candidate.revision(),
            resolution,
            objects: vec![projection].into(),
            changes,
        },
        prepared: Box::new(prepared),
    })
}

pub(super) fn no_change(
    document: &PortableShowDocument,
    command: &PlaybackTopologyCommand,
    resolution: PlaybackTopologyResolution,
    objects: Vec<PlaybackTopologyObjectProjection>,
) -> PreparedActiveShowTransaction<PreparedTopology> {
    PreparedActiveShowTransaction::NoChange(PreparedTopology {
        show_id: command.show_id,
        show_revision: document.revision(),
        resolution,
        objects: objects.into(),
        changes: Vec::new(),
    })
}

fn object_change(projection: &PlaybackTopologyObjectProjection) -> ActiveShowObjectChange {
    match projection {
        PlaybackTopologyObjectProjection::Present {
            kind,
            object_id,
            object_revision,
            raw_body,
        } => ActiveShowObjectChange::present(
            *kind,
            object_id.clone(),
            *object_revision,
            raw_body.as_ref().clone(),
        )
        .expect("prepared Playback topology projection must match its typed family"),
        PlaybackTopologyObjectProjection::Deleted {
            kind,
            object_id,
            object_revision,
        } => ActiveShowObjectChange {
            kind: *kind,
            object_id: object_id.clone(),
            object_revision: *object_revision,
            body: None,
            deleted: true,
        },
    }
}
