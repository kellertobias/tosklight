use super::{
    ActiveShowObjectBody, ActiveShowObjectChange, ActiveShowObjectMutation,
    ActiveShowObjectMutationKind, MutateActiveShowObjectsCommand,
};
use crate::{
    ActionError, ActionErrorKind, prepare_show_candidate,
    show_compiler::prepare_normalized_show_candidate_incremental,
};
use light_core::Revision;
use light_dynamics::{DynamicDefinition, validate_definition};
use light_playback::{CueList, PlaybackDefinition, PlaybackPage};
use light_programmer::{GroupDefinition, Preset};
use light_show::{LosslessBody, PortableShowDocument, PortableShowObject, PortableShowTransaction};
use std::collections::HashSet;

pub(super) struct PreparedObjectChanges {
    pub(super) transaction: PortableShowTransaction,
    pub(super) snapshot: light_engine::EngineSnapshot,
    pub(super) changes: Vec<ActiveShowObjectChange>,
}

pub(super) fn prepare_object_mutation(
    document: &PortableShowDocument,
    command: &MutateActiveShowObjectsCommand,
    previous: Option<&light_engine::EngineSnapshot>,
) -> Result<PreparedObjectChanges, ActionError> {
    validate_command(document, command)?;
    let mut transaction = document.transaction();
    let mut changes = Vec::with_capacity(command.mutations.len());
    for mutation in &command.mutations {
        let existing = document.object(mutation.kind.as_str(), &mutation.object_id);
        validate_object_revision(existing, mutation)?;
        changes.push(apply_mutation(&mut transaction, existing, mutation)?);
    }
    let prepared = if let Some(previous) =
        previous.filter(|snapshot| snapshot.revision == document.revision().value())
    {
        prepare_normalized_show_candidate_incremental(document, transaction, previous)?
    } else {
        prepare_show_candidate(document, transaction)?
    };
    let (transaction, snapshot) = prepared.into_parts();
    Ok(PreparedObjectChanges {
        transaction,
        snapshot,
        changes,
    })
}

fn validate_command(
    document: &PortableShowDocument,
    command: &MutateActiveShowObjectsCommand,
) -> Result<(), ActionError> {
    if document.id() != command.show_id {
        return Err(not_found("requested show is not active"));
    }
    if command.mutations.is_empty() {
        return Err(invalid("at least one show-object mutation is required"));
    }
    let mut targets = HashSet::with_capacity(command.mutations.len());
    for mutation in &command.mutations {
        if mutation.object_id.is_empty() {
            return Err(invalid("show object id cannot be empty"));
        }
        if mutation.kind == super::ActiveShowObjectKind::AttributeConfiguration {
            if mutation.object_id != "default" {
                return Err(invalid(
                    "attribute_configuration must use the singleton storage id default",
                ));
            }
            if matches!(mutation.mutation, ActiveShowObjectMutationKind::Delete) {
                return Err(invalid(
                    "attribute_configuration cannot be deleted; restore recommended defaults instead",
                ));
            }
        }
        if !targets.insert((mutation.kind, mutation.object_id.as_str())) {
            return Err(invalid(format!(
                "duplicate {} {} mutation",
                mutation.kind.as_str(),
                mutation.object_id
            )));
        }
    }
    Ok(())
}

fn validate_object_revision(
    existing: Option<&PortableShowObject>,
    mutation: &ActiveShowObjectMutation,
) -> Result<(), ActionError> {
    let current = existing.map_or(0, PortableShowObject::revision);
    if matches!(mutation.mutation, ActiveShowObjectMutationKind::Delete) && existing.is_none() {
        return Err(not_found(format!(
            "{} {} does not exist",
            mutation.kind.as_str(),
            mutation.object_id
        )));
    }
    if current == mutation.expected_object_revision {
        Ok(())
    } else {
        Err(ActionError::new(
            ActionErrorKind::Conflict,
            format!(
                "stale {} {} revision conflict",
                mutation.kind.as_str(),
                mutation.object_id
            ),
        )
        .at_revision(current))
    }
}

fn apply_mutation(
    transaction: &mut PortableShowTransaction,
    existing: Option<&PortableShowObject>,
    mutation: &ActiveShowObjectMutation,
) -> Result<ActiveShowObjectChange, ActionError> {
    let revision = next_revision(mutation.expected_object_revision)?;
    match &mutation.mutation {
        ActiveShowObjectMutationKind::Put { body } => {
            let existing_body = existing
                .map(|object| ActiveShowObjectBody::decode(mutation.kind, object.body().clone()))
                .transpose()
                .map_err(invalid)?;
            let body = normalize_body(existing_body.as_ref(), mutation, body)?;
            transaction.put(
                mutation.kind.as_str(),
                mutation.object_id.clone(),
                body.encode(),
            );
            Ok(ActiveShowObjectChange {
                kind: mutation.kind,
                object_id: mutation.object_id.clone(),
                object_revision: revision,
                body: Some(body),
                deleted: false,
            })
        }
        ActiveShowObjectMutationKind::Delete => {
            transaction.delete(mutation.kind.as_str(), mutation.object_id.clone());
            Ok(ActiveShowObjectChange {
                kind: mutation.kind,
                object_id: mutation.object_id.clone(),
                object_revision: revision,
                body: None,
                deleted: true,
            })
        }
    }
}

fn normalize_body(
    existing: Option<&ActiveShowObjectBody>,
    mutation: &ActiveShowObjectMutation,
    request: &ActiveShowObjectBody,
) -> Result<ActiveShowObjectBody, ActionError> {
    if request.kind() != mutation.kind {
        return Err(invalid(format!(
            "{} mutation cannot carry a {} body",
            mutation.kind.as_str(),
            request.kind().as_str()
        )));
    }
    match request {
        ActiveShowObjectBody::AttributeConfiguration(request) => normalize_attribute_configuration(
            existing.and_then(ActiveShowObjectBody::attribute_configuration),
            mutation,
            request,
        )
        .map(ActiveShowObjectBody::AttributeConfiguration),
        ActiveShowObjectBody::CueList(request) => normalize_cue_list(
            existing.and_then(ActiveShowObjectBody::cue_list),
            mutation,
            request,
        )
        .map(ActiveShowObjectBody::CueList),
        ActiveShowObjectBody::Dynamic(request) => normalize_dynamic(
            existing.and_then(ActiveShowObjectBody::dynamic),
            mutation,
            request,
        )
        .map(ActiveShowObjectBody::Dynamic),
        ActiveShowObjectBody::Group(request) => normalize_group(
            existing.and_then(ActiveShowObjectBody::group),
            mutation,
            request,
        )
        .map(ActiveShowObjectBody::Group),
        ActiveShowObjectBody::PatchLayer(request) => normalize_passthrough(
            existing.and_then(ActiveShowObjectBody::patch_layer),
            request,
        )
        .map(ActiveShowObjectBody::PatchLayer),
        ActiveShowObjectBody::Playback(request) => normalize_playback(
            existing.and_then(ActiveShowObjectBody::playback),
            mutation,
            request,
        )
        .map(ActiveShowObjectBody::Playback),
        ActiveShowObjectBody::PlaybackPage(request) => normalize_playback_page(
            existing.and_then(ActiveShowObjectBody::playback_page),
            mutation,
            request,
        )
        .map(ActiveShowObjectBody::PlaybackPage),
        ActiveShowObjectBody::Preset(request) => normalize_preset(
            existing.and_then(ActiveShowObjectBody::preset),
            mutation,
            request,
        )
        .map(ActiveShowObjectBody::Preset),
        ActiveShowObjectBody::Schedule(request) => normalize_schedule(
            existing.and_then(ActiveShowObjectBody::schedule),
            mutation,
            request,
        )
        .map(ActiveShowObjectBody::Schedule),
        ActiveShowObjectBody::StageLayout(request) => normalize_passthrough(
            existing.and_then(ActiveShowObjectBody::stage_layout),
            request,
        )
        .map(ActiveShowObjectBody::StageLayout),
        ActiveShowObjectBody::UserLayout(request) => normalize_passthrough(
            existing.and_then(ActiveShowObjectBody::user_layout),
            request,
        )
        .map(ActiveShowObjectBody::UserLayout),
    }
}

fn normalize_attribute_configuration(
    existing: Option<&LosslessBody<light_core::AttributeConfiguration>>,
    mutation: &ActiveShowObjectMutation,
    request: &LosslessBody<light_core::AttributeConfiguration>,
) -> Result<LosslessBody<light_core::AttributeConfiguration>, ActionError> {
    if mutation.object_id != "default" {
        return Err(invalid(
            "attribute_configuration must use the singleton storage id default",
        ));
    }
    let normalized = request.typed().clone();
    normalized.validate().map_err(invalid)?;
    LosslessBody::merge_normalized_body(existing, request, normalized).map_err(invalid)
}

fn normalize_dynamic(
    existing: Option<&LosslessBody<DynamicDefinition>>,
    mutation: &ActiveShowObjectMutation,
    request: &LosslessBody<DynamicDefinition>,
) -> Result<LosslessBody<DynamicDefinition>, ActionError> {
    let mut normalized = request.typed().clone();
    let object_id = uuid::Uuid::parse_str(&mutation.object_id)
        .map_err(|error| invalid(format!("invalid Dynamic storage id: {error}")))?;
    normalized.id = object_id;
    validate_definition(&normalized).map_err(invalid)?;
    LosslessBody::merge_normalized_body(existing, request, normalized).map_err(invalid)
}

fn normalize_cue_list(
    existing: Option<&LosslessBody<CueList>>,
    mutation: &ActiveShowObjectMutation,
    request: &LosslessBody<CueList>,
) -> Result<LosslessBody<CueList>, ActionError> {
    let requested = request.typed();
    let mut normalized = requested.clone();
    if let Ok(id) = uuid::Uuid::parse_str(&mutation.object_id) {
        normalized.id = light_core::CueListId(id);
    }
    normalized.validate().map_err(invalid)?;
    let mut merged =
        LosslessBody::merge_normalized_body(existing, request, normalized).map_err(invalid)?;
    merged.strip_zero_u64_echo("chaser_xfade_millis");
    merged.strip_nested_array_object_key("cues", "phasers");
    Ok(merged)
}

fn normalize_group(
    existing: Option<&LosslessBody<GroupDefinition>>,
    mutation: &ActiveShowObjectMutation,
    request: &LosslessBody<GroupDefinition>,
) -> Result<LosslessBody<GroupDefinition>, ActionError> {
    let requested = request.typed();
    let mut normalized = requested.clone();
    normalized.id.clone_from(&mutation.object_id);
    let mut merged =
        LosslessBody::merge_normalized_body(existing, request, normalized).map_err(invalid)?;
    merged.strip_object_key("master");
    merged.strip_object_key("playback_fader");
    Ok(merged)
}

fn normalize_preset(
    existing: Option<&LosslessBody<Preset>>,
    mutation: &ActiveShowObjectMutation,
    request: &LosslessBody<Preset>,
) -> Result<LosslessBody<Preset>, ActionError> {
    let requested = request.typed();
    let mut normalized = requested.clone();
    normalized
        .reconcile_address(&mutation.object_id)
        .map_err(invalid)?;
    LosslessBody::merge_normalized_body(existing, request, normalized).map_err(invalid)
}

fn normalize_schedule(
    existing: Option<&LosslessBody<crate::ScheduleDefinition>>,
    mutation: &ActiveShowObjectMutation,
    request: &LosslessBody<crate::ScheduleDefinition>,
) -> Result<LosslessBody<crate::ScheduleDefinition>, ActionError> {
    let mut normalized = request.typed().clone();
    normalized.id = crate::ScheduleId(
        uuid::Uuid::parse_str(&mutation.object_id)
            .map_err(|error| invalid(format!("invalid Schedule storage id: {error}")))?,
    );
    normalized.validate().map_err(invalid)?;
    LosslessBody::merge_normalized_body(existing, request, normalized).map_err(invalid)
}

fn normalize_playback(
    existing: Option<&LosslessBody<PlaybackDefinition>>,
    mutation: &ActiveShowObjectMutation,
    request: &LosslessBody<PlaybackDefinition>,
) -> Result<LosslessBody<PlaybackDefinition>, ActionError> {
    let requested = request.typed();
    let mut normalized = requested.clone();
    normalized.number = parse_storage_number(&mutation.object_id, "playback")?;
    normalized.validate().map_err(invalid)?;
    LosslessBody::merge_normalized_body(existing, request, normalized).map_err(invalid)
}

fn normalize_playback_page(
    existing: Option<&LosslessBody<PlaybackPage>>,
    mutation: &ActiveShowObjectMutation,
    request: &LosslessBody<PlaybackPage>,
) -> Result<LosslessBody<PlaybackPage>, ActionError> {
    let requested = request.typed();
    let mut normalized = requested.clone();
    normalized.number = parse_storage_number(&mutation.object_id, "playback page")?;
    normalized.validate().map_err(invalid)?;
    LosslessBody::merge_normalized_body(existing, request, normalized).map_err(invalid)
}

fn normalize_passthrough<T>(
    existing: Option<&LosslessBody<T>>,
    request: &LosslessBody<T>,
) -> Result<LosslessBody<T>, ActionError>
where
    T: Clone + serde::Serialize + serde::de::DeserializeOwned,
{
    let typed = request.typed().clone();
    LosslessBody::merge_normalized_body(existing, request, typed).map_err(invalid)
}

fn parse_storage_number<T>(object_id: &str, label: &str) -> Result<T, ActionError>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    object_id
        .parse::<T>()
        .map_err(|error| invalid(format!("invalid {label} storage id: {error}")))
}

fn next_revision(current: Revision) -> Result<Revision, ActionError> {
    current.checked_add(1).ok_or_else(|| {
        ActionError::new(
            ActionErrorKind::Invalid,
            "show object revision cannot be incremented",
        )
        .at_revision(current)
    })
}

fn invalid(error: impl std::fmt::Display) -> ActionError {
    ActionError::new(ActionErrorKind::Invalid, error.to_string())
}

fn not_found(message: impl Into<String>) -> ActionError {
    ActionError::new(ActionErrorKind::NotFound, message)
}

#[cfg(test)]
mod tests;
