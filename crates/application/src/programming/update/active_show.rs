use super::{
    AtomicUpdatePlan, PlannedUpdateObject, ProgrammingUpdateCommand, ProgrammingUpdateMenuInput,
    ProgrammingUpdateObjectReference, ProgrammingUpdateOutcome, ProgrammingUpdatePorts,
    ProgrammingUpdatePreviewRequest, ProgrammingUpdatePreviewResult, ProgrammingUpdateProjection,
    ProgrammingUpdateTargetRequest, ProgrammingUpdateTargetsRequest,
    ProgrammingUpdateTargetsResult, UpdateMode, plan_cue_update, plan_group_update,
    plan_preset_update, preview_cue_update, preview_group_update, preview_preset_update,
};
use crate::active_show::{CompletedActiveShowTransaction, PreparedActiveShowTransaction};
use crate::{
    ActionContext, ActionError, ActiveShowObjectChange, ActiveShowObjectKind,
    ActiveShowObjectsChange, ActiveShowService, EventBus, EventDraft, lossless_json,
    prepare_show_candidate,
};
use light_playback::CueList;
use light_programmer::{GroupDefinition, Preset, ProgrammerUpdateContent, resolve_group};
use light_show::{PortableShowDocument, PortableShowObject, PortableShowRevision};
use std::sync::Arc;

use super::resolution::{
    find_cue_list, find_group, find_preset, invalid, resolve_requested_cue, update_error,
    validate_show_revision,
};

impl ActiveShowService {
    pub(crate) fn programming_update_targets<P: ProgrammingUpdatePorts>(
        &self,
        context: &ActionContext,
        request_id: String,
        request: &ProgrammingUpdateTargetsRequest,
        input: &ProgrammingUpdateMenuInput,
        ports: &P,
    ) -> Result<ProgrammingUpdateTargetsResult, ActionError> {
        self.transact(
            context,
            request.show_id,
            ports,
            "programming-update-targets",
            |document| {
                let active = ports.active_update_cue_contexts(context)?;
                let entries =
                    super::menu::preview_update_menu(document, &active, input, request.filter);
                Ok(PreparedActiveShowTransaction::NoChange((
                    document.revision(),
                    entries,
                )))
            },
            |_, _, context, completed| ProgrammingUpdateTargetsResult {
                context: context.clone(),
                request_id,
                correlation_id: context.correlation_id,
                show_revision: completed.state.0,
                entries: completed.state.1,
            },
        )
    }

    pub(crate) fn preview_programming_update<P: ProgrammingUpdatePorts>(
        &self,
        context: &ActionContext,
        request_id: String,
        request: &ProgrammingUpdatePreviewRequest,
        content: &ProgrammerUpdateContent,
        programmer_revision: String,
        ports: &P,
    ) -> Result<ProgrammingUpdatePreviewResult, ActionError> {
        self.transact(
            context,
            request.show_id,
            ports,
            "preview-update",
            |document| {
                let active = ports.active_update_cue_contexts(context)?;
                let (object, preview) = preview_document(document, request, content, &active)?;
                Ok(PreparedActiveShowTransaction::NoChange((
                    document.revision(),
                    object,
                    preview,
                )))
            },
            |_, _, context, completed| ProgrammingUpdatePreviewResult {
                context: context.clone(),
                request_id,
                correlation_id: context.correlation_id,
                show_revision: completed.state.0,
                object_revision: completed.state.1.object_revision,
                object: completed.state.1,
                programmer_revision,
                preview: completed.state.2,
            },
        )
    }

    pub(crate) fn commit_programming_update<P: ProgrammingUpdatePorts>(
        &self,
        context: &ActionContext,
        command: &ProgrammingUpdateCommand,
        content: &ProgrammerUpdateContent,
        ports: &P,
    ) -> Result<ProgrammingUpdateOutcome, ActionError> {
        self.transact(
            context,
            command.show_id,
            ports,
            "programming-update",
            |document| {
                let active = ports.active_update_cue_contexts(context)?;
                prepare_update(document, command, content, &active)
            },
            complete_update,
        )
    }
}

fn preview_document(
    document: &PortableShowDocument,
    request: &ProgrammingUpdatePreviewRequest,
    content: &ProgrammerUpdateContent,
    active: &[super::ActiveCueContext],
) -> Result<(ProgrammingUpdateObjectReference, super::UpdatePreview), ActionError> {
    match (&request.target, request.mode) {
        (ProgrammingUpdateTargetRequest::Cue { .. }, UpdateMode::Cue(mode)) => {
            let target = resolve_requested_cue(&request.target, active)?;
            let (object, cue_list) = find_cue_list(document, target.cue_list_id)?;
            let preview =
                preview_cue_update(&cue_list, &target, mode, content).map_err(update_error)?;
            Ok((
                object_reference(ActiveShowObjectKind::CueList, object),
                preview,
            ))
        }
        (
            ProgrammingUpdateTargetRequest::Preset { object_id },
            UpdateMode::ExistingContent(mode),
        ) => {
            let (object, preset) = find_preset(document, object_id)?;
            let preview =
                preview_preset_update(object_id, &preset, mode, content).map_err(update_error)?;
            Ok((
                object_reference(ActiveShowObjectKind::Preset, object),
                preview,
            ))
        }
        (
            ProgrammingUpdateTargetRequest::Group { object_id },
            UpdateMode::ExistingContent(mode),
        ) => {
            let (object, group, groups) = find_group(document, object_id)?;
            let membership = resolve_group(object_id, &groups).map_err(invalid)?;
            let preview =
                preview_group_update(&group, &membership, mode, content).map_err(update_error)?;
            Ok((
                object_reference(ActiveShowObjectKind::Group, object),
                preview,
            ))
        }
        _ => Err(invalid("Update mode does not match its target family")),
    }
}

fn object_reference(
    kind: ActiveShowObjectKind,
    object: &PortableShowObject,
) -> ProgrammingUpdateObjectReference {
    ProgrammingUpdateObjectReference {
        kind,
        object_id: object.key().id().to_owned(),
        object_revision: object.revision(),
    }
}

fn prepare_update(
    document: &PortableShowDocument,
    command: &ProgrammingUpdateCommand,
    content: &ProgrammerUpdateContent,
    active: &[super::ActiveCueContext],
) -> Result<PreparedActiveShowTransaction<PreparedUpdate>, ActionError> {
    validate_confirmed_cue_identity(command)?;
    validate_show_revision(document, command.expected_show_revision)?;
    let (object, before, plan) = plan_document(document, command, content, active)?;
    let kind = plan_kind(&plan);
    let object_id = object.key().id().to_owned();
    let raw_body = merged_body(object, &before, &plan.object)?;
    let mut transaction = document.transaction();
    transaction.put(kind.as_str(), object_id.clone(), raw_body);
    let prepared = prepare_show_candidate(document, transaction)?;
    let (projection, show_revision) = {
        let candidate = document
            .candidate(prepared.transaction())
            .map_err(|error| invalid(error.to_string()))?;
        let candidate_object = candidate
            .object(kind.as_str(), &object_id)
            .ok_or_else(|| invalid("prepared Update target is missing"))?;
        (
            ProgrammingUpdateProjection {
                show_id: command.show_id,
                kind,
                object_id,
                object_revision: candidate_object.revision(),
                raw_body: Arc::new(candidate_object.body().clone()),
            },
            candidate.revision(),
        )
    };
    let summary = plan.complete(projection.object_revision);
    Ok(PreparedActiveShowTransaction::PreparedCommit {
        prepared: Box::new(prepared),
        state: PreparedUpdate {
            projection,
            show_revision,
            summary,
        },
    })
}

fn validate_confirmed_cue_identity(command: &ProgrammingUpdateCommand) -> Result<(), ActionError> {
    let confirmed = command.expected_object_revision.is_some()
        || command.expected_programmer_revision.is_some()
        || command.expected_show_revision.is_some();
    let ProgrammingUpdateTargetRequest::Cue {
        cue_id, cue_number, ..
    } = &command.target
    else {
        return Ok(());
    };
    if confirmed && (cue_id.is_none() || cue_number.is_none()) {
        Err(super::resolution::conflict(
            "confirmed Cue Update requires the exact previewed Cue",
        ))
    } else {
        Ok(())
    }
}

enum StoredUpdateObject {
    CueList(CueList),
    Preset(Preset),
    Group(GroupDefinition),
}

fn plan_document<'a>(
    document: &'a PortableShowDocument,
    command: &ProgrammingUpdateCommand,
    content: &ProgrammerUpdateContent,
    active: &[super::ActiveCueContext],
) -> Result<(&'a PortableShowObject, StoredUpdateObject, AtomicUpdatePlan), ActionError> {
    match (&command.target, command.mode) {
        (ProgrammingUpdateTargetRequest::Cue { .. }, UpdateMode::Cue(mode)) => {
            let target = resolve_requested_cue(&command.target, active)?;
            let (object, cue_list) = find_cue_list(document, target.cue_list_id)?;
            let plan = plan_cue_update(
                &cue_list,
                object.revision(),
                command
                    .expected_object_revision
                    .unwrap_or(object.revision()),
                &target,
                mode,
                content,
            )
            .map_err(update_error)?;
            Ok((object, StoredUpdateObject::CueList(cue_list), plan))
        }
        (
            ProgrammingUpdateTargetRequest::Preset { object_id },
            UpdateMode::ExistingContent(mode),
        ) => {
            let (object, preset) = find_preset(document, object_id)?;
            let plan = plan_preset_update(
                object_id,
                &preset,
                object.revision(),
                command
                    .expected_object_revision
                    .unwrap_or(object.revision()),
                mode,
                content,
            )
            .map_err(update_error)?;
            Ok((object, StoredUpdateObject::Preset(preset), plan))
        }
        (
            ProgrammingUpdateTargetRequest::Group { object_id },
            UpdateMode::ExistingContent(mode),
        ) => {
            let (object, group, groups) = find_group(document, object_id)?;
            let membership = resolve_group(object_id, &groups).map_err(invalid)?;
            let plan = plan_group_update(
                &group,
                &membership,
                object.revision(),
                command
                    .expected_object_revision
                    .unwrap_or(object.revision()),
                mode,
                content,
            )
            .map_err(update_error)?;
            Ok((object, StoredUpdateObject::Group(group), plan))
        }
        _ => Err(invalid("Update mode does not match its target family")),
    }
}

fn complete_update<P: ProgrammingUpdatePorts>(
    events: &EventBus,
    ports: &P,
    context: &ActionContext,
    completed: CompletedActiveShowTransaction<PreparedUpdate>,
) -> ProgrammingUpdateOutcome {
    let mut prepared = completed.state;
    let commit = completed
        .commit
        .expect("a successful Update always commits one real change");
    prepared.show_revision = commit.revision();
    ports.reconcile_programming_update(&prepared.projection);
    let change = ActiveShowObjectChange {
        kind: prepared.projection.kind,
        object_id: prepared.projection.object_id.clone(),
        object_revision: prepared.projection.object_revision,
        body: Some(prepared.projection.raw_body.as_ref().clone()),
        deleted: false,
    };
    let event_sequence = events
        .publish(EventDraft::active_show_objects_changed(
            context,
            ActiveShowObjectsChange {
                show_id: prepared.projection.show_id,
                show_revision: prepared.show_revision,
                changes: vec![change],
            },
        ))
        .sequence;
    ProgrammingUpdateOutcome {
        projection: Arc::new(prepared.projection),
        show_revision: prepared.show_revision,
        event_sequence,
        summary: prepared.summary,
    }
}

struct PreparedUpdate {
    projection: ProgrammingUpdateProjection,
    show_revision: PortableShowRevision,
    summary: super::UpdateResult,
}

fn merged_body(
    object: &PortableShowObject,
    before: &StoredUpdateObject,
    after: &PlannedUpdateObject,
) -> Result<serde_json::Value, ActionError> {
    let merged = match (before, after) {
        (StoredUpdateObject::CueList(before), PlannedUpdateObject::CueList(after)) => {
            lossless_json::merge_typed(object.body(), before, after)
        }
        (StoredUpdateObject::Preset(before), PlannedUpdateObject::Preset(after)) => {
            lossless_json::merge_typed(object.body(), before, after)
        }
        (StoredUpdateObject::Group(before), PlannedUpdateObject::Group(after)) => {
            lossless_json::merge_typed(object.body(), before, after)
        }
        _ => {
            return Err(invalid(
                "Update planner returned the wrong target object kind",
            ));
        }
    };
    merged.map_err(|error| invalid(error.to_string()))
}

fn plan_kind(plan: &AtomicUpdatePlan) -> ActiveShowObjectKind {
    match plan.object {
        PlannedUpdateObject::CueList(_) => ActiveShowObjectKind::CueList,
        PlannedUpdateObject::Preset(_) => ActiveShowObjectKind::Preset,
        PlannedUpdateObject::Group(_) => ActiveShowObjectKind::Group,
    }
}
