use super::{
    PlaybackTopologyCommand, PlaybackTopologyObjectProjection, PlaybackTopologyResolution,
    stored::{Stored, candidate_projection, invalid, stored_projection},
};
use crate::active_show::PreparedActiveShowTransaction;
use crate::{ActionError, ActiveShowObjectChange, ActiveShowObjectKind, prepare_show_candidate};
use light_playback::{PlaybackDefinition, PlaybackPage};
use light_show::{PortableShowDocument, PortableShowRevision};
use serde_json::Value;
use std::sync::Arc;

pub(super) struct PreparedTopology {
    pub show_id: light_core::ShowId,
    pub show_revision: PortableShowRevision,
    pub resolution: PlaybackTopologyResolution,
    pub objects: Arc<[PlaybackTopologyObjectProjection]>,
    pub changes: Vec<ActiveShowObjectChange>,
}

pub(super) fn changed_configure(
    document: &PortableShowDocument,
    command: &PlaybackTopologyCommand,
    resolution: PlaybackTopologyResolution,
    writes: Vec<(ActiveShowObjectKind, String, Value)>,
    playback: Option<Stored<PlaybackDefinition>>,
    page: Option<Stored<PlaybackPage>>,
) -> Result<PreparedActiveShowTransaction<PreparedTopology>, ActionError> {
    let mut prepared = changed_present(document, command, resolution, writes, Vec::new())?;
    let state = match &mut prepared {
        PreparedActiveShowTransaction::PreparedCommit { state, .. } => state,
        PreparedActiveShowTransaction::NoChange(_) => unreachable!("configure has a change"),
    };
    ensure_present_projection(
        &mut state.objects,
        ActiveShowObjectKind::Playback,
        playback.as_ref(),
    );
    ensure_present_projection(
        &mut state.objects,
        ActiveShowObjectKind::PlaybackPage,
        page.as_ref(),
    );
    Ok(prepared)
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
    let prepared = prepare_show_candidate(document, transaction)?;
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

fn ensure_present_projection<T>(
    objects: &mut Arc<[PlaybackTopologyObjectProjection]>,
    kind: ActiveShowObjectKind,
    stored: Option<&Stored<T>>,
) {
    if objects.iter().any(|object| object.kind() == kind) {
        return;
    }
    let Some(stored) = stored else {
        debug_assert!(
            false,
            "new configured object must be among changed projections"
        );
        return;
    };
    let mut expanded = objects.to_vec();
    expanded.push(stored_projection(kind, stored));
    expanded.sort_by_key(|projection| projection.kind() != ActiveShowObjectKind::Playback);
    *objects = expanded.into();
}

fn object_change(projection: &PlaybackTopologyObjectProjection) -> ActiveShowObjectChange {
    match projection {
        PlaybackTopologyObjectProjection::Present {
            kind,
            object_id,
            object_revision,
            raw_body,
        } => ActiveShowObjectChange {
            kind: *kind,
            object_id: object_id.clone(),
            object_revision: *object_revision,
            body: Some(raw_body.as_ref().clone()),
            deleted: false,
        },
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
