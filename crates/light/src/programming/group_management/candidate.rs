use super::{
    GroupManagementActiveShowPorts, GroupManagementCommit, GroupManagementCommitResult,
    GroupManagementOperation, GroupManagementProjection, GroupManagementSelection,
    GroupPropertiesUpdate, GroupSourceExpectation, invalid, not_found,
};
use crate::active_show::PreparedActiveShowTransaction;
use crate::show_compiler::prepare_show_candidate_preserving_object;
use crate::{ActionError, ActionErrorKind, ActiveShowUnitOfWork, lossless_json};
use light_programmer::{FrozenGroup, GroupDefinition, resolve_group};
use light_show::{PortableShowDocument, PortableShowObject, PortableShowRevision};
use std::collections::HashMap;
use std::sync::Arc;

pub(crate) struct PreparedGroupManagement {
    pub(crate) result: GroupManagementCommitResult,
}

/// Resolves and validates the whole operation against one coherent open document.
pub(crate) fn prepare_group_management<P>(
    ports: &P,
    unit: &P::UnitOfWork,
    commit: &GroupManagementCommit,
) -> Result<PreparedActiveShowTransaction<PreparedGroupManagement>, ActionError>
where
    P: GroupManagementActiveShowPorts,
{
    let document = unit.document();
    validate_show(document, commit)?;
    let existing = document
        .object("group", &commit.group_id)
        .ok_or_else(|| not_found(format!("Group {} does not exist", commit.group_id)))?;
    validate_revision(existing, commit)?;
    if matches!(commit.operation(), GroupManagementOperation::Undo) {
        return prepare_undo(ports, unit, commit);
    }
    let groups = decode_groups(document)?;
    let current = groups
        .get(&commit.group_id)
        .ok_or_else(|| not_found(format!("Group {} does not exist", commit.group_id)))?;
    let (updated, selection) = apply_operation(document, commit, current, &groups)?;
    prepare_put(document, commit, existing, &updated, selection)
}

fn apply_operation(
    document: &PortableShowDocument,
    commit: &GroupManagementCommit,
    current: &GroupDefinition,
    groups: &HashMap<String, GroupDefinition>,
) -> Result<(GroupDefinition, Option<GroupManagementSelection>), ActionError> {
    match commit.operation() {
        GroupManagementOperation::UpdateProperties(update) => {
            Ok((updated_properties(current, update), None))
        }
        GroupManagementOperation::RefreshFrozen { expected_source } => {
            refresh_frozen(document, commit, current, groups, expected_source.as_ref())
        }
        GroupManagementOperation::DetachDerived { expected_source } => {
            detach_derived(commit, current, groups, expected_source.as_ref())
                .map(|group| (group, None))
        }
        GroupManagementOperation::Undo => Err(ActionError::new(
            ActionErrorKind::Internal,
            "Undo is prepared from adapter-owned object history",
        )),
    }
}

fn updated_properties(
    current: &GroupDefinition,
    update: &GroupPropertiesUpdate,
) -> GroupDefinition {
    let mut group = current.clone();
    group.name.clone_from(&update.name);
    group.color.clone_from(&update.color);
    group.icon.clone_from(&update.icon);
    group
}

fn refresh_frozen(
    document: &PortableShowDocument,
    commit: &GroupManagementCommit,
    current: &GroupDefinition,
    groups: &HashMap<String, GroupDefinition>,
    expected_source: Option<&GroupSourceExpectation>,
) -> Result<(GroupDefinition, Option<GroupManagementSelection>), ActionError> {
    let frozen = current
        .frozen_from
        .as_ref()
        .ok_or_else(|| invalid(format!("Group {} is not a frozen group", commit.group_id)))?;
    let source_group_id = frozen.source_group_id.clone();
    validate_source(document, groups, &source_group_id, expected_source)?;
    let fixtures = resolve_group(&source_group_id, groups).map_err(decode_error)?;
    let source_revision = document.revision().value();
    let mut group = current.clone();
    group.fixtures.clone_from(&fixtures);
    group.frozen_from = Some(FrozenGroup {
        source_group_id: source_group_id.clone(),
        source_revision,
        captured_at: chrono::Utc::now(),
    });
    let selection = GroupManagementSelection {
        source_group_id,
        source_revision,
        fixtures,
    };
    Ok((group, Some(selection)))
}

fn detach_derived(
    commit: &GroupManagementCommit,
    current: &GroupDefinition,
    groups: &HashMap<String, GroupDefinition>,
    expected_source: Option<&GroupSourceExpectation>,
) -> Result<GroupDefinition, ActionError> {
    let derived = current
        .derived_from
        .as_ref()
        .ok_or_else(|| invalid(format!("Group {} is not derived", commit.group_id)))?;
    let source_group_id = derived.source_group_id.clone();
    if let Some(expectation) = expected_source
        && expectation.source_group_id != source_group_id
    {
        return Err(source_conflict(&source_group_id));
    }
    // Resolve the currently materialized membership through the derivation chain so the detached
    // Group keeps exactly what the operator sees, in order.
    let fixtures = resolve_group(&commit.group_id, groups).map_err(decode_error)?;
    let mut group = current.clone();
    group.fixtures = fixtures;
    group.derived_from = None;
    Ok(group)
}

/// A frozen source must exist and match any declared identity/revision before anything mutates.
fn validate_source(
    document: &PortableShowDocument,
    groups: &HashMap<String, GroupDefinition>,
    source_group_id: &str,
    expected_source: Option<&GroupSourceExpectation>,
) -> Result<(), ActionError> {
    if !groups.contains_key(source_group_id) {
        return Err(not_found(format!(
            "source Group {source_group_id} does not exist"
        )));
    }
    let Some(expectation) = expected_source else {
        return Ok(());
    };
    if expectation.source_group_id != source_group_id {
        return Err(source_conflict(source_group_id));
    }
    let Some(expected_revision) = expectation.expected_source_revision else {
        return Ok(());
    };
    let current = document
        .object("group", source_group_id)
        .map_or(0, PortableShowObject::revision);
    if current == expected_revision {
        Ok(())
    } else {
        Err(
            ActionError::new(ActionErrorKind::Conflict, "stale source Group revision")
                .at_related_revision(current),
        )
    }
}

fn source_conflict(source_group_id: &str) -> ActionError {
    ActionError::new(
        ActionErrorKind::Conflict,
        format!("Group source is no longer {source_group_id}"),
    )
}

fn prepare_put(
    document: &PortableShowDocument,
    commit: &GroupManagementCommit,
    existing: &PortableShowObject,
    group: &GroupDefinition,
    selection: Option<GroupManagementSelection>,
) -> Result<PreparedActiveShowTransaction<PreparedGroupManagement>, ActionError> {
    let raw_body = merged_body(existing, group)?;
    if existing.body() == &raw_body {
        return Ok(PreparedActiveShowTransaction::NoChange(
            PreparedGroupManagement {
                result: completion(
                    document.revision(),
                    commit,
                    existing.revision(),
                    raw_body,
                    false,
                    selection,
                ),
            },
        ));
    }
    let mut transaction = document.transaction();
    transaction.put("group", commit.group_id.clone(), raw_body);
    let prepared =
        prepare_show_candidate_preserving_object(document, transaction, "group", &commit.group_id)?;
    let (show_revision, object_revision, raw_body) = candidate_group(document, &prepared, commit)?;
    Ok(PreparedActiveShowTransaction::PreparedCommit {
        prepared: Box::new(prepared),
        state: PreparedGroupManagement {
            result: completion(
                show_revision,
                commit,
                object_revision,
                raw_body,
                true,
                selection,
            ),
        },
    })
}

fn prepare_undo<P>(
    ports: &P,
    unit: &P::UnitOfWork,
    commit: &GroupManagementCommit,
) -> Result<PreparedActiveShowTransaction<PreparedGroupManagement>, ActionError>
where
    P: GroupManagementActiveShowPorts,
{
    let document = unit.document();
    let undo = ports.prepare_object_undo(
        unit,
        "group",
        &commit.group_id,
        commit.expected_object_revision,
    )?;
    let raw_body = undo.body().clone();
    let object_revision = commit
        .expected_object_revision
        .checked_add(1)
        .ok_or_else(|| revision_overflow(commit.expected_object_revision))?;
    let mut transaction = document.transaction();
    transaction.undo_object(undo);
    let prepared =
        prepare_show_candidate_preserving_object(document, transaction, "group", &commit.group_id)?;
    let (show_revision, _, _) = candidate_group(document, &prepared, commit)?;
    Ok(PreparedActiveShowTransaction::PreparedCommit {
        prepared: Box::new(prepared),
        state: PreparedGroupManagement {
            result: completion(show_revision, commit, object_revision, raw_body, true, None),
        },
    })
}

fn completion(
    show_revision: PortableShowRevision,
    commit: &GroupManagementCommit,
    object_revision: u64,
    raw_body: serde_json::Value,
    changed: bool,
    selection: Option<GroupManagementSelection>,
) -> GroupManagementCommitResult {
    GroupManagementCommitResult {
        changed,
        projection: GroupManagementProjection {
            show_id: commit.show_id,
            object_id: commit.group_id.clone(),
            object_revision,
            raw_body: Arc::new(raw_body),
        },
        show_revision,
        event_sequence: None,
        selection,
    }
}

fn validate_show(
    document: &PortableShowDocument,
    commit: &GroupManagementCommit,
) -> Result<(), ActionError> {
    if document.id() != commit.show_id {
        return Err(not_found("requested show is not active"));
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
    existing: &PortableShowObject,
    commit: &GroupManagementCommit,
) -> Result<(), ActionError> {
    if existing.revision() == commit.expected_object_revision {
        Ok(())
    } else {
        Err(
            ActionError::new(ActionErrorKind::Conflict, "stale Group object revision")
                .at_revision(existing.revision()),
        )
    }
}

fn decode_groups(
    document: &PortableShowDocument,
) -> Result<HashMap<String, GroupDefinition>, ActionError> {
    document
        .objects_of_kind("group")
        .map(|object| {
            let mut group = serde_json::from_value::<GroupDefinition>(object.body().clone())
                .map_err(decode_error)?;
            group.id = object.key().id().to_owned();
            Ok((group.id.clone(), group))
        })
        .collect()
}

/// Applies the typed change onto the retained raw JSON so unowned fields survive exactly.
fn merged_body(
    existing: &PortableShowObject,
    group: &GroupDefinition,
) -> Result<serde_json::Value, ActionError> {
    let mut before =
        serde_json::from_value::<GroupDefinition>(existing.body().clone()).map_err(decode_error)?;
    if existing.body().get("id").is_none() {
        before.id.clone_from(&group.id);
    }
    let before = serde_json::to_value(before).map_err(decode_error)?;
    let after = serde_json::to_value(group).map_err(decode_error)?;
    let mut merged = existing.body().clone();
    lossless_json::apply_delta(&mut merged, &before, &after);
    Ok(merged)
}

fn candidate_group(
    document: &PortableShowDocument,
    prepared: &crate::PreparedShowCandidate,
    commit: &GroupManagementCommit,
) -> Result<(PortableShowRevision, u64, serde_json::Value), ActionError> {
    let candidate = document
        .candidate(prepared.transaction())
        .map_err(decode_error)?;
    let object = candidate
        .object("group", &commit.group_id)
        .ok_or_else(|| ActionError::new(ActionErrorKind::Internal, "prepared Group is missing"))?;
    Ok((
        candidate.revision(),
        object.revision(),
        object.body().clone(),
    ))
}

fn decode_error(error: impl std::fmt::Display) -> ActionError {
    invalid(error.to_string())
}

fn revision_overflow(revision: u64) -> ActionError {
    ActionError::new(
        ActionErrorKind::Invalid,
        "Group object revision cannot be incremented",
    )
    .at_revision(revision)
}
