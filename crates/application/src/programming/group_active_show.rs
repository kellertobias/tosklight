use super::{
    ProgrammingGroupActiveShowPorts, ProgrammingGroupCommit, ProgrammingGroupCommitResult,
    ProgrammingGroupProjection, ProgrammingGroupRevisionExpectation,
};
use crate::active_show::{CompletedActiveShowTransaction, PreparedActiveShowTransaction};
use crate::show_compiler::prepare_show_candidate_preserving_object;
use crate::{
    ActionContext, ActionError, ActionErrorKind, ActiveShowObjectChange, ActiveShowObjectKind,
    ActiveShowObjectsChange, ActiveShowService, EventBus, EventDraft, lossless_json,
    prepare_show_candidate,
};
use light_programmer::{GroupDefinition, group_delete_blocker};
use light_show::{PortableShowDocument, PortableShowObject, PortableShowRevision};
use std::collections::HashMap;
use std::sync::Arc;

impl ActiveShowService {
    pub fn commit_programming_group<P>(
        &self,
        context: &ActionContext,
        commit: &ProgrammingGroupCommit,
        ports: &P,
    ) -> Result<ProgrammingGroupCommitResult, ActionError>
    where
        P: ProgrammingGroupActiveShowPorts,
    {
        self.transact(
            context,
            commit.show_id,
            ports,
            "record-group",
            |document| prepare_recording(document, commit),
            complete_recording,
        )
    }
}

fn prepare_recording(
    document: &PortableShowDocument,
    commit: &ProgrammingGroupCommit,
) -> Result<PreparedActiveShowTransaction<PreparedRecording>, ActionError> {
    validate_show(document, commit)?;
    let groups = decode_groups(document)?;
    let existing_object = document.object("group", &commit.group_id);
    validate_revision(existing_object, commit)?;
    let existing_group = groups.get(&commit.group_id);
    if commit.deletes_target() {
        return prepare_delete(document, commit, existing_object, &groups);
    }
    let group = commit
        .updated_group(existing_group, &groups)?
        .ok_or_else(|| {
            ActionError::new(
                ActionErrorKind::Internal,
                "non-delete Group recording did not produce a body",
            )
        })?;
    prepare_put(document, commit, existing_object, &group)
}

fn prepare_put(
    document: &PortableShowDocument,
    commit: &ProgrammingGroupCommit,
    existing_object: Option<&PortableShowObject>,
    group: &GroupDefinition,
) -> Result<PreparedActiveShowTransaction<PreparedRecording>, ActionError> {
    let raw_body = merged_body(existing_object, group)?;
    let current_revision = existing_object.map_or(0, PortableShowObject::revision);
    if existing_object.is_some_and(|object| object.body() == &raw_body) {
        return Ok(no_change(
            document,
            commit,
            current_revision,
            Some(raw_body),
        ));
    }
    let mut transaction = document.transaction();
    transaction.put("group", commit.group_id.clone(), raw_body);
    let prepared =
        prepare_show_candidate_preserving_object(document, transaction, "group", &commit.group_id)?;
    let (show_revision, object_revision, raw_body) = candidate_group(document, &prepared, commit)?;
    Ok(PreparedActiveShowTransaction::PreparedCommit {
        prepared: Box::new(prepared),
        state: PreparedRecording {
            result: completion(show_revision, commit, object_revision, Some(raw_body), true),
        },
    })
}

fn prepare_delete(
    document: &PortableShowDocument,
    commit: &ProgrammingGroupCommit,
    existing: Option<&PortableShowObject>,
    groups: &HashMap<String, GroupDefinition>,
) -> Result<PreparedActiveShowTransaction<PreparedRecording>, ActionError> {
    let existing = existing.ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::NotFound,
            format!("Group {} does not exist", commit.group_id),
        )
    })?;
    if let Some(dependent) = group_delete_blocker(&commit.group_id, groups) {
        return Err(ActionError::new(
            ActionErrorKind::Conflict,
            format!(
                "cannot delete Group {}; derived Group {dependent} depends on it",
                commit.group_id
            ),
        ));
    }
    let object_revision = existing
        .revision()
        .checked_add(1)
        .ok_or_else(|| revision_overflow(existing.revision()))?;
    let mut transaction = document.transaction();
    transaction.delete("group", commit.group_id.clone());
    let prepared = prepare_show_candidate(document, transaction)?;
    let show_revision = prepared_show_revision(document, &prepared, commit, true)?;
    Ok(PreparedActiveShowTransaction::PreparedCommit {
        prepared: Box::new(prepared),
        state: PreparedRecording {
            result: completion(show_revision, commit, object_revision, None, true),
        },
    })
}

fn no_change(
    document: &PortableShowDocument,
    commit: &ProgrammingGroupCommit,
    object_revision: u64,
    raw_body: Option<serde_json::Value>,
) -> PreparedActiveShowTransaction<PreparedRecording> {
    PreparedActiveShowTransaction::NoChange(PreparedRecording {
        result: completion(
            document.revision(),
            commit,
            object_revision,
            raw_body,
            false,
        ),
    })
}

fn complete_recording<P: ProgrammingGroupActiveShowPorts>(
    events: &EventBus,
    _ports: &P,
    context: &ActionContext,
    completed: CompletedActiveShowTransaction<PreparedRecording>,
) -> ProgrammingGroupCommitResult {
    let mut result = completed.state.result;
    let Some(commit) = completed.commit else {
        return result;
    };
    result.show_revision = commit.revision();
    let change = ActiveShowObjectChange {
        kind: ActiveShowObjectKind::Group,
        object_id: result.projection.object_id.clone(),
        object_revision: result.projection.object_revision,
        body: result
            .projection
            .raw_body
            .as_deref()
            .map(serde_json::Value::clone),
        deleted: result.projection.deleted,
    };
    result.event_sequence = Some(
        events
            .publish(EventDraft::active_show_objects_changed(
                context,
                ActiveShowObjectsChange {
                    show_id: result.projection.show_id,
                    show_revision: result.show_revision,
                    changes: vec![change],
                },
            ))
            .sequence,
    );
    result
}

struct PreparedRecording {
    result: ProgrammingGroupCommitResult,
}

fn completion(
    show_revision: PortableShowRevision,
    commit: &ProgrammingGroupCommit,
    object_revision: u64,
    raw_body: Option<serde_json::Value>,
    changed: bool,
) -> ProgrammingGroupCommitResult {
    let deleted = raw_body.is_none();
    ProgrammingGroupCommitResult {
        changed,
        projection: ProgrammingGroupProjection {
            show_id: commit.show_id,
            object_id: commit.group_id.clone(),
            object_revision,
            raw_body: raw_body.map(Arc::new),
            deleted,
        },
        show_revision,
        event_sequence: None,
    }
}

fn validate_show(
    document: &PortableShowDocument,
    commit: &ProgrammingGroupCommit,
) -> Result<(), ActionError> {
    if document.id() != commit.show_id {
        return Err(ActionError::new(
            ActionErrorKind::NotFound,
            "requested show is not active",
        ));
    }
    if let Some(expected) = commit.expected_show_revision
        && expected != document.revision()
    {
        return Err(
            ActionError::new(ActionErrorKind::Conflict, "stale active-show revision")
                .at_related_revision(document.revision().value()),
        );
    }
    Ok(())
}

fn validate_revision(
    existing: Option<&PortableShowObject>,
    commit: &ProgrammingGroupCommit,
) -> Result<(), ActionError> {
    let current = existing.map_or(0, PortableShowObject::revision);
    match commit.expected_object_revision {
        ProgrammingGroupRevisionExpectation::Current => Ok(()),
        ProgrammingGroupRevisionExpectation::Exact(expected) if expected == current => Ok(()),
        ProgrammingGroupRevisionExpectation::Exact(_) => Err(ActionError::new(
            ActionErrorKind::Conflict,
            "stale Group object revision",
        )
        .at_revision(current)),
    }
}

fn decode_groups(
    document: &PortableShowDocument,
) -> Result<HashMap<String, GroupDefinition>, ActionError> {
    document
        .objects_of_kind("group")
        .map(|object| {
            let mut group = serde_json::from_value::<GroupDefinition>(object.body().clone())
                .map_err(invalid)?;
            group.id = object.key().id().to_owned();
            Ok((group.id.clone(), group))
        })
        .collect()
}

fn merged_body(
    existing: Option<&PortableShowObject>,
    group: &GroupDefinition,
) -> Result<serde_json::Value, ActionError> {
    let Some(object) = existing else {
        return serde_json::to_value(group).map_err(invalid);
    };
    let mut before =
        serde_json::from_value::<GroupDefinition>(object.body().clone()).map_err(invalid)?;
    if object.body().get("id").is_none() {
        before.id.clone_from(&group.id);
    }
    let before = serde_json::to_value(before).map_err(invalid)?;
    let after = serde_json::to_value(group).map_err(invalid)?;
    let mut merged = object.body().clone();
    lossless_json::apply_delta(&mut merged, &before, &after);
    Ok(merged)
}

fn candidate_group(
    document: &PortableShowDocument,
    prepared: &crate::PreparedShowCandidate,
    commit: &ProgrammingGroupCommit,
) -> Result<(PortableShowRevision, u64, serde_json::Value), ActionError> {
    let candidate = document
        .candidate(prepared.transaction())
        .map_err(invalid)?;
    let object = candidate
        .object("group", &commit.group_id)
        .ok_or_else(|| ActionError::new(ActionErrorKind::Internal, "prepared Group is missing"))?;
    Ok((
        candidate.revision(),
        object.revision(),
        object.body().clone(),
    ))
}

fn prepared_show_revision(
    document: &PortableShowDocument,
    prepared: &crate::PreparedShowCandidate,
    commit: &ProgrammingGroupCommit,
    expect_deleted: bool,
) -> Result<PortableShowRevision, ActionError> {
    let candidate = document
        .candidate(prepared.transaction())
        .map_err(invalid)?;
    if expect_deleted && candidate.object("group", &commit.group_id).is_some() {
        return Err(ActionError::new(
            ActionErrorKind::Internal,
            "prepared Group deletion retained the target",
        ));
    }
    Ok(candidate.revision())
}

fn revision_overflow(revision: u64) -> ActionError {
    ActionError::new(
        ActionErrorKind::Invalid,
        "Group object revision cannot be incremented",
    )
    .at_revision(revision)
}

fn invalid(error: impl std::fmt::Display) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, error.to_string())
}

#[cfg(test)]
#[path = "group_active_show_tests.rs"]
mod tests;
